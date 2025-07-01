/// 空闲状态管理器 - 负责GPU空闲状态管理
#[derive(Clone)]
pub struct IdleManager {
    /// 负载区域持续计数
    pub load_zone_counter: i32,
    /// 低负载计数器
    pub load_low: i64,
    /// 是否空闲
    pub is_idle: bool,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            load_zone_counter: 0,
            load_low: 0,
            is_idle: false,
        }
    }

    /// 检查是否空闲
    pub fn check_idle_state(&mut self, util: i32) {
        if util <= 0 {
            self.load_low += 1;
            if self.load_low >= 60 {
                self.is_idle = true;
            }
        } else {
            self.load_low = 0;
            if util > 50 {
                self.is_idle = false;
            }
        }
    }

    /// 重置负载区域计数器
    pub fn reset_load_zone_counter(&mut self) {
        self.load_zone_counter = 0;
    }

    /// 是否空闲
    pub fn is_idle(&self) -> bool {
        self.is_idle
    }

    /// 设置空闲状态
    pub fn set_idle(&mut self, idle: bool) {
        self.is_idle = idle;
    }
}

impl Default for IdleManager {
    fn default() -> Self {
        Self::new()
    }
}
