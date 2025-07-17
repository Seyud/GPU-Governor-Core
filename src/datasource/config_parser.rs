use crate::datasource::file_path::CONFIG_TOML_FILE;
use crate::model::gpu::GPU;
use anyhow::Result;
use log::info;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
pub struct Config {
    global: Global,
    powersave: ModeParams,
    balance: ModeParams,
    performance: ModeParams,
    fast: ModeParams,
}

#[derive(Deserialize)]
pub struct Global {
    mode: String,
    idle_threshold: i32,
}

#[derive(Deserialize)]
pub struct ModeParams {
    very_high_load_threshold: i32,
    margin: i64,
    down_threshold: i64,
    aggressive_down: bool,
    sampling_interval: u64,
}

pub fn load_config(gpu: &mut GPU) -> Result<()> {
    let content = fs::read_to_string(CONFIG_TOML_FILE)?;
    let config: Config = toml::from_str(&content)?;

    gpu.idle_manager_mut()
        .set_idle_threshold(config.global.idle_threshold);

    let params = match config.global.mode.as_str() {
        "powersave" => &config.powersave,
        "balance" => &config.balance,
        "performance" => &config.performance,
        "fast" => &config.fast,
        _ => {
            info!("Invalid mode '{}', using balance mode", config.global.mode);
            &config.balance
        }
    };

    let strategy = gpu.frequency_strategy_mut();
    strategy.very_high_load_threshold = params.very_high_load_threshold;
    strategy.set_margin(params.margin);
    strategy.set_down_threshold(params.down_threshold);
    strategy.set_aggressive_down(params.aggressive_down);
    strategy.set_sampling_interval(params.sampling_interval);

    info!("Loaded config for mode: {}", config.global.mode);
    Ok(())
}
