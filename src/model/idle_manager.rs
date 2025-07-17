/// 空闲状态管理器 - 负责GPU空闲状态管理
#[derive(Clone)]
pub struct IdleManager {
    /// 负载区域持续计数
    pub load_zone_counter: i32,
    /// 是否空闲
    pub is_idle: bool,
    /// 空闲阈值
    pub idle_threshold: i32,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            load_zone_counter: 0,
            is_idle: false,
            idle_threshold: crate::utils::constants::strategy::IDLE_THRESHOLD,
        }
    }

    /// 设置空闲阈值
    pub fn set_idle_threshold(&mut self, threshold: i32) {
        self.idle_threshold = threshold;
    }

    /// 重置负载区域计数器
    pub fn reset_load_zone_counter(&mut self) {
        self.load_zone_counter = 0;
    }

    /// 是否空闲
    pub fn is_idle(&self) -> bool {
        self.is_idle
    }
}

impl Default for IdleManager {
    fn default() -> Self {
        Self::new()
    }
}
