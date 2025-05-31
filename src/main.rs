#![allow(non_snake_case)]
mod datasource;
mod model;
mod utils;

use std::{env, path::Path, thread, time::Duration};

use anyhow::Result;
use log::{error, info, warn};

use crate::{
    datasource::{
        config_parser::config_read,
        file_path::*,
        foreground_app::monitor_foreground_app,
        freq_table::gpufreq_table_init,
        load_monitor::utilization_init,
        node_monitor::{monitor_config, monitor_gaming},
    },
    model::gpu::GPU,
    utils::{
        file_status::get_status,
        log_monitor::monitor_log_level,
        logger::init_logger
    },
};

const NOTES: &str = "Mediatek Mali GPU Governor";
const AUTHOR: &str = "Author: walika @CoolApk, rtools @CoolApk";
const SPECIAL: &str = "Special Thanks: HamJin @CoolApk, asto18089 @CoolApk and helloklf @Github";
const VERSION: &str = "Version: v2.5";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let i = 1;
        match args[i].as_str() {
            "-h" => {
                println!("{}", NOTES);
                println!("{}", AUTHOR);
                println!("{}", SPECIAL);
                println!("Usage:");
                println!("\t-v show version");
                println!("\t-h show help");
                return Ok(());
            }
            "-v" => {
                println!("{}", NOTES);
                println!("{}", AUTHOR);
                println!("{}", SPECIAL);
                println!("{}", VERSION);
                return Ok(());
            }
            _ => {
                println!("Unknown argument: {}", args[i]);
                println!("Use -h for help");
                return Ok(());
            }
        }
    }

    init_logger()?;

    info!("{}", NOTES);
    info!("{}", AUTHOR);
    info!("{}", SPECIAL);
    info!("{}", VERSION);

    // Init
    let mut gpu = GPU::new();
    info!("Loading");

    // 先初始化负载监控
    utilization_init()?;

    // 先从配置文件读取频率表
    let config_file = CONFIG_FILE_TR.to_string();
    if Path::new(&config_file).exists() {
        info!("Reading config file: {}", config_file);
        if let Err(e) = config_read(&config_file, &mut gpu) {
            error!("Failed to read config file: {}", e);
            return Err(anyhow::anyhow!("Failed to read config file: {}", e));
        }
    } else {
        error!("Config file not found: {}", config_file);
        return Err(anyhow::anyhow!("Config file not found: {}", config_file));
    }

    // 然后初始化GPU频率表（只检测驱动类型，不读取系统支持的频率）
    gpufreq_table_init(&mut gpu)?;

    gpu.set_precise(get_status(DEBUG_DVFS_LOAD) || get_status(DEBUG_DVFS_LOAD_OLD));

    // Start monitoring threads
    let gpu_clone1 = gpu.clone();
    thread::spawn(move || {
        if let Err(e) = monitor_gaming(gpu_clone1) {
            error!("Gaming monitor error: {}", e);
        }
    });

    let gpu_clone2 = gpu.clone();
    thread::spawn(move || {
        if let Err(e) = monitor_config(gpu_clone2) {
            error!("Config monitor error: {}", e);
        }
    });

    // 启动前台应用监控线程（延迟一分钟启动）
    thread::spawn(move || {
        // 延迟一分钟后再启动前台应用监控
        info!("Foreground app monitor will start in 60 seconds");
        thread::sleep(Duration::from_secs(60));
        info!("Starting foreground app monitor now");

        if let Err(e) = monitor_foreground_app() {
            error!("Foreground app monitor error: {}", e);
        }
    });

    // 启动日志等级监控线程
    thread::spawn(move || {
        if let Err(e) = monitor_log_level() {
            error!("Log level monitor error: {}", e);
        }
    });

    info!("Monitor Inited");
    thread::sleep(Duration::from_secs(5));

    gpu.set_cur_freq(gpu.get_freq_by_index(0));
    gpu.gen_cur_volt();

    if get_status(DEBUG_DVFS_LOAD) || get_status(DEBUG_DVFS_LOAD_OLD) {
        gpu.set_precise(true);
    }

    // 设置主线程名称
    info!("{} Start", MAIN_THREAD);

    // Bootstrap information
    info!("BootFreq: {}KHz", gpu.get_cur_freq());
    info!(
        "Driver: gpufreq{}",
        if gpu.is_gpuv2() { "v2" } else { "v1" }
    );
    info!(
        "Is Precise: {}",
        if gpu.is_precise() { "Yes" } else { "No" }
    );

    // 显示频率范围信息
    info!("Max Freq: {}KHz", gpu.get_max_freq());
    info!("Middle Freq: {}KHz", gpu.get_middle_freq());
    info!("Min Freq: {}KHz", gpu.get_min_freq());

    // 显示当前余量值
    info!("Current Margin: {}%", gpu.get_margin());

    // 显示DCS状态
    if gpu.is_gpuv2() {
        info!("DCS: {}", if gpu.is_dcs_enabled() { "Enabled" } else { "Disabled" });
        info!("V2 Driver Force Write Threshold: {} times", gpu.get_force_write_threshold());
    }

    // 显示内存频率信息
    if gpu.is_ddr_freq_fixed() {
        info!("DDR Frequency: Fixed at {}", gpu.get_ddr_freq());
    } else {
        info!("DDR Frequency: Auto mode");
    }

    // 显示可用的内存频率选项
    match gpu.get_ddr_freq_table() {
        Ok(freq_table) => {
            info!("Available DDR frequency options:");
            for (i, (opp, desc)) in freq_table.iter().enumerate().take(3) {
                info!("  Option {}: OPP={}, Description: {}", i+1, opp, desc);
            }
            if freq_table.len() > 3 {
                info!("  ... and {} more options", freq_table.len() - 3);
            }
        },
        Err(e) => {
            warn!("Failed to get DDR frequency table: {}", e);
        }
    }

    // 显示v2 driver支持的内存频率和GPU频率
    if gpu.is_gpuv2() {
        let ddr_freqs = gpu.get_ddr_v2_supported_freqs();
        if !ddr_freqs.is_empty() {
            info!("V2 driver supported DDR frequencies: {:?}", ddr_freqs);
        }

        let gpu_freqs = gpu.get_v2_supported_freqs();
        if !gpu_freqs.is_empty() {
            info!("V2 driver supported GPU frequencies: {:?}", gpu_freqs);
        }
    }

    // 显示第二高频率，用于性能模式
    info!("Second highest frequency: {}KHz", gpu.get_second_highest_freq());

    // 显示日志文件路径
    info!("Log level file path: {}", LOG_LEVEL_PATH);

    // 显示当前升频延迟和降频阈值
    info!("Current Up Rate Delay: {}ms", gpu.get_up_rate_delay());
    info!("Current Down Threshold: {}", gpu.get_down_threshold());

    // 显示当前负载趋势
    let trend_desc = match gpu.get_load_trend() {
        1 => "Rising",
        -1 => "Falling",
        _ => "Stable"
    };
    info!("Current Load Trend: {}", trend_desc);

    // 设置新的调速器参数
    // 游戏模式下使用更激进的升频策略，普通模式下使用更激进的降频策略
    if gpu.is_gaming_mode() {
        // 游戏模式：更激进的升频，更保守的降频
        gpu.set_load_thresholds(5, 20, 60, 85); // 更低的高负载阈值，更快进入高负载区域
        gpu.set_load_stability_threshold(2);    // 更低的稳定性阈值，更快响应负载变化
        gpu.set_aggressive_down(false);         // 禁用激进降频，保持性能

        // 设置游戏模式的滞后阈值和去抖动时间
        gpu.set_hysteresis_thresholds(65, 40);  // 游戏模式使用更宽松的滞后阈值，更容易升频
        gpu.set_debounce_times(10, 30);         // 游戏模式使用更短的去抖动时间，更快响应

        // 设置游戏模式的自适应采样参数
        gpu.set_adaptive_sampling(true, 8, 50); // 游戏模式使用更短的采样间隔范围

        info!("Game mode detected: Using performance-oriented governor settings");
        info!("Game mode hysteresis: up=65%, down=40%, debounce: up=10ms, down=30ms");
    } else {
        // 普通模式：更保守的升频，更激进的降频
        gpu.set_load_thresholds(10, 30, 70, 90); // 默认负载阈值
        gpu.set_load_stability_threshold(3);     // 默认稳定性阈值
        gpu.set_aggressive_down(true);           // 启用激进降频，节省功耗

        // 设置普通模式的滞后阈值和去抖动时间
        gpu.set_hysteresis_thresholds(75, 30);   // 普通模式使用更严格的滞后阈值，更难升频
        gpu.set_debounce_times(20, 50);          // 普通模式使用更长的去抖动时间，更稳定

        // 设置普通模式的自适应采样参数
        gpu.set_adaptive_sampling(true, 10, 100); // 普通模式使用更宽的采样间隔范围

        info!("Normal mode detected: Using power-saving governor settings");
        info!("Normal mode hysteresis: up=75%, down=30%, debounce: up=20ms, down=50ms");
    }

    // 设置基础采样间隔
    gpu.set_sampling_interval(16); // 16ms采样间隔，约60Hz

    // 设置余量值为5%
    gpu.set_margin(5);

    // 检查GPU频率限制文件
    info!("Checking GPU frequency limit files:");
    if Path::new(GEDFREQ_MAX).exists() {
        info!("  Max frequency limit file: {} (Found)", GEDFREQ_MAX);
    } else {
        info!("  Max frequency limit file: {} (Not Found)", GEDFREQ_MAX);
    }

    if Path::new(GEDFREQ_MIN).exists() {
        info!("  Min frequency limit file: {} (Found)", GEDFREQ_MIN);
    } else {
        info!("  Min frequency limit file: {} (Not Found)", GEDFREQ_MIN);
    }

    // 检查GPU电源策略文件
    if Path::new(GPU_POWER_POLICY).exists() {
        info!("GPU power policy file: {} (Found)", GPU_POWER_POLICY);
        // 读取当前电源策略
        if let Ok(content) = std::fs::read_to_string(GPU_POWER_POLICY) {
            info!("Current GPU power policy: {}", content.trim());
        }
    } else {
        info!("GPU power policy file: {} (Not Found)", GPU_POWER_POLICY);
    }

    // 检查FPS状态文件
    if Path::new(FPS_STATUS).exists() {
        info!("FPS status file: {} (Found)", FPS_STATUS);
        if let Ok(content) = std::fs::read_to_string(FPS_STATUS) {
            info!("Current FPS status: {}", content.trim());
        }
    } else {
        info!("FPS status file: {} (Not Found)", FPS_STATUS);
    }

    // 检查前台进程ID文件
    if Path::new(TOP_PID).exists() {
        info!("Top process ID file: {} (Found)", TOP_PID);
    } else {
        info!("Top process ID file: {} (Not Found)", TOP_PID);
    }

    // 显示频率写入器线程名称
    info!("Frequency writer thread name: {}", FW);

    // 显示GED锁定器名称
    info!("GED locker name: {}", GED_LOCKER);

    info!("Advanced GPU Governor Started");

    // Adjust GPU frequency
    gpu.adjust_gpufreq()
}
