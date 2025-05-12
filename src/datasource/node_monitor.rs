use anyhow::Result;
use inotify::WatchMask;
use log::{debug, error, info, warn};

use crate::{
    datasource::{
        config_parser::{config_read, gen_default_freq_table},
        file_path::*,
    },
    model::gpu::GPU,
    utils::{
        file_operate::{check_read_simple, read_file},
        inotify::InotifyWatcher,
    },
};

// 定义游戏模式和普通模式的升频延迟常量
const GAME_MODE_UP_RATE_DELAY: u64 = 20; // 游戏模式使用20ms的升频延迟
const NORMAL_MODE_UP_RATE_DELAY: u64 = 50; // 普通模式使用50ms的升频延迟

// 定义游戏模式和普通模式的降频阈值常量
const GAME_MODE_DOWN_THRESHOLD: i64 = 27; // 游戏模式保持原有的27次阈值
const NORMAL_MODE_DOWN_THRESHOLD: i64 = 10; // 普通模式使用更低的10次阈值，更积极降频

pub fn monitor_gaming(mut gpu: GPU) -> Result<()> {
    // Set thread name (in Rust we can't set the current thread name easily)
    info!("{} Start", GAME_THREAD);

    // 默认设置为非游戏模式
    gpu.set_gaming_mode(false);

    // 检查游戏模式文件路径
    if !check_read_simple(GPU_GOVERNOR_GAME_MODE_PATH) {
        // 如果文件不存在，记录日志
        info!(
            "Game mode file does not exist: {}",
            GPU_GOVERNOR_GAME_MODE_PATH
        );
    } else {
        info!("Using game mode path: {}", GPU_GOVERNOR_GAME_MODE_PATH);

        // 初始读取游戏模式状态
        if let Ok(buf) = read_file(GPU_GOVERNOR_GAME_MODE_PATH, 3) {
            let value = buf.trim().parse::<i32>().unwrap_or(0);
            let is_gaming = value != 0;
            gpu.set_gaming_mode(is_gaming);

            // 根据初始游戏模式设置不同的升频延迟和降频阈值
            let up_rate_delay = if is_gaming {
                GAME_MODE_UP_RATE_DELAY
            } else {
                NORMAL_MODE_UP_RATE_DELAY
            };

            let down_threshold = if is_gaming {
                GAME_MODE_DOWN_THRESHOLD
            } else {
                NORMAL_MODE_DOWN_THRESHOLD
            };

            gpu.set_up_rate_delay(up_rate_delay);
            gpu.set_down_threshold(down_threshold);
            info!("Initial game mode {}", if is_gaming { "enabled" } else { "disabled" });

            // 设置初始高级调速器参数
            if is_gaming {
                // 游戏模式：更激进的升频，更保守的降频
                gpu.set_load_thresholds(5, 20, 60, 85); // 更低的高负载阈值，更快进入高负载区域
                gpu.set_load_stability_threshold(2);    // 更低的稳定性阈值，更快响应负载变化
                gpu.set_aggressive_down(false);         // 禁用激进降频，保持性能
                info!("Initial game mode enabled: Using performance-oriented governor settings");
            } else {
                // 普通模式：更保守的升频，更激进的降频
                gpu.set_load_thresholds(10, 30, 70, 90); // 默认负载阈值
                gpu.set_load_stability_threshold(3);     // 默认稳定性阈值
                gpu.set_aggressive_down(true);           // 启用激进降频，节省功耗
                info!("Initial game mode disabled: Using power-saving governor settings");
            }

            info!("Initial game mode value: {}", value);
        } else {
            info!("Failed to read initial game mode value, defaulting to non-gaming mode");
            // 默认为普通模式
            gpu.set_up_rate_delay(NORMAL_MODE_UP_RATE_DELAY);
            gpu.set_down_threshold(NORMAL_MODE_DOWN_THRESHOLD);
            info!("Setting default up rate delay to {}ms, down threshold to {}",
                 NORMAL_MODE_UP_RATE_DELAY, NORMAL_MODE_DOWN_THRESHOLD);

            // 设置默认高级调速器参数（普通模式）
            gpu.set_load_thresholds(10, 30, 70, 90); // 默认负载阈值
            gpu.set_load_stability_threshold(3);     // 默认稳定性阈值
            gpu.set_aggressive_down(true);           // 启用激进降频，节省功耗
            info!("Default mode: Using power-saving governor settings");
        }
    }

    // 设置文件监控
    let mut inotify = InotifyWatcher::new()?;
    inotify.add(
        GPU_GOVERNOR_GAME_MODE_PATH,
        WatchMask::CLOSE_WRITE | WatchMask::MODIFY,
    )?;

    // 主循环
    loop {
        inotify.wait_and_handle()?;

        // 检查文件是否存在
        if !check_read_simple(GPU_GOVERNOR_GAME_MODE_PATH) {
            // 如果文件不存在，设置为非游戏模式
            gpu.set_gaming_mode(false);
            debug!("Game mode file no longer exists, setting to non-gaming mode");
            continue;
        }

        // 读取文件内容
        match read_file(GPU_GOVERNOR_GAME_MODE_PATH, 3) {
            Ok(buf) => {
                let value = buf.trim().parse::<i32>().unwrap_or(0);
                let is_gaming = value != 0;
                gpu.set_gaming_mode(is_gaming);

                // 根据游戏模式设置不同的升频延迟和降频阈值
                let up_rate_delay = if is_gaming {
                    GAME_MODE_UP_RATE_DELAY
                } else {
                    NORMAL_MODE_UP_RATE_DELAY
                };

                let down_threshold = if is_gaming {
                    GAME_MODE_DOWN_THRESHOLD
                } else {
                    NORMAL_MODE_DOWN_THRESHOLD
                };

                gpu.set_up_rate_delay(up_rate_delay);
                gpu.set_down_threshold(down_threshold);
                debug!("Game mode {}", if is_gaming { "enabled" } else { "disabled" });

                // 更新高级调速器参数
                if is_gaming {
                    // 游戏模式：更激进的升频，更保守的降频
                    gpu.set_load_thresholds(5, 20, 60, 85); // 更低的高负载阈值，更快进入高负载区域
                    gpu.set_load_stability_threshold(2);    // 更低的稳定性阈值，更快响应负载变化
                    gpu.set_aggressive_down(false);         // 禁用激进降频，保持性能
                    info!("Game mode enabled: Using performance-oriented governor settings");
                } else {
                    // 普通模式：更保守的升频，更激进的降频
                    gpu.set_load_thresholds(10, 30, 70, 90); // 默认负载阈值
                    gpu.set_load_stability_threshold(3);     // 默认稳定性阈值
                    gpu.set_aggressive_down(true);           // 启用激进降频，节省功耗
                    info!("Game mode disabled: Using power-saving governor settings");
                }

                debug!("Game mode changed: {}", is_gaming);
            }
            Err(e) => {
                warn!("Failed to read game mode file: {}", e);
                // 如果读取失败，设置为非游戏模式
                gpu.set_gaming_mode(false);
            }
        }
    }
}

pub fn monitor_config(mut gpu: GPU) -> Result<()> {
    // Set thread name (in Rust we can't set the current thread name easily)
    info!("{} Start", CONF_THREAD);

    // 只使用 CONFIG_FILE_TR 配置文件
    let config_file = CONFIG_FILE_TR.to_string();

    // 检查配置文件是否存在
    if !check_read_simple(&config_file) {
        error!("CONFIG NOT FOUND: {}", std::io::Error::last_os_error());
        warn!("Using default freq table");
        gen_default_freq_table(&mut gpu)?;
        return Ok(());
    };

    info!("Using Config: {}", config_file);

    let mut inotify = InotifyWatcher::new()?;
    inotify.add(&config_file, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;

    // Initial read of config
    config_read(&config_file, &mut gpu)?;

    loop {
        inotify.wait_and_handle()?;
        config_read(&config_file, &mut gpu)?;
    }
}
