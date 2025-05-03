use std::path::Path;

use anyhow::{Context, Result};
use chrono::Local;
use log::{Level, LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;

use crate::datasource::file_path::LOG_LEVEL_PATH;

// Custom logger implementation - 只输出到控制台
struct CustomLogger;

impl log::Log for CustomLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let now = Local::now();
            let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
            let level_str = record.level().to_string();
            let log_message = format!("[{}][{}]: {}\n", timestamp, level_str, record.args());

            // 只输出到控制台
            print!("{}", log_message);
        }
    }

    fn flush(&self) {
        // 无需刷新文件
    }
}

// Global logger instance
static LOGGER: Lazy<CustomLogger> = Lazy::new(|| CustomLogger);

pub fn init_logger() -> Result<()> {
    // 读取日志等级配置
    let log_level = read_log_level_config()?;

    // 设置日志记录器
    log::set_logger(&*LOGGER)
        .map(|()| log::set_max_level(log_level))
        .with_context(|| "Failed to set logger")?;

    // 记录当前使用的日志等级
    log::info!("Logger initialized with level: {}", log_level);
    log::info!("Console output only mode");

    Ok(())
}

// 读取日志等级配置文件
fn read_log_level_config() -> Result<LevelFilter> {
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
