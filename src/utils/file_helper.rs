use log::debug;
use std::{fs::OpenOptions, io::Write, path::Path};

/// 改进的文件操作辅助工具
/// 提供统一的文件读写接口，减少重复代码
/// 文件操作辅助结构
pub struct FileHelper;

impl FileHelper {
    /// 尝试写入文件，失败时只记录调试信息，不终止程序
    pub fn write_string_safe<P: AsRef<Path>>(path: P, content: &str) -> bool {
        let path = path.as_ref();
        match OpenOptions::new().write(true).open(path) {
            Ok(mut file) => match file.write_all(content.as_bytes()) {
                Ok(_) => true,
                Err(e) => {
                    debug!(
                        "Failed to write file: {} - Error: {} (continuing execution)",
                        path.display(),
                        e
                    );
                    false
                }
            },
            Err(e) => {
                debug!(
                    "Failed to open file: {} - Error: {} (continuing execution)",
                    path.display(),
                    e
                );
                false
            }
        }
    }
}
