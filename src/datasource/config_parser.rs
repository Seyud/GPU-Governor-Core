use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};

use crate::model::gpu::{TabType, GPU};

fn volt_is_valid(v: i64) -> bool {
    v != 0 && v % 625 == 0
}

pub fn config_read(config_file: &str, gpu: &mut GPU) -> Result<()> {
    let file = File::open(config_file)
        .with_context(|| format!("Failed to open config file: {config_file}"))?;

    let reader = BufReader::new(file);
    let mut new_config_list = Vec::new();
    let mut new_fvtab = HashMap::new();
    let mut new_fdtab = HashMap::new();

    for line in reader.lines() {
        let line = line?;

        // 去除空白字符
        let trimmed = line.trim().to_string();

        // 跳过空行
        if trimmed.is_empty() {
            continue;
        }

        // 解析Margin配置，确保不是注释行
        if trimmed.starts_with("Margin=") && !trimmed.starts_with("#") {
            let margin_str = trimmed.trim_start_matches("Margin=");
            if let Ok(margin) = margin_str.parse::<i64>() {
                info!("Read Margin value from config file: {margin}%");
                gpu.set_margin(margin);
            } else {
                warn!("Invalid Margin value: {margin_str}");
            }
            continue;
        }

        // 跳过注释行
        if trimmed.starts_with('#') {
            continue;
        }

        debug!("{trimmed}");

        // 解析频率、电压和内存频率值
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            if let (Ok(freq), Ok(volt), Ok(dram)) = (
                parts[0].parse::<i64>(),
                parts[1].parse::<i64>(),
                parts[2].parse::<i64>(),
            ) {
                // 验证电压是否有效
                if !volt_is_valid(volt) {
                    error!("{trimmed} is invalid: volt {volt} is not valid");
                    continue;
                }

                // 对于v2 driver设备，验证频率是否在系统支持范围内
                if gpu.is_gpuv2() && !gpu.is_freq_supported_by_v2_driver(freq) {
                    warn!(
                        "{trimmed} is not supported by V2 driver: freq {freq} is not in supported range"
                    );
                    // 不跳过，仍然添加到配置中，但会发出警告
                }

                new_config_list.push(freq);
                new_fvtab.insert(freq, volt);
                new_fdtab.insert(freq, dram);
            }
        }
    }

    // 如果没有找到有效的条目，返回错误
    if new_config_list.is_empty() {
        error!("No valid frequency entries found in config file");
        return Err(anyhow::anyhow!(
            "No valid frequency entries found in config file: {config_file}"
        ));
    }

    // 直接使用配置文件中的频率，不进行任何系统支持检查
    info!("Using frequencies directly from config file without system support check");

    // 输出频率表条目数量
    info!(
        "Loaded {} frequency entries from config file (no limit)",
        new_config_list.len()
    );

    // 使用新配置更新GPU
    gpu.set_config_list(new_config_list);
    gpu.replace_tab(TabType::FreqVolt, new_fvtab);
    gpu.replace_tab(TabType::FreqDram, new_fdtab);

    info!("Load config succeed");

    // Log the configuration
    for &freq in &gpu.get_config_list() {
        let volt = gpu.read_tab(TabType::FreqVolt, freq);
        let dram = gpu.read_tab(TabType::FreqDram, freq);
        info!("Freq={freq}, Volt={volt}, Dram={dram}");
    }

    Ok(())
}
