use std::sync::mpsc::Sender;

use anyhow::Result;
use inotify::WatchMask;
use log::{error, info, warn};

use crate::{
    datasource::{
        config_parser::{Config, ConfigDelta, read_config_delta},
        file_path::*,
        freq_table_parser::freq_table_read,
    },
    model::gpu::GPU,
    utils::{
        file_operate::{check_read_simple, write_file},
        inotify::InotifyWatcher,
    },
};

pub fn monitor_freq_table_config(mut gpu: GPU) -> Result<()> {
    // 设置线程名称（在Rust中无法轻易设置当前线程名称）
    info!("{FREQ_TABLE_MONITOR_THREAD} Start");

    // 使用频率表配置文件
    let config_path = std::path::Path::new(FREQ_TABLE_CONFIG_FILE);
    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("/"));
    let config_filename = config_path
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("gpu_freq_table.toml"))
        .to_string_lossy()
        .to_string();

    // 检查频率表配置文件是否存在
    if !check_read_simple(FREQ_TABLE_CONFIG_FILE) {
        error!("CONFIG NOT FOUND: {}", std::io::Error::last_os_error());
        // 即使文件不存在，也继续尝试监控目录
    }

    info!("Using Config: {FREQ_TABLE_CONFIG_FILE}");

    // 使用read_freq_ge和read_freq_le方法获取频率范围
    let min_freq = gpu.get_min_freq();
    let max_freq = gpu.get_max_freq();
    // 使用read_freq_ge方法获取大于等于特定频率的最小频率
    let target_freq = 600000; // 600MHz
    let _ge_freq = gpu.read_freq_ge(target_freq);
    // 使用read_freq_le方法获取小于等于特定频率的最大频率
    let target_freq2 = 800000; // 800MHz
    let _le_freq = gpu.read_freq_le(target_freq2);

    // 从GPU对象获取margin值
    let margin = gpu.get_margin();

    info!("Config values: min_freq={min_freq}KHz, max_freq={max_freq}KHz, margin={margin}%");

    let mut inotify = InotifyWatcher::new()?;
    // 监听目录的 MOVED_TO 和 CLOSE_WRITE
    inotify.add(
        config_dir,
        WatchMask::MOVED_TO | WatchMask::CLOSE_WRITE | WatchMask::MODIFY,
    )?;

    // 初始读取频率表配置
    if check_read_simple(FREQ_TABLE_CONFIG_FILE) {
        freq_table_read(FREQ_TABLE_CONFIG_FILE, &mut gpu)?;
    }

    loop {
        let events = inotify.wait_and_handle()?;

        // 检查是否有针对配置文件的事件
        let mut config_changed = false;
        for event in events {
            if let Some(name) = &event.name
                && name == &config_filename
            {
                config_changed = true;
                break;
            }
        }

        if config_changed {
            info!("Detected change in freq table config: {FREQ_TABLE_CONFIG_FILE}");
            freq_table_read(FREQ_TABLE_CONFIG_FILE, &mut gpu)?;
        }
    }
}

pub fn monitor_custom_config(tx: Sender<ConfigDelta>) -> Result<()> {
    // 设置线程名称
    info!("{CONFIG_MONITOR_THREAD} Start");

    // 使用自定义配置文件
    let config_path = std::path::Path::new(CONFIG_TOML_FILE);
    let config_dir = config_path.parent().unwrap_or(std::path::Path::new("/"));
    let config_filename = config_path
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("config.toml"))
        .to_string_lossy()
        .to_string();

    // 检查自定义配置文件是否存在
    if !check_read_simple(CONFIG_TOML_FILE) {
        warn!("Custom config file not found: {CONFIG_TOML_FILE}");
        // 即使文件不存在，我们也应该监控目录，以便文件被创建时能检测到
    }

    info!(
        "Monitoring custom config directory: {}",
        config_dir.display()
    );

    let mut inotify = InotifyWatcher::new()?;
    // 监听目录的 MOVED_TO (mv覆盖) 和 CLOSE_WRITE (直接编辑)
    // 注意：InotifyWatcher::add 会自动添加 DELETE_SELF 和 MOVE_SELF，这对目录监控也是有用的
    inotify.add(config_dir, WatchMask::MOVED_TO | WatchMask::CLOSE_WRITE)?;

    // 记录上一次的全局模式（启动时读取一次，失败则留空）
    let mut last_mode: Option<String> = std::fs::read_to_string(CONFIG_TOML_FILE)
        .ok()
        .and_then(|c| toml::from_str::<Config>(&c).ok())
        .map(|cfg| cfg.global_mode().to_string());

    loop {
        // 等待事件
        let events = inotify.wait_and_handle()?;

        // 检查是否有针对 config.toml 的事件
        let mut config_changed = false;
        for event in events {
            if let Some(name) = &event.name
                && name == &config_filename
            {
                config_changed = true;
                break;
            }
        }

        if !config_changed {
            continue;
        }

        info!("Detected change in config file: {CONFIG_TOML_FILE}");

        // 先发送参数增量
        match read_config_delta(None) {
            Ok(delta) => {
                if tx.send(delta).is_ok() {
                    info!("Custom config delta sent");
                }
            }
            Err(e) => warn!("Failed to parse custom config: {e}"),
        }

        // 检测全局模式是否变化，若变化则更新 CURRENT_MODE_PATH
        match std::fs::read_to_string(CONFIG_TOML_FILE) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(cfg) => {
                    let mode_now = cfg.global_mode().to_string();
                    if last_mode.as_deref() != Some(mode_now.as_str()) {
                        // 更新文件
                        match write_file(CURRENT_MODE_PATH, mode_now.as_bytes(), 1024) {
                            Ok(_) => info!(
                                "Global mode changed -> {mode_now}, current_mode file updated"
                            ),
                            Err(e) => warn!("Failed to write current_mode file: {e}"),
                        }
                        last_mode = Some(mode_now);
                    }
                }
                Err(e) => warn!("Failed to parse config.toml when checking mode change: {e}"),
            },
            Err(e) => warn!("Failed to read config.toml when checking mode change: {e}"),
        }
    }
}
