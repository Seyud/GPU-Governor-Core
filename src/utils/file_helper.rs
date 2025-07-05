use anyhow::{Context, Result};
use log::debug;
/// 改进的文件操作辅助工具
/// 提供统一的文件读写接口，减少重复代码
use std::fs;
use std::path::Path;

/// 文件操作辅助结构
pub struct FileHelper;

impl FileHelper {
    /// 安全地写入文件内容
    pub fn write_string<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        let path = path.as_ref();
        fs::write(path, content)
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    /// 尝试写入文件，失败时只记录调试信息，不终止程序
    pub fn write_string_safe<P: AsRef<Path>>(path: P, content: &str) -> bool {
        let path = path.as_ref();
        match fs::write(path, content) {
            Ok(_) => true,
            Err(e) => {
                debug!(
                    "Failed to write file: {} - Error: {} (continuing execution)",
                    path.display(),
                    e
                );
                false
            }
        }
    }
}
