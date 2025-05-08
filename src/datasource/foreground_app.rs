use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use inotify::WatchMask;
use log::{debug, info, warn};
use regex::Regex;

use crate::{
    datasource::file_path::*,
    utils::{
        file_operate::{check_read_simple, write_file},
        inotify::InotifyWatcher,
    },
};

// 缓存前台应用信息，避免频繁调用系统命令
struct ForegroundAppCache {
    package_name: String,
    last_update: Instant,
}

impl ForegroundAppCache {
    fn new() -> Self {
        Self {
            package_name: String::new(),
            last_update: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.last_update.elapsed() > ttl
    }

    fn update(&mut self, package_name: String) {
        self.package_name = package_name;
        self.last_update = Instant::now();
    }
}

// 使用dumpsys window命令获取前台应用包名
fn get_foreground_app_window() -> Result<String> {
    debug!("Trying to get foreground app using dumpsys window method");

    let output = loop {
        match Command::new("/system/bin/dumpsys")
            .args(["window"])
            .output()
        {
            Ok(o) => {
                break String::from_utf8_lossy(&o.stdout).to_string();
            }
            Err(e) => {
                log::error!("Unable to execute dumpsys window command: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    };

    // 记录输出预览用于调试
    debug!("Dumpsys window output preview: {}",
        output.chars().take(100).collect::<String>());

    // 逐行分析输出
    for line in output.lines() {
        // 检查mCurrentFocus和mFocusedWindow
        if line.contains("mCurrentFocus") || line.contains("mFocusedWindow") {
            debug!("Found focus line: {}", line);

            // 查找最后一个空格后的内容
            if let Some(last_space_pos) = line.rfind(' ') {
                let last_field = &line[last_space_pos + 1..];
                debug!("Last field: {}", last_field);

                // 查找斜杠前的包名
                if let Some(slash_pos) = last_field.find('/') {
                    let mut package_name = &last_field[..slash_pos];

                    // 移除可能的前缀字符
                    if package_name.starts_with('*') || package_name.starts_with('{') {
                        package_name = &package_name[1..];
                    }

                    debug!("Extracted package name: {}", package_name);
                    return Ok(package_name.to_string());
                }
            }
        }
    }

    debug!("Failed to find foreground app using dumpsys window method");
    Err(anyhow!("Failed to find foreground app in dumpsys window output"))
}

// 使用dumpsys activity lru命令获取前台应用包名
fn get_foreground_app_activity() -> Result<String> {
    debug!("Trying to get foreground app using dumpsys activity lru method");

    let output = loop {
        match Command::new("/system/bin/dumpsys")
            .args(["activity", "lru"])
            .output()
        {
            Ok(o) => {
                break String::from_utf8_lossy(&o.stdout).to_string();
            }
            Err(e) => {
                log::error!("Unable to get foreground application: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    };

    // 方法1：使用Android 10+的方式查找前台应用
    // 查找包含" TOP"关键字的行，但不包含"BTOP"的行
    for line in output.lines() {
        if line.contains(" TOP") && !line.contains("BTOP") && line.contains("fg") {
            debug!("Found matching line: {}", line);

            // 查找":"字符后的包名
            if let Some(colon_pos) = line.find(':') {
                let after_colon = &line[colon_pos + 1..];

                // 查找"/"字符前的包名
                if let Some(slash_pos) = after_colon.find('/') {
                    let package_name = &after_colon[..slash_pos];
                    debug!("Extracted package name: {}", package_name);
                    return Ok(package_name.to_string());
                }
            }
        }
    }

    // 方法2：使用正则表达式作为备选方法
    for line in output.lines() {
        if line.contains("fg") && line.contains("TOP") && !line.contains("BTOP") {
            debug!("Trying regex on line: {}", line);

            // 使用正则表达式提取包名部分
            let re = Regex::new(r"(\d+):([a-zA-Z][a-zA-Z0-9_]*(\.[a-zA-Z][a-zA-Z0-9_]*)+)/").unwrap();
            if let Some(caps) = re.captures(line) {
                let package_name = caps[2].to_string();
                debug!("Extracted package name with regex: {}", package_name);
                return Ok(package_name);
            }
        }
    }

    // 如果上面的匹配失败，记录一些调试信息
    debug!("Failed to find foreground app using dumpsys activity lru method");
    debug!("Dumpsys activity lru output first few lines:");
    for (i, line) in output.lines().take(5).enumerate() {
        debug!("Line {}: {}", i + 1, line);
    }
    debug!("Lines containing 'TOP':");
    for line in output.lines().filter(|l| l.contains("TOP")) {
        debug!("Line with TOP: {}", line);
    }
    Err(anyhow!("Failed to find foreground app in dumpsys activity lru output"))
}

// 获取前台应用包名（组合方法）
fn get_foreground_app() -> Result<String> {
    // 首先尝试使用window方法
    match get_foreground_app_window() {
        Ok(package_name) => {
            debug!("Successfully got foreground app using window method: {}", package_name);
            return Ok(package_name);
        }
        Err(e) => {
            debug!("Window method failed: {}, trying activity lru method", e);
        }
    }

    // 如果window方法失败，尝试使用activity lru方法
    match get_foreground_app_activity() {
        Ok(package_name) => {
            debug!("Successfully got foreground app using activity lru method: {}", package_name);
            return Ok(package_name);
        }
        Err(e) => {
            debug!("Activity lru method also failed: {}", e);
        }
    }

    // 如果两种方法都失败，返回错误
    Err(anyhow!("Failed to get foreground app using all available methods"))
}

// 读取游戏列表
fn read_games_list(path: &str) -> Result<HashSet<String>> {
    let mut games = HashSet::new();

    if !check_read_simple(path) {
        return Ok(games);
    }

    let file =
        File::open(path).with_context(|| format!("Failed to open games list file: {}", path))?;

    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();

        // 跳过空行和注释
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        games.insert(trimmed.to_string());
    }

    Ok(games)
}

// 监控前台应用
pub fn monitor_foreground_app() -> Result<()> {
    // 设置线程名称
    info!("{} Start", FOREGROUND_APP_THREAD);

    // 初始化缓存
    let mut app_cache = ForegroundAppCache::new();
    let cache_ttl = Duration::from_millis(1000); // 缓存有效期1秒

    // 读取游戏列表
    let mut games = read_games_list(GAMES_CONF_PATH)?;
    info!("Loaded {} games from {}", games.len(), GAMES_CONF_PATH);

    // 设置文件监控
    let mut inotify = InotifyWatcher::new()?;

    // 如果游戏列表文件存在，监控它的变化
    if check_read_simple(GAMES_CONF_PATH) {
        inotify.add(GAMES_CONF_PATH, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;
        info!("Watching games list file: {}", GAMES_CONF_PATH);
    } else {
        info!("Games list file does not exist: {}", GAMES_CONF_PATH);
    }

    // 主循环
    loop {
        // 检查游戏列表文件是否有变化
        if let Ok(_) = inotify.wait_and_handle() {
            // 重新读取游戏列表
            match read_games_list(GAMES_CONF_PATH) {
                Ok(new_games) => {
                    info!("Games list updated, now contains {} games", new_games.len());
                    games = new_games;
                }
                Err(e) => {
                    warn!("Failed to read updated games list: {}", e);
                }
            }
        }

        // 获取前台应用
        if app_cache.is_expired(cache_ttl) {
            match get_foreground_app() {
                Ok(package_name) => {
                    // 只有当包名变化时才记录日志
                    if package_name != app_cache.package_name {
                        info!("Foreground app changed: {}", package_name);

                        // 检查是否是游戏
                        let is_game = games.contains(&package_name);

                        // 写入游戏模式文件
                        if let Err(e) = write_file(
                            GPU_GOVERNOR_GAME_MODE_PATH,
                            if is_game { "1" } else { "0" },
                            3,
                        ) {
                            warn!("Failed to write game mode: {}", e);
                        } else {
                            debug!(
                                "Wrote game mode {} to file",
                                if is_game { "1" } else { "0" }
                            );
                        }

                        if is_game {
                            info!("Game detected: {}", package_name);
                        }
                    }

                    // 更新缓存
                    app_cache.update(package_name);
                }
                Err(e) => {
                    warn!("Failed to get foreground app: {}", e);
                }
            }
        }

        // 休眠一段时间
        thread::sleep(Duration::from_millis(100));
    }
}
