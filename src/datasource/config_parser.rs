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

impl Config {
    pub fn global_mode(&self) -> &str {
        &self.global.mode
    }
}

#[derive(Deserialize)]
pub struct Global {
    mode: String,
    idle_threshold: i32,
}

#[derive(Deserialize)]
pub struct ModeParams {
    margin: i64,
    aggressive_down: bool,
    sampling_interval: u64,
    gaming_mode: bool,
    adaptive_sampling: bool,
    min_adaptive_interval: u64,
    max_adaptive_interval: u64,
    up_rate_delay: u64,
    down_rate_delay: u64,
}

pub fn load_config(gpu: &mut GPU, target_mode: Option<&str>) -> Result<()> {
    let content = fs::read_to_string(CONFIG_TOML_FILE)?;
    let config: Config = toml::from_str(&content)?;

    gpu.idle_manager_mut()
        .set_idle_threshold(config.global.idle_threshold);

    let mode = target_mode.unwrap_or(&config.global.mode);
    // 存储当前模式以便访问
    gpu.set_current_mode(mode.to_string());
    let params = match mode {
        "powersave" => &config.powersave,
        "balance" => &config.balance,
        "performance" => &config.performance,
        "fast" => &config.fast,
        _ => {
            info!("Invalid mode '{mode}', using balance mode");
            &config.balance
        }
    };

    let strategy = gpu.frequency_strategy_mut();
    strategy.set_margin(params.margin.try_into().unwrap());
    strategy.set_aggressive_down(params.aggressive_down);
    strategy.set_sampling_interval(params.sampling_interval);

    // 使用GPU配置方法
    gpu.set_gaming_mode(params.gaming_mode);
    gpu.set_adaptive_sampling(
        params.adaptive_sampling,
        params.min_adaptive_interval,
        params.max_adaptive_interval,
        params.sampling_interval,
    );
    gpu.set_up_rate_delay(params.up_rate_delay);
    gpu.set_debounce_times(params.up_rate_delay, params.down_rate_delay);

    info!("Loaded config for mode: {}", config.global.mode);
    Ok(())
}
