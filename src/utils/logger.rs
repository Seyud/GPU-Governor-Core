use std::path::Path;

use anyhow::{Context, Result};
use chrono::Local;
use log::{LevelFilter, Metadata, Record};
use once_cell::sync::Lazy;

use crate::datasource::file_path::{LOG_LEVEL_PATH, LOG_PATH};

// Custom logger implementation - 只输出到控制台
struct CustomLogger;

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

        // 只输出到控制台
        print!("{}", log_message);
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
    log::info!("Log file path (not used): {}", LOG_PATH);
    log::info!("Log level config path: {}", LOG_LEVEL_PATH);

    // 在debug级别记录一条消息，说明某些错误只会在debug级别显示
    log::debug!("Some error messages (like writing to /proc/gpufreqv2/fix_target_opp_index) will only be shown at debug level");

    Ok(())
}

// 读取日志等级配置文件
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

// 更新日志等级
pub fn update_log_level() -> Result<()> {
    // 读取新的日志等级
    let new_level = read_log_level_config()?;

    // 更新全局日志等级
    log::set_max_level(new_level);

    // 记录日志等级变更
    log::info!("Log level updated to: {}", new_level);

    Ok(())
}
