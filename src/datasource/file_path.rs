// Thread names
#![allow(dead_code)]
pub const MAIN_THREAD: &str = "LoadMonitor";
pub const GAME_THREAD: &str = "GameModeWatcher";
pub const CONF_THREAD: &str = "ConfigWatcher";

pub const GED_LOCKER: &str = "GedLocker";
pub const FOREGROUND_APP_THREAD: &str = "ForegroundAppWatcher";

// File paths
pub const GPU_GOVERNOR_GAME_MODE_PATH: &str = "/data/adb/gpu_governor/game/game_mode";
pub const MODULE_LOAD: &str = "/sys/module/ged/parameters/gpu_loading";
pub const MODULE_IDLE: &str = "/sys/module/ged/parameters/gpu_idle";
pub const KERNEL_LOAD: &str = "/sys/kernel/ged/hal/gpu_utilization";
pub const KERNEL_DEBUG_LOAD: &str = "/sys/kernel/d/ged/hal/gpu_utilization";
pub const GPU_CURRENT_FREQ_PATH: &str = "/sys/kernel/ged/hal/current_freqency";
pub const KERNEL_D_LOAD: &str = "/sys/kernel/debug/ged/hal/gpu_utilization";
pub const GPU_FREQ_LOAD_PATH: &str = "/proc/gpufreq/gpufreq_var_dump";
pub const PROC_MALI_LOAD: &str = "/proc/mali/utilization";
pub const PROC_MTK_LOAD: &str = "/proc/mtk_mali/utilization";
pub const DEBUG_DVFS_LOAD: &str = "/sys/kernel/debug/mali0/dvfs_utilization";
pub const DEBUG_DVFS_LOAD_OLD: &str = "/proc/mali/dvfs_utilization";
pub const GPUFREQ_TABLE: &str = "/proc/gpufreq/gpufreq_opp_dump";
pub const GPUFREQV2_TABLE: &str = "/proc/gpufreqv2/stack_working_opp_table";
pub const GEDFREQ_MAX: &str = "/sys/kernel/ged/hal/custom_upbound_gpu_freq";
pub const GEDFREQ_MIN: &str = "/sys/kernel/ged/hal/custom_boost_gpu_freq";
pub const GPUFREQ_OPP: &str = "/proc/gpufreq/gpufreq_opp_freq";
pub const GPUFREQV2_OPP: &str = "/proc/gpufreqv2/fix_target_opp_index";
pub const GPUFREQ_VOLT: &str = "/proc/gpufreq/gpufreq_fixed_freq_volt";
pub const GPUFREQV2_VOLT: &str = "/proc/gpufreqv2/fix_custom_freq_volt";
pub const CONFIG_FILE_TR: &str = "/data/gpu_freq_table.conf";
pub const LOG_PATH: &str = "/data/adb/gpu_governor/log/gpu_gov.log";
pub const LOG_LEVEL_PATH: &str = "/data/adb/gpu_governor/log/log_level";
pub const GPU_POWER_POLICY: &str = "/sys/class/misc/mali0/device/power_policy";
pub const GAMES_CONF_PATH: &str = "/data/adb/gpu_governor/game/games.conf";

// 内存频率相关路径 - v1 driver
pub const DVFSRC_V1_PATH: &str = "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp";
pub const DVFSRC_V1_OPP_TABLE: &str = "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_opp_table";

// 内存频率相关路径 - v2 driver
pub const DVFSRC_V2_PATH_1: &str = "/sys/devices/platform/soc/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_force_vcore_dvfs_opp";
pub const DVFSRC_V2_PATH_2: &str = "/sys/devices/platform/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_force_vcore_dvfs_opp";
pub const DVFSRC_V2_OPP_TABLE_1: &str = "/sys/devices/platform/soc/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_opp_table";
pub const DVFSRC_V2_OPP_TABLE_2: &str = "/sys/devices/platform/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_opp_table";

// 内存频率固定值 - 用于设置内存频率
pub const DDR_AUTO_MODE_V1: i64 = -1;   // v1 driver自动模式，系统自己选择内存频率
pub const DDR_AUTO_MODE_V2: i64 = 999;  // v2 driver自动模式，系统自己选择内存频率
pub const DDR_HIGHEST_FREQ: i64 = 0;    // 最高内存频率和电压（第一档）
pub const DDR_SECOND_FREQ: i64 = 1;     // 第二档内存频率和电压
pub const DDR_THIRD_FREQ: i64 = 2;      // 第三档内存频率和电压
pub const DDR_FOURTH_FREQ: i64 = 3;     // 第四档内存频率和电压
pub const DDR_FIFTH_FREQ: i64 = 4;      // 第五档内存频率和电压

pub const FW: &str = "FreqWriter";
pub const FPS_STATUS: &str = "/sys/kernel/fpsgo/fstb/fpsgo_status";
pub const TOP_PID: &str = "/sys/kernel/gbe/gbe2_fg_pid";

// Constants
pub const RESP_TIME: u64 = 16;
