use anyhow::Result;
use log::{debug, info, warn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{datasource::load_monitor::get_gpu_load, model::gpu::GPU, utils::constants::strategy};

/// GPU频率调整引擎 - 负责执行智能调频算法
pub struct FrequencyAdjustmentEngine;

impl FrequencyAdjustmentEngine {
    /// 主要的频率调整循环
    pub fn run_adjustment_loop(gpu: &mut GPU) -> Result<()> {
        info!("Starting advanced GPU governor with ultra-simplified 90% threshold strategy");

        debug!(
            "config:{:?}, freq:{}",
            gpu.get_config_list(),
            gpu.get_cur_freq()
        );

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
        // 根据负载动态调整采样间隔（如果启用了自适应采样）
        gpu.adjust_sampling_interval_by_load(load);

        // 检查空闲状态
        if load <= gpu.idle_manager.idle_threshold {
            Self::handle_idle_state(gpu);
            return Ok(());
        }

        // 执行频率调整逻辑，包含降频计数器
        Self::execute_frequency_adjustment_with_counter(gpu, load, current_time)
    }

    /// 更新当前GPU频率
    fn update_current_frequency(gpu: &mut GPU) -> Result<()> {
        use crate::datasource::load_monitor::get_gpu_current_freq;

        // 传递驱动类型信息：!gpu.is_gpuv2() 表示是v1驱动
        match get_gpu_current_freq(!gpu.is_gpuv2()) {
            Ok(current_freq) => {
                if current_freq > 0 {
                    gpu.set_cur_freq(current_freq);
                    gpu.frequency_mut().cur_freq_idx =
                        gpu.frequency().read_freq_index(current_freq);
                    debug!("Updated current GPU frequency from file: {current_freq}");
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
        Ok(())
    }

    /// 处理空闲状态
    fn handle_idle_state(gpu: &GPU) {
        let idle_sleep_time = if gpu.is_precise() { 200 } else { 160 };
        debug!("Idle state, sleeping for {idle_sleep_time}ms");
        std::thread::sleep(Duration::from_millis(idle_sleep_time));
    }

    /// 执行频率调整逻辑（包含降频计数器）
    fn execute_frequency_adjustment_with_counter(
        gpu: &mut GPU,
        load: i32,
        current_time: u64,
    ) -> Result<()> {
        debug!("Executing frequency adjustment for load: {load}%");

        let current_freq = gpu.get_cur_freq();
        let current_idx = gpu.frequency().cur_freq_idx;
        let max_idx = (gpu.get_config_list().len() - 1) as i64;
        let down_counter_threshold = gpu.frequency_strategy.down_counter_threshold;

        // 检查是否需要升频（负载达到90%或以上）
        if load >= strategy::ULTRA_SIMPLE_THRESHOLD {
            debug!(
                "Load {}% >= {}%, checking up rate delay",
                load,
                strategy::ULTRA_SIMPLE_THRESHOLD
            );

            // 重置降频计数器（因为检测到高负载）
            if down_counter_threshold > 0 {
                gpu.idle_manager.load_zone_counter = 0;
                debug!("Reset down counter due to high load");
            }

            // 检查升频延迟
            let last_adjust_time = gpu.frequency_strategy.last_adjustment_time;
            let up_delay = gpu.frequency_strategy.up_debounce_time;
            if current_time - last_adjust_time < up_delay {
                debug!(
                    "Up rate delay not met: {}ms < {}ms, skipping frequency change",
                    current_time - last_adjust_time,
                    up_delay
                );
                return Ok(());
            }

            let next_idx = (current_idx + 1).min(max_idx);
            let target_freq = gpu.get_freq_by_index(next_idx);
            if target_freq != current_freq {
                Self::apply_frequency_change(gpu, target_freq, next_idx, current_time)?;
            }
            return Ok(());
        }

        // 处理降频逻辑
        if down_counter_threshold > 0 {
            // 启用降频计数器模式
            debug!(
                "Load {}% < {}%, using down counter (threshold: {})",
                load,
                strategy::ULTRA_SIMPLE_THRESHOLD,
                down_counter_threshold
            );

            // 当前负载低于阈值，增加计数器
            gpu.idle_manager.load_zone_counter += 1;
            debug!(
                "Down counter: {}/{}",
                gpu.idle_manager.load_zone_counter, down_counter_threshold
            );

            // 检查是否达到降频条件
            if gpu.idle_manager.load_zone_counter >= down_counter_threshold as i32 {
                debug!("Down counter threshold reached, checking down rate delay");

                // 检查降频延迟
                let last_adjust_time = gpu.frequency_strategy.last_adjustment_time;
                let down_delay = gpu.frequency_strategy.down_debounce_time;
                if current_time - last_adjust_time < down_delay {
                    debug!(
                        "Down rate delay not met: {}ms < {}ms, skipping frequency change",
                        current_time - last_adjust_time,
                        down_delay
                    );
                    return Ok(());
                }

                // 执行降频
                let next_idx = (current_idx - 1).max(0);
                let target_freq = gpu.get_freq_by_index(next_idx);
                if target_freq != current_freq {
                    Self::apply_frequency_change(gpu, target_freq, next_idx, current_time)?;
                    // 重置计数器
                    gpu.idle_manager.load_zone_counter = 0;
                }
            }
        } else {
            // 禁用降频计数器模式，负载低于90%，降频一级
            debug!(
                "Load {}% < {}%, checking down rate delay (no counter)",
                load,
                strategy::ULTRA_SIMPLE_THRESHOLD
            );

            // 检查降频延迟
            let last_adjust_time = gpu.frequency_strategy.last_adjustment_time;
            let down_delay = gpu.frequency_strategy.down_debounce_time;
            if current_time - last_adjust_time < down_delay {
                debug!(
                    "Down rate delay not met: {}ms < {}ms, skipping frequency change",
                    current_time - last_adjust_time,
                    down_delay
                );
                return Ok(());
            }

            let next_idx = (current_idx - 1).max(0);
            let target_freq = gpu.get_freq_by_index(next_idx);
            if target_freq != current_freq {
                Self::apply_frequency_change(gpu, target_freq, next_idx, current_time)?;
            }
        }

        Ok(())
    }

    /// 应用频率变化
    fn apply_frequency_change(
        gpu: &mut GPU,
        new_freq: i64,
        freq_index: i64,
        current_time: u64,
    ) -> Result<()> {
        debug!("Applying frequency change: {new_freq}KHz (index: {freq_index})");

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
        gpu.frequency_strategy_mut()
            .update_last_adjustment_time(current_time);

        Ok(())
    }

    /// 在游戏模式下更新DDR频率
    fn update_ddr_if_gaming(gpu: &mut GPU, freq: i64) -> Result<()> {
        if gpu.is_gaming_mode() {
            use crate::model::gpu::TabType;
            let ddr_opp = gpu.read_tab(TabType::FreqDram, freq);
            if ddr_opp > 0 || ddr_opp == crate::datasource::file_path::DDR_HIGHEST_FREQ {
                if let Err(e) = gpu.set_ddr_freq(ddr_opp) {
                    warn!("Failed to update DDR frequency: {e}");
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

        let sleep_time = gpu.frequency_strategy.get_sampling_interval();

        debug!("Sleeping for {sleep_time}ms");
        std::thread::sleep(Duration::from_millis(sleep_time));
    }
}
