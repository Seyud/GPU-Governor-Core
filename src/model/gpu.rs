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
    DefVolt,
}

impl TabType {
    /// 获取表类型的描述字符串
    pub fn description(&self) -> &'static str {
        match self {
            TabType::FreqVolt => "Frequency to Voltage mapping",
            TabType::FreqDram => "Frequency to DDR mapping",
            TabType::DefVolt => "Default voltage mapping",
        }
    }
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
}

impl GPU {
    pub fn new() -> Self {
        Self {
            frequency_manager: FrequencyManager::new(),
            frequency_strategy: FrequencyStrategy::new(),
            ddr_manager: DdrManager::new(),
            idle_manager: IdleManager::new(),
            gpuv2: false,
            v2_supported_freqs: Vec::new(),
            dcs_enable: false,
            need_dcs: false,
            gaming_mode: false,
            precise: false,
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
    // 为其他组件提供直接访问，减少委托样板代码
    pub fn frequency_strategy(&self) -> &FrequencyStrategy {
        &self.frequency_strategy
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
            // 应用游戏模式调频策略
            self.frequency_strategy.set_gaming_mode_params();

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

            debug!(
                "Game mode: using DDR_OPP {} for frequency {}KHz",
                ddr_opp, freq_to_use
            );
            if let Err(e) = self.set_ddr_freq(ddr_opp) {
                warn!("Failed to set DDR frequency in game mode: {}", e);
            }
        } else {
            // 应用普通模式调频策略
            self.frequency_strategy.set_normal_mode_params();

            // 恢复自动DDR频率模式
            if self.is_ddr_freq_fixed() {
                if let Err(e) = self.set_ddr_freq(999) {
                    warn!("Failed to restore auto DDR mode: {}", e);
                }
            }
        }
    }

    // 精确模式相关方法
    pub fn is_precise(&self) -> bool {
        self.precise
    }

    pub fn set_precise(&mut self, precise: bool) {
        self.precise = precise;
    }

    /// 读取映射表值 - 使用更简洁的模式匹配
    pub fn read_tab(&self, tab_type: TabType, freq: i64) -> i64 {
        use TabType::*;
        match tab_type {
            FreqVolt => self.frequency_manager.read_freq_volt(freq),
            FreqDram => self.frequency_manager.read_freq_dram(freq),
            DefVolt => self.frequency_manager.read_def_volt(freq),
        }
    }

    /// 替换映射表 - 使用更简洁的模式匹配
    pub fn replace_tab(&mut self, tab_type: TabType, tab: HashMap<i64, i64>) {
        use TabType::*;
        match tab_type {
            FreqVolt => self.frequency_manager.replace_freq_volt_tab(tab),
            FreqDram => self.frequency_manager.replace_freq_dram_tab(tab),
            DefVolt => self.frequency_manager.replace_def_volt_tab(tab),
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

    /// 获取v2 driver支持的最接近频率
    pub fn get_closest_v2_supported_freq(&self, freq: i64) -> i64 {
        if !self.gpuv2
            || self.v2_supported_freqs.is_empty()
            || self.is_freq_supported_by_v2_driver(freq)
        {
            // 如果不是v2 driver或者没有读取到支持的频率，或者频率已经在支持范围内，则直接返回原频率
            freq
        } else {
            // 找到最接近的支持频率
            let mut closest_freq = self.v2_supported_freqs[0];
            let mut min_diff = (freq - closest_freq).abs();

            for &supported_freq in &self.v2_supported_freqs {
                let diff = (freq - supported_freq).abs();
                if diff < min_diff {
                    min_diff = diff;
                    closest_freq = supported_freq;
                }
            }

            debug!(
                "Freq {} not supported by V2 driver, using closest supported freq: {}",
                freq, closest_freq
            );
            closest_freq
        }
    }

    /// 快捷方法组合 - 提供更符合 Rust 习惯的API

    // 最常用的频率操作
    pub fn get_freq_by_index(&self, idx: i64) -> i64 {
        self.frequency_manager.get_freq_by_index(idx)
    }

    pub fn read_freq_index(&self, freq: i64) -> i64 {
        self.frequency_manager.read_freq_index(freq)
    }

    pub fn get_middle_freq(&self) -> i64 {
        self.frequency_manager.get_middle_freq()
    }

    pub fn get_second_highest_freq(&self) -> i64 {
        self.frequency_manager.get_second_highest_freq()
    }

    pub fn get_config_list(&self) -> Vec<i64> {
        self.frequency_manager.get_config_list()
    }

    pub fn set_config_list(&mut self, config_list: Vec<i64>) {
        self.frequency_manager.set_config_list(config_list);
    }

    // 最常用的空闲状态操作
    pub fn check_idle_state(&mut self, util: i32) {
        self.idle_manager.check_idle_state(util)
    }

    pub fn reset_load_zone_counter(&mut self) {
        self.idle_manager.reset_load_zone_counter()
    }

    pub fn is_idle(&self) -> bool {
        self.idle_manager.is_idle()
    }

    pub fn set_idle(&mut self, idle: bool) {
        self.idle_manager.set_idle(idle);
    }

    // 最常用的策略操作
    pub fn get_margin(&self) -> i64 {
        self.frequency_strategy.get_margin()
    }

    pub fn set_margin(&mut self, margin: i64) {
        self.frequency_strategy.set_margin(margin);
    }

    pub fn get_down_threshold(&self) -> i64 {
        self.frequency_strategy.get_down_threshold()
    }

    pub fn set_down_threshold(&mut self, threshold: i64) {
        self.frequency_strategy.set_down_threshold(threshold);
    }

    // 批量设置方法 - 减少重复调用
    pub fn configure_strategy(
        &mut self,
        margin: i64,
        down_threshold: i64,
        sampling_interval: u64,
        aggressive_down: bool,
    ) {
        let strategy = &mut self.frequency_strategy;
        strategy.set_margin(margin);
        strategy.set_down_threshold(down_threshold);
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

    pub fn set_load_thresholds(&mut self, very_low: i32, low: i32, high: i32, very_high: i32) {
        self.frequency_strategy
            .set_load_thresholds(very_low, low, high, very_high);
    }

    pub fn set_load_stability_threshold(&mut self, threshold: i32) {
        self.frequency_strategy
            .set_load_stability_threshold(threshold);
    }

    pub fn set_aggressive_down(&mut self, aggressive: bool) {
        self.frequency_strategy.set_aggressive_down(aggressive);
    }

    pub fn set_hysteresis_thresholds(&mut self, up_threshold: i32, down_threshold: i32) {
        self.frequency_strategy
            .set_hysteresis_thresholds(up_threshold, down_threshold);
    }

    pub fn set_debounce_times(&mut self, up_time: u64, down_time: u64) {
        self.frequency_strategy
            .set_debounce_times(up_time, down_time);
    }

    pub fn set_adaptive_sampling(&mut self, enabled: bool, min_interval: u64, max_interval: u64) {
        self.frequency_strategy
            .set_adaptive_sampling(enabled, min_interval, max_interval);
    }

    // 添加缺失的频率管理委托方法
    pub fn read_freq_ge(&self, freq: i64) -> i64 {
        self.frequency_manager.read_freq_ge(freq)
    }

    pub fn read_freq_le(&self, freq: i64) -> i64 {
        self.frequency_manager.read_freq_le(freq)
    }

    // 主要的频率调整方法 - 现在使用新的引擎
    pub fn adjust_gpufreq(&mut self) -> Result<()> {
        use crate::model::frequency_engine::FrequencyAdjustmentEngine;
        FrequencyAdjustmentEngine::run_adjustment_loop(self)
    }

    // 写入频率方法 - 简化版
    pub fn write_freq(&self) -> Result<()> {
        self.frequency_manager
            .write_freq(self.need_dcs, self.is_idle())
    }

    // 其他必要的实用方法
    pub fn find_closest_gpu_freq(&self, target_freq: i64) -> i64 {
        if self.get_config_list().is_empty() {
            0
        } else {
            let config_list = self.get_config_list();
            let mut closest_freq = config_list[0];
            let mut min_diff = (target_freq - closest_freq).abs();

            for &freq in &config_list {
                let diff = (target_freq - freq).abs();
                if diff < min_diff {
                    min_diff = diff;
                    closest_freq = freq;
                }
            }

            closest_freq
        }
    }
}

impl Default for GPU {
    fn default() -> Self {
        Self::new()
    }
}
