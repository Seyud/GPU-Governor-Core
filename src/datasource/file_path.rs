// Thread names
pub const GAME_THREAD: &str = "GameModeWatcher";
pub const CONF_THREAD: &str = "ConfigWatcher";
pub const FOREGROUND_APP_THREAD: &str = "ForegroundAppWatcher";

// File paths
pub const GPU_GOVERNOR_GAME_MODE_PATH: &str = "/data/adb/gpu_governor/game_mode";
pub const MODULE_LOAD: &str = "/sys/module/ged/parameters/gpu_loading";
pub const MODULE_IDLE: &str = "/sys/module/ged/parameters/gpu_idle";
pub const KERNEL_LOAD: &str = "/sys/kernel/ged/hal/gpu_utilization";
pub const KERNEL_DEBUG_LOAD: &str = "/sys/kernel/d/ged/hal/gpu_utilization";
pub const KERNEL_D_LOAD: &str = "/sys/kernel/debug/ged/hal/gpu_utilization";
pub const GPU_FREQ_LOAD_PATH: &str = "/proc/gpufreq/gpufreq_var_dump";
pub const PROC_MALI_LOAD: &str = "/proc/mali/utilization";
pub const PROC_MTK_LOAD: &str = "/proc/mtk_mali/utilization";
pub const DEBUG_DVFS_LOAD: &str = "/sys/kernel/debug/mali0/dvfs_utilization";
pub const DEBUG_DVFS_LOAD_OLD: &str = "/proc/mali/dvfs_utilization";

pub const GEDFREQ_MAX: &str = "/sys/kernel/ged/hal/custom_upbound_gpu_freq";
pub const GEDFREQ_MIN: &str = "/sys/kernel/ged/hal/custom_boost_gpu_freq";
pub const GPUFREQ_OPP: &str = "/proc/gpufreq/gpufreq_opp_freq";
pub const GPUFREQV2_OPP: &str = "/proc/gpufreqv2/fix_target_opp_index";
pub const GPUFREQ_VOLT: &str = "/proc/gpufreq/gpufreq_fixed_freq_volt";
pub const GPUFREQV2_VOLT: &str = "/proc/gpufreqv2/fix_custom_freq_volt";
pub const CONFIG_FILE_TR: &str = "/data/gpu_freq_table.conf";
pub const LOG_LEVEL_PATH: &str = "/data/adb/gpu_governor/log_level";
pub const GAMES_CONF_PATH: &str = "/data/adb/gpu_governor/games.conf";

// Constants
pub const RESP_TIME: u64 = 16;
