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
            gpu.set_gaming_mode(value != 0);
            info!("Initial game mode value: {}", value);
        } else {
            info!("Failed to read initial game mode value, defaulting to non-gaming mode");
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
