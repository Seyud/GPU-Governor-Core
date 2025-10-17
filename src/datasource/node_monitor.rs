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
    let config_file = FREQ_TABLE_CONFIG_FILE.to_string();

    // 检查频率表配置文件是否存在
    if !check_read_simple(&config_file) {
        error!("CONFIG NOT FOUND: {}", std::io::Error::last_os_error());
        return Err(anyhow::anyhow!(
            "Frequency table config file not found: {}",
            config_file
        ));
    };

    info!("Using Config: {config_file}");

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
    inotify.add(&config_file, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;

    // 初始读取频率表配置
    freq_table_read(&config_file, &mut gpu)?;

    loop {
        inotify.wait_and_handle()?;
        freq_table_read(&config_file, &mut gpu)?;
    }
}

pub fn monitor_custom_config(tx: Sender<ConfigDelta>) -> Result<()> {
    // 设置线程名称
    info!("{CONFIG_MONITOR_THREAD} Start");

    // 使用自定义配置文件
    let config_file = CONFIG_TOML_FILE.to_string();

    // 检查自定义配置文件是否存在
    if !check_read_simple(&config_file) {
        warn!("Custom config file not found: {config_file}");
        return Ok(());
    }

    info!("Monitoring custom config: {config_file}");

    let mut inotify = InotifyWatcher::new()?;
    inotify.add(&config_file, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;

    // 记录上一次的全局模式（启动时读取一次，失败则留空）
    let mut last_mode: Option<String> = std::fs::read_to_string(&config_file)
        .ok()
        .and_then(|c| toml::from_str::<Config>(&c).ok())
        .map(|cfg| cfg.global_mode().to_string());

    loop {
        inotify.wait_and_handle()?;

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
        match std::fs::read_to_string(&config_file) {
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
