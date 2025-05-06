use std::{collections::HashMap, thread, time::Duration};

use anyhow::Result;
use log::{debug, info, warn};

use crate::{
    datasource::{
        file_path::*,
        load_monitor::{get_gpu_current_freq, get_gpu_load},
    },
    utils::file_operate::write_file_safe,
};

// Macro to simulate goto in Rust
macro_rules! goto_gen_volt {
    ($self:expr, $util:expr) => {
        if $self.load_low >= 60 {
            $self.is_idle = true;
        }
        if $util > 50 {
            $self.is_idle = false;
        }
        $self.cur_volt = $self.gen_cur_volt();
        $self.write_freq()?;
        if $self.is_idle {
            if $self.precise {
                thread::sleep(Duration::from_millis(200));
            } else {
                thread::sleep(Duration::from_millis(160));
            }
            continue;
        }
        $self.is_idle = false;
        if $self.precise {
            continue;
        } else {
            thread::sleep(Duration::from_millis(RESP_TIME));
            continue;
        }
    };
}

pub enum TabType {
    FreqVolt,
    FreqDram,
    #[allow(dead_code)]
    DefVolt,
}

// Writer options
pub enum WriterOpt {
    Idle,
    NoVolt,
    Normal,
}

#[derive(Clone)]
pub struct GPU {
    config_list: Vec<i64>,
    freq_volt: HashMap<i64, i64>,
    freq_dram: HashMap<i64, i64>,
    def_volt: HashMap<i64, i64>,
    v2_supported_freqs: Vec<i64>, // v2 driver支持的频率列表
    cur_freq: i64,
    cur_freq_idx: i64,
    cur_volt: i64,
    load_low: i64,
    is_idle: bool,
    gpuv2: bool,
    dcs_enable: bool,
    gaming_mode: bool,
    precise: bool,
    margin: i64, // 频率计算的余量百分比
    up_rate_delay: u64, // 升频延迟（毫秒）
}

impl GPU {
    pub fn new() -> Self {
        Self {
            config_list: Vec::new(),
            freq_volt: HashMap::new(),
            freq_dram: HashMap::new(),
            def_volt: HashMap::new(),
            v2_supported_freqs: Vec::new(),
            cur_freq: 0,
            cur_freq_idx: 0,
            cur_volt: 0,
            load_low: 0,
            is_idle: false,
            gpuv2: false,
            dcs_enable: false,
            gaming_mode: false,
            precise: false,
            margin: 10, // 默认余量为10%
            up_rate_delay: 50, // 默认升频延迟为50ms
        }
    }

    // 获取当前余量值
    pub fn get_margin(&self) -> i64 {
        self.margin
    }

    // 设置余量值
    pub fn set_margin(&mut self, margin: i64) {
        self.margin = margin;
        info!("Set GPU frequency calculation margin to: {}%", margin);
    }

    fn unify_id(&self, id: i64) -> i64 {
        if id < 0 {
            return 0;
        }
        if id >= self.config_list.len() as i64 {
            return (self.config_list.len() - 1) as i64;
        }
        id
    }

    pub fn read_freq_ge(&self, freq: i64) -> i64 {
        debug!("readFreqGe={}", freq);
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

    pub fn read_freq_le(&self, freq: i64) -> i64 {
        debug!("readFreqLe={}", freq);
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

    pub fn read_freq_index(&self, freq: i64) -> i64 {
        for (i, &cfreq) in self.config_list.iter().enumerate() {
            if cfreq == freq {
                return i as i64;
            }
        }
        0
    }

    pub fn get_freq_by_index(&self, idx: i64) -> i64 {
        let idx = self.unify_id(idx);
        if self.config_list.is_empty() {
            return 0;
        }
        self.config_list[idx as usize]
    }

    pub fn get_volt(&self, freq: i64) -> i64 {
        *self.freq_volt.get(&freq).unwrap_or(&0)
    }

    pub fn adjust_gpufreq(&mut self) -> Result<()> {
        let mut util;
        let mut target_freq;
        let mut final_freq;
        let mut final_freq_index;
        let mut margin;

        debug!("config:{:?}, freq:{}", self.config_list, self.cur_freq);

        loop {
            // 读取当前GPU频率
            match get_gpu_current_freq() {
                Ok(current_freq) => {
                    if current_freq > 0 {
                        // 只有当读取到的频率大于0时才更新
                        self.cur_freq = current_freq;
                        // 更新频率索引
                        self.cur_freq_idx = self.read_freq_index(self.cur_freq);
                        debug!("Updated current GPU frequency from file: {}", current_freq);
                    }
                },
                Err(e) => {
                    // 如果无法读取GPU频率，记录错误并退出
                    return Err(e);
                }
            }

            util = get_gpu_load()?;
            // 使用自定义margin值，游戏模式时增加10%的余量
            margin = if self.gaming_mode { self.margin + 10 } else { self.margin };
            debug!("Current margin value: {}%", margin);

            if util <= 0 {
                self.load_low += 1;
                if self.load_low >= 60 {
                    self.is_idle = true;
                }
                if self.is_idle {
                    thread::sleep(Duration::from_millis(160));
                    continue;
                }
            } else {
                self.load_low = 0;
            }

            let now_freq = self.cur_freq;
            debug!("now_freq {} util={}", now_freq, util);

            target_freq = now_freq * (util as i64 + margin) / 100;
            if now_freq < target_freq {
                final_freq = self.read_freq_ge(target_freq);
            } else {
                final_freq = self.read_freq_le(target_freq);
            }

            final_freq_index = self.read_freq_index(final_freq);
            debug!(
                "target_freq:{}, cur_freq:{}, final_freq:{}, down_freq:{}, up_freq:{}",
                target_freq,
                self.cur_freq,
                final_freq,
                self.gen_cur_freq(self.cur_freq_idx - 1),
                self.gen_cur_freq(final_freq_index)
            );

            if final_freq > self.cur_freq
                || (final_freq == self.cur_freq && target_freq > self.cur_freq)
            {
                debug!("go up");

                // 如果设置了升频延迟，则等待指定的时间
                if self.up_rate_delay > 0 {
                    debug!("Applying up rate delay: {}ms", self.up_rate_delay);
                    thread::sleep(Duration::from_millis(self.up_rate_delay));
                }

                let new_freq = self.gen_cur_freq(final_freq_index);

                // 对于v2 driver设备，验证频率是否在系统支持范围内
                if self.gpuv2 && !self.is_freq_supported_by_v2_driver(new_freq) {
                    debug!(
                        "Freq {} not supported by V2 driver, finding closest supported freq",
                        new_freq
                    );
                    // 如果新频率不在v2 driver支持的范围内，找到最接近的支持频率
                    self.cur_freq = self.get_closest_v2_supported_freq(new_freq);
                    // 更新频率索引
                    self.cur_freq_idx = self.read_freq_index(self.cur_freq);
                } else {
                    self.cur_freq = new_freq;
                    self.cur_freq_idx = final_freq_index;
                }

                self.load_low = 0;
                goto_gen_volt!(self, util);
            }

            if util <= 30 {
                self.load_low += 1;
            } else {
                self.load_low = 0;
            }
            // 使用san常数
            if self.load_low >= 27 {
                debug!("detect down");
                let new_freq = self.gen_cur_freq(final_freq_index);

                // 对于v2 driver设备，验证频率是否在系统支持范围内
                if self.gpuv2 && !self.is_freq_supported_by_v2_driver(new_freq) {
                    debug!(
                        "Freq {} not supported by V2 driver, finding closest supported freq",
                        new_freq
                    );
                    // 如果新频率不在v2 driver支持的范围内，找到最接近的支持频率
                    self.cur_freq = self.get_closest_v2_supported_freq(new_freq);
                    // 更新频率索引
                    self.cur_freq_idx = self.read_freq_index(self.cur_freq);
                } else {
                    self.cur_freq = new_freq;
                    self.cur_freq_idx = final_freq_index;
                }

                goto_gen_volt!(self, util);
            }

            if self.load_low >= 60 {
                self.is_idle = true;
            }
            if util > 50 {
                self.is_idle = false;
            }

            self.cur_volt = self.gen_cur_volt();
            self.write_freq()?;

            if self.is_idle {
                if self.precise {
                    thread::sleep(Duration::from_millis(200));
                } else {
                    thread::sleep(Duration::from_millis(160));
                }
                continue;
            }

            self.is_idle = false;
            if self.precise {
                continue;
            } else {
                thread::sleep(Duration::from_millis(RESP_TIME));
            }
        }
    }

    pub fn write_freq(&self) -> Result<()> {
        // 对于v2 driver设备，获取支持的最接近频率
        let freq_to_use = self.get_closest_v2_supported_freq(self.cur_freq);

        let content = freq_to_use.to_string();
        let volt_content = format!("{} {}", freq_to_use, self.cur_volt);
        let volt_reset = "0 0";
        // 对于v2 driver设备，先尝试写入"-1"，再尝试写入"0"
        let opp_reset_minus_one = "-1";
        let opp_reset_zero = "0";
        let opp_reset_v1 = "0";

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
        let volt_path_exists = std::path::Path::new(volt_path).exists();
        let opp_path_exists = std::path::Path::new(opp_path).exists();

        if !volt_path_exists || !opp_path_exists {
            // 记录警告但不中断程序
            if !volt_path_exists {
                warn!("Voltage control file does not exist: {}", volt_path);
            }
            if !opp_path_exists {
                warn!("Frequency control file does not exist: {}", opp_path);
            }
            // 如果文件不存在，直接返回成功，不尝试写入
            return Ok(());
        }

        let opt = if self.is_idle {
            WriterOpt::Idle
        } else if self.cur_volt == 0 {
            WriterOpt::NoVolt
        } else {
            WriterOpt::Normal
        };

        // 使用安全写入函数
        match opt {
            WriterOpt::Idle => {
                debug!("is idle");
                write_file_safe(volt_path, volt_reset, volt_reset.len())?;

                // 对于v2 driver设备，先尝试写入"-1"，再尝试写入"0"
                if self.gpuv2 {
                    // 先尝试写入"-1"
                    let result = write_file_safe(opp_path, opp_reset_minus_one, opp_reset_minus_one.len());
                    if result.is_err() || result.unwrap() == 0 {
                        debug!("Failed to write '-1' to v2 opp_path, trying '0'");
                        // 如果写入"-1"失败，尝试写入"0"
                        write_file_safe(opp_path, opp_reset_zero, opp_reset_zero.len())?;
                    }
                } else {
                    write_file_safe(opp_path, opp_reset_v1, opp_reset_v1.len())?;
                }
            }
            WriterOpt::NoVolt => {
                debug!("writer has no volt");
                debug!("write {} to opp path", content);
                write_file_safe(volt_path, volt_reset, volt_reset.len())?;
                write_file_safe(opp_path, &content, content.len())?;
            }
            WriterOpt::Normal => {
                debug!("write {} to volt {}", volt_content, opp_path);

                // 对于v2 driver设备，先尝试写入"-1"，再尝试写入"0"
                if self.gpuv2 {
                    // 先尝试写入"-1"
                    let result = write_file_safe(opp_path, opp_reset_minus_one, opp_reset_minus_one.len());
                    if result.is_err() || result.unwrap() == 0 {
                        debug!("Failed to write '-1' to v2 opp_path, trying '0'");
                        // 如果写入"-1"失败，尝试写入"0"
                        write_file_safe(opp_path, opp_reset_zero, opp_reset_zero.len())?;
                    }
                } else {
                    write_file_safe(opp_path, opp_reset_v1, opp_reset_v1.len())?;
                }

                debug!("write {} to volt {}", volt_content, volt_path);
                write_file_safe(volt_path, &volt_content, volt_content.len())?;
            }
        }

        Ok(())
    }

    pub fn gen_cur_freq(&self, idx: i64) -> i64 {
        self.get_freq_by_index(idx)
    }

    pub fn get_config_list(&self) -> Vec<i64> {
        self.config_list.clone()
    }

    pub fn set_config_list(&mut self, config_list: Vec<i64>) {
        self.config_list = config_list;
    }

    pub fn replace_tab(&mut self, tab_type: TabType, tab: HashMap<i64, i64>) {
        match tab_type {
            TabType::FreqVolt => self.freq_volt = tab,
            TabType::FreqDram => self.freq_dram = tab,
            TabType::DefVolt => self.def_volt = tab,
        }
    }

    pub fn read_tab(&self, tab_type: TabType, freq: i64) -> i64 {
        match tab_type {
            TabType::FreqVolt => *self.freq_volt.get(&freq).unwrap_or(&0),
            TabType::FreqDram => *self.freq_dram.get(&freq).unwrap_or(&0),
            TabType::DefVolt => *self.def_volt.get(&freq).unwrap_or(&0),
        }
    }

    pub fn get_cur_freq(&self) -> i64 {
        self.cur_freq
    }

    pub fn set_cur_freq(&mut self, cur_freq: i64) {
        self.cur_freq = cur_freq;
    }

    pub fn is_gpuv2(&self) -> bool {
        self.gpuv2
    }

    pub fn set_gpuv2(&mut self, gpuv2: bool) {
        self.gpuv2 = gpuv2;
    }

    pub fn set_dcs_enable(&mut self, dcs_enable: bool) {
        self.dcs_enable = dcs_enable;
    }

    pub fn set_gaming_mode(&mut self, gaming_mode: bool) {
        self.gaming_mode = gaming_mode;
    }

    pub fn gen_cur_volt(&mut self) -> i64 {
        // 对于v2 driver设备，获取支持的最接近频率
        let freq_to_use = self.get_closest_v2_supported_freq(self.cur_freq);

        self.cur_volt = self.get_volt(freq_to_use);
        self.cur_volt
    }

    pub fn is_precise(&self) -> bool {
        self.precise
    }

    pub fn set_precise(&mut self, precise: bool) {
        self.precise = precise;
    }

    // 获取当前升频延迟值
    pub fn get_up_rate_delay(&self) -> u64 {
        self.up_rate_delay
    }

    // 设置升频延迟值
    pub fn set_up_rate_delay(&mut self, delay: u64) {
        self.up_rate_delay = delay;
        info!("Set GPU up rate delay to: {}ms", delay);
    }

    #[allow(dead_code)]
    pub fn get_v2_supported_freqs(&self) -> Vec<i64> {
        self.v2_supported_freqs.clone()
    }

    pub fn set_v2_supported_freqs(&mut self, freqs: Vec<i64>) {
        self.v2_supported_freqs = freqs;
    }

    pub fn is_freq_supported_by_v2_driver(&self, freq: i64) -> bool {
        if !self.gpuv2 || self.v2_supported_freqs.is_empty() {
            // 如果不是v2 driver或者没有读取到支持的频率，则不进行验证
            return true;
        }

        // 检查频率是否在支持的范围内
        self.v2_supported_freqs.contains(&freq)
    }

    // 获取v2 driver支持的最接近频率
    pub fn get_closest_v2_supported_freq(&self, freq: i64) -> i64 {
        if !self.gpuv2
            || self.v2_supported_freqs.is_empty()
            || self.is_freq_supported_by_v2_driver(freq)
        {
            // 如果不是v2 driver或者没有读取到支持的频率，或者频率已经在支持范围内，则直接返回原频率
            return freq;
        }

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
