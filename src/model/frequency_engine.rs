use std::time::{SystemTime, UNIX_EPOCH, Duration};
use anyhow::Result;
use log::{debug, info, warn};

use crate::{
    datasource::load_monitor::get_gpu_load,
    model::gpu::GPU,
    utils::constants::strategy,
};

/// GPU频率调整引擎 - 负责执行智能调频算法
pub struct FrequencyAdjustmentEngine;

impl FrequencyAdjustmentEngine {
    /// 主要的频率调整循环
    pub fn run_adjustment_loop(gpu: &mut GPU) -> Result<()> {
        info!("Starting advanced GPU governor with ultra-simplified 99% threshold strategy");
        
        debug!("config:{:?}, freq:{}", gpu.get_config_list(), gpu.get_cur_freq());

        loop {
            let current_time = Self::get_current_time_ms();
            
            // 更新当前GPU频率
            Self::update_current_frequency(gpu)?;

            // 读取当前GPU负载
            let load = get_gpu_load()?;

            // 处理负载
            Self::process_load(gpu, load, current_time)?;

            // 应用采样睡眠
            Self::apply_sampling_sleep(gpu);
        }
    }

    /// 获取当前时间戳（毫秒）
    fn get_current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// 处理负载数据
    fn process_load(gpu: &mut GPU, load: i32, current_time: u64) -> Result<()> {
        // 检查空闲状态
        if load <= strategy::IDLE_THRESHOLD {
            Self::handle_idle_state(gpu);
            return Ok(());
        }

        // 执行简单的频率调整逻辑
        Self::execute_frequency_adjustment(gpu, load, current_time)
    }

    /// 更新当前GPU频率
    fn update_current_frequency(gpu: &mut GPU) -> Result<()> {
        use crate::datasource::load_monitor::get_gpu_current_freq;
        
        match get_gpu_current_freq() {
            Ok(current_freq) => {
                if current_freq > 0 {
                    gpu.set_cur_freq(current_freq);
                    gpu.frequency_mut().cur_freq_idx = gpu.frequency().read_freq_index(current_freq);
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

    /// 执行频率调整逻辑
    fn execute_frequency_adjustment(
        gpu: &mut GPU, 
        load: i32, 
        current_time: u64
    ) -> Result<()> {
        debug!("Executing frequency adjustment for load: {}%", load);

        let current_freq = gpu.get_cur_freq();
        let current_idx = gpu.frequency().cur_freq_idx;
        let max_idx = (gpu.get_config_list().len() - 1) as i64;

        let (target_freq, target_idx) = if load >= strategy::ULTRA_SIMPLE_THRESHOLD {
            // 负载达到99%或以上，升频一级
            debug!("Load {}% >= {}%, upgrading frequency", load, strategy::ULTRA_SIMPLE_THRESHOLD);
            let next_idx = (current_idx + 1).min(max_idx);
            (gpu.get_freq_by_index(next_idx), next_idx)
        } else {
            // 负载低于99%，降频一级
            debug!("Load {}% < {}%, downscaling frequency", load, strategy::ULTRA_SIMPLE_THRESHOLD);
            let next_idx = (current_idx - 1).max(0);
            (gpu.get_freq_by_index(next_idx), next_idx)
        };

        // 应用频率变化
        if target_freq != current_freq {
            Self::apply_frequency_change(gpu, target_freq, target_idx, current_time)?;
        }

        Ok(())
    }

    /// 应用频率变化
    fn apply_frequency_change(gpu: &mut GPU, new_freq: i64, freq_index: i64, current_time: u64) -> Result<()> {
        debug!("Applying frequency change: {}KHz (index: {})", new_freq, freq_index);

        // 更新频率管理器
        gpu.frequency_mut().cur_freq = new_freq;
        gpu.frequency_mut().cur_freq_idx = freq_index;

        // 检查DCS条件
        gpu.need_dcs = gpu.dcs_enable && gpu.is_gpuv2() && new_freq < gpu.get_min_freq();

        // 生成电压并写入
        gpu.frequency_mut().gen_cur_volt();
        gpu.frequency().write_freq(gpu.need_dcs, gpu.is_idle())?;

        // 更新游戏模式下的DDR频率
        Self::update_ddr_if_gaming(gpu, new_freq)?;

        // 重置计数器并更新时间
        gpu.reset_load_zone_counter();
        gpu.frequency_strategy_mut().update_last_adjustment_time(current_time);

        Ok(())
    }

    /// 在游戏模式下更新DDR频率
    fn update_ddr_if_gaming(gpu: &mut GPU, freq: i64) -> Result<()> {
        if gpu.is_gaming_mode() {
            use crate::model::gpu::TabType;
            let ddr_opp = gpu.read_tab(TabType::FreqDram, freq);
            if ddr_opp > 0 || ddr_opp == crate::datasource::file_path::DDR_HIGHEST_FREQ {
                if let Err(e) = gpu.set_ddr_freq(ddr_opp) {
                    warn!("Failed to update DDR frequency: {}", e);
                }
            }
        }
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
