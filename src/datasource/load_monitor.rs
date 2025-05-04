use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};

use crate::{
    datasource::file_path::*,
    utils::{
        file_operate::{check_read, read_file},
        file_status::{get_status, write_status},
    },
};

fn module_ged_load() -> Result<i32> {
    if !get_status(MODULE_LOAD) {
        return Ok(-1);
    }

    let buf = read_file(MODULE_LOAD, 32)?;
    let load = buf
        .trim()
        .parse::<i32>()
        .with_context(|| format!("Failed to parse GPU load from {}", MODULE_LOAD))?;

    Ok(load)
}

fn module_ged_idle() -> Result<i32> {
    if !get_status(MODULE_IDLE) {
        return module_ged_load();
    }

    let buf = read_file(MODULE_IDLE, 32)?;
    let idle = buf
        .trim()
        .parse::<i32>()
        .with_context(|| format!("Failed to parse GPU idle from {}", MODULE_IDLE))?;

    debug!("module {}", 100 - idle);
    Ok(100 - idle)
}

fn kernel_ged_load() -> Result<i32> {
    if !get_status(KERNEL_LOAD) {
        return module_ged_idle();
    }

    let buf = read_file(KERNEL_LOAD, 32)?;
    let parts: Vec<&str> = buf.split_whitespace().collect();

    if parts.len() >= 3 {
        if let Ok(idle) = parts[2].parse::<i32>() {
            debug!("gedload {}", 100 - idle);
            return Ok(if 100 - idle == 0 {
                module_ged_load()?
            } else {
                100 - idle
            });
        }
    }

    module_ged_idle()
}

fn kernel_debug_ged_load() -> Result<i32> {
    if !get_status(KERNEL_D_LOAD) {
        return kernel_ged_load();
    }

    let buf = read_file(KERNEL_D_LOAD, 32)?;
    let parts: Vec<&str> = buf.split_whitespace().collect();

    if parts.len() >= 3 {
        if let Ok(idle) = parts[2].parse::<i32>() {
            debug!("dbggedload {}", 100 - idle);
            return Ok(if 100 - idle == 0 {
                kernel_ged_load()?
            } else {
                100 - idle
            });
        }
    }

    kernel_ged_load()
}

fn kernel_d_ged_load() -> Result<i32> {
    if !get_status(KERNEL_DEBUG_LOAD) {
        return kernel_debug_ged_load();
    }

    let buf = read_file(KERNEL_DEBUG_LOAD, 32)?;
    let parts: Vec<&str> = buf.split_whitespace().collect();

    if parts.len() >= 3 {
        if let Ok(idle) = parts[2].parse::<i32>() {
            debug!("dgedload {}", 100 - idle);
            return Ok(if 100 - idle == 0 {
                kernel_debug_ged_load()?
            } else {
                100 - idle
            });
        }
    }

    kernel_debug_ged_load()
}

fn mali_load() -> Result<i32> {
    if !get_status(PROC_MALI_LOAD) {
        return kernel_d_ged_load();
    }

    let buf = read_file(PROC_MALI_LOAD, 256)?;

    // Parse "gpu/cljs0/cljs1=XX" format
    if let Some(pos) = buf.find('=') {
        if let Ok(load) = buf[pos + 1..].trim().parse::<i32>() {
            debug!("mali {}", load);
            return Ok(if load == 0 {
                kernel_d_ged_load()?
            } else {
                load
            });
        }
    }

    kernel_d_ged_load()
}

fn mtk_load() -> Result<i32> {
    if !get_status(PROC_MTK_LOAD) {
        return mali_load();
    }

    let buf = read_file(PROC_MTK_LOAD, 256)?;

    // Parse "ACTIVE=XX" format
    if let Some(pos) = buf.find("ACTIVE=") {
        if let Ok(load) = buf[pos + 7..].trim().parse::<i32>() {
            debug!("mtk_mali {}", load);
            return Ok(if load == 0 { mali_load()? } else { load });
        }
    }

    mali_load()
}

fn gpufreq_load() -> Result<i32> {
    if !get_status(GPU_FREQ_LOAD_PATH) {
        return mtk_load();
    }

    let file = match File::open(GPU_FREQ_LOAD_PATH) {
        Ok(file) => file,
        Err(_) => {
            write_status(GPU_FREQ_LOAD_PATH, false);
            return Ok(0);
        }
    };

    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;

        // Parse "gpu_loading = XX" format
        if let Some(pos) = line.find("gpu_loading = ") {
            if let Ok(load) = line[pos + 14..].trim().parse::<i32>() {
                debug!("gpufreq {}", load);
                return Ok(if load == 0 { mtk_load()? } else { load });
            }
        }
    }

    mtk_load()
}

fn debug_dvfs_load_func() -> Result<i32> {
    // Check if debug_dvfs_load or debug_dvfs_load_old exists
    let path = if get_status(DEBUG_DVFS_LOAD) {
        DEBUG_DVFS_LOAD
    } else if get_status(DEBUG_DVFS_LOAD_OLD) {
        DEBUG_DVFS_LOAD_OLD
    } else {
        return gpufreq_load();
    };

    let buf = read_file(path, 256)?;
    let lines: Vec<&str> = buf.lines().collect();

    if lines.len() < 2 {
        return gpufreq_load();
    }

    // Static variables to keep track of previous values
    static mut PREV_BUSY: i64 = 0;
    static mut PREV_IDLE: i64 = 0;
    static mut PREV_PROTM: i64 = 0;

    // Parse the second line which contains the values
    let parts: Vec<&str> = lines[1].split_whitespace().collect();

    if parts.len() >= 3 {
        if let (Ok(busy), Ok(idle), Ok(protm)) = (
            parts[0].parse::<i64>(),
            parts[1].parse::<i64>(),
            parts[2].parse::<i64>(),
        ) {
            // Get previous values safely
            let (prev_busy, prev_idle, prev_protm) = unsafe { (PREV_BUSY, PREV_IDLE, PREV_PROTM) };

            // Calculate differences
            let diff_busy = busy - prev_busy;
            let diff_idle = idle - prev_idle;
            let diff_protm = protm - prev_protm;

            // Update previous values
            unsafe {
                PREV_BUSY = busy;
                PREV_IDLE = idle;
                PREV_PROTM = protm;
            }

            // Calculate load percentage
            let total = diff_busy + diff_idle + diff_protm;
            if total > 0 {
                let load = ((diff_busy + diff_protm) * 100 / total) as i32;
                let load = if load < 0 { 0 } else { load };

                debug!(
                    "debugutil: {} {} {} {}",
                    load, diff_busy, diff_idle, diff_protm
                );
                return Ok(if load == 0 { mtk_load()? } else { load });
            }
        }
    }

    gpufreq_load()
}

pub fn get_gpu_load() -> Result<i32> {
    debug_dvfs_load_func()
}

pub fn get_gpu_current_freq() -> Result<i64> {
    if !get_status(GPU_CURRENT_FREQ_PATH) {
        debug!("GPU current frequency path not available: {}", GPU_CURRENT_FREQ_PATH);
        return Ok(0);
    }

    let buf = read_file(GPU_CURRENT_FREQ_PATH, 64)?;
    let parts: Vec<&str> = buf.split_whitespace().collect();

    // 读取第二个整数作为当前频率
    if parts.len() >= 2 {
        if let Ok(freq) = parts[1].parse::<i64>() {
            debug!("Current GPU frequency: {}", freq);
            return Ok(freq);
        } else {
            debug!("Failed to parse second value as frequency from: {}", buf);
        }
    } else {
        debug!("Not enough values in GPU frequency file, content: {}", buf);
    }

    // 如果无法读取或解析，返回0
    Ok(0)
}

pub fn utilization_init() -> Result<()> {
    let mut is_good = false;
    info!("Init LoadMonitor");
    info!("Testing GED...");

    // Method 1: Read From /sys/module/ged
    info!("{}: {}", MODULE_LOAD, check_read(MODULE_LOAD, &mut is_good));
    info!("{}: {}", MODULE_IDLE, check_read(MODULE_IDLE, &mut is_good));

    // Method 2: Read From /sys/kernel/ged
    info!("{}: {}", KERNEL_LOAD, check_read(KERNEL_LOAD, &mut is_good));

    // Method 3: Read From /sys/kernel/debug/ged
    info!(
        "{}: {}",
        KERNEL_DEBUG_LOAD,
        check_read(KERNEL_DEBUG_LOAD, &mut is_good)
    );
    info!(
        "{}: {}",
        KERNEL_D_LOAD,
        check_read(KERNEL_D_LOAD, &mut is_good)
    );

    // Method 4: Read From /proc/gpufreq
    info!("Testing gpufreq Driver...");
    info!(
        "{}: {}",
        GPU_FREQ_LOAD_PATH,
        check_read(GPU_FREQ_LOAD_PATH, &mut is_good)
    );

    // Method 5: Read From Mali Driver
    info!("Testing mali driver ...");
    info!(
        "{}: {}",
        PROC_MTK_LOAD,
        check_read(PROC_MTK_LOAD, &mut is_good)
    );
    info!(
        "{}: {}",
        PROC_MALI_LOAD,
        check_read(PROC_MALI_LOAD, &mut is_good)
    );

    // Method 6: Read precise load from Mali Driver
    info!(
        "{}: {}",
        DEBUG_DVFS_LOAD,
        check_read(DEBUG_DVFS_LOAD, &mut is_good)
    );
    info!(
        "{}: {}",
        DEBUG_DVFS_LOAD_OLD,
        check_read(DEBUG_DVFS_LOAD_OLD, &mut is_good)
    );

    // Determine if it's OK
    if !is_good {
        error!("Can't Monitor GPU Loading!");
        return Err(anyhow!("Can't Monitor GPU Loading!"));
    }

    info!("Test Finished.");
    Ok(())
}
