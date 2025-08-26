use anyhow::Result;
use log::{debug, warn};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{datasource::load_monitor::get_gpu_load, model::gpu::GPU};

/// GPU频率调整引擎 - 负责执行智能调频算法
pub struct FrequencyAdjustmentEngine;

impl FrequencyAdjustmentEngine {
    /// 主要的频率调整循环
    pub fn run_adjustment_loop(gpu: &mut GPU) -> Result<()> {
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

        // 执行频率调整逻辑，使用连续调频公式
        Self::execute_frequency_adjustment_with_formula(gpu, load, current_time)
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

    /// 执行频率调整逻辑（使用连续调频公式）
    fn execute_frequency_adjustment_with_formula(
        gpu: &mut GPU,
        load: i32,
        current_time: u64,
    ) -> Result<()> {
        debug!("Executing frequency adjustment for load: {load}%");

        let current_freq = gpu.get_cur_freq();
        let margin = gpu.frequency_strategy.margin;

        // 使用新的连续调频公式：targetFreq = now_freq * (util + margin) / 100
        // 其中util是负载百分比，margin是调整余量
        let load_factor = (load as f64 + margin as f64) / 100.0;
        let raw_target_freq = (current_freq as f64 * load_factor) as i64;

        // 确保目标频率在有效范围内
        let min_freq = gpu.get_min_freq();
        let max_freq = gpu.get_max_freq();
        let target_freq = raw_target_freq.clamp(min_freq, max_freq);

        debug!(
            "Current freq: {current_freq}KHz, load: {load}%, margin: {margin}%, calculated target: {target_freq}KHz"
        );

        // 如果频率没有变化，直接返回
        if target_freq == current_freq {
            debug!("No frequency change needed");
            return Ok(());
        }

        // 确定频率变化方向用于防抖延迟
        let is_increasing = target_freq > current_freq;

        // 检查防抖延迟
        let last_adjust_time = gpu.frequency_strategy.last_adjustment_time;
        let delay = if is_increasing {
            gpu.frequency_strategy.up_debounce_time
        } else {
            gpu.frequency_strategy.down_debounce_time
        };

        if current_time - last_adjust_time < delay {
            debug!(
                "Rate delay not met: {}ms < {}ms, skipping frequency change",
                current_time - last_adjust_time,
                delay
            );
            return Ok(());
        }

        // 找到最接近目标频率的索引
        let target_idx = gpu.find_closest_freq_index(target_freq);
        Self::apply_frequency_change(gpu, target_freq, target_idx, current_time)?;

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
            if (ddr_opp > 0 || ddr_opp == crate::datasource::file_path::DDR_HIGHEST_FREQ)
                && let Err(e) = gpu.set_ddr_freq(ddr_opp)
            {
                warn!("Failed to update DDR frequency: {e}");
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
