use log::debug;

/// 调频策略配置 - 负责GPU调频的策略和参数管理
#[derive(Clone)]
pub struct FrequencyStrategy {
    /// 多级负载阈值
    pub very_low_load_threshold: i32, // 极低负载阈值
    pub low_load_threshold: i32,       // 低负载阈值
    pub high_load_threshold: i32,      // 高负载阈值
    pub very_high_load_threshold: i32, // 极高负载阈值

    /// 负载稳定性阈值
    pub load_stability_threshold: i32, // 需要连续多少次采样才确认负载区域变化

    /// 滞后与去抖动机制
    pub hysteresis_up_threshold: i32, // 升频滞后阈值（百分比）
    pub hysteresis_down_threshold: i32, // 降频滞后阈值（百分比）
    pub debounce_time_up: u64,          // 升频去抖动时间（毫秒）
    pub debounce_time_down: u64,        // 降频去抖动时间（毫秒）

    /// 频率调整策略
    pub aggressive_down: bool, // 是否使用激进降频策略
    pub margin: i64,         // 频率计算的余量百分比
    pub up_rate_delay: u64,  // 升频延迟（毫秒）
    pub down_threshold: i64, // 降频阈值

    /// 采样相关
    pub sampling_interval: u64, // 采样间隔（毫秒）
    pub adaptive_sampling: bool,    // 是否启用自适应采样
    pub min_sampling_interval: u64, // 最小采样间隔（毫秒）
    pub max_sampling_interval: u64, // 最大采样间隔（毫秒）

    /// 时间戳
    pub last_adjustment_time: u64, // 上次频率调整时间（毫秒）
}

impl FrequencyStrategy {
    pub fn new() -> Self {
        Self {
            // 超简化：只保留99%阈值，其他阈值不重要
            very_low_load_threshold: 0,   // 不使用
            low_load_threshold: 0,        // 不使用
            high_load_threshold: 0,       // 不使用
            very_high_load_threshold: 99, // 唯一重要的阈值：99%升频

            // 简化的稳定性设置
            load_stability_threshold: 1, // 立即响应

            // 完全禁用滞后与去抖动机制
            hysteresis_up_threshold: 0,
            hysteresis_down_threshold: 0,
            debounce_time_up: 0,
            debounce_time_down: 0,

            // 简化的频率调整策略
            aggressive_down: true, // 启用激进降频
            margin: 0,             // 无余量
            up_rate_delay: 0,      // 无升频延迟
            down_threshold: 1,     // 降频阈值为1

            // 固定采样设置 - 120Hz
            sampling_interval: 8,     // 固定8ms采样间隔，约120Hz
            adaptive_sampling: false, // 禁用自适应采样
            min_sampling_interval: 8, // 固定最小采样间隔
            max_sampling_interval: 8, // 固定最大采样间隔

            // 时间戳默认值
            last_adjustment_time: 0,
        }
    }

    /// 设置游戏模式参数 - 超简化版
    pub fn set_gaming_mode_params(&mut self) {
        // 游戏模式：99%升频，相对保守的降频
        self.very_high_load_threshold = 99; // 唯一重要的阈值
        self.load_stability_threshold = 1; // 立即响应
        self.aggressive_down = false; // 游戏模式较保守的降频

        // 固定120Hz采样
        self.sampling_interval = 8;
        self.adaptive_sampling = false;

        debug!("Gaming mode: 99% upgrade threshold, conservative downscaling, 120Hz sampling");
    }

    /// 设置普通模式参数 - 超简化版
    pub fn set_normal_mode_params(&mut self) {
        // 普通模式：99%升频，激进降频节省功耗
        self.very_high_load_threshold = 99; // 唯一重要的阈值
        self.load_stability_threshold = 1; // 立即响应
        self.aggressive_down = true; // 激进降频节省功耗

        // 固定120Hz采样
        self.sampling_interval = 8;
        self.adaptive_sampling = false;

        debug!("Normal mode: 99% upgrade threshold, aggressive downscaling, 120Hz sampling");
    }

    /// 设置负载阈值
    pub fn set_load_thresholds(&mut self, very_low: i32, low: i32, high: i32, very_high: i32) {
        self.very_low_load_threshold = very_low;
        self.low_load_threshold = low;
        self.high_load_threshold = high;
        self.very_high_load_threshold = very_high;
        debug!(
            "Set load thresholds: very_low={very_low}%, low={low}%, high={high}%, very_high={very_high}%"
        );
    }

    /// 设置滞后阈值
    pub fn set_hysteresis_thresholds(&mut self, up_threshold: i32, down_threshold: i32) {
        self.hysteresis_up_threshold = up_threshold;
        self.hysteresis_down_threshold = down_threshold;
        debug!(
            "Set hysteresis thresholds: up={up_threshold}%, down={down_threshold}%"
        );
    }

    /// 设置去抖动时间
    pub fn set_debounce_times(&mut self, up_time: u64, down_time: u64) {
        self.debounce_time_up = up_time;
        self.debounce_time_down = down_time;
        debug!("Set debounce times: up={up_time}ms, down={down_time}ms");
    }

    /// 设置自适应采样参数
    pub fn set_adaptive_sampling(&mut self, enabled: bool, min_interval: u64, max_interval: u64) {
        self.adaptive_sampling = enabled;
        self.min_sampling_interval = min_interval;
        self.max_sampling_interval = max_interval;
        debug!(
            "Set adaptive sampling: enabled={enabled}, min={min_interval}ms, max={max_interval}ms"
        );
    }

    /// 更新最后调整时间
    pub fn update_last_adjustment_time(&mut self, time: u64) {
        self.last_adjustment_time = time;
    }

    // Getter和Setter方法 - 手动实现，确保编译通过
    pub fn get_margin(&self) -> i64 {
        self.margin
    }

    pub fn set_margin(&mut self, margin: i64) {
        self.margin = margin;
        debug!("Set margin to: {margin}%");
    }

    pub fn set_up_rate_delay(&mut self, up_rate_delay: u64) {
        self.up_rate_delay = up_rate_delay;
        debug!("Set up rate delay to: {up_rate_delay}ms");
    }

    pub fn get_down_threshold(&self) -> i64 {
        self.down_threshold
    }

    pub fn set_down_threshold(&mut self, down_threshold: i64) {
        self.down_threshold = down_threshold;
        debug!("Set down threshold to: {down_threshold}");
    }

    pub fn get_sampling_interval(&self) -> u64 {
        self.sampling_interval
    }

    pub fn set_sampling_interval(&mut self, sampling_interval: u64) {
        self.sampling_interval = sampling_interval;
        debug!("Set sampling interval to: {sampling_interval}ms");
    }

    pub fn set_load_stability_threshold(&mut self, load_stability_threshold: i32) {
        self.load_stability_threshold = load_stability_threshold;
        debug!(
            "Set load stability threshold to: {load_stability_threshold}"
        );
    }

    pub fn set_aggressive_down(&mut self, aggressive: bool) {
        self.aggressive_down = aggressive;
        debug!(
            "Set aggressive downscaling: {}",
            if aggressive { "enabled" } else { "disabled" }
        );
    }
}

impl Default for FrequencyStrategy {
    fn default() -> Self {
        Self::new()
    }
}
