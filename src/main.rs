#![allow(non_snake_case)]
mod datasource;
mod model;
mod utils;

use std::{env, path::Path, thread, time::Duration};

use anyhow::Result;
use log::{error, info, warn};

use crate::{
    datasource::{
        config_parser::{config_read, gen_default_freq_table},
        file_path::*,
        foreground_app::monitor_foreground_app,
        freq_table::gpufreq_table_init,
        load_monitor::utilization_init,
        node_monitor::{monitor_config, monitor_gaming},
    },
    model::gpu::GPU,
    utils::{file_status::get_status, logger::init_logger},
};

const NOTES: &str = "Mediatek Mali GPU Governor";
const AUTHOR: &str = "Author: walika @CoolApk, rtools @CoolApk";
const SPECIAL: &str = "Special Thanks: HamJin @CoolApk, asto18089 @CoolApk and helloklf @Github";
const VERSION: &str = "Version: v2.2";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        for arg in &args[1..] {
            match arg.as_str() {
                "-h" => {
                    println!("{}", NOTES);
                    println!("{}", AUTHOR);
                    println!("{}", SPECIAL);
                    println!("Usage:\n\t-v show version\n\t-h show help");
                    return Ok(());
                }
                "-v" => {
                    println!("{}", NOTES);
                    println!("{}", AUTHOR);
                    println!("{}", SPECIAL);
                    println!("{}", VERSION);
                    return Ok(());
                }
                _ => {}
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
            warn!("Failed to read config file: {}", e);
            // 如果配置文件读取失败，生成默认配置
            gen_default_freq_table(&mut gpu)?;
        }
    } else {
        warn!("Config file not found: {}", config_file);
        // 如果配置文件不存在，生成默认配置
        gen_default_freq_table(&mut gpu)?;
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

    // 启动前台应用监控线程
    thread::spawn(move || {
        if let Err(e) = monitor_foreground_app() {
            error!("Foreground app monitor error: {}", e);
        }
    });

    info!("Monitor Inited");
    thread::sleep(Duration::from_secs(5));

    gpu.set_cur_freq(gpu.get_freq_by_index(0));
    gpu.gen_cur_volt();

    if get_status(DEBUG_DVFS_LOAD) || get_status(DEBUG_DVFS_LOAD_OLD) {
        gpu.set_precise(true);
    }

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

    // 初始升频延迟将由游戏模式监控线程设置
    info!("Up Rate Delay will be set based on game mode");

    info!("Governor Started");

    // Adjust GPU frequency
    gpu.adjust_gpufreq()
}
