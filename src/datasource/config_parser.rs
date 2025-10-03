use crate::datasource::file_path::CONFIG_TOML_FILE;
use crate::model::gpu::GPU;
use anyhow::Result;
use log::{debug, info, warn};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Clone)]
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

#[derive(Deserialize, Clone)]
pub struct Global {
    mode: String,
    idle_threshold: i32,
}

#[derive(Deserialize, Clone)]
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

    if gpu.current_mode() == mode {
        debug!("Mode `{}` 已经生效，跳过重新加载", mode);
        return Ok(());
    }

    // 存储当前模式以便访问
    gpu.set_current_mode(mode.to_string());
    let params = match mode {
        "powersave" => &config.powersave,
        "balance" => &config.balance,
        "performance" => &config.performance,
        "fast" => &config.fast,
        _ => {
            // 非法模式：采用回退策略并给出警告
            warn!("Invalid mode '{mode}', using balance mode");
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

    info!("Loaded config for mode: {}", mode);
    Ok(())
}

#[derive(Clone, Debug)]
pub struct ConfigDelta {
    pub margin: i64,
    pub aggressive_down: bool,
    pub sampling_interval: u64,
    pub gaming_mode: bool,
    pub adaptive_sampling: bool,
    pub min_adaptive_interval: u64,
    pub max_adaptive_interval: u64,
    pub up_rate_delay: u64,
    pub down_rate_delay: u64,
    pub idle_threshold: Option<i32>,
    pub mode: Option<String>, // 新增：用于同步 global.mode / 当前模式名
}

pub fn read_config_delta(target_mode: Option<&str>) -> Result<ConfigDelta> {
    let content = std::fs::read_to_string(CONFIG_TOML_FILE)?;
    let config: Config = toml::from_str(&content)?;
    let mode = target_mode.unwrap_or(&config.global.mode);
    let params = match mode {
        "powersave" => &config.powersave,
        "balance" => &config.balance,
        "performance" => &config.performance,
        "fast" => &config.fast,
        _ => &config.balance,
    };
    Ok(ConfigDelta {
        margin: params.margin,
        aggressive_down: params.aggressive_down,
        sampling_interval: params.sampling_interval,
        gaming_mode: params.gaming_mode,
        adaptive_sampling: params.adaptive_sampling,
        min_adaptive_interval: params.min_adaptive_interval,
        max_adaptive_interval: params.max_adaptive_interval,
        up_rate_delay: params.up_rate_delay,
        down_rate_delay: params.down_rate_delay,
        idle_threshold: Some(config.global.idle_threshold),
        mode: Some(config.global.mode.clone()),
    })
}
