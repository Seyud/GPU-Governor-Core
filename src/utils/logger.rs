use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::Local;
use log::{LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;

use crate::{
    datasource::file_path::{LOG_LEVEL_PATH, LOG_PATH},
    utils::log_level_manager::{get_current_log_level, LogLevelManager},
};

// 日志轮转配置常量
const MAX_LOG_SIZE_BYTES: u64 = 10 * 1024 * 1024; // 10MB
const LOG_ROTATION_THRESHOLD: f64 = 0.8; // 80%阈值触发轮转

// 自定义日志实现 - 支持文件写入和轮转
struct CustomLogger {
    file_writer: Mutex<Option<BufWriter<File>>>,
}

impl CustomLogger {
    fn new() -> Self {
        Self {
            file_writer: Mutex::new(None),
        }
    }

    fn ensure_log_file(&self) -> Result<()> {
        let mut writer = self.file_writer.lock().unwrap();

        if writer.is_none() {
            // 只在debug等级时检查并执行日志轮转
            let current_level = get_current_log_level();
            if current_level == LevelFilter::Debug {
                self.check_and_rotate_log()?;
            }

            // 创建或打开日志文件
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(LOG_PATH)
                .with_context(|| format!("Failed to open log file: {LOG_PATH}"))?;

            *writer = Some(BufWriter::new(file));
        }

        Ok(())
    }

    fn check_and_rotate_log(&self) -> Result<()> {
        let log_path = Path::new(LOG_PATH);

        // 如果日志文件不存在，无需轮转
        if !log_path.exists() {
            return Ok(());
        }

        // 获取文件大小
        let metadata = log_path
            .metadata()
            .with_context(|| format!("Failed to get metadata for log file: {LOG_PATH}"))?;

        let file_size = metadata.len();
        let threshold_size = (MAX_LOG_SIZE_BYTES as f64 * LOG_ROTATION_THRESHOLD) as u64;

        // 如果文件大小超过阈值，执行轮转
        if file_size > threshold_size {
            self.rotate_log_file()?;
        }

        Ok(())
    }

    fn rotate_log_file(&self) -> Result<()> {
        let log_path = Path::new(LOG_PATH);
        let backup_path = format!("{LOG_PATH}.bak");

        // 如果备份文件已存在，删除它
        if Path::new(&backup_path).exists() {
            std::fs::remove_file(&backup_path)
                .with_context(|| format!("Failed to remove old backup file: {backup_path}"))?;
        }

        // 将当前日志文件重命名为备份文件
        std::fs::rename(log_path, &backup_path)
            .with_context(|| format!("Failed to rename log file to backup: {backup_path}"))?;

        // 创建新的日志文件并写入轮转信息
        let mut new_file = File::create(log_path)
            .with_context(|| format!("Failed to create new log file: {LOG_PATH}"))?;

        let rotation_msg = format!(
            "{} - Log rotated, previous log backed up to {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            backup_path
        );
        new_file
            .write_all(rotation_msg.as_bytes())
            .with_context(|| "Failed to write rotation message to new log file")?;

        new_file
            .flush()
            .with_context(|| "Failed to flush new log file")?;

        Ok(())
    }

    fn write_to_file(&self, message: &str) -> Result<()> {
        // 确保日志文件存在并检查轮转
        self.ensure_log_file()?;

        let mut writer = self.file_writer.lock().unwrap();
        if let Some(ref mut buf_writer) = *writer {
            buf_writer
                .write_all(message.as_bytes())
                .with_context(|| "Failed to write to log file")?;
            buf_writer
                .flush()
                .with_context(|| "Failed to flush log file")?;
        }

        Ok(())
    }
}

impl log::Log for CustomLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        // 这个方法只检查日志级别是否被启用
        // 实际的过滤由log库根据设置的max_level完成
        true
    }

    fn log(&self, record: &Record) {
        // 这里不需要再次检查enabled，因为log库已经根据max_level过滤了
        let now = Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let level_str = record.level().to_string();
        let log_message = format!("[{}][{}]: {}\n", timestamp, level_str, record.args());

        // 只写入到文件（忽略错误以避免程序崩溃）
        if let Err(e) = self.write_to_file(&log_message) {
            // 如果文件写入失败，仍然输出到stderr以便调试
            eprintln!("Warning: Failed to write to log file: {e}");
        }
    }

    fn flush(&self) {
        let mut writer = self.file_writer.lock().unwrap();
        if let Some(ref mut buf_writer) = *writer {
            let _ = buf_writer.flush();
        }
    }
}

// 全局日志实例
static LOGGER: Lazy<CustomLogger> = Lazy::new(CustomLogger::new);

pub fn init_logger() -> Result<()> {
    // 读取日志等级配置
    let log_level = LogLevelManager::read_log_level_config()?;

    // 设置日志记录器
    log::set_logger(&*LOGGER)
        .map(|()| log::set_max_level(log_level))
        .map_err(|e| anyhow::anyhow!("Failed to set logger: {e:?}"))?;

    // 记录当前使用的日志等级
    log::info!("Logger initialized with level: {log_level}");
    log::info!("Log file path: {LOG_PATH}");
    log::info!("Log level config path: {LOG_LEVEL_PATH}");
    log::info!("Max log file size: {}MB", MAX_LOG_SIZE_BYTES / 1024 / 1024);
    log::info!(
        "Log rotation threshold: {}%",
        (LOG_ROTATION_THRESHOLD * 100.0) as u8
    );

    // 在debug级别记录一条消息，说明某些错误只会在debug级别显示
    log::debug!("Some error messages will only be shown at debug level");

    Ok(())
}
