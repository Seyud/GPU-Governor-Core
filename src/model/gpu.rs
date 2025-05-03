use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use log::{debug, warn};

use crate::datasource::file_path::*;
use crate::datasource::load_monitor::get_gpu_load;
use crate::utils::file_operate::write_file_safe;

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
    cur_freq: i64,
    cur_freq_idx: i64,
    cur_volt: i64,
    load_low: i64,
    is_idle: bool,
    gpuv2: bool,
    dcs_enable: bool,
    gaming_mode: bool,
    precise: bool,
}

impl GPU {
    pub fn new() -> Self {
        Self {
            config_list: Vec::new(),
            freq_volt: HashMap::new(),
            freq_dram: HashMap::new(),
            def_volt: HashMap::new(),
            cur_freq: 0,
            cur_freq_idx: 0,
            cur_volt: 0,
            load_low: 0,
            is_idle: false,
            gpuv2: false,
            dcs_enable: false,
            gaming_mode: false,
            precise: false,
        }
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
        let margin = if self.gaming_mode { 30 } else { 20 };

        loop {
            util = get_gpu_load()?;
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
                self.cur_freq = self.gen_cur_freq(final_freq_index);
                self.cur_freq_idx = final_freq_index;
                self.load_low = 0;
                goto_gen_volt!(self, util);
            }

            if util <= 30 {
                self.load_low += 1;
            } else {
                self.load_low = 0;
            }

            if self.load_low >= 30 {
                debug!("detect down");
                self.cur_freq = self.gen_cur_freq(final_freq_index);
                self.cur_freq_idx = final_freq_index;
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
        let content = self.cur_freq.to_string();
        let volt_content = format!("{} {}", self.cur_freq, self.cur_volt);
        let volt_reset = "0 0";
        let opp_reset = "0";
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
                warn!("电压控制文件不存在: {}", volt_path);
            }
            if !opp_path_exists {
                warn!("频率控制文件不存在: {}", opp_path);
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
                write_file_safe(
                    opp_path,
                    if self.gpuv2 { opp_reset } else { opp_reset_v1 },
                    if self.gpuv2 {
                        opp_reset.len()
                    } else {
                        opp_reset_v1.len()
                    },
                )?;
            }
            WriterOpt::NoVolt => {
                debug!("writer has no volt");
                debug!("write {} to opp path", content);
                write_file_safe(volt_path, volt_reset, volt_reset.len())?;
                write_file_safe(opp_path, &content, content.len())?;
            }
            WriterOpt::Normal => {
                debug!("write {} to volt path", volt_content);
                write_file_safe(
                    opp_path,
                    if self.gpuv2 { opp_reset } else { opp_reset_v1 },
                    if self.gpuv2 {
                        opp_reset.len()
                    } else {
                        opp_reset_v1.len()
                    },
                )?;
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
        self.cur_volt = self.get_volt(self.cur_freq);
        self.cur_volt
    }

    pub fn is_precise(&self) -> bool {
        self.precise
    }

    pub fn set_precise(&mut self, precise: bool) {
        self.precise = precise;
    }
}
