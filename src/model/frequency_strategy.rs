use log::debug;

/// 调频策略配置 - 负责GPU调频的策略和参数管理
#[derive(Clone)]
pub struct FrequencyStrategy {
    /// 多级负载阈值
    pub very_low_load_threshold: i32,  // 极低负载阈值
    pub low_load_threshold: i32,       // 低负载阈值
    pub high_load_threshold: i32,      // 高负载阈值
    pub very_high_load_threshold: i32, // 极高负载阈值

    /// 负载稳定性阈值
    pub load_stability_threshold: i32, // 需要连续多少次采样才确认负载区域变化

    /// 滞后与去抖动机制
    pub hysteresis_up_threshold: i32,   // 升频滞后阈值（百分比）
    pub hysteresis_down_threshold: i32, // 降频滞后阈值（百分比）
    pub debounce_time_up: u64,         // 升频去抖动时间（毫秒）
    pub debounce_time_down: u64,       // 降频去抖动时间（毫秒）

    /// 频率调整策略
    pub aggressive_down: bool,         // 是否使用激进降频策略
    pub margin: i64,                   // 频率计算的余量百分比
    pub up_rate_delay: u64,           // 升频延迟（毫秒）
    pub down_threshold: i64,          // 降频阈值

    /// 采样相关
    pub sampling_interval: u64,        // 采样间隔（毫秒）
    pub adaptive_sampling: bool,       // 是否启用自适应采样
    pub min_sampling_interval: u64,    // 最小采样间隔（毫秒）
    pub max_sampling_interval: u64,    // 最大采样间隔（毫秒）

    /// 时间戳
    pub last_adjustment_time: u64,     // 上次频率调整时间（毫秒）
}

impl FrequencyStrategy {
    pub fn new() -> Self {
        Self {
            // 多级负载阈值默认值
            very_low_load_threshold: 10,   // 10% 以下为极低负载
            low_load_threshold: 30,        // 30% 以下为低负载
            high_load_threshold: 70,       // 70% 以上为高负载
            very_high_load_threshold: 85,  // 85% 以上为极高负载

            // 负载稳定性默认值
            load_stability_threshold: 3,   // 需要连续3次采样确认负载区域变化

            // 滞后与去抖动机制默认值
            hysteresis_up_threshold: 75,   // 默认升频滞后阈值为75%
            hysteresis_down_threshold: 30, // 默认降频滞后阈值为30%
            debounce_time_up: 20,          // 默认升频去抖动时间为20ms
            debounce_time_down: 50,        // 默认降频去抖动时间为50ms

            // 频率调整策略默认值
            aggressive_down: true,         // 默认启用激进降频
            margin: 5,                     // 默认余量为5%
            up_rate_delay: 50,            // 默认升频延迟为50ms
            down_threshold: 10,           // 默认降频阈值为10

            // 采样相关默认值
            sampling_interval: 16,         // 默认采样间隔16ms
            adaptive_sampling: true,       // 默认启用自适应采样
            min_sampling_interval: 10,     // 最小采样间隔为10ms
            max_sampling_interval: 100,    // 最大采样间隔为100ms

            // 时间戳默认值
            last_adjustment_time: 0,
        }
    }

    /// 确定当前负载所属区域，考虑滞后阈值
    pub fn determine_load_zone(&self, load: i32) -> i32 {
        if load <= self.very_low_load_threshold {
            0 // 极低负载区域
        } else if load <= self.low_load_threshold {
            1 // 低负载区域
        } else if load < self.high_load_threshold {
            2 // 中等负载区域
        } else if load < self.very_high_load_threshold {
            3 // 高负载区域
        } else {
            4 // 极高负载区域
        }
    }

    /// 检查是否满足去抖动时间要求
    pub fn check_debounce_time(&self, current_time: u64, target_higher: bool) -> bool {
        let elapsed = current_time - self.last_adjustment_time;
        let required_time = if target_higher {
            self.debounce_time_up
        } else {
            self.debounce_time_down
        };
        
        elapsed >= required_time
    }

    /// 获取所需的去抖动时间
    pub fn get_required_debounce_time(&self, target_higher: bool) -> u64 {
        if target_higher {
            self.debounce_time_up
        } else {
            self.debounce_time_down
        }
    }

    /// 根据负载波动性和当前负载调整采样间隔
    pub fn adjust_sampling_interval(&mut self, load: i32) -> u64 {
        if !self.adaptive_sampling {
            return self.sampling_interval;
        }

        // 根据负载值调整采样间隔
        // 高负载时使用更短的采样间隔，低负载时使用更长的采样间隔
        let load_factor = if load > 80 {
            0.5 // 高负载时减半
        } else if load > 50 {
            0.8 // 中等负载时稍微减少
        } else if load < 20 {
            1.5 // 低负载时增加
        } else {
            1.0 // 正常负载
        };

        let new_interval = (self.sampling_interval as f64 * load_factor) as u64;
        self.sampling_interval = new_interval.clamp(self.min_sampling_interval, self.max_sampling_interval);

        debug!("Adjusted sampling interval to {}ms based on load {}%", self.sampling_interval, load);
        self.sampling_interval
    }

    /// 计算调整后的margin值
    pub fn calculate_margin(&self, load_trend: i32, gaming_mode: bool) -> i64 {
        let mut margin = if gaming_mode { 
            self.margin + 10 
        } else { 
            self.margin 
        };

        // 根据负载趋势适度调整margin
        if load_trend > 0 {
            margin += 3; // 负载上升趋势，适度增加margin
        } else if load_trend < 0 {
            margin = if margin > 3 { margin - 3 } else { margin }; // 负载下降趋势，适度减少margin
        }

        debug!("Calculated margin: {}% (trend: {}, gaming: {})", margin, load_trend, gaming_mode);
        margin
    }

    /// 设置游戏模式参数
    pub fn set_gaming_mode_params(&mut self) {
        // 游戏模式：更激进的升频，更保守的降频
        self.set_load_thresholds(5, 20, 60, 85); // 更低的高负载阈值
        self.load_stability_threshold = 2;       // 更低的稳定性阈值
        self.aggressive_down = false;            // 禁用激进降频
        self.set_hysteresis_thresholds(65, 40);  // 更宽松的滞后阈值
        self.set_debounce_times(10, 30);         // 更短的去抖动时间
        self.set_adaptive_sampling(true, 8, 50); // 更短的采样间隔范围

        debug!("Applied gaming mode frequency strategy");
    }

    /// 设置普通模式参数
    pub fn set_normal_mode_params(&mut self) {
        // 普通模式：更保守的升频，更激进的降频
        self.set_load_thresholds(10, 30, 70, 90); // 默认负载阈值
        self.load_stability_threshold = 3;         // 默认稳定性阈值
        self.aggressive_down = true;               // 启用激进降频
        self.set_hysteresis_thresholds(75, 30);    // 更严格的滞后阈值
        self.set_debounce_times(20, 50);           // 更长的去抖动时间
        self.set_adaptive_sampling(true, 10, 100); // 更宽的采样间隔范围

        debug!("Applied normal mode frequency strategy");
    }

    /// 设置负载阈值
    pub fn set_load_thresholds(&mut self, very_low: i32, low: i32, high: i32, very_high: i32) {
        self.very_low_load_threshold = very_low;
        self.low_load_threshold = low;
        self.high_load_threshold = high;
        self.very_high_load_threshold = very_high;
        debug!("Set load thresholds: very_low={}%, low={}%, high={}%, very_high={}%",
               very_low, low, high, very_high);
    }

    /// 设置滞后阈值
    pub fn set_hysteresis_thresholds(&mut self, up_threshold: i32, down_threshold: i32) {
        self.hysteresis_up_threshold = up_threshold;
        self.hysteresis_down_threshold = down_threshold;
        debug!("Set hysteresis thresholds: up={}%, down={}%", up_threshold, down_threshold);
    }

    /// 设置去抖动时间
    pub fn set_debounce_times(&mut self, up_time: u64, down_time: u64) {
        self.debounce_time_up = up_time;
        self.debounce_time_down = down_time;
        debug!("Set debounce times: up={}ms, down={}ms", up_time, down_time);
    }

    /// 设置自适应采样参数
    pub fn set_adaptive_sampling(&mut self, enabled: bool, min_interval: u64, max_interval: u64) {
        self.adaptive_sampling = enabled;
        self.min_sampling_interval = min_interval;
        self.max_sampling_interval = max_interval;
        debug!("Set adaptive sampling: enabled={}, min={}ms, max={}ms", 
               enabled, min_interval, max_interval);
    }

    /// 更新最后调整时间
    pub fn update_last_adjustment_time(&mut self, time: u64) {
        self.last_adjustment_time = time;
    }

    // Getter方法
    pub fn get_margin(&self) -> i64 { self.margin }
    pub fn get_up_rate_delay(&self) -> u64 { self.up_rate_delay }
    pub fn get_down_threshold(&self) -> i64 { self.down_threshold }
    pub fn get_sampling_interval(&self) -> u64 { self.sampling_interval }
    pub fn is_aggressive_down(&self) -> bool { self.aggressive_down }
    pub fn get_load_stability_threshold(&self) -> i32 { self.load_stability_threshold }

    // Setter方法
    pub fn set_margin(&mut self, margin: i64) {
        self.margin = margin;
        debug!("Set margin to: {}%", margin);
    }

    pub fn set_up_rate_delay(&mut self, delay: u64) {
        self.up_rate_delay = delay;
        debug!("Set up rate delay to: {}ms", delay);
    }

    pub fn set_down_threshold(&mut self, threshold: i64) {
        self.down_threshold = threshold;
        debug!("Set down threshold to: {}", threshold);
    }

    pub fn set_sampling_interval(&mut self, interval: u64) {
        self.sampling_interval = interval;
        debug!("Set sampling interval to: {}ms", interval);
    }

    pub fn set_aggressive_down(&mut self, aggressive: bool) {
        self.aggressive_down = aggressive;
        debug!("Set aggressive downscaling: {}", if aggressive { "enabled" } else { "disabled" });
    }

    pub fn set_load_stability_threshold(&mut self, threshold: i32) {
        self.load_stability_threshold = threshold;
        debug!("Set load stability threshold to: {}", threshold);
    }
}

impl Default for FrequencyStrategy {
    fn default() -> Self {
        Self::new()
    }
}
