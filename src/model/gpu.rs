use std::collections::HashMap;

use anyhow::Result;
use log::{debug, warn};

use crate::{
    datasource::file_path::*,
    model::{
        ddr_manager::DdrManager, frequency_manager::FrequencyManager,
        frequency_strategy::FrequencyStrategy, idle_manager::IdleManager,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabType {
    FreqVolt,
    FreqDram,
}

#[derive(Clone)]
#[allow(clippy::upper_case_acronyms)]
pub struct GPU {
    /// 频率管理器
    pub frequency_manager: FrequencyManager,
    /// 调频策略
    pub frequency_strategy: FrequencyStrategy,
    /// DDR管理器
    pub ddr_manager: DdrManager,
    /// 空闲状态管理器
    pub idle_manager: IdleManager,
    /// GPU版本相关
    pub gpuv2: bool,
    pub v2_supported_freqs: Vec<i64>,
    /// DCS相关
    pub dcs_enable: bool,
    pub need_dcs: bool,
    /// 游戏模式
    pub gaming_mode: bool,
    /// 精确模式
    pub precise: bool,
    /// 当前工作模式
    current_mode: String,
    /// 自适应采样相关
    adaptive_sampling_enabled: bool,
    min_adaptive_interval: u64,
    max_adaptive_interval: u64,
    last_load: i32,
}

impl GPU {
    pub fn new() -> Self {
        Self {
            frequency_manager: FrequencyManager::new(),
            frequency_strategy: FrequencyStrategy::new(500, 500),
            ddr_manager: DdrManager::new(),
            idle_manager: IdleManager::new(),
            gpuv2: false,
            v2_supported_freqs: Vec::new(),
            dcs_enable: false,
            need_dcs: false,
            gaming_mode: false,
            precise: false,
            current_mode: String::new(),
            adaptive_sampling_enabled: false,
            min_adaptive_interval: 2,
            max_adaptive_interval: 20,
            last_load: 0,
        }
    }

    // 频率管理相关 - 使用 Deref 模式减少样板代码
    pub fn get_cur_freq(&self) -> i64 {
        self.frequency_manager.cur_freq
    }

    pub fn set_cur_freq(&mut self, cur_freq: i64) {
        self.frequency_manager.cur_freq = cur_freq;
    }

    // 将频率管理方法直接暴露为引用，减少委托
    pub fn frequency(&self) -> &FrequencyManager {
        &self.frequency_manager
    }

    pub fn frequency_mut(&mut self) -> &mut FrequencyManager {
        &mut self.frequency_manager
    }

    // 保留最常用的快捷方法
    pub fn get_max_freq(&self) -> i64 {
        self.frequency_manager.get_max_freq()
    }

    pub fn get_min_freq(&self) -> i64 {
        self.frequency_manager.get_min_freq()
    }
    pub fn frequency_strategy_mut(&mut self) -> &mut FrequencyStrategy {
        &mut self.frequency_strategy
    }

    pub fn ddr_manager(&self) -> &DdrManager {
        &self.ddr_manager
    }

    pub fn ddr_manager_mut(&mut self) -> &mut DdrManager {
        &mut self.ddr_manager
    }

    // DCS相关方法
    pub fn is_dcs_enabled(&self) -> bool {
        self.dcs_enable
    }

    pub fn set_dcs_enable(&mut self, dcs_enable: bool) {
        self.dcs_enable = dcs_enable;
        debug!(
            "DCS {} for GPU frequency control",
            if dcs_enable { "enabled" } else { "disabled" }
        );
    }

    // 游戏模式相关方法
    pub fn is_gaming_mode(&self) -> bool {
        self.gaming_mode
    }

    pub fn set_gaming_mode(&mut self, gaming_mode: bool) {
        self.gaming_mode = gaming_mode;

        if gaming_mode {
            // 设置游戏模式下的DDR频率
            let freq_to_use = if self.get_cur_freq() > 0 {
                self.get_cur_freq()
            } else if !self.get_config_list().is_empty() {
                self.get_config_list()[0]
            } else {
                0
            };

            let mut ddr_opp = 999; // 默认自动模式
            if freq_to_use > 0 {
                let config_ddr_opp = self.read_tab(TabType::FreqDram, freq_to_use);
                if config_ddr_opp > 0 || config_ddr_opp == DDR_HIGHEST_FREQ {
                    ddr_opp = config_ddr_opp;
                }
            }

            debug!("Game mode: using DDR_OPP {ddr_opp} for frequency {freq_to_use}KHz");
            if let Err(e) = self.set_ddr_freq(ddr_opp) {
                warn!("Failed to set DDR frequency in game mode: {e}");
            }
        } else if self.is_ddr_freq_fixed()
            && let Err(e) = self.set_ddr_freq(999)
        {
            // 恢复自动DDR频率模式
            warn!("Failed to restore auto DDR mode: {e}");
        }
    }

    // 精确模式相关方法
    pub fn is_precise(&self) -> bool {
        self.precise
    }

    pub fn set_precise(&mut self, precise: bool) {
        self.precise = precise;
    }

    /// 设置当前工作模式
    pub fn set_current_mode(&mut self, mode: String) {
        self.current_mode = mode;
    }

    /// 读取映射表值 - 使用更简洁的模式匹配
    pub fn read_tab(&self, tab_type: TabType, freq: i64) -> i64 {
        match tab_type {
            TabType::FreqVolt => self.frequency_manager.read_freq_volt(freq),
            TabType::FreqDram => self.frequency_manager.read_freq_dram(freq),
        }
    }

    /// 替换映射表 - 使用更简洁的模式匹配
    pub fn replace_tab(&mut self, tab_type: TabType, tab: HashMap<i64, i64>) {
        match tab_type {
            TabType::FreqVolt => self.frequency_manager.replace_freq_volt_tab(tab),
            TabType::FreqDram => self.frequency_manager.replace_freq_dram_tab(tab),
        }
    }

    // GPU版本相关方法
    pub fn is_gpuv2(&self) -> bool {
        self.gpuv2
    }

    pub fn set_gpuv2(&mut self, gpuv2: bool) {
        self.gpuv2 = gpuv2;
    }

    pub fn get_v2_supported_freqs(&self) -> Vec<i64> {
        self.v2_supported_freqs.clone()
    }

    pub fn set_v2_supported_freqs(&mut self, freqs: Vec<i64>) {
        self.v2_supported_freqs = freqs;
    }

    /// 检查频率是否被v2驱动支持
    pub fn is_freq_supported_by_v2_driver(&self, freq: i64) -> bool {
        if !self.gpuv2 || self.v2_supported_freqs.is_empty() {
            // 如果不是v2 driver或者没有读取到支持的频率，则不进行验证
            true
        } else {
            // 检查频率是否在支持的范围内
            self.v2_supported_freqs.contains(&freq)
        }
    }

    /// 快捷方法组合 - 提供更符合 Rust 习惯的API
    // 最常用的频率操作
    pub fn get_freq_by_index(&self, idx: i64) -> i64 {
        self.frequency_manager.get_freq_by_index(idx)
    }

    pub fn get_middle_freq(&self) -> i64 {
        self.frequency_manager.get_middle_freq()
    }

    pub fn get_config_list(&self) -> Vec<i64> {
        self.frequency_manager.get_config_list()
    }

    pub fn set_config_list(&mut self, config_list: Vec<i64>) {
        self.frequency_manager.set_config_list(config_list);
    }

    // 最常用的空闲状态操作
    pub fn reset_load_zone_counter(&mut self) {
        self.idle_manager.reset_load_zone_counter()
    }

    pub fn is_idle(&self) -> bool {
        self.idle_manager.is_idle()
    }

    // 最常用的策略操作
    pub fn get_margin(&self) -> i64 {
        self.frequency_strategy.get_margin() as i64
    }

    pub fn get_down_counter_threshold(&self) -> i64 {
        self.frequency_strategy.get_down_counter_threshold() as i64
    }

    // 批量设置方法 - 减少重复调用
    pub fn configure_strategy(
        &mut self,
        margin: i64,
        down_counter_threshold: i64,
        sampling_interval: u64,
        aggressive_down: bool,
    ) {
        let strategy = &mut self.frequency_strategy;
        strategy.set_margin(margin.try_into().unwrap());
        strategy.set_down_counter_threshold(down_counter_threshold.try_into().unwrap());
        strategy.set_sampling_interval(sampling_interval);
        strategy.set_aggressive_down(aggressive_down);
    }

    // 最常用的DDR操作
    pub fn set_ddr_freq(&mut self, freq: i64) -> Result<()> {
        self.ddr_manager.set_ddr_freq(freq)
    }

    pub fn is_ddr_freq_fixed(&self) -> bool {
        self.ddr_manager.is_ddr_freq_fixed()
    }

    // 添加缺失的策略委托方法
    pub fn set_up_rate_delay(&mut self, delay: u64) {
        self.frequency_strategy.set_up_rate_delay(delay);
    }

    pub fn set_debounce_times(&mut self, up_time: u64, down_time: u64) {
        self.frequency_strategy
            .set_debounce_times(up_time, down_time);
    }

    pub fn set_adaptive_sampling(
        &mut self,
        enabled: bool,
        min_interval: u64,
        max_interval: u64,
        fixed_interval: u64,
    ) {
        if enabled {
            // 启用自适应采样，初始设置为最小间隔
            self.frequency_strategy.set_sampling_interval(min_interval);
            self.adaptive_sampling_enabled = true;
            self.min_adaptive_interval = min_interval;
            self.max_adaptive_interval = max_interval;
        } else {
            // 禁用自适应采样，使用固定间隔
            self.frequency_strategy
                .set_sampling_interval(fixed_interval);
            self.adaptive_sampling_enabled = false;
        }
    }

    /// 根据GPU负载动态调整采样间隔
    pub fn adjust_sampling_interval_by_load(&mut self, current_load: i32) {
        if !self.adaptive_sampling_enabled {
            return;
        }

        let load_diff = (current_load - self.last_load).abs();

        // 根据负载变化调整采样间隔
        let new_interval = if load_diff > 30 {
            // 负载变化大，使用最小间隔快速响应
            self.min_adaptive_interval
        } else if load_diff < 5 {
            // 负载变化小，使用最大间隔降低CPU占用
            self.max_adaptive_interval
        } else {
            // 线性插值计算间隔
            let ratio = load_diff as f64 / 30.0;
            let range = self.max_adaptive_interval - self.min_adaptive_interval;
            self.max_adaptive_interval - (ratio * range as f64) as u64
        };

        // 确保在有效范围内
        let new_interval =
            new_interval.clamp(self.min_adaptive_interval, self.max_adaptive_interval);

        self.frequency_strategy.set_sampling_interval(new_interval);
        self.last_load = current_load;
    }

    // 添加缺失的频率管理委托方法
    pub fn read_freq_ge(&self, freq: i64) -> i64 {
        self.frequency_manager.read_freq_ge(freq)
    }

    pub fn read_freq_le(&self, freq: i64) -> i64 {
        self.frequency_manager.read_freq_le(freq)
    }

    /// 找到最接近目标频率的索引
    pub fn find_closest_freq_index(&self, target_freq: i64) -> i64 {
        let config_list = self.get_config_list();
        if config_list.is_empty() {
            return 0;
        }

        let mut closest_idx = 0;
        let mut min_diff = (config_list[0] - target_freq).abs();

        for (idx, &freq) in config_list.iter().enumerate() {
            let diff = (freq - target_freq).abs();
            if diff < min_diff {
                min_diff = diff;
                closest_idx = idx as i64;
            }
        }

        closest_idx
    }

    // 主要的频率调整方法 - 现在使用新的引擎
    pub fn adjust_gpufreq(&mut self) -> Result<()> {
        use crate::model::frequency_engine::FrequencyAdjustmentEngine;
        FrequencyAdjustmentEngine::run_adjustment_loop(self)
    }
}

impl Default for GPU {
    fn default() -> Self {
        Self::new()
    }
}

impl GPU {
    pub fn idle_manager_mut(&mut self) -> &mut IdleManager {
        &mut self.idle_manager
    }
}
