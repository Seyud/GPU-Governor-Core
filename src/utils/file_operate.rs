use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
};

use anyhow::{Context, Result};
use log::{debug, error};

use crate::{
    datasource::file_path::{GPUFREQV2_OPP, GPUFREQ_OPP},
    utils::file_status::write_status,
};

pub fn check_read<P: AsRef<Path>>(path: P, status: &mut bool) -> String {
    let path_ref = path.as_ref();
    if path_ref.exists() && path_ref.is_file() {
        *status = true;
        write_status(path_ref.to_str().unwrap_or(""), true);
        "OK".to_string()
    } else {
        write_status(path_ref.to_str().unwrap_or(""), false);
        format!("Failed: {}", std::io::Error::last_os_error())
    }
}

pub fn check_read_simple<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists() && path.as_ref().is_file()
}

pub fn read_file<P: AsRef<Path>>(path: P, max_len: usize) -> Result<String> {
    let path_ref = path.as_ref();
    let mut file = File::open(path_ref)
        .with_context(|| format!("Failed to open file for reading: {}", path_ref.display()))?;

    let mut content = String::with_capacity(max_len);
    let bytes_read = file
        .read_to_string(&mut content)
        .with_context(|| format!("Failed to read from file: {}", path_ref.display()))?;

    content.truncate(bytes_read);
    Ok(content)
}

pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    content: C,
    max_len: usize,
) -> Result<usize> {
    let path_ref = path.as_ref();

    // 设置文件权限为可写
    if path_ref.exists() {
        let metadata = path_ref
            .metadata()
            .with_context(|| format!("Failed to get metadata for: {}", path_ref.display()))?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(path_ref, perms)
            .with_context(|| format!("Failed to set permissions for: {}", path_ref.display()))?;
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path_ref)
        .with_context(|| format!("Failed to open file for writing: {}", path_ref.display()))?;

    let content_ref = content.as_ref();
    let len = std::cmp::min(content_ref.len(), max_len);
    let bytes_written = match file.write(&content_ref[..len]) {
        Ok(n) => n,
        Err(e) => {
            // 检查是否是特定文件路径，如果是则使用debug级别记录错误
            let path_str = path_ref.to_str().unwrap_or("");
            if path_str == GPUFREQV2_OPP || path_str == GPUFREQ_OPP {
                debug!(
                    "Failed to write to file: {}. Error: {}",
                    path_ref.display(),
                    e
                );
            } else {
                error!(
                    "Failed to write to file: {}. Error: {}",
                    path_ref.display(),
                    e
                );
            }
            return Err(anyhow::anyhow!(""));
        }
    };

    // 设置文件权限为只读
    let metadata = path_ref
        .metadata()
        .with_context(|| format!("Failed to get metadata for: {}", path_ref.display()))?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o444);
    std::fs::set_permissions(path_ref, perms)
        .with_context(|| format!("Failed to set permissions for: {}", path_ref.display()))?;

    Ok(bytes_written)
}

/// 安全地写入文件，如果文件不存在则记录错误但不中断程序
pub fn write_file_safe<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    content: C,
    max_len: usize,
) -> Result<usize> {
    let path_ref = path.as_ref();

    // 检查文件是否存在
    if !path_ref.exists() {
        // 文件不存在，记录错误但不中断程序
        debug!(
            "File does not exist, skipping writing: {}",
            path_ref.display()
        );
        return Ok(0);
    }

    // 检查父目录是否存在
    if let Some(parent) = path_ref.parent() {
        if !parent.exists() {
            debug!(
                "Parent directory does not exist, skipping writing.: {}",
                parent.display()
            );
            return Ok(0);
        }
    }

    // 尝试写入文件
    match write_file(path_ref, content, max_len) {
        Ok(bytes) => Ok(bytes),
        Err(e) => {
            // 检查是否是特定文件路径，如果是则使用debug级别记录错误
            let path_str = path_ref.to_str().unwrap_or("");
            if path_str == GPUFREQV2_OPP || path_str == GPUFREQ_OPP {
                // 对于特定文件路径，使用debug级别记录错误
                debug!(
                    "Failed to write to file: {}. Error: {}",
                    path_ref.display(),
                    e
                );
            } else {
                // 对于其他文件路径，使用error级别记录错误
                error!(
                    "Failed to write to file, but continued execution: {}-Error: {}",
                    path_ref.display(),
                    e
                );
            }
            Ok(0)
        }
    }
}
