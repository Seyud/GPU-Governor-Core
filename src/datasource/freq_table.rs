use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::{Context, Result};
use log::{debug, info, warn};

use crate::{
    datasource::file_path::*,
    model::gpu::GPU,
    utils::file_operate::check_read_simple,
};

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

// 读取v2 driver设备的频率表
fn read_v2_driver_freq_table() -> Result<Vec<i64>> {
    let mut freq_list = Vec::new();

    // 检查频率表文件是否存在
    if !Path::new(GPUFREQV2_TABLE).exists() || !check_read_simple(GPUFREQV2_TABLE) {
        warn!(
            "V2 driver frequency table file not found: {}",
            GPUFREQV2_TABLE
        );
        return Ok(freq_list);
    }

    // 打开并读取频率表文件
    let file = File::open(GPUFREQV2_TABLE).with_context(|| {
        format!(
            "Failed to open V2 driver frequency table file: {}",
            GPUFREQV2_TABLE
        )
    })?;

    let reader = BufReader::new(file);

    // 解析每一行，提取频率值
    for line in reader.lines() {
        let line = line?;

        // 查找频率值
        if let Some(freq_pos) = line.find("freq: ") {
            let freq_str = line[freq_pos + 6..].split(',').next().unwrap_or("0");
            if let Ok(freq) = freq_str.trim().parse::<i64>() {
                freq_list.push(freq);
                debug!("Found V2 driver frequency: {}", freq);
            }
        }
    }

    // 按降序排序（从高到低）
    freq_list.sort_by(|a, b| b.cmp(a));

    info!("Read {} frequencies from V2 driver table", freq_list.len());

    Ok(freq_list)
}

// 验证频率是否在v2 driver支持的范围内
#[allow(dead_code)]
pub fn validate_freq_for_v2_driver(freq: i64, supported_freqs: &[i64]) -> bool {
    if supported_freqs.is_empty() {
        // 如果没有读取到支持的频率，则不进行验证
        return true;
    }

    // 检查频率是否在支持的范围内
    supported_freqs.contains(&freq)
}

pub fn gpufreq_table_init(gpu: &mut GPU) -> Result<()> {
    // 检测GPU驱动类型
    detect_gpu_driver_type(gpu)?;

    // 如果是v2 driver，读取系统支持的频率表
    let v2_supported_freqs = if gpu.is_gpuv2() {
        info!("Reading V2 driver frequency table");
        read_v2_driver_freq_table()?
    } else {
        Vec::new()
    };

    // 保存v2 driver支持的频率列表到GPU对象
    if gpu.is_gpuv2() && !v2_supported_freqs.is_empty() {
        // 将支持的频率列表保存到GPU对象，以便后续使用
        gpu.set_v2_supported_freqs(v2_supported_freqs.clone());

        if let Some(&max_freq) = v2_supported_freqs.first() {
            info!("V2 Driver Max Supported Freq: {}", max_freq);
        }

        if let Some(&min_freq) = v2_supported_freqs.last() {
            info!("V2 Driver Min Supported Freq: {}", min_freq);
        }

        info!(
            "V2 Driver Supported Frequencies Total: {}",
            v2_supported_freqs.len()
        );
    } else if gpu.is_gpuv2() {
        warn!("No frequencies found in V2 driver table");
    } else {
        // 对于v1 driver，输出当前配置信息
        info!("Using frequencies from config file only");
    }

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
