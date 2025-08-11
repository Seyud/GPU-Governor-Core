use anyhow::{Context, Result};
use chrono::Local;
use log::{debug, info, warn, LevelFilter};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::datasource::file_path::LOG_PATH;
use crate::utils::log_level_manager::get_current_log_level;

/// 日志轮转管理器
pub struct LogRotationManager {
    max_size_bytes: u64,
    rotation_threshold: f64,
    monitor_running: Arc<AtomicBool>,
    monitor_interval: Duration,
}

/// 后台监控线程句柄
pub struct LogRotationMonitor {
    running: Arc<AtomicBool>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl LogRotationManager {
    /// 创建新的日志轮转管理器
    ///
    /// # Arguments
    /// * `max_size_mb` - 最大日志文件大小（MB）
    /// * `rotation_threshold` - 轮转阈值（0.0-1.0），默认0.8表示80%
    /// * `monitor_interval_seconds` - 监控检查间隔（秒），默认30秒
    pub fn new(
        max_size_mb: u64,
        rotation_threshold: Option<f64>,
        monitor_interval_seconds: Option<u64>,
    ) -> Self {
        Self {
            max_size_bytes: max_size_mb * 1024 * 1024,
            rotation_threshold: rotation_threshold.unwrap_or(0.8),
            monitor_running: Arc::new(AtomicBool::new(false)),
            monitor_interval: Duration::from_secs(monitor_interval_seconds.unwrap_or(30)),
        }
    }

    /// 创建默认的日志轮转管理器（10MB，80%阈值，60秒检查间隔）
    pub fn default() -> Self {
        Self::new(10, Some(0.8), Some(60))
    }

    /// 检查是否需要轮转日志
    pub fn should_rotate(&self, log_file_path: &str) -> Result<bool> {
        // 只有在debug日志等级时才检测日志文件大小
        if get_current_log_level() != LevelFilter::Debug {
            return Ok(false);
        }

        let path = Path::new(log_file_path);

        if !path.exists() {
            return Ok(false);
        }

        let metadata = path
            .metadata()
            .with_context(|| format!("Failed to get metadata for: {log_file_path}"))?;

        let file_size = metadata.len();
        let threshold_size = (self.max_size_bytes as f64 * self.rotation_threshold) as u64;

        debug!("Log file size: {file_size} bytes, threshold: {threshold_size} bytes");

        Ok(file_size > threshold_size)
    }

    /// 执行日志轮转
    pub fn rotate_log(&self, log_file_path: &str) -> Result<()> {
        let log_path = Path::new(log_file_path);

        if !log_path.exists() {
            debug!("Log file does not exist, no rotation needed: {log_file_path}");
            return Ok(());
        }

        let backup_path = format!("{log_file_path}.bak");

        // 如果备份文件已存在，删除它
        if Path::new(&backup_path).exists() {
            fs::remove_file(&backup_path)
                .with_context(|| format!("Failed to remove old backup file: {backup_path}"))?;
            debug!("Removed old backup file: {backup_path}");
        }

        // 将当前日志文件重命名为备份文件
        fs::rename(log_path, &backup_path)
            .with_context(|| format!("Failed to rename log file to backup: {backup_path}"))?;

        info!("Log file rotated: {log_file_path} -> {backup_path}");

        // 创建新的日志文件并写入轮转信息
        let rotation_msg = format!(
            "{} - Log rotated, previous log backed up to {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            backup_path
        );

        fs::write(log_path, rotation_msg)
            .with_context(|| format!("Failed to create new log file: {log_file_path}"))?;

        info!("New log file created: {log_file_path}");

        Ok(())
    }

    /// 检查并在需要时执行日志轮转
    pub fn check_and_rotate(&self, log_file_path: &str) -> Result<bool> {
        if self.should_rotate(log_file_path)? {
            self.rotate_log(log_file_path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 启动后台日志监控线程
    pub fn start_background_monitor(&self) -> Result<LogRotationMonitor> {
        if self.monitor_running.load(Ordering::Relaxed) {
            return Err(anyhow::anyhow!("Background monitor is already running"));
        }

        self.monitor_running.store(true, Ordering::Relaxed);
        let running_flag = Arc::clone(&self.monitor_running);
        let monitor_interval = self.monitor_interval;
        let max_size_bytes = self.max_size_bytes;
        let rotation_threshold = self.rotation_threshold;

        let join_handle = thread::Builder::new()
            .name("LogRotationMonitor".to_string())
            .spawn(move || {
                info!("Log rotation background monitor started");
                debug!(
                    "Monitor interval: {:?}, max_size: {}MB, threshold: {}%",
                    monitor_interval,
                    max_size_bytes / 1024 / 1024,
                    (rotation_threshold * 100.0) as u8
                );

                while running_flag.load(Ordering::Relaxed) {
                    // 使用临时的管理器实例来执行检查
                    let temp_manager = LogRotationManager {
                        max_size_bytes,
                        rotation_threshold,
                        monitor_running: Arc::new(AtomicBool::new(false)), // 临时的，不使用
                        monitor_interval,
                    };

                    match temp_manager.check_and_rotate(LOG_PATH) {
                        Ok(rotated) => {
                            if rotated {
                                info!("Background monitor: Log file rotated successfully");
                            } else {
                                debug!("Background monitor: Log file size within limits");
                            }
                        }
                        Err(e) => {
                            warn!("Background monitor: Failed to check/rotate log file: {}", e);
                        }
                    }

                    // 等待下一次检查，但要响应停止信号
                    let sleep_duration = Duration::from_millis(1000); // 1秒为单位检查停止信号
                    let total_iterations = monitor_interval.as_secs();

                    for _ in 0..total_iterations {
                        if !running_flag.load(Ordering::Relaxed) {
                            break;
                        }
                        thread::sleep(sleep_duration);
                    }
                }

                info!("Log rotation background monitor stopped");
            })?;

        Ok(LogRotationMonitor {
            running: Arc::clone(&self.monitor_running),
            join_handle: Some(join_handle),
        })
    }

    /// 获取最大日志文件大小（字节）
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_bytes
    }

    /// 获取日志轮转阈值（0.0-1.0）
    pub fn rotation_threshold(&self) -> f64 {
        self.rotation_threshold
    }
}

impl LogRotationMonitor {
    /// 停止后台监控
    pub fn stop(&mut self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            info!("Stopping log rotation background monitor...");
            self.running.store(false, Ordering::Relaxed);

            if let Some(handle) = self.join_handle.take() {
                handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("Failed to join monitor thread"))?;
            }
            info!("Log rotation background monitor stopped");
        }
        Ok(())
    }
}

impl Drop for LogRotationMonitor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// 全局日志轮转管理器实例
static LOG_ROTATION_MANAGER: once_cell::sync::Lazy<LogRotationManager> =
    once_cell::sync::Lazy::new(LogRotationManager::default);

/// 检查并轮转主日志文件
pub fn check_and_rotate_main_log() -> Result<bool> {
    LOG_ROTATION_MANAGER.check_and_rotate(LOG_PATH)
}

/// 启动主日志文件的后台监控
pub fn start_main_log_monitor() -> Result<LogRotationMonitor> {
    LOG_ROTATION_MANAGER.start_background_monitor()
}
