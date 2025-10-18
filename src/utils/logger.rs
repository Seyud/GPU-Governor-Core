use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    sync::Mutex,
};

use anyhow::{Context, Result};
use chrono::Local;
use log::{LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;

use crate::{
    datasource::file_path::{LOG_LEVEL_PATH, LOG_PATH},
    utils::log_level_manager::LogLevelManager,
    utils::log_rotation::{LogRotationManager, check_and_rotate_main_log, start_main_log_monitor},
};

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

    fn reset_writer(&self) -> Result<()> {
        let mut writer = self.file_writer.lock().unwrap();
        if let Some(ref mut buf_writer) = *writer {
            buf_writer
                .flush()
                .with_context(|| "Failed to flush log file during reset")?;
        }
        *writer = None;
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
        let log_message = format!("[{}] [{}]: {}\n", timestamp, level_str, record.args());

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

pub fn reset_log_file_writer() -> Result<()> {
    LOGGER.reset_writer()
}

pub fn init_logger() -> Result<()> {
    // 启动时清空日志文件，保证每次启动都是新日志
    let _ = File::create(LOG_PATH)?;
    // 读取日志等级配置
    let log_level = LogLevelManager::read_log_level_config()?;

    // 设置日志记录器
    log::set_logger(&*LOGGER)
        .map(|()| log::set_max_level(log_level))
        .map_err(|e| anyhow::anyhow!("Failed to set logger: {e:?}"))?;

    // 记录当前使用的日志等级
    log::info!("Logger initialized with level: {log_level}");

    // 获取当前日志等级
    let current_level = crate::utils::log_level_manager::get_current_log_level();
    log::info!("Current log level from manager: {current_level}");
    log::info!("Log file path: {LOG_PATH}");
    log::info!("Log level config path: {LOG_LEVEL_PATH}");

    // 初始化日志轮转管理器
    let rotation_manager = LogRotationManager::default();
    log::info!(
        "Max log file size: {}MB",
        rotation_manager.max_size_bytes() / 1024 / 1024
    );
    log::info!(
        "Log rotation threshold: {}%",
        (rotation_manager.rotation_threshold() * 100.0) as u8
    );

    // 检查并执行日志轮转（仅在debug等级时）
    if log_level == LevelFilter::Debug {
        if let Err(e) = check_and_rotate_main_log() {
            log::warn!("Failed to check/rotate main log file: {}", e);
        }

        // 启动后台日志监控（仅在debug等级时）
        if let Err(e) = start_main_log_monitor() {
            log::warn!("Failed to start main log monitor: {}", e);
        }
    }

    // 在debug级别记录一条消息，说明某些错误只会在debug级别显示
    log::debug!("Some error messages will only be shown at debug level");

    Ok(())
}
