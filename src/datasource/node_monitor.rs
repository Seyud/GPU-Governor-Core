use anyhow::Result;
use inotify::WatchMask;
use log::{error, info};

use crate::{
    datasource::{file_path::*, freq_table_parser::freq_table_read},
    model::gpu::GPU,
    utils::{file_operate::check_read_simple, inotify::InotifyWatcher},
};

pub fn monitor_config(mut gpu: GPU) -> Result<()> {
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
