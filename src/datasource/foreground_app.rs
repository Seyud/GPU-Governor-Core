use std::{
    collections::HashMap,
    sync::{Mutex, mpsc::Sender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use dumpsys_rs::Dumpsys;
use inotify::WatchMask;
use log::{debug, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;

use crate::{
    datasource::{
        config_parser::{Config, ConfigDelta, load_config},
        file_path::*,
    },
    model::gpu::GPU,
    utils::{
        file_operate::{check_read_simple, write_file},
        inotify::InotifyWatcher,
    },
};

#[derive(Debug, Deserialize)]
struct GameEntry {
    package: String,
    mode: String,
}

#[derive(Debug, Deserialize)]
struct GamesConfig {
    games: Vec<GameEntry>,
}

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

// 警告日志限流器，避免频繁显示相同的警告
struct WarningThrottler {
    last_warning_time: Instant,
    throttle_duration: Duration,
}

impl WarningThrottler {
    fn new(throttle_seconds: u64) -> Self {
        Self {
            last_warning_time: Instant::now()
                .checked_sub(Duration::from_secs(throttle_seconds))
                .unwrap_or(Instant::now()),
            throttle_duration: Duration::from_secs(throttle_seconds),
        }
    }

    // 检查是否应该显示警告
    fn should_warn(&mut self) -> bool {
        let elapsed = self.last_warning_time.elapsed();
        if elapsed >= self.throttle_duration {
            self.last_warning_time = Instant::now();
            true
        } else {
            false
        }
    }
}

// 使用dumpsys activity lru命令获取前台应用包名
fn get_foreground_app_activity() -> Result<String> {
    debug!("Trying to get foreground app using dumpsys activity lru method");

    // 新增：为error日志添加12小时限流器
    static ERROR_THROTTLER: Lazy<Mutex<WarningThrottler>> =
        Lazy::new(|| Mutex::new(WarningThrottler::new(43200)));
    let dumper = loop {
        match Dumpsys::new("activity") {
            Some(s) => break s,
            None => std::thread::sleep(std::time::Duration::from_secs(1)),
        };
    };
    let output = loop {
        match dumper.dump(&["lru"]) {
            Ok(d) => break d,
            Err(e) => {
                // 线程安全的全局限流器
                {
                    let mut throttler = ERROR_THROTTLER.lock().unwrap();
                    if throttler.should_warn() {
                        log::error!("Unable to get foreground application: {e}");
                    } else {
                        log::debug!("Unable to get foreground application (throttled): {e}");
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        };
    };

    // 使用正则表达式提取前台应用包名
    let re = Regex::new(r"(\d+):([a-zA-Z][a-zA-Z0-9_]*(\.[a-zA-Z][a-zA-Z0-9_]*)+)/").unwrap();
    for line in output.lines() {
        if line.contains("fg") && line.contains("TOP") && !line.contains("BTOP") {
            debug!("Trying regex on line: {line}");

            // 使用正则表达式提取包名部分
            if let Some(caps) = re.captures(line) {
                let package_name = caps[2].to_string();
                debug!("Extracted package name with regex: {package_name}");
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
        debug!("Line with TOP: {line}");
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
            debug!("Successfully got foreground app using activity lru method: {package_name}");
            Ok(package_name)
        }
        Err(e) => {
            // 如果失败，直接返回错误
            debug!("Activity lru method failed: {e}");
            Err(anyhow!("Failed to get foreground app: {e}"))
        }
    }
}

// 读取游戏列表
fn read_games_list(path: &str) -> Result<HashMap<String, String>> {
    if !check_read_simple(path) {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read games list file: {path}"))?;

    let config: GamesConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML from games list file: {path}"))?;

    Ok(config
        .games
        .into_iter()
        .map(|entry| (entry.package, entry.mode))
        .collect())
}

// 监控前台应用
pub fn monitor_foreground_app(mut gpu: GPU, tx: Option<Sender<ConfigDelta>>) -> Result<()> {
    // 设置线程名称
    info!("{FOREGROUND_APP_THREAD} Start");

    // 初始化缓存
    let mut app_cache = ForegroundAppCache::new();
    let cache_ttl = Duration::from_millis(1000); // 缓存有效期1秒
    // 初始化警告限流器，设置60秒的限流时间
    let mut warning_throttler = WarningThrottler::new(43200); // 12小时限流

    // 读取游戏列表
    let mut games = read_games_list(GAMES_CONF_PATH)?;
    info!("Loaded {} games from {}", games.len(), GAMES_CONF_PATH);

    // 设置文件监控
    let mut inotify = InotifyWatcher::new()?;

    // 如果游戏列表文件存在，监控它的变化
    if check_read_simple(GAMES_CONF_PATH) {
        inotify.add(GAMES_CONF_PATH, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;
        info!("Watching games list file: {GAMES_CONF_PATH}");
    } else {
        info!("Games list file does not exist: {GAMES_CONF_PATH}");
    }

    // 主循环
    loop {
        // 检查inotify事件，只在游戏列表文件变化时才重新读取
        if let Ok(events) = inotify.check_events()
            && !events.is_empty()
        {
            debug!("Detected changes in games list file");
            games = read_games_list(GAMES_CONF_PATH)?;
            info!(
                "The game configuration file has changed. Loaded {} games.",
                games.len()
            );
        }

        // 获取前台应用
        if app_cache.is_expired(cache_ttl) {
            match get_foreground_app() {
                Ok(package_name) => {
                    // 只有当包名变化时才处理
                    if package_name == app_cache.package_name {
                        return Ok(());
                    }
                    // 将前台应用变化的日志改为debug级别
                    debug!("Foreground app changed: {package_name}");

                    // 检查是否是游戏
                    let is_game = games.contains_key(&package_name); // 将 contains 改为 contains_key

                    // 检查前一个应用是否是游戏
                    let prev_is_game = !app_cache.package_name.is_empty()
                        && games.contains_key(&app_cache.package_name); // 将 contains 改为 contains_key

                    // 只有在游戏模式状态变化时才记录info级别日志
                    if is_game {
                        if !prev_is_game {
                            info!("Game mode enabled: {package_name}");
                        } else {
                            // 游戏切换到另一个游戏时也记录
                            info!("Game changed: {package_name}");
                        }
                    } else if prev_is_game {
                        // 读取全局模式名称用于日志显示
                        let global_mode = match std::fs::read_to_string(CONFIG_TOML_FILE) {
                            Ok(content) => match toml::from_str::<Config>(&content) {
                                Ok(config) => config.global_mode().to_string(),
                                Err(_) => "balance".to_string(), // 默认模式
                            },
                            Err(_) => "balance".to_string(), // 默认模式
                        };
                        info!(
                            "Game mode disabled: switching to global mode ({global_mode}): {package_name}"
                        );
                    }

                    // 根据应用类型写入对应的模式文件
                    if is_game {
                        if let Some(target_mode) = games.get(&package_name) {
                            info!("Game detected, applying {target_mode} mode");
                            if let Err(e) = load_config(&mut gpu, Some(target_mode)) {
                                warn!("Failed to apply game-specific mode: {e}");
                            } else {
                                // 写入当前模式到文件
                                if let Err(e) = write_file(
                                    CURRENT_MODE_PATH,
                                    target_mode.as_bytes(),
                                    target_mode.len(),
                                ) {
                                    warn!("Failed to write current mode file: {e}");
                                }

                                // 通过 channel 发送配置增量到主调频循环
                                if let Some(ref sender) = tx {
                                    match crate::datasource::config_parser::read_config_delta(Some(
                                        target_mode,
                                    )) {
                                        Ok(delta) => {
                                            if sender.send(delta).is_ok() {
                                                info!(
                                                    "Game mode config delta sent to main loop: {}",
                                                    target_mode
                                                );
                                            } else {
                                                warn!("Failed to send game mode config delta");
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to read config delta for game mode: {e}")
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // 当不在游戏中时恢复全局模式
                        if let Err(e) = load_config(&mut gpu, None) {
                            warn!("Failed to revert to global mode: {e}");
                        } else {
                            // 读取全局模式并写入当前模式文件
                            match std::fs::read_to_string(CONFIG_TOML_FILE) {
                                Ok(content) => match toml::from_str::<Config>(&content) {
                                    Ok(config) => {
                                        let global_mode = config.global_mode().to_string();
                                        if let Err(e) = write_file(
                                            CURRENT_MODE_PATH,
                                            global_mode.as_bytes(),
                                            1024,
                                        ) {
                                            warn!("Failed to write current mode file: {e}");
                                        }

                                        // 通过 channel 发送配置增量到主调频循环
                                        if let Some(ref sender) = tx {
                                            match crate::datasource::config_parser::read_config_delta(None) {
                                                    Ok(delta) => {
                                                        if sender.send(delta).is_ok() {
                                                            info!("Global mode config delta sent to main loop: {}", global_mode);
                                                        } else {
                                                            warn!("Failed to send global mode config delta");
                                                        }
                                                    }
                                                    Err(e) => warn!("Failed to read config delta for global mode: {e}"),
                                                }
                                        }
                                    }
                                    Err(e) => warn!("Failed to parse config.toml: {e}"),
                                },
                                Err(e) => warn!("Failed to read config.toml: {e}"),
                            }
                        }
                    }

                    // 更新缓存
                    app_cache.update(package_name);
                }
                Err(e) => {
                    // 使用警告限流器检查是否应该显示警告
                    if warning_throttler.should_warn() {
                        warn!("Failed to get foreground app: {e}");
                    } else {
                        // 如果不应该显示警告，则降级为debug日志
                        debug!("Failed to get foreground app (throttled warning): {e}");
                    }
                }
            }
        }

        // 休眠一段时间
        thread::sleep(Duration::from_millis(1000));
    }
}
