use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader},
};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use serde::Deserialize;

use crate::model::gpu::{TabType, GPU};

#[derive(Deserialize)]
struct Config {
    margin: i64,
    vec: String,
}

fn volt_is_valid(v: i64) -> bool {
    v != 0 && v % 625 == 0
}

pub fn config_read(config_file: &str, gpu: &mut GPU) -> Result<()> {
    let file = fs::read_to_string(config_file)?;
    let toml: Config = toml::from_str(&file)?;
    let mut new_config_list = Vec::new();
    let mut new_fvtab = HashMap::new();
    let mut new_fdtab = HashMap::new();

    gpu.set_margin(toml.margin);

    for line in toml.vec.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            if let (Ok(freq), Ok(volt), Ok(dram)) = (
                parts[0].parse::<i64>(),
                parts[1].parse::<i64>(),
                parts[2].parse::<i64>(),
            ) {
                if !volt_is_valid(volt) {
                    error!("{line} is invalid: volt {volt} is not valid");
                    continue;
                }

                if gpu.is_gpuv2() && !gpu.is_freq_supported_by_v2_driver(freq) {
                    warn!(
                        "{line} is not supported by V2 driver: freq {freq} is not in supported range"
                    );
                }

                new_config_list.push(freq);
                new_fvtab.insert(freq, volt);
                new_fdtab.insert(freq, dram);
            }
        }
    }

    if new_config_list.is_empty() {
        error!("No valid frequency entries found in config file");
        return Err(anyhow::anyhow!(
            "No valid frequency entries found in config file: {config_file}"
        ));
    }

    info!("Using frequencies directly from config file without system support check");

    info!(
        "Loaded {} frequency entries from config file (no limit)",
        new_config_list.len()
    );

    gpu.set_config_list(new_config_list);
    gpu.replace_tab(TabType::FreqVolt, new_fvtab);
    gpu.replace_tab(TabType::FreqDram, new_fdtab);

    info!("Load config succeed");

    for &freq in &gpu.get_config_list() {
        let volt = gpu.read_tab(TabType::FreqVolt, freq);
        let dram = gpu.read_tab(TabType::FreqDram, freq);
        info!("Freq={freq}, Volt={volt}, Dram={dram}");
    }
    Ok(())
}
