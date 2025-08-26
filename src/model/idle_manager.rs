/// 空闲状态管理器 - 负责GPU空闲状态管理
#[derive(Clone)]
pub struct IdleManager {
    /// 是否空闲
    pub is_idle: bool,
    /// 空闲阈值
    pub idle_threshold: i32,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            is_idle: false,
            idle_threshold: crate::utils::constants::strategy::IDLE_THRESHOLD,
        }
    }

    /// 设置空闲阈值
    pub fn set_idle_threshold(&mut self, threshold: i32) {
        self.idle_threshold = threshold;
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
