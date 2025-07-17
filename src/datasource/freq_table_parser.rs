use std::{
    collections::HashMap,
    fs::{self},
};

use anyhow::Result;
use log::{error, info, warn};
use serde::Deserialize;

use crate::model::gpu::{TabType, GPU};

#[derive(Deserialize)]
struct FreqTableEntry {
    freq: i64,
    volt: i64,
    ddr_opp: i64,
}

#[derive(Deserialize)]
struct FreqTableConfig {
    #[serde(default)]
    freq_table: Vec<FreqTableEntry>,
}

fn volt_is_valid(v: i64) -> bool {
    v != 0 && v % 625 == 0
}

pub fn freq_table_read(config_file: &str, gpu: &mut GPU) -> Result<()> {
    let file = fs::read_to_string(config_file)?;
    let toml: FreqTableConfig = toml::from_str(&file)?;
    let mut new_config_list = Vec::new();
    let mut new_fvtab = HashMap::new();
    let mut new_fdtab = HashMap::new();

    for entry in toml.freq_table {
        let freq = entry.freq;
        let volt = entry.volt;
        let dram = entry.ddr_opp;

        if !volt_is_valid(volt) {
            error!("Entry freq={freq}, volt={volt}, ddr_opp={dram} is invalid: volt {volt} is not valid");
            continue;
        }

        if gpu.is_gpuv2() && !gpu.is_freq_supported_by_v2_driver(freq) {
            warn!(
                "Entry freq={freq}, volt={volt}, ddr_opp={dram} is not supported by V2 driver: freq {freq} is not in supported range"
            );
        }

        new_config_list.push(freq);
        new_fvtab.insert(freq, volt);
        new_fdtab.insert(freq, dram);
    }

    if new_config_list.is_empty() {
        error!("No valid frequency entries found in frequency table config file");
        return Err(anyhow::anyhow!(
            "No valid frequency entries found in frequency table config file: {config_file}"
        ));
    }

    info!(
        "Using frequencies directly from frequency table config file without system support check"
    );

    info!(
        "Loaded {} frequency entries from frequency table config file (no limit)",
        new_config_list.len()
    );

    gpu.set_config_list(new_config_list);
    gpu.replace_tab(TabType::FreqVolt, new_fvtab);
    gpu.replace_tab(TabType::FreqDram, new_fdtab);

    info!("Load frequency table config succeed");

    for &freq in &gpu.get_config_list() {
        let volt = gpu.read_tab(TabType::FreqVolt, freq);
        let dram = gpu.read_tab(TabType::FreqDram, freq);
        info!("Freq={freq}, Volt={volt}, Dram={dram}");
    }
    Ok(())
}
