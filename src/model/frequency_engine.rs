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

            // 简化的99%升频策略：去除复杂的负载分析
            // 检查空闲状态
            if util <= 5 {
                Self::handle_idle_state(gpu);
                continue;
            }

            // 简单的频率调整逻辑
            Self::execute_simple_frequency_adjustment(gpu, util, current_time)?;

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

    /// 超简化的频率调整 - 纯99%升频策略
    fn execute_simple_frequency_adjustment(
        gpu: &mut GPU, 
        util: i32, 
        current_time: u64
    ) -> Result<()> {
        debug!("Executing simple frequency adjustment for load: {}%", util);

        let now_freq = gpu.get_cur_freq();
        let current_idx = gpu.frequency_manager.cur_freq_idx;
        let max_idx = (gpu.get_config_list().len() - 1) as i64;

        let (final_freq, final_freq_index) = if util >= 99 {
            // 负载达到99%或以上，升频一级
            debug!("Load {}% >= 99%, upgrading frequency by 1 step", util);
            let next_higher_idx = (current_idx + 1).min(max_idx);
            let new_freq = gpu.get_freq_by_index(next_higher_idx);
            (new_freq, next_higher_idx)
        } else {
            // 负载低于99%，降频一级（尽可能降频）
            debug!("Load {}% < 99%, downscaling frequency by 1 step", util);
            let next_lower_idx = (current_idx - 1).max(0);
            let new_freq = gpu.get_freq_by_index(next_lower_idx);
            (new_freq, next_lower_idx)
        };

        // 如果频率有变化，应用新频率
        if final_freq != now_freq {
            Self::apply_new_frequency(gpu, final_freq, final_freq_index, current_time)?;
        }

        Ok(())
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
