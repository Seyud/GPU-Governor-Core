use anyhow::{Context, Result};
use chrono::Local;
use log::{debug, info};
use std::fs;
use std::path::Path;

use crate::datasource::file_path::LOG_PATH;

/// 日志轮转管理器
pub struct LogRotationManager {
    max_size_bytes: u64,
    rotation_threshold: f64,
}

impl LogRotationManager {
    /// 创建新的日志轮转管理器
    ///
    /// # Arguments
    /// * `max_size_mb` - 最大日志文件大小（MB）
    /// * `rotation_threshold` - 轮转阈值（0.0-1.0），默认0.8表示80%
    pub fn new(max_size_mb: u64, rotation_threshold: Option<f64>) -> Self {
        Self {
            max_size_bytes: max_size_mb * 1024 * 1024,
            rotation_threshold: rotation_threshold.unwrap_or(0.8),
        }
    }

    /// 创建默认的日志轮转管理器（10MB，80%阈值）
    pub fn default() -> Self {
        Self::new(10, Some(0.8))
    }

    /// 检查是否需要轮转日志
    pub fn should_rotate(&self, log_file_path: &str) -> Result<bool> {
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

    /// 获取最大日志文件大小（字节）
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_bytes
    }

    /// 获取日志轮转阈值（0.0-1.0）
    pub fn rotation_threshold(&self) -> f64 {
        self.rotation_threshold
    }
}

/// 全局日志轮转管理器实例
static LOG_ROTATION_MANAGER: once_cell::sync::Lazy<LogRotationManager> =
    once_cell::sync::Lazy::new(LogRotationManager::default);

/// 检查并轮转主日志文件
pub fn check_and_rotate_main_log() -> Result<bool> {
    LOG_ROTATION_MANAGER.check_and_rotate(LOG_PATH)
}
