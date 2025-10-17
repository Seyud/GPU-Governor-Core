mod datasource;
mod model;
mod utils;

use std::{fs, path::Path, thread, time::Duration};

use anyhow::Result;
use log::{error, info, warn};

use crate::{
    datasource::{
        config_parser::{ConfigDelta, load_config, read_config_delta},
        file_path::*,
        foreground_app::monitor_foreground_app,
        freq_table::gpufreq_table_init,
        freq_table_parser::freq_table_read,
        load_monitor::utilization_init,
        node_monitor::{monitor_custom_config, monitor_freq_table_config},
    },
    model::gpu::GPU,
    utils::{
        constants::strategy, file_status::get_status,
        log_level_manager::start_unified_log_level_monitor, logger::init_logger,
    },
};

/// 初始化GPU配置
fn initialize_gpu_config(gpu: &mut GPU) -> Result<()> {
    // 先初始化负载监控
    utilization_init()?;

    // 读取频率表配置文件
    if fs::exists(FREQ_TABLE_CONFIG_FILE)? {
        info!("Reading frequency table config file: {config_file}");
        freq_table_read(FREQ_TABLE_CONFIG_FILE, gpu)
            .map_err(|e| anyhow::anyhow!("Failed to read frequency table config file: {}", e))?;
    } else {
        return Err(anyhow::anyhow!(
            "Frequency table config file not found: {}",
            FREQ_TABLE_CONFIG_FILE
        ));
    }

    // 尝试加载TOML策略配置
    if fs::exists(CONFIG_TOML_FILE)? {
        info!("Reading TOML config file: {CONFIG_TOML_FILE}");
        if let Err(e) = load_config(gpu, None) {
            warn!("Failed to load TOML config: {e}, using default settings");
        }
    } else {
        warn!("TOML config file not found: {CONFIG_TOML_FILE}, using default settings");
    }

    // 初始化GPU频率表
    gpufreq_table_init(gpu)?;

    // 设置精确模式
    gpu.set_precise(get_status(DEBUG_DVFS_LOAD) || get_status(DEBUG_DVFS_LOAD_OLD));

    Ok(())
}

/// 启动监控线程
fn start_monitoring_threads(gpu: GPU, tx: std::sync::mpsc::Sender<ConfigDelta>) {
    // 频率表配置监控线程
    let gpu_clone2 = gpu.clone();
    thread::Builder::new()
        .name(FREQ_TABLE_MONITOR_THREAD.to_string())
        .spawn(move || {
            if let Err(e) = monitor_freq_table_config(gpu_clone2) {
                error!("Frequency table config monitor error: {e}");
            }
        })
        .expect("Failed to spawn frequency table config monitor thread");

    // 前台应用监控线程（延迟启动）
    let gpu_clone = gpu.clone();
    let tx_clone = tx.clone(); // 克隆 sender 用于前台应用监控
    thread::Builder::new()
        .name(FOREGROUND_APP_THREAD.to_string())
        .spawn(move || {
            info!(
                "Foreground app monitor will start in {} seconds",
                strategy::FOREGROUND_APP_STARTUP_DELAY
            );
            thread::sleep(Duration::from_secs(strategy::FOREGROUND_APP_STARTUP_DELAY));
            info!("Starting foreground app monitor now");

            if let Err(e) = monitor_foreground_app(gpu_clone, Some(tx_clone)) {
                error!("Foreground app monitor error: {e}");
            }
        })
        .expect("Failed to spawn foreground app monitor thread");

    // 统一的日志等级监控线程
    thread::Builder::new()
        .name(LOG_LEVEL_MONITOR_THREAD.to_string())
        .spawn(move || {
            if let Err(e) = start_unified_log_level_monitor() {
                error!("Unified log level monitor error: {e}");
            }
        })
        .expect("Failed to spawn log level monitor thread");

    // 自定义配置监控线程
    let tx_clone = tx.clone();
    thread::Builder::new()
        .name(CONFIG_MONITOR_THREAD.to_string())
        .spawn(move || {
            if let Err(e) = monitor_custom_config(tx_clone) {
                error!("Custom config monitor error: {e}");
            }
        })
        .expect("Failed to spawn custom config monitor thread");
}

/// 显示系统信息
fn display_system_info(gpu: &GPU) {
    info!("Monitor Inited");
    info!("{MAIN_THREAD} Start");

    // 频率信息
    info!("BootFreq: {}KHz", gpu.get_cur_freq());
    info!(
        "Driver: gpufreq{}",
        if gpu.is_gpuv2() { "v2" } else { "v1" }
    );
    info!(
        "Is Precise: {}",
        if gpu.is_precise() { "Yes" } else { "No" }
    );
    info!("Max Freq: {}KHz", gpu.get_max_freq());
    info!("Middle Freq: {}KHz", gpu.get_middle_freq());
    info!("Min Freq: {}KHz", gpu.get_min_freq());
    info!("Current Margin: {}%", gpu.get_margin());

    // DCS信息
    if gpu.is_gpuv2() {
        info!(
            "DCS: {}",
            if gpu.is_dcs_enabled() {
                "Enabled"
            } else {
                "Disabled"
            }
        );
    }

    // DDR频率信息
    display_ddr_info(gpu);
}

/// 显示DDR相关信息
fn display_ddr_info(gpu: &GPU) {
    if gpu.is_ddr_freq_fixed() {
        info!(
            "DDR Frequency: Fixed at {}",
            gpu.ddr_manager().get_ddr_freq()
        );
    } else {
        info!("DDR Frequency: Auto mode");
    }

    match gpu.ddr_manager().get_ddr_freq_table() {
        Ok(freq_table) => {
            info!("Available DDR frequency options:");
            for (i, (opp, desc)) in freq_table.iter().enumerate().take(3) {
                info!("  Option {}: OPP={}, Description: {}", i + 1, opp, desc);
            }
            if freq_table.len() > 3 {
                info!("  ... and {} more options", freq_table.len() - 3);
            }
        }
        Err(e) => {
            warn!("Failed to get DDR frequency table: {e}");
        }
    }

    if gpu.is_gpuv2() {
        let ddr_freqs = gpu.ddr_manager().get_ddr_v2_supported_freqs();
        if !ddr_freqs.is_empty() {
            info!("V2 driver supported DDR frequencies: {ddr_freqs:?}");
        }

        let gpu_freqs = gpu.get_v2_supported_freqs();
        if !gpu_freqs.is_empty() {
            info!("V2 driver supported GPU frequencies: {gpu_freqs:?}");
        }
    }
}

fn main() -> Result<()> {
    // 设置主线程名称（使用pthread_setname_np）
    unsafe {
        let name = std::ffi::CString::new(MAIN_THREAD).unwrap();
        let result = libc::pthread_setname_np(libc::pthread_self(), name.as_ptr());
        if result != 0 {
            eprintln!("Warning: Failed to set main thread name: {result}");
        }
    }

    // 初始化日志
    init_logger()?;

    // 版本信息写入到日志文件
    info!("{}", crate::utils::constants::NOTES);
    info!("{}", crate::utils::constants::AUTHOR);
    info!("{}", crate::utils::constants::SPECIAL);
    info!("{}", crate::utils::constants::VERSION);

    // 初始化GPU
    let mut gpu = GPU::new();
    info!("Loading");

    // 初始化GPU配置
    initialize_gpu_config(&mut gpu)?;

    // 启动监控线程
    let (tx, rx) = std::sync::mpsc::channel::<ConfigDelta>();
    start_monitoring_threads(gpu.clone(), tx);

    // 发送一次初始配置增量（非必须，保证与初始化加载一致）
    if let Ok(delta) = read_config_delta(None) {
        gpu.apply_config_delta(&delta);
    }

    // 等待线程启动
    thread::sleep(Duration::from_secs(5));

    // 初始化频率和电压
    gpu.set_cur_freq(gpu.get_freq_by_index(0));
    gpu.frequency_mut().gen_cur_volt();

    // 显示系统信息
    display_system_info(&gpu);

    info!("Advanced GPU Governor Started");

    // 开始频率调整
    gpu.adjust_gpufreq_with_updates(rx)
}
