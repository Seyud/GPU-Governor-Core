use std::time::{SystemTime, UNIX_EPOCH, Duration};
use anyhow::Result;
use log::{debug, info, warn};

use crate::{
    datasource::load_monitor::get_gpu_load,
    model::gpu::GPU,
};

/// GPU频率调整引擎 - 负责执行智能调频算法
pub struct FrequencyAdjustmentEngine;

impl FrequencyAdjustmentEngine {
    /// 主要的频率调整循环
    pub fn run_adjustment_loop(gpu: &mut GPU) -> Result<()> {
        info!("Starting advanced GPU governor with enhanced multi-threshold strategy");
        info!("Load thresholds: very_low={}%, low={}%, high={}%, very_high={}%",
              gpu.frequency_strategy.very_low_load_threshold, 
              gpu.frequency_strategy.low_load_threshold,
              gpu.frequency_strategy.high_load_threshold, 
              gpu.frequency_strategy.very_high_load_threshold);

        debug!("config:{:?}, freq:{}", gpu.get_config_list(), gpu.get_cur_freq());

        loop {
            // 获取当前时间戳
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            // 更新当前GPU频率
            Self::update_current_frequency(gpu)?;

            // 读取当前GPU负载
            let util = get_gpu_load()?;

            // 更新负载历史记录并分析趋势
            let load_trend = gpu.update_load_history(util);

            // 检查空闲状态
            gpu.load_analyzer.check_idle_state(util);
            if gpu.is_idle() {
                Self::handle_idle_state(gpu);
                continue;
            }

            // 根据负载波动性调整采样间隔
            if gpu.frequency_strategy.adaptive_sampling {
                gpu.frequency_strategy.adjust_sampling_interval(util);
            }

            // 计算调整后的margin值
            let margin = gpu.frequency_strategy.calculate_margin(load_trend, gpu.gaming_mode);

            // 确定当前负载区域
            let new_load_zone = gpu.determine_load_zone(util);
            gpu.load_analyzer.set_current_load_zone(new_load_zone);

            // 检查是否需要调整频率
            if Self::should_adjust_frequency(gpu, current_time, new_load_zone) {
                Self::execute_frequency_adjustment(gpu, util, margin, load_trend, current_time)?;
            }

            // 应用采样间隔睡眠
            Self::apply_sampling_sleep(gpu);
        }
    }

    /// 更新当前GPU频率
    fn update_current_frequency(gpu: &mut GPU) -> Result<()> {
        use crate::datasource::load_monitor::get_gpu_current_freq;
        
        match get_gpu_current_freq() {
            Ok(current_freq) => {
                if current_freq > 0 {
                    gpu.set_cur_freq(current_freq);
                    gpu.frequency_manager.cur_freq_idx = gpu.read_freq_index(current_freq);
                    debug!("Updated current GPU frequency from file: {}", current_freq);
                }
            },
            Err(e) => {
                return Err(e);
            }
        }
        Ok(())
    }

    /// 处理空闲状态
    fn handle_idle_state(gpu: &GPU) {
        let idle_sleep_time = if gpu.is_precise() { 200 } else { 160 };
        debug!("Idle state, sleeping for {}ms", idle_sleep_time);
        std::thread::sleep(Duration::from_millis(idle_sleep_time));
    }

    /// 检查是否应该调整频率
    fn should_adjust_frequency(gpu: &GPU, current_time: u64, new_load_zone: i32) -> bool {
        let load_zone_counter = gpu.load_analyzer.get_load_zone_counter();
        let load_stability_threshold = gpu.frequency_strategy.load_stability_threshold;

        // 检查负载区域稳定性
        let zone_stable = load_zone_counter >= load_stability_threshold;
        let extreme_zone = new_load_zone == 0 || new_load_zone == 4;

        if !zone_stable && !extreme_zone {
            return false;
        }

        // 检查去抖动时间
        let target_higher = new_load_zone >= 3;
        gpu.frequency_strategy.check_debounce_time(current_time, target_higher)
    }

    /// 执行频率调整
    fn execute_frequency_adjustment(
        gpu: &mut GPU, 
        util: i32, 
        margin: i64, 
        load_trend: i32, 
        current_time: u64
    ) -> Result<()> {
        debug!("Executing frequency adjustment for load zone: {}", gpu.load_analyzer.current_load_zone);

        let now_freq = gpu.get_cur_freq();
        let (final_freq, final_freq_index) = match gpu.load_analyzer.current_load_zone {
            0 => Self::handle_very_low_load(gpu, util, margin, load_trend, now_freq),
            1 => Self::handle_low_load(gpu, util, margin, load_trend, now_freq),
            2 => Self::handle_medium_load(gpu, util, margin, load_trend, now_freq),
            3 => Self::handle_high_load(gpu, util, margin, load_trend, now_freq),
            4 => Self::handle_very_high_load(gpu, util, margin, load_trend, now_freq),
            _ => (now_freq, gpu.frequency_manager.cur_freq_idx),
        };

        // 应用新频率
        if final_freq != now_freq {
            Self::apply_new_frequency(gpu, final_freq, final_freq_index, current_time)?;
        }

        Ok(())
    }

    /// 处理极低负载区域
    fn handle_very_low_load(gpu: &GPU, util: i32, _margin: i64, load_trend: i32, now_freq: i64) -> (i64, i64) {
        debug!("Very low load zone ({}%) detected", util);

        let final_freq = if gpu.is_gaming_mode() {
            // 游戏模式：步进式降频
            let current_idx = gpu.frequency_manager.cur_freq_idx;
            if current_idx > 0 {
                let next_lower_idx = (current_idx - 1).max(0);
                gpu.get_freq_by_index(next_lower_idx)
            } else {
                now_freq
            }
        } else if gpu.frequency_strategy.is_aggressive_down() {
            // 普通模式：激进降频
            if load_trend > 0 {
                gpu.get_second_lowest_freq() // 负载上升时保守降频
            } else {
                gpu.get_min_freq() // 直接降到最低频率
            }
        } else {
            gpu.get_second_lowest_freq()
        };

        let final_freq_index = gpu.read_freq_index(final_freq);
        debug!("Very low load adjustment: {} -> {}KHz", now_freq, final_freq);
        (final_freq, final_freq_index)
    }

    /// 处理低负载区域
    fn handle_low_load(gpu: &GPU, util: i32, margin: i64, load_trend: i32, now_freq: i64) -> (i64, i64) {
        debug!("Low load zone ({}%) detected", util);

        let mut target_freq = now_freq * (util as i64 + margin) / 100;

        // 根据负载趋势调整
        if load_trend < 0 {
            target_freq = target_freq * 95 / 100; // 负载下降，更激进降频
        }

        let final_freq = if target_freq < now_freq * 85 / 100 {
            // 显著降频
            let current_idx = gpu.frequency_manager.cur_freq_idx;
            let step_size = if gpu.frequency_strategy.is_aggressive_down() { 2 } else { 1 };
            let next_lower_idx = (current_idx - step_size).max(0);
            gpu.get_freq_by_index(next_lower_idx)
        } else {
            now_freq // 保持当前频率
        };

        let final_freq_index = gpu.read_freq_index(final_freq);
        (final_freq, final_freq_index)
    }    /// 处理中等负载区域
    fn handle_medium_load(_gpu: &GPU, _util: i32, _margin: i64, _load_trend: i32, _now_freq: i64) -> (i64, i64) {
        debug!("Medium load zone detected, maintaining current frequency");
        // 中等负载保持当前频率
        (_gpu.get_cur_freq(), _gpu.frequency_manager.cur_freq_idx)
    }

    /// 处理高负载区域
    fn handle_high_load(gpu: &GPU, util: i32, margin: i64, load_trend: i32, now_freq: i64) -> (i64, i64) {
        debug!("High load zone ({}%) detected", util);

        let mut target_freq = now_freq * (util as i64 + margin) / 100;

        // 根据负载趋势调整
        if load_trend > 0 {
            target_freq = target_freq * 115 / 100; // 负载上升，更积极升频
        }

        let final_freq = if target_freq > now_freq {
            // 步进式升频
            let current_idx = gpu.frequency_manager.cur_freq_idx;
            let max_idx = (gpu.get_config_list().len() - 1) as i64;
            let next_higher_idx = (current_idx + 1).min(max_idx);
            gpu.get_freq_by_index(next_higher_idx)
        } else {
            now_freq
        };

        let final_freq_index = gpu.read_freq_index(final_freq);
        (final_freq, final_freq_index)
    }

    /// 处理极高负载区域
    fn handle_very_high_load(gpu: &GPU, util: i32, _margin: i64, load_trend: i32, now_freq: i64) -> (i64, i64) {
        debug!("Very high load zone ({}%) detected", util);

        let current_idx = gpu.frequency_manager.cur_freq_idx;
        let max_idx = (gpu.get_config_list().len() - 1) as i64;
        
        // 计算步进大小
        let freq_position = current_idx as f64 / max_idx as f64;
        let step_size = if freq_position > 0.8 {
            if load_trend > 0 && util >= 95 { 2 } else { 1 }
        } else {
            if load_trend > 0 { 3 } else { 2 }
        };

        let next_higher_idx = (current_idx + step_size).min(max_idx);
        let final_freq = gpu.get_freq_by_index(next_higher_idx);
        
        debug!("Very high load adjustment: {} -> {}KHz (step: {})", now_freq, final_freq, step_size);
        (final_freq, next_higher_idx)
    }

    /// 应用新频率
    fn apply_new_frequency(gpu: &mut GPU, new_freq: i64, freq_index: i64, current_time: u64) -> Result<()> {
        debug!("Applying new frequency: {}KHz (index: {})", new_freq, freq_index);

        // 更新频率管理器
        gpu.frequency_manager.cur_freq = new_freq;
        gpu.frequency_manager.cur_freq_idx = freq_index;

        // 检查DCS条件
        gpu.need_dcs = gpu.dcs_enable && gpu.is_gpuv2() && new_freq < gpu.get_min_freq();

        // 生成电压
        gpu.gen_cur_volt();

        // 写入频率
        gpu.frequency_manager.write_freq(gpu.need_dcs, gpu.is_idle())?;

        // 更新游戏模式下的DDR频率
        if gpu.is_gaming_mode() {
            let ddr_opp = gpu.read_tab(crate::model::gpu::TabType::FreqDram, new_freq);
            if ddr_opp > 0 || ddr_opp == crate::datasource::file_path::DDR_HIGHEST_FREQ {
                if let Err(e) = gpu.set_ddr_freq(ddr_opp) {
                    warn!("Failed to update DDR frequency: {}", e);
                }
            }
        }

        // 重置计数器并更新时间
        gpu.load_analyzer.reset_load_zone_counter();
        gpu.frequency_strategy.update_last_adjustment_time(current_time);

        Ok(())
    }

    /// 应用采样间隔睡眠
    fn apply_sampling_sleep(gpu: &GPU) {
        if gpu.is_precise() {
            return; // 精确模式不睡眠
        }

        let sleep_time = if gpu.frequency_strategy.adaptive_sampling {
            gpu.frequency_strategy.get_sampling_interval()
        } else {
            gpu.frequency_strategy.get_sampling_interval()
        };

        debug!("Sleeping for {}ms", sleep_time);
        std::thread::sleep(Duration::from_millis(sleep_time));
    }
}
