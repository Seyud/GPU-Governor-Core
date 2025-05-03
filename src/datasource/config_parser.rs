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

pub fn gen_default_freq_table(gpu: &mut GPU) -> Result<()> {
    // 使用一个简单的默认配置
    warn!("Using minimal default config");

    let mut new_config_list = Vec::new();
    let mut new_fvtab = HashMap::new();
    let mut new_fdtab = HashMap::new();

    // 只添加一个默认频率
    new_config_list.push(500000); // 500MHz作为默认值
    new_fvtab.insert(500000, 50000); // 默认电压
    new_fdtab.insert(500000, 0); // 默认DRAM设置

    gpu.set_config_list(new_config_list);
    gpu.replace_tab(TabType::FreqVolt, new_fvtab);
    gpu.replace_tab(TabType::FreqDram, new_fdtab);

    info!("Generated minimal default frequency table");

    Ok(())
}

pub fn config_read(config_file: &str, gpu: &mut GPU) -> Result<()> {
    let file = File::open(config_file)
        .with_context(|| format!("Failed to open config file: {}", config_file))?;

    let reader = BufReader::new(file);
    let mut new_config_list = Vec::new();
    let mut new_fvtab = HashMap::new();
    let mut new_fdtab = HashMap::new();

    for line in reader.lines() {
        let line = line?;

        // Trim whitespace
        let trimmed = line.trim().to_string();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        debug!("{}", trimmed);

        // Parse frequency, voltage, and dram values
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            if let (Ok(freq), Ok(volt), Ok(dram)) = (
                parts[0].parse::<i64>(),
                parts[1].parse::<i64>(),
                parts[2].parse::<i64>(),
            ) {
                // 只验证电压是否有效
                if !volt_is_valid(volt) {
                    error!("{} is invalid: volt {} is not valid", trimmed, volt);
                    continue;
                }

                new_config_list.push(freq);
                new_fvtab.insert(freq, volt);
                new_fdtab.insert(freq, dram);
            }
        }
    }

    // If no valid entries were found, generate default table
    if new_config_list.is_empty() {
        error!("Reload config FAILED, generating default config");
        gen_default_freq_table(gpu)?;
        return Ok(());
    }

    // Sort the frequency list in descending order (highest frequency first)
    new_config_list.sort_by(|a, b| b.cmp(a));

    // 直接使用配置文件中的频率，不进行任何系统支持检查
    info!("Using frequencies directly from config file without system support check");

    // 输出频率表条目数量
    info!(
        "Loaded {} frequency entries from config file (no limit)",
        new_config_list.len()
    );

    // Update GPU with new configuration
    gpu.set_config_list(new_config_list);
    gpu.replace_tab(TabType::FreqVolt, new_fvtab);
    gpu.replace_tab(TabType::FreqDram, new_fdtab);

    info!("Load config succeed");

    // Log the configuration
    for &freq in &gpu.get_config_list() {
        info!(
            "Freq={}, Volt={}, Dram={}",
            freq,
            gpu.read_tab(TabType::FreqVolt, freq),
            gpu.read_tab(TabType::FreqDram, freq)
        );
    }

    Ok(())
}
