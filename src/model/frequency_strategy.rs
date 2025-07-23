/// 调频策略配置 - 负责GPU调频的策略和参数管理
#[derive(Clone)]
pub struct FrequencyStrategy {
    /// 升频延迟
    pub up_debounce_time: u64, // 升频防抖时间（毫秒）
    /// 降频防抖时间
    pub down_debounce_time: u64, // 降频防抖时间（毫秒）
    /// 负载阈值
    pub very_low_load_threshold: u32, // 极低负载阈值（百分比）
    pub low_load_threshold: u32,       // 低负载阈值（百分比）
    pub high_load_threshold: u32,      // 高负载阈值（百分比）
    pub very_high_load_threshold: u32, // 极高负载阈值（百分比）
    /// 调整余量
    pub margin: u32,   // 频率调整余量（MHz）
    /// 降频阈值
    pub down_threshold: u32, // 降频判断阈值（百分比）
    /// 激进降频开关
    pub aggressive_down: bool, // 是否启用激进降频
    /// 采样间隔
    pub sampling_interval: u64, // 采样间隔（毫秒）
    /// 上次调整时间
    pub last_adjustment_time: u64, // 上次频率调整时间戳（毫秒）
}

impl FrequencyStrategy {
    pub fn new(up_time: u64, down_time: u64) -> Self {
        Self {
            up_debounce_time: up_time,
            very_low_load_threshold: 20,
            low_load_threshold: 40,
            high_load_threshold: 70,
            very_high_load_threshold: 90,
            margin: 50,
            down_threshold: 10,
            aggressive_down: true,
            sampling_interval: 8,
            last_adjustment_time: 0,
            down_debounce_time: down_time,
        }
    }

    /// 设置频率调整余量
    pub fn set_margin(&mut self, margin: u32) {
        self.margin = margin;
    }

    /// 设置降频阈值
    pub fn set_down_threshold(&mut self, threshold: u32) {
        self.down_threshold = threshold;
    }

    /// 设置激进降频开关
    pub fn set_aggressive_down(&mut self, enable: bool) {
        self.aggressive_down = enable;
    }

    /// 设置采样间隔
    pub fn set_sampling_interval(&mut self, interval: u64) {
        self.sampling_interval = interval;
    }

    /// 获取采样间隔
    pub fn get_sampling_interval(&self) -> u64 {
        self.sampling_interval
    }

    /// 获取余量
    pub fn get_margin(&self) -> u32 {
        self.margin
    }

    /// 获取降频阈值
    pub fn get_down_threshold(&self) -> u32 {
        self.down_threshold
    }

    /// 更新最后调整时间
    pub fn update_last_adjustment_time(&mut self, time: u64) {
        self.last_adjustment_time = time;
    }

    /// 设置升频延迟
    pub fn set_up_rate_delay(&mut self, delay: u64) {
        self.up_debounce_time = delay;
    }

    /// 设置防抖时间（升频和降频）
    pub fn set_debounce_times(&mut self, up_time: u64, down_time: u64) {
        self.up_debounce_time = up_time;
        self.down_debounce_time = down_time;
    }

    /// 设置负载阈值
    pub fn set_load_thresholds(&mut self, very_low: u32, low: u32, high: u32, very_high: u32) {
        self.very_low_load_threshold = very_low;
        self.low_load_threshold = low;
        self.high_load_threshold = high;
        self.very_high_load_threshold = very_high;
    }
}

impl Default for FrequencyStrategy {
    fn default() -> Self {
        Self::new(500, 500)
    }
}
