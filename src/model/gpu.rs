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
    down_threshold: i64, // 降频阈值，达到此值时触发降频

    // 新增多级负载阈值
    very_low_load_threshold: i32, // 极低负载阈值
    low_load_threshold: i32,      // 低负载阈值
    high_load_threshold: i32,     // 高负载阈值
    very_high_load_threshold: i32, // 极高负载阈值

    // 负载稳定性计数器
    load_zone_counter: i32,       // 当前负载区域持续计数
    current_load_zone: i32,       // 当前负载区域 (0=极低, 1=低, 2=中, 3=高, 4=极高)
    load_stability_threshold: i32, // 负载稳定性阈值，需要连续多少次采样才确认负载区域变化

    // 频率调整策略标志
    aggressive_down: bool,        // 是否使用激进降频策略
    last_adjustment_time: u64,    // 上次频率调整时间（毫秒）
    sampling_interval: u64,       // 采样间隔（毫秒）
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
            down_threshold: 10, // 默认降频阈值为10

            // 多级负载阈值默认值
            very_low_load_threshold: 10,  // 10% 以下为极低负载
            low_load_threshold: 30,       // 30% 以下为低负载
            high_load_threshold: 70,      // 70% 以上为高负载
            very_high_load_threshold: 90, // 90% 以上为极高负载

            // 负载稳定性默认值
            load_zone_counter: 0,
            current_load_zone: 2,         // 默认为中等负载区域
            load_stability_threshold: 3,  // 需要连续3次采样确认负载区域变化

            // 频率调整策略默认值
            aggressive_down: true,        // 默认启用激进降频
            last_adjustment_time: 0,
            sampling_interval: 16,        // 默认采样间隔16ms
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
        use std::time::{SystemTime, UNIX_EPOCH};

        let mut util;
        let mut target_freq;
        let mut final_freq;
        let mut final_freq_index;
        let mut margin;
        let mut new_load_zone;
        let mut current_time;

        info!("Starting advanced GPU governor with multi-threshold strategy");
        info!("Load thresholds: very_low={}%, low={}%, high={}%, very_high={}%",
              self.very_low_load_threshold, self.low_load_threshold,
              self.high_load_threshold, self.very_high_load_threshold);
        debug!("config:{:?}, freq:{}", self.config_list, self.cur_freq);

        loop {
            // 获取当前时间戳（毫秒）
            current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

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

            // 读取当前GPU负载
            util = get_gpu_load()?;

            // 使用自定义margin值，游戏模式时增加10%的余量
            margin = if self.gaming_mode { self.margin + 10 } else { self.margin };
            debug!("Current margin value: {}%, GPU load: {}%", margin, util);

            // 确定当前负载区域
            new_load_zone = self.determine_load_zone(util);

            // 负载区域稳定性检查
            if new_load_zone == self.current_load_zone {
                // 如果负载区域没有变化，增加计数器
                self.load_zone_counter += 1;
            } else {
                // 如果负载区域发生变化，重置计数器
                self.load_zone_counter = 1;
                debug!("Load zone changed from {} to {}", self.current_load_zone, new_load_zone);
            }

            // 更新当前负载区域
            self.current_load_zone = new_load_zone;

            // 特殊处理：如果负载为0，增加idle计数
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
                // 如果负载不为0，重置idle计数
                self.load_low = 0;
            }

            let now_freq = self.cur_freq;
            debug!("Current freq: {}KHz, load: {}%", now_freq, util);

            // 检查是否需要调整频率
            // 只有当负载区域稳定或者处于极端负载区域时才调整频率
            if self.load_zone_counter >= self.load_stability_threshold ||
               self.current_load_zone == 0 || self.current_load_zone == 4 {

                // 根据负载区域选择调频策略
                match self.current_load_zone {
                    0 => {
                        // 极低负载区域 - 策略二：目标式跳转（激进降频）
                        debug!("Very low load zone ({}%) detected, applying aggressive downscaling", util);

                        if self.aggressive_down {
                            // 直接跳转到最低频率（Race to Idle）
                            final_freq = self.get_min_freq();
                            debug!("Aggressive down: jumping to min freq: {}KHz", final_freq);
                        } else {
                            // 保守降频：降低到次低频率
                            final_freq = self.get_second_lowest_freq();
                            debug!("Conservative down: stepping to second lowest freq: {}KHz", final_freq);
                        }

                        final_freq_index = self.read_freq_index(final_freq);

                        // 应用新频率
                        let new_freq = self.gen_cur_freq(final_freq_index);
                        self.apply_new_frequency(new_freq, final_freq_index)?;

                        // 重置负载区域计数器
                        self.load_zone_counter = 0;
                        self.last_adjustment_time = current_time;

                        // 更新电压并写入
                        goto_gen_volt!(self, util);
                    },
                    1 => {
                        // 低负载区域 - 策略一：步进式调整（保守降频）
                        debug!("Low load zone ({}%) detected, applying conservative downscaling", util);

                        // 计算目标频率
                        target_freq = now_freq * (util as i64 + margin) / 100;

                        // 如果目标频率低于当前频率，降低一个档位
                        if target_freq < now_freq {
                            // 步进式降频：降低一个档位
                            let next_lower_idx = self.cur_freq_idx + 1; // 注意：频率表是从高到低排序的
                            final_freq = self.gen_cur_freq(next_lower_idx);
                            final_freq_index = next_lower_idx;

                            debug!("Stepping down one level to: {}KHz", final_freq);

                            // 应用新频率
                            let new_freq = self.gen_cur_freq(final_freq_index);
                            self.apply_new_frequency(new_freq, final_freq_index)?;

                            // 重置负载区域计数器
                            self.load_zone_counter = 0;
                            self.last_adjustment_time = current_time;

                            // 更新电压并写入
                            goto_gen_volt!(self, util);
                        }
                    },
                    2 => {
                        // 中等负载区域 - 保持当前频率或微调
                        debug!("Medium load zone ({}%) detected, fine-tuning frequency", util);

                        // 计算目标频率
                        target_freq = now_freq * (util as i64 + margin) / 100;

                        // 根据目标频率微调
                        if target_freq > now_freq * 110 / 100 {
                            // 如果目标频率比当前频率高10%以上，升高一个档位
                            let next_higher_idx = self.cur_freq_idx - 1; // 注意：频率表是从高到低排序的
                            final_freq = self.gen_cur_freq(next_higher_idx);
                            final_freq_index = next_higher_idx;

                            debug!("Fine-tuning: stepping up one level to: {}KHz", final_freq);

                            // 应用升频延迟
                            if self.up_rate_delay > 0 {
                                debug!("Applying up rate delay: {}ms", self.up_rate_delay);
                                thread::sleep(Duration::from_millis(self.up_rate_delay));
                            }

                            // 应用新频率
                            let new_freq = self.gen_cur_freq(final_freq_index);
                            self.apply_new_frequency(new_freq, final_freq_index)?;

                            // 重置负载区域计数器
                            self.load_zone_counter = 0;
                            self.last_adjustment_time = current_time;

                            // 更新电压并写入
                            goto_gen_volt!(self, util);
                        } else if target_freq < now_freq * 90 / 100 {
                            // 如果目标频率比当前频率低10%以上，降低一个档位
                            let next_lower_idx = self.cur_freq_idx + 1; // 注意：频率表是从高到低排序的
                            final_freq = self.gen_cur_freq(next_lower_idx);
                            final_freq_index = next_lower_idx;

                            debug!("Fine-tuning: stepping down one level to: {}KHz", final_freq);

                            // 应用新频率
                            let new_freq = self.gen_cur_freq(final_freq_index);
                            self.apply_new_frequency(new_freq, final_freq_index)?;

                            // 重置负载区域计数器
                            self.load_zone_counter = 0;
                            self.last_adjustment_time = current_time;

                            // 更新电压并写入
                            goto_gen_volt!(self, util);
                        }
                    },
                    3 => {
                        // 高负载区域 - 策略一：步进式调整（保守升频）
                        debug!("High load zone ({}%) detected, applying conservative upscaling", util);

                        // 计算目标频率
                        target_freq = now_freq * (util as i64 + margin) / 100;

                        // 如果目标频率高于当前频率，升高一个档位
                        if target_freq > now_freq {
                            // 步进式升频：升高一个档位
                            let next_higher_idx = self.cur_freq_idx - 1; // 注意：频率表是从高到低排序的
                            final_freq = self.gen_cur_freq(next_higher_idx);
                            final_freq_index = next_higher_idx;

                            debug!("Stepping up one level to: {}KHz", final_freq);

                            // 应用升频延迟
                            if self.up_rate_delay > 0 {
                                debug!("Applying up rate delay: {}ms", self.up_rate_delay);
                                thread::sleep(Duration::from_millis(self.up_rate_delay));
                            }

                            // 应用新频率
                            let new_freq = self.gen_cur_freq(final_freq_index);
                            self.apply_new_frequency(new_freq, final_freq_index)?;

                            // 重置负载区域计数器
                            self.load_zone_counter = 0;
                            self.last_adjustment_time = current_time;

                            // 更新电压并写入
                            goto_gen_volt!(self, util);
                        }
                    },
                    4 => {
                        // 极高负载区域 - 策略二：目标式跳转（激进升频）
                        debug!("Very high load zone ({}%) detected, applying aggressive upscaling", util);

                        // 直接跳转到最高频率或次高频率
                        final_freq = self.get_max_freq();
                        final_freq_index = self.read_freq_index(final_freq);

                        debug!("Aggressive up: jumping to max freq: {}KHz", final_freq);

                        // 应用升频延迟
                        if self.up_rate_delay > 0 {
                            debug!("Applying up rate delay: {}ms", self.up_rate_delay);
                            thread::sleep(Duration::from_millis(self.up_rate_delay));
                        }

                        // 应用新频率
                        let new_freq = self.gen_cur_freq(final_freq_index);
                        self.apply_new_frequency(new_freq, final_freq_index)?;

                        // 重置负载区域计数器
                        self.load_zone_counter = 0;
                        self.last_adjustment_time = current_time;

                        // 更新电压并写入
                        goto_gen_volt!(self, util);
                    },
                    _ => {
                        // 不应该到达这里
                        warn!("Invalid load zone: {}", self.current_load_zone);
                    }
                }
            } else {
                debug!("Load zone {} not stable yet (counter: {}/{}), maintaining current frequency",
                       self.current_load_zone, self.load_zone_counter, self.load_stability_threshold);
            }

            // 处理空闲状态
            if self.load_low >= 60 {
                self.is_idle = true;
            }
            if util > 50 {
                self.is_idle = false;
            }

            // 更新电压并写入频率
            self.cur_volt = self.gen_cur_volt();
            self.write_freq()?;

            // 根据状态决定休眠时间
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
                thread::sleep(Duration::from_millis(self.sampling_interval));
            }
        }
    }

    // 辅助方法：应用新频率
    fn apply_new_frequency(&mut self, new_freq: i64, freq_index: i64) -> Result<()> {
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
            self.cur_freq_idx = freq_index;
        }

        debug!("Applied new frequency: {}KHz (index: {})", self.cur_freq, self.cur_freq_idx);
        Ok(())
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

    pub fn is_gaming_mode(&self) -> bool {
        self.gaming_mode
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

    // 获取当前降频阈值
    pub fn get_down_threshold(&self) -> i64 {
        self.down_threshold
    }

    // 设置降频阈值
    pub fn set_down_threshold(&mut self, threshold: i64) {
        self.down_threshold = threshold;
        info!("Set GPU down threshold to: {}", threshold);
    }

    #[allow(dead_code)]
    pub fn get_v2_supported_freqs(&self) -> Vec<i64> {
        self.v2_supported_freqs.clone()
    }

    pub fn set_v2_supported_freqs(&mut self, freqs: Vec<i64>) {
        self.v2_supported_freqs = freqs;
    }

    // 设置负载阈值
    pub fn set_load_thresholds(&mut self, very_low: i32, low: i32, high: i32, very_high: i32) {
        self.very_low_load_threshold = very_low;
        self.low_load_threshold = low;
        self.high_load_threshold = high;
        self.very_high_load_threshold = very_high;
        info!("Set GPU load thresholds: very_low={}%, low={}%, high={}%, very_high={}%",
              very_low, low, high, very_high);
    }

    // 设置负载稳定性阈值
    pub fn set_load_stability_threshold(&mut self, threshold: i32) {
        self.load_stability_threshold = threshold;
        info!("Set GPU load stability threshold: {} consecutive samples", threshold);
    }

    // 设置采样间隔
    pub fn set_sampling_interval(&mut self, interval: u64) {
        self.sampling_interval = interval;
        info!("Set GPU sampling interval: {}ms", interval);
    }

    // 设置激进降频模式
    pub fn set_aggressive_down(&mut self, aggressive: bool) {
        self.aggressive_down = aggressive;
        info!("Set GPU aggressive downscaling: {}", if aggressive { "enabled" } else { "disabled" });
    }

    // 确定当前负载所属区域
    pub fn determine_load_zone(&self, load: i32) -> i32 {
        if load <= self.very_low_load_threshold {
            return 0; // 极低负载区域
        } else if load <= self.low_load_threshold {
            return 1; // 低负载区域
        } else if load >= self.very_high_load_threshold {
            return 4; // 极高负载区域
        } else if load >= self.high_load_threshold {
            return 3; // 高负载区域
        } else {
            return 2; // 中等负载区域
        }
    }

    // 获取最高频率
    pub fn get_max_freq(&self) -> i64 {
        if self.config_list.is_empty() {
            return 0;
        }
        *self.config_list.iter().max().unwrap_or(&0)
    }

    // 获取最低频率
    pub fn get_min_freq(&self) -> i64 {
        if self.config_list.is_empty() {
            return 0;
        }
        *self.config_list.iter().min().unwrap_or(&0)
    }

    // 获取次高频率
    pub fn get_second_highest_freq(&self) -> i64 {
        if self.config_list.len() < 2 {
            return self.get_max_freq();
        }

        // 找出次高频率
        let mut sorted_freqs = self.config_list.clone();
        sorted_freqs.sort_by(|a, b| b.cmp(a)); // 降序排序
        sorted_freqs[1] // 第二个元素是次高频率
    }

    // 获取次低频率
    pub fn get_second_lowest_freq(&self) -> i64 {
        if self.config_list.len() < 2 {
            return self.get_min_freq();
        }

        // 找出次低频率
        let mut sorted_freqs = self.config_list.clone();
        sorted_freqs.sort(); // 升序排序
        sorted_freqs[1] // 第二个元素是次低频率
    }

    // 获取中间频率
    pub fn get_middle_freq(&self) -> i64 {
        if self.config_list.is_empty() {
            return 0;
        }

        // 对频率列表进行排序，然后取中间值
        let mut sorted_freqs = self.config_list.clone();
        sorted_freqs.sort(); // 升序排序
        let mid_idx = sorted_freqs.len() / 2;
        sorted_freqs[mid_idx]
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
