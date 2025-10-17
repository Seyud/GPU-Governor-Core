use std::{
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use anyhow::Result;
use inotify::WatchMask;
use log::{LevelFilter, debug, info, warn};

use crate::{
    datasource::file_path::LOG_LEVEL_PATH,
    utils::{
        file_operate::check_read_simple, inotify::InotifyWatcher, log_rotation::LogRotationMonitor,
    },
};

/// 统一的日志等级管理器
pub struct LogLevelManager {
    current_level: Arc<Mutex<LevelFilter>>,
    rotation_monitor: Arc<Mutex<Option<LogRotationMonitor>>>,
}

impl LogLevelManager {
    /// 创建新的日志等级管理器
    pub fn new() -> Self {
        Self {
            current_level: Arc::new(Mutex::new(LevelFilter::Info)),
            rotation_monitor: Arc::new(Mutex::new(None)),
        }
    }

    /// 读取日志等级配置文件
    pub fn read_log_level_config() -> Result<LevelFilter> {
        // 默认日志等级为Info
        let default_level = LevelFilter::Info;

        // 检查配置文件是否存在
        if !Path::new(LOG_LEVEL_PATH).exists() {
            return Ok(default_level);
        }

        // 尝试读取配置文件
        let content = match std::fs::read_to_string(LOG_LEVEL_PATH) {
            Ok(content) => content,
            Err(_) => return Ok(default_level),
        };

        // 解析日志等级
        let level_str = content.trim().to_lowercase();
        match level_str.as_str() {
            "debug" => Ok(LevelFilter::Debug),
            "info" => Ok(LevelFilter::Info),
            "warn" => Ok(LevelFilter::Warn),
            "error" => Ok(LevelFilter::Error),
            _ => Ok(default_level),
        }
    }

    /// 获取当前日志等级
    pub fn get_current_level(&self) -> LevelFilter {
        *self.current_level.lock().unwrap()
    }

    /// 更新日志等级
    pub fn update_level(&self, new_level: LevelFilter) {
        let mut current = self.current_level.lock().unwrap();
        if *current != new_level {
            let old_level = *current;
            *current = new_level;
            drop(current); // 释放锁

            // 更新全局日志等级
            log::set_max_level(new_level);
            info!("Log level updated to: {new_level}");

            // 根据日志等级变化管理日志轮转监控
            self.manage_log_rotation_monitor(old_level, new_level);
        }
    }

    /// 根据日志等级管理日志轮转监控
    fn manage_log_rotation_monitor(&self, old_level: LevelFilter, new_level: LevelFilter) {
        let mut monitor = self.rotation_monitor.lock().unwrap();

        // 如果新等级是Debug，且之前不是Debug，则启动监控
        if new_level == LevelFilter::Debug && old_level != LevelFilter::Debug {
            if monitor.is_none() {
                match crate::utils::log_rotation::start_main_log_monitor() {
                    Ok(rotation_monitor) => {
                        *monitor = Some(rotation_monitor);
                        info!("Log rotation background monitor started (Debug mode enabled)");
                    }
                    Err(e) => {
                        warn!("Failed to start log rotation monitor: {}", e);
                    }
                }
            }
        }
        // 如果新等级不是Debug，且之前是Debug，则停止监控
        else if new_level != LevelFilter::Debug
            && old_level == LevelFilter::Debug
            && let Some(mut rotation_monitor) = monitor.take()
        {
            if let Err(e) = rotation_monitor.stop() {
                warn!("Failed to stop log rotation monitor: {}", e);
            } else {
                info!("Log rotation background monitor stopped (Debug mode disabled)");
            }
        }
    }

    /// 启动日志等级监控线程
    pub fn start_monitoring(self: Arc<Self>) -> Result<()> {
        info!("Starting unified log level monitor");

        // 检查日志等级文件路径
        if !check_read_simple(LOG_LEVEL_PATH) {
            info!("Log level file does not exist: {LOG_LEVEL_PATH}");
        } else {
            info!("Using log level path: {LOG_LEVEL_PATH}");
        }

        // 初始化当前日志等级
        match Self::read_log_level_config() {
            Ok(level) => {
                self.update_level(level);
                info!("Initial log level set to: {level}");
            }
            Err(e) => {
                warn!("Failed to read initial log level config: {e}");
            }
        }

        // 设置文件监控
        let mut inotify = InotifyWatcher::new()?;
        inotify.add(LOG_LEVEL_PATH, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;

        // 主监控循环
        loop {
            // 等待文件变化事件
            if let Err(e) = inotify.wait_and_handle() {
                warn!("Inotify error in log level monitor: {e}");
                thread::sleep(Duration::from_secs(1));
                continue;
            }

            // 检查文件是否存在
            if !check_read_simple(LOG_LEVEL_PATH) {
                debug!("Log level file no longer exists");
                continue;
            }

            // 读取新的日志等级配置
            match Self::read_log_level_config() {
                Ok(new_level) => {
                    self.update_level(new_level);
                }
                Err(e) => {
                    warn!("Failed to update log level: {e}");
                }
            }
        }
    }
}

/// 全局日志等级管理器实例
static LOG_LEVEL_MANAGER: once_cell::sync::Lazy<Arc<LogLevelManager>> =
    once_cell::sync::Lazy::new(|| Arc::new(LogLevelManager::new()));

/// 获取全局日志等级管理器
pub fn get_log_level_manager() -> Arc<LogLevelManager> {
    LOG_LEVEL_MANAGER.clone()
}

/// 启动统一的日志等级监控
pub fn start_unified_log_level_monitor() -> Result<()> {
    let manager = get_log_level_manager();
    manager.start_monitoring()
}

/// 获取当前日志等级（便捷函数）
pub fn get_current_log_level() -> LevelFilter {
    get_log_level_manager().get_current_level()
}
