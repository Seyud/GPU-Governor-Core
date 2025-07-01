use anyhow::Result;
use inotify::WatchMask;
use log::{debug, info, warn};

use crate::{
    datasource::file_path::LOG_LEVEL_PATH,
    utils::{file_operate::check_read_simple, inotify::InotifyWatcher, logger::update_log_level},
};

pub fn monitor_log_level() -> Result<()> {
    // 设置线程名称
    info!("LogLevelMonitor Start");

    // 检查日志等级文件路径
    if !check_read_simple(LOG_LEVEL_PATH) {
        // 如果文件不存在，记录日志
        info!("Log level file does not exist: {LOG_LEVEL_PATH}");
    } else {
        info!("Using log level path: {LOG_LEVEL_PATH}");
    }

    // 设置文件监控
    let mut inotify = InotifyWatcher::new()?;
    inotify.add(LOG_LEVEL_PATH, WatchMask::CLOSE_WRITE | WatchMask::MODIFY)?;

    // 主循环
    loop {
        inotify.wait_and_handle()?;

        // 检查文件是否存在
        if !check_read_simple(LOG_LEVEL_PATH) {
            debug!("Log level file no longer exists");
            continue;
        }

        // 更新日志等级
        match update_log_level() {
            Ok(_) => debug!("Log level updated successfully"),
            Err(e) => warn!("Failed to update log level: {e}"),
        }
    }
}
