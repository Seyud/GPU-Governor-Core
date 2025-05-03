use std::path::Path;

use anyhow::Result;
use log::{info, warn};

use crate::{datasource::file_path::*, model::gpu::GPU, utils::file_operate::check_read_simple};

// 检测GPU驱动类型，但不读取系统支持的频率表
fn detect_gpu_driver_type(gpu: &mut GPU) -> Result<()> {
    // 检查v1驱动的电压和频率控制文件
    let v1_volt_exists = Path::new(GPUFREQ_VOLT).exists() && check_read_simple(GPUFREQ_VOLT);
    let v1_opp_exists = Path::new(GPUFREQ_OPP).exists() && check_read_simple(GPUFREQ_OPP);

    // 检查v2驱动的电压和频率控制文件
    let v2_volt_exists = Path::new(GPUFREQV2_VOLT).exists() && check_read_simple(GPUFREQV2_VOLT);
    let v2_opp_exists = Path::new(GPUFREQV2_OPP).exists() && check_read_simple(GPUFREQV2_OPP);

    // 记录检测到的文件
    info!("GPU Driver Files Detection:");
    info!(
        "  V1 Voltage File: {}",
        if v1_volt_exists { "Found" } else { "Not Found" }
    );
    info!(
        "  V1 Frequency File: {}",
        if v1_opp_exists { "Found" } else { "Not Found" }
    );
    info!(
        "  V2 Voltage File: {}",
        if v2_volt_exists { "Found" } else { "Not Found" }
    );
    info!(
        "  V2 Frequency File: {}",
        if v2_opp_exists { "Found" } else { "Not Found" }
    );

    // 检查v1驱动
    if v1_volt_exists || v1_opp_exists {
        gpu.set_gpuv2(false);
        gpu.set_dcs_enable(false);
        info!("Detected gpufreq Driver (v1)");

        // 警告如果某些文件不存在
        if !v1_volt_exists {
            warn!("V1 voltage control file not found: {}", GPUFREQ_VOLT);
        }
        if !v1_opp_exists {
            warn!("V1 frequency control file not found: {}", GPUFREQ_OPP);
        }

        return Ok(());
    }

    // 检查v2驱动
    if v2_volt_exists || v2_opp_exists {
        gpu.set_gpuv2(true);
        gpu.set_dcs_enable(true);
        info!("Detected gpufreqv2 Driver (v2)");

        // 警告如果某些文件不存在
        if !v2_volt_exists {
            warn!("V2 voltage control file not found: {}", GPUFREQV2_VOLT);
        }
        if !v2_opp_exists {
            warn!("V2 frequency control file not found: {}", GPUFREQV2_OPP);
        }

        return Ok(());
    }

    // 如果没有检测到任何驱动，默认使用v1
    warn!("No valid GPU frequency driver detected, defaulting to gpufreq (v1)");
    warn!("The program may not be able to control GPU frequency!");
    gpu.set_gpuv2(false);
    gpu.set_dcs_enable(false);

    Ok(())
}

pub fn gpufreq_table_init(gpu: &mut GPU) -> Result<()> {
    // 只检测GPU驱动类型，不读取系统支持的频率表
    detect_gpu_driver_type(gpu)?;

    // 输出当前配置信息
    info!("Using frequencies from config file only");

    // 输出当前频率表信息
    let config_list = gpu.get_config_list();
    if !config_list.is_empty() {
        if let Some(&max_freq) = config_list.first() {
            info!("Config Max Freq: {}", max_freq);
        }

        if let Some(&min_freq) = config_list.last() {
            info!("Config Min Freq: {}", min_freq);
        }

        info!("Config Frequencies Total: {}", config_list.len());
    } else {
        warn!("No frequencies in config list yet");
    }

    Ok(())
}
