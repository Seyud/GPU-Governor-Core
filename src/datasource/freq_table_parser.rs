use std::{
    collections::HashMap,
    fs::{self},
};

use anyhow::Result;
use log::{error, info, warn};
use serde::Deserialize;
use serde::de::{self, Visitor};

use crate::model::gpu::{GPU, TabType};

fn de_i64_lenient<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct I64LenientVisitor;

    impl<'de> Visitor<'de> for I64LenientVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter
                .write_str("an integer, an integer-like float (e.g. 999.0), or a numeric string")
        }

        fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E> {
            Ok(v)
        }

        fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            i64::try_from(v).map_err(|_| E::custom("integer out of range for i64"))
        }

        fn visit_f64<E>(self, v: f64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            if !v.is_finite() {
                return Err(E::custom("floating point value is not finite"));
            }
            if v.fract() != 0.0 {
                return Err(E::custom("floating point value is not an integer"));
            }
            if v < i64::MIN as f64 || v > i64::MAX as f64 {
                return Err(E::custom("integer out of range for i64"));
            }
            Ok(v as i64)
        }

        fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            let trimmed = v.trim();
            if let Ok(i) = trimmed.parse::<i64>() {
                return Ok(i);
            }
            let parsed = trimmed
                .parse::<f64>()
                .map_err(|_| E::custom("string is not a valid number"))?;
            self.visit_f64(parsed)
        }

        fn visit_string<E>(self, v: String) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&v)
        }
    }

    deserializer.deserialize_any(I64LenientVisitor)
}

#[derive(Deserialize)]
struct FreqTableEntry {
    #[serde(deserialize_with = "de_i64_lenient")]
    freq: i64,
    #[serde(deserialize_with = "de_i64_lenient")]
    volt: i64,
    #[serde(deserialize_with = "de_i64_lenient")]
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
    let toml: FreqTableConfig = toml::from_str(&file).map_err(|e| {
        error!("TOML解析失败（{config_file}）: {e}");
        anyhow::anyhow!("Failed to parse frequency table: {}", e)
    })?;
    let mut new_config_list = Vec::new();
    let mut new_fvtab = HashMap::new();
    let mut new_fdtab = HashMap::new();

    for entry in toml.freq_table {
        let freq = entry.freq;
        let volt = entry.volt;
        let dram = entry.ddr_opp;

        if !volt_is_valid(volt) {
            error!(
                "Entry freq={freq}, volt={volt}, ddr_opp={dram} is invalid: volt {volt} is not valid"
            );
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
