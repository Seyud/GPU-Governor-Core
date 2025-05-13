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

    // 使用正则表达式提取前台应用包名
    for line in output.lines() {
        if line.contains("fg") && line.contains("TOP") && !line.contains("BTOP") {
            debug!("Trying regex on line: {}", line);

            // 使用正则表达式提取包名部分
            let re =
                Regex::new(r"(\d+):([a-zA-Z][a-zA-Z0-9_]*(\.[a-zA-Z][a-zA-Z0-9_]*)+)/").unwrap();
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
    Err(anyhow!(
        "Failed to find foreground app in dumpsys activity lru output"
    ))
}

// 获取前台应用包名
fn get_foreground_app() -> Result<String> {
    // 直接使用activity lru方法
    match get_foreground_app_activity() {
        Ok(package_name) => {
            debug!(
                "Successfully got foreground app using activity lru method: {}",
                package_name
            );
            return Ok(package_name);
        }
        Err(e) => {
            // 如果失败，直接返回错误
            debug!("Activity lru method failed: {}", e);
            return Err(anyhow!("Failed to get foreground app: {}", e));
        }
    }
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
        // 检查inotify事件，只在游戏列表文件变化时才重新读取
        if let Ok(events) = inotify.check_events() {
            if !events.is_empty() {
                debug!("Detected changes in games list file");
                games = read_games_list(GAMES_CONF_PATH)?;
                info!("The game configuration file has changed. Loaded {} games.", games.len());
            }
        }

        // 获取前台应用
        if app_cache.is_expired(cache_ttl) {
            match get_foreground_app() {
                Ok(package_name) => {
                    // 只有当包名变化时才处理
                    if package_name != app_cache.package_name {
                        // 将前台应用变化的日志改为debug级别
                        debug!("Foreground app changed: {}", package_name);

                        // 检查是否是游戏
                        let is_game = games.contains(&package_name);

                        // 检查前一个应用是否是游戏
                        let prev_is_game = !app_cache.package_name.is_empty() &&
                                          games.contains(&app_cache.package_name);

                        // 只有在游戏模式状态变化时才记录info级别日志
                        if is_game {
                            if !prev_is_game {
                                info!("Game mode enabled: {}", package_name);
                            } else {
                                // 游戏切换到另一个游戏时也记录
                                info!("Game changed: {}", package_name);
                            }
                        } else if prev_is_game {
                            info!("Game mode disabled: switching from game to normal app: {}", package_name);
                        }

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
