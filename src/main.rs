mod datasource;
mod model;
mod utils;

use std::env;
use std::path::Path;
use std::process;
use std::thread;
use std::time::Duration;

use log::{info, warn, error};
use anyhow::{Result, Context};

use crate::datasource::file_path::*;
use crate::datasource::freq_table::gpufreq_table_init;
use crate::datasource::load_monitor::utilization_init;
use crate::datasource::node_monitor::{monitor_gaming, monitor_config};
use crate::datasource::foreground_app::monitor_foreground_app;
use crate::datasource::config_parser::{config_read, gen_default_freq_table};
use crate::model::gpu::GPU;
use crate::utils::file_operate::{check_read, write_file};
use crate::utils::file_status::get_status;
use crate::utils::logger::init_logger;

const NOTES: &str = "Mediatek Mali GPU Governor";
const AUTHOR: &str = "Author: walika @CoolApk";
const SPECIAL: &str = "Special Thanks: HamJin @CoolApk, asto18089 @CoolApk and helloklf @Github";
const VERSION: &str = "Version: v2.1";

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
                },
                "-v" => {
                    println!("{}", NOTES);
                    println!("{}", AUTHOR);
                    println!("{}", SPECIAL);
                    println!("{}", VERSION);
                    return Ok(());
                },
                _ => {}
            }
        }
    }

    main_func()
}

fn main_func() -> Result<()> {
    // Initialize logger
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
    let gaming_handle = thread::spawn(move || {
        if let Err(e) = monitor_gaming(gpu_clone1) {
            error!("Gaming monitor error: {}", e);
        }
    });

    let gpu_clone2 = gpu.clone();
    let config_handle = thread::spawn(move || {
        if let Err(e) = monitor_config(gpu_clone2) {
            error!("Config monitor error: {}", e);
        }
    });

    // 启动前台应用监控线程
    let gpu_clone3 = gpu.clone();
    let foreground_app_handle = thread::spawn(move || {
        if let Err(e) = monitor_foreground_app(gpu_clone3) {
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
    info!("Driver: gpufreq{}", if gpu.is_gpuv2() { "v2" } else { "v1" });
    info!("Is Precise: {}", if gpu.is_precise() { "Yes" } else { "No" });
    info!("Governor Started");

    // Adjust GPU frequency
    gpu.adjust_gpufreq()
}

fn lock_ged() -> Result<()> {
    // Set thread name
    // Note: In Rust, we can't set thread name for current thread directly
    // We would need to use nix or libc bindings for this

    let idm = "99";
    let idM = "0";

    info!("Locking GED Freq");

    loop {
        write_file(GEDFREQ_MIN, idm, idm.len())?;
        write_file(GEDFREQ_MAX, idM, idM.len())?;
        thread::sleep(Duration::from_secs(1));
    }
}
