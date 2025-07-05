use anyhow::Result;
use log::debug;
use std::collections::HashMap;

use crate::datasource::file_path::*;
use crate::utils::file_helper::FileHelper;

/// 频率管理器 - 负责GPU频率的计算和调整逻辑
#[derive(Clone)]
pub struct FrequencyManager {
    /// 可用频率列表
    pub config_list: Vec<i64>,
    /// 频率到电压的映射
    pub freq_volt: HashMap<i64, i64>,
    /// 频率到DDR的映射
    pub freq_dram: HashMap<i64, i64>,
    /// 当前频率
    pub cur_freq: i64,
    /// 当前频率索引
    pub cur_freq_idx: i64,
    /// 当前电压
    pub cur_volt: i64,
    /// 是否使用v2驱动
    pub gpuv2: bool,
    /// v2驱动支持的频率列表
    pub v2_supported_freqs: Vec<i64>,
}

impl FrequencyManager {
    pub fn new() -> Self {
        Self {
            config_list: Vec::new(),
            freq_volt: HashMap::new(),
            freq_dram: HashMap::new(),
            cur_freq: 0,
            cur_freq_idx: 0,
            cur_volt: 0,
            gpuv2: false,
            v2_supported_freqs: Vec::new(),
        }
    }

    /// 获取频率对应的电压
    pub fn get_volt(&self, freq: i64) -> i64 {
        *self.freq_volt.get(&freq).unwrap_or(&0)
    }

    /// 根据索引获取频率
    pub fn get_freq_by_index(&self, idx: i64) -> i64 {
        let unified_idx = self.unify_id(idx);
        self.config_list
            .get(unified_idx as usize)
            .copied()
            .unwrap_or(0)
    }

    /// 获取大于等于指定频率的最小频率
    pub fn read_freq_ge(&self, freq: i64) -> i64 {
        debug!("readFreqGe={freq}");
        if freq <= 0 {
            return *self.config_list.last().unwrap_or(&0);
        }
        for &cfreq in &self.config_list {
            if cfreq >= freq {
                return cfreq;
            }
        }
        *self.config_list.last().unwrap_or(&0)
    }

    /// 获取小于等于指定频率的最大频率
    pub fn read_freq_le(&self, freq: i64) -> i64 {
        debug!("readFreqLe={freq}");
        if freq <= 0 {
            return *self.config_list.first().unwrap_or(&0);
        }
        for &cfreq in self.config_list.iter().rev() {
            if cfreq <= freq {
                return cfreq;
            }
        }
        *self.config_list.first().unwrap_or(&0)
    }

    /// 获取频率对应的索引
    pub fn read_freq_index(&self, freq: i64) -> i64 {
        for (i, &cfreq) in self.config_list.iter().enumerate() {
            if cfreq == freq {
                return i as i64;
            }
        }
        0
    }

    /// 获取最高频率
    pub fn get_max_freq(&self) -> i64 {
        *self.config_list.last().unwrap_or(&0)
    }

    /// 获取最低频率
    pub fn get_min_freq(&self) -> i64 {
        *self.config_list.first().unwrap_or(&0)
    }

    /// 获取中等频率
    pub fn get_middle_freq(&self) -> i64 {
        if self.config_list.is_empty() {
            return 0;
        }
        let mid_idx = self.config_list.len() / 2;
        self.config_list[mid_idx]
    }

    /// 获取第二高频率
    pub fn get_second_highest_freq(&self) -> i64 {
        if self.config_list.len() < 2 {
            return self.get_max_freq();
        }
        self.config_list[self.config_list.len() - 2]
    }

    /// 获取v2驱动支持的最接近频率
    pub fn get_closest_v2_supported_freq(&self, target_freq: i64) -> i64 {
        if self.v2_supported_freqs.is_empty() {
            return target_freq;
        }

        let mut closest_freq = self.v2_supported_freqs[0];
        let mut min_diff = (target_freq - closest_freq).abs();

        for &freq in &self.v2_supported_freqs {
            let diff = (target_freq - freq).abs();
            if diff < min_diff {
                min_diff = diff;
                closest_freq = freq;
            }
        }

        closest_freq
    }

    /// 生成当前电压
    pub fn gen_cur_volt(&mut self) -> i64 {
        // 对于v2 driver设备，获取支持的最接近频率
        let freq_to_use = self.get_closest_v2_supported_freq(self.cur_freq);

        // 获取电压值，优先使用频率-电压表，如果没有则尝试使用默认电压表
        self.cur_volt = self.get_volt(freq_to_use);

        self.cur_volt
    }

    /// 写入频率到系统文件
    pub fn write_freq(&self, need_dcs: bool, is_idle: bool) -> Result<()> {
        // 根据驱动类型获取要使用的频率
        let freq_to_use = if self.gpuv2 {
            self.get_closest_v2_supported_freq(self.cur_freq)
        } else {
            self.cur_freq
        };

        let content = freq_to_use.to_string();
        let volt_content = format!("{} {}", freq_to_use, self.cur_volt);
        let volt_reset = "0 0";
        let opp_reset_minus_one = "-1";
        let opp_reset_zero = "0";

        let volt_path = if self.gpuv2 {
            GPUFREQV2_VOLT
        } else {
            GPUFREQ_VOLT
        };
        let opp_path = if self.gpuv2 {
            GPUFREQV2_OPP
        } else {
            GPUFREQ_OPP
        };

        // 检查文件是否存在
        if !std::path::Path::new(volt_path).exists() || !std::path::Path::new(opp_path).exists() {
            return Ok(());
        }

        // 确定写入模式
        if is_idle {
            self.write_idle_mode(volt_path, opp_path, volt_reset, opp_reset_zero)?;
        } else if need_dcs && self.gpuv2 && self.cur_freq_idx == 0 {
            self.write_dcs_mode(
                volt_path,
                opp_path,
                volt_reset,
                opp_reset_minus_one,
                opp_reset_zero,
            )?;
        } else if self.cur_volt == 0 {
            self.write_no_volt_mode(volt_path, opp_path, volt_reset, &content)?;
        } else {
            self.write_normal_mode(
                volt_path,
                opp_path,
                volt_reset,
                opp_reset_minus_one,
                opp_reset_zero,
                &volt_content,
            )?;
        }

        Ok(())
    }

    /// 空闲模式写入
    fn write_idle_mode(
        &self,
        volt_path: &str,
        opp_path: &str,
        volt_reset: &str,
        opp_reset_zero: &str,
    ) -> Result<()> {
        debug!("Writing in idle mode");
        if self.gpuv2 {
            FileHelper::write_string_safe(volt_path, volt_reset);
            let result = FileHelper::write_string_safe(opp_path, "-1");
            if !result {
                FileHelper::write_string_safe(opp_path, opp_reset_zero);
            }
        } else {
            FileHelper::write_string_safe(volt_path, volt_reset);
            FileHelper::write_string_safe(opp_path, opp_reset_zero);
        }
        Ok(())
    }

    /// DCS模式写入
    fn write_dcs_mode(
        &self,
        volt_path: &str,
        opp_path: &str,
        volt_reset: &str,
        opp_reset_minus_one: &str,
        opp_reset_zero: &str,
    ) -> Result<()> {
        debug!("Writing in DCS mode");
        FileHelper::write_string_safe(volt_path, volt_reset);
        let result = FileHelper::write_string_safe(opp_path, opp_reset_minus_one);
        if !result {
            FileHelper::write_string_safe(opp_path, opp_reset_zero);
        }
        Ok(())
    }

    /// 无电压模式写入
    fn write_no_volt_mode(
        &self,
        volt_path: &str,
        opp_path: &str,
        volt_reset: &str,
        content: &str,
    ) -> Result<()> {
        debug!("Writing in no-volt mode");
        FileHelper::write_string_safe(volt_path, volt_reset);
        FileHelper::write_string_safe(opp_path, content);
        Ok(())
    }

    /// 正常模式写入
    fn write_normal_mode(
        &self,
        volt_path: &str,
        opp_path: &str,
        volt_reset: &str,
        opp_reset_minus_one: &str,
        opp_reset_zero: &str,
        volt_content: &str,
    ) -> Result<()> {
        debug!("Writing in normal mode");
        if self.gpuv2 {
            FileHelper::write_string_safe(volt_path, volt_reset);
            let result = FileHelper::write_string_safe(opp_path, opp_reset_minus_one);
            if !result {
                FileHelper::write_string_safe(opp_path, opp_reset_zero);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            FileHelper::write_string_safe(volt_path, volt_content);
        } else {
            FileHelper::write_string_safe(opp_path, opp_reset_zero);
            FileHelper::write_string_safe(volt_path, volt_content);
        }
        Ok(())
    }

    /// 统一ID范围
    fn unify_id(&self, id: i64) -> i64 {
        if id < 0 {
            return 0;
        }
        if id >= self.config_list.len() as i64 {
            return (self.config_list.len() - 1) as i64;
        }
        id
    }

    /// 设置配置列表
    pub fn set_config_list(&mut self, config_list: Vec<i64>) {
        self.config_list = config_list;
    }

    /// 获取配置列表
    pub fn get_config_list(&self) -> Vec<i64> {
        self.config_list.clone()
    }

    /// 替换映射表
    pub fn replace_freq_volt_tab(&mut self, tab: HashMap<i64, i64>) {
        self.freq_volt = tab;
    }

    pub fn replace_freq_dram_tab(&mut self, tab: HashMap<i64, i64>) {
        self.freq_dram = tab;
    }

    /// 读取映射表值
    pub fn read_freq_volt(&self, freq: i64) -> i64 {
        *self.freq_volt.get(&freq).unwrap_or(&0)
    }

    pub fn read_freq_dram(&self, freq: i64) -> i64 {
        *self.freq_dram.get(&freq).unwrap_or(&0)
    }
}

impl Default for FrequencyManager {
    fn default() -> Self {
        Self::new()
    }
}
