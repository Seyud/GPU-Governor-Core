use anyhow::{Context, Result};
use log::{debug, warn};
/// 改进的文件操作辅助工具
/// 提供统一的文件读写接口，减少重复代码
use std::fs;
use std::path::Path;

/// 文件操作辅助结构
pub struct FileHelper;

impl FileHelper {
    /// 安全地读取文件内容
    pub fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
        let path = path.as_ref();
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))
    }

    /// 安全地写入文件内容
    pub fn write_string<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
        let path = path.as_ref();
        fs::write(path, content)
            .with_context(|| format!("Failed to write file: {}", path.display()))
    }

    /// 检查文件是否可读
    pub fn is_readable<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();
        path.exists() && path.is_file()
    }

    /// 检查文件是否可写
    pub fn is_writable<P: AsRef<Path>>(path: P) -> bool {
        let path = path.as_ref();
        if path.exists() {
            // 检查是否有写权限
            fs::OpenOptions::new().write(true).open(path).is_ok()
        } else {
            // 检查父目录是否存在且可写
            if let Some(parent) = path.parent() {
                parent.exists() && parent.is_dir()
            } else {
                false
            }
        }
    }

    /// 尝试从多个路径中读取第一个可用的文件
    pub fn read_from_any<P: AsRef<Path>>(paths: &[P]) -> Result<(String, String)> {
        for path in paths {
            let path_ref = path.as_ref();
            if Self::is_readable(path_ref) {
                match Self::read_to_string(path_ref) {
                    Ok(content) => return Ok((content, path_ref.display().to_string())),
                    Err(e) => {
                        debug!("Failed to read {}: {}", path_ref.display(), e);
                        continue;
                    }
                }
            }
        }
        anyhow::bail!("No readable file found in provided paths")
    }

    /// 尝试向多个路径中写入，直到成功
    pub fn write_to_any<P: AsRef<Path>>(paths: &[P], content: &str) -> Result<String> {
        for path in paths {
            let path_ref = path.as_ref();
            if Self::is_writable(path_ref) {
                match Self::write_string(path_ref, content) {
                    Ok(()) => return Ok(path_ref.display().to_string()),
                    Err(e) => {
                        debug!("Failed to write to {}: {}", path_ref.display(), e);
                        continue;
                    }
                }
            }
        }
        anyhow::bail!("No writable file found in provided paths")
    }

    /// 带重试的文件写入
    pub fn write_with_retry<P: AsRef<Path>>(
        path: P,
        content: &str,
        max_retries: u32,
    ) -> Result<()> {
        let path = path.as_ref();

        for attempt in 1..=max_retries {
            match Self::write_string(path, content) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt == max_retries {
                        return Err(e);
                    }
                    warn!(
                        "Write attempt {} failed for {}: {}",
                        attempt,
                        path.display(),
                        e
                    );
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }

        anyhow::bail!(
            "Failed to write to {} after {} attempts",
            path.display(),
            max_retries
        )
    }

    /// 解析整数值从文件内容
    pub fn parse_int_from_file<P: AsRef<Path>>(path: P) -> Result<i64> {
        let content = Self::read_to_string(path)?;
        content
            .trim()
            .parse::<i64>()
            .with_context(|| "Failed to parse integer from file content")
    }

    /// 解析整数值从多个可能的文件中
    pub fn parse_int_from_any<P: AsRef<Path>>(paths: &[P]) -> Result<(i64, String)> {
        let (content, path) = Self::read_from_any(paths)?;
        let value = content
            .trim()
            .parse::<i64>()
            .with_context(|| "Failed to parse integer from file content")?;
        Ok((value, path))
    }
}

// 便利函数，保持向后兼容
pub fn write_file<P: AsRef<Path>>(path: P, content: &str) -> Result<()> {
    FileHelper::write_string(path, content)
}

pub fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
    FileHelper::read_to_string(path)
}

pub fn is_readable<P: AsRef<Path>>(path: P) -> bool {
    FileHelper::is_readable(path)
}
