use log::debug;

/// 负载分析器 - 负责GPU负载监控和趋势分析
#[derive(Clone)]
pub struct LoadAnalyzer {
    /// 负载历史记录
    pub load_history: Vec<i32>,
    /// 负载历史记录大小
    pub load_history_size: usize,
    /// 负载趋势 (-1=下降, 0=稳定, 1=上升)
    pub load_trend: i32,
    /// 当前负载区域 (0=极低, 1=低, 2=中, 3=高, 4=极高)
    pub current_load_zone: i32,
    /// 负载区域持续计数
    pub load_zone_counter: i32,
    /// 低负载计数器
    pub load_low: i64,
    /// 是否空闲
    pub is_idle: bool,
}

impl LoadAnalyzer {
    pub fn new() -> Self {
        Self {
            load_history: Vec::with_capacity(5),
            load_history_size: 5,
            load_trend: 0,
            current_load_zone: 2,
            load_zone_counter: 0,
            load_low: 0,
            is_idle: false,
        }
    }

    /// 更新负载历史记录并分析趋势
    pub fn update_load_history(&mut self, load: i32) -> i32 {
        // 添加新的负载值到历史记录
        self.load_history.push(load);

        // 保持历史记录大小不超过设定值
        if self.load_history.len() > self.load_history_size {
            self.load_history.remove(0);
        }

        // 分析负载趋势
        self.analyze_load_trend()
    }

    /// 分析负载趋势
    fn analyze_load_trend(&mut self) -> i32 {
        if self.load_history.len() < 3 {
            self.load_trend = 0; // 数据不足，认为稳定
            return self.load_trend;
        }

        let len = self.load_history.len();
        let recent_avg = self.load_history[len - 2..].iter().sum::<i32>() as f64 / 2.0;
        let older_avg = self.load_history[..len - 2].iter().sum::<i32>() as f64 / (len - 2) as f64;

        let trend_threshold = 5.0; // 5%的变化才认为是趋势
        
        if recent_avg > older_avg + trend_threshold {
            self.load_trend = 1; // 上升
        } else if recent_avg < older_avg - trend_threshold {
            self.load_trend = -1; // 下降
        } else {
            self.load_trend = 0; // 稳定
        }

        debug!("Load trend: {} (recent: {:.1}%, older: {:.1}%)", 
               match self.load_trend {
                   1 => "Rising",
                   -1 => "Falling", 
                   _ => "Stable"
               }, recent_avg, older_avg);

        self.load_trend
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

    /// 获取当前负载趋势
    pub fn get_load_trend(&self) -> i32 {
        self.load_trend
    }

    /// 获取当前负载区域
    pub fn get_current_load_zone(&self) -> i32 {
        self.current_load_zone
    }

    /// 设置当前负载区域
    pub fn set_current_load_zone(&mut self, zone: i32) {
        if zone != self.current_load_zone {
            debug!("Load zone changed from {} to {}", self.current_load_zone, zone);
            self.load_zone_counter = 1;
        } else {
            self.load_zone_counter += 1;
        }
        self.current_load_zone = zone;
    }

    /// 获取负载区域计数器
    pub fn get_load_zone_counter(&self) -> i32 {
        self.load_zone_counter
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

impl Default for LoadAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
