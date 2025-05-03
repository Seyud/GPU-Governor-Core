/*
   Copyright (c) 2022 Ham Jin
   loadmonitor_gpuv2 is licensed under Mulan PSL v2.
   You can use this software according to the terms and conditions of the Mulan PSL v2.
   You may obtain a copy of Mulan PSL v2 at:
               http://license.coscl.org.cn/MulanPSL2
   THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT, MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
   See the Mulan PSL v2 for more details.
 */
/**
 * Mediatek Mali GPU Load-Based Frequency Adjustment
 * File:           file_path.rs
 * Original Author: HamJin @CoolApk
 * Rust Port:       Seyud
 * Create Date:     2025/04/30
 * Feature:         File paths constants
*/

// Thread names
#[allow(dead_code)]
pub const MAIN_THREAD: &str = "LoadMonitor";
pub const GAME_THREAD: &str = "GameModeWatcher";
pub const CONF_THREAD: &str = "ConfigWatcher";
#[allow(dead_code)]
pub const GED_LOCKER: &str = "GedLocker";
pub const FOREGROUND_APP_THREAD: &str = "ForegroundAppWatcher";

// File paths
#[allow(dead_code)]
pub const GAME_MODE_PATH: &str = "/dev/asopt_game";
pub const GPU_GOVERNOR_GAME_MODE_PATH: &str = "/data/adb/gpu_governor/game_mode";
pub const MODULE_LOAD: &str = "/sys/module/ged/parameters/gpu_loading";
pub const MODULE_IDLE: &str = "/sys/module/ged/parameters/gpu_idle";
pub const KERNEL_LOAD: &str = "/sys/kernel/ged/hal/gpu_utilization";
pub const KERNEL_DEBUG_LOAD: &str = "/sys/kernel/d/ged/hal/gpu_utilization";
#[allow(dead_code)]
pub const GPU_CURRENT_FREQ_PATH: &str = "/sys/kernel/ged/hal/current_freqency";
pub const KERNEL_D_LOAD: &str = "/sys/kernel/debug/ged/hal/gpu_utilization";
pub const GPU_FREQ_LOAD_PATH: &str = "/proc/gpufreq/gpufreq_var_dump";
pub const PROC_MALI_LOAD: &str = "/proc/mali/utilization";
pub const PROC_MTK_LOAD: &str = "/proc/mtk_mali/utilization";
pub const DEBUG_DVFS_LOAD: &str = "/sys/kernel/debug/mali0/dvfs_utilization";
pub const DEBUG_DVFS_LOAD_OLD: &str = "/proc/mali/dvfs_utilization";
#[allow(dead_code)]
pub const GPUFREQ_TABLE: &str = "/proc/gpufreq/gpufreq_opp_dump";
#[allow(dead_code)]
pub const GPUFREQV2_TABLE: &str = "/proc/gpufreqv2/stack_working_opp_table";
#[allow(dead_code)]
pub const GEDFREQ_MAX: &str = "/sys/kernel/ged/hal/custom_upbound_gpu_freq";
#[allow(dead_code)]
pub const GEDFREQ_MIN: &str = "/sys/kernel/ged/hal/custom_boost_gpu_freq";
pub const GPUFREQ_OPP: &str = "/proc/gpufreq/gpufreq_opp_freq";
pub const GPUFREQV2_OPP: &str = "/proc/gpufreqv2/fix_target_opp_index";
pub const GPUFREQ_VOLT: &str = "/proc/gpufreq/gpufreq_fixed_freq_volt";
pub const GPUFREQV2_VOLT: &str = "/proc/gpufreqv2/fix_custom_freq_volt";
pub const CONFIG_FILE_TR: &str = "/data/gpu_freq_table.conf";
#[allow(dead_code)]
pub const LOG_PATH: &str = "/data/adb/gpu_governor/log/gpu_gov.log";
pub const LOG_LEVEL_PATH: &str = "/data/adb/gpu_governor/log_level";
#[allow(dead_code)]
pub const GPU_POWER_POLICY: &str = "/sys/class/misc/mali0/device/power_policy";
pub const GAMES_CONF_PATH: &str = "/data/adb/gpu_governor/games.conf";

#[allow(dead_code)]
pub const FW: &str = "FreqWriter";
#[allow(dead_code)]
pub const FPS_STATUS: &str = "/sys/kernel/fpsgo/fstb/fpsgo_status";
#[allow(dead_code)]
pub const TOP_PID: &str = "/sys/kernel/gbe/gbe2_fg_pid";

// Constants
pub const RESP_TIME: u64 = 16;
