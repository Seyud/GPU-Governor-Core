/// 调频策略配置 - 负责GPU调频的策略和参数管理
#[derive(Clone)]
pub struct FrequencyStrategy {
    /// 升频延迟
    pub up_debounce_time: u64, // 升频防抖时间（毫秒）
    /// 降频防抖时间
    pub down_debounce_time: u64, // 降频防抖时间（毫秒）
    /// 调整余量
    pub margin: u32, // 频率调整余量（MHz）
    /// 降频计数器配置值（0=禁用降频计数器功能）
    pub down_counter_threshold: u32, // 降频计数器触发阈值
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
            margin: 50,
            down_counter_threshold: 0,
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

    /// 设置降频计数器配置值
    pub fn set_down_counter_threshold(&mut self, threshold: u32) {
        self.down_counter_threshold = threshold;
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

    /// 获取降频计数器配置值
    pub fn get_down_counter_threshold(&self) -> u32 {
        self.down_counter_threshold
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
}

impl Default for FrequencyStrategy {
    fn default() -> Self {
        Self::new(500, 500)
    }
}
