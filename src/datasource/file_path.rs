//! 系统路径和常量定义模块
//!
//! 该模块定义了GPU负载监控器使用的所有系统路径常量、线程名称和配置参数。
//! 包括GPU监控路径、配置文件路径、DDR频率控制路径等关键系统接口。

// =============================================================================
// 线程名称常量
// =============================================================================

/// 主监控线程名称
pub const MAIN_THREAD: &str = "LoadMonitor";
/// 频率表监控线程名称
pub const FREQ_TABLE_MONITOR_THREAD: &str = "FreqTabMonitor";
/// 前台应用监控线程名称
pub const FOREGROUND_APP_THREAD: &str = "FgAppWatcher";
/// 日志级别监控线程名称
pub const LOG_LEVEL_MONITOR_THREAD: &str = "LogLevelMonitor";
/// 配置文件监控线程名称
pub const CONFIG_MONITOR_THREAD: &str = "ConfigMonitor";

// =============================================================================
// 配置文件路径常量
// =============================================================================

/// 主配置文件路径 - TOML格式的主要配置文件
pub const CONFIG_TOML_FILE: &str = "/data/adb/gpu_governor/config/config.toml";
/// GPU频率表配置文件路径 - 定义GPU频率和电压表
pub const FREQ_TABLE_CONFIG_FILE: &str = "/data/adb/gpu_governor/config/gpu_freq_table.toml";
/// 当前工作模式文件路径 - 存储当前使用的调频模式
pub const CURRENT_MODE_PATH: &str = "/data/adb/gpu_governor/config/current_mode";
/// 游戏配置文件路径 - 游戏应用检测和优化配置
pub const GAMES_CONF_PATH: &str = "/data/adb/gpu_governor/game/games.toml";

// =============================================================================
// 日志系统路径常量
// =============================================================================

/// 主日志文件路径
pub const LOG_PATH: &str = "/data/adb/gpu_governor/log/gpu_gov.log";
/// 动态日志级别控制文件路径
pub const LOG_LEVEL_PATH: &str = "/data/adb/gpu_governor/log/log_level";

// =============================================================================
// GPU负载监控路径常量
// =============================================================================

/// GED模块GPU负载路径 - 通过内核模块参数获取
pub const MODULE_LOAD: &str = "/sys/module/ged/parameters/gpu_loading";
/// GED模块GPU空闲率路径
pub const MODULE_IDLE: &str = "/sys/module/ged/parameters/gpu_idle";
/// 内核HAL GPU利用率路径 - 标准内核接口
pub const KERNEL_LOAD: &str = "/sys/kernel/ged/hal/gpu_utilization";
/// 内核调试GPU利用率路径 - 调试版本接口
pub const KERNEL_DEBUG_LOAD: &str = "/sys/kernel/d/ged/hal/gpu_utilization";
/// 内核调试GPU利用率路径（完整路径）
pub const KERNEL_D_LOAD: &str = "/sys/kernel/debug/ged/hal/gpu_utilization";

// =============================================================================
// GPU频率监控路径常量
// =============================================================================

/// GPU当前频率路径 - 标准HAL接口
pub const GPU_CURRENT_FREQ_PATH: &str = "/sys/kernel/ged/hal/current_freqency";
/// GPU当前频率路径 - 调试接口
pub const GPU_DEBUG_CURRENT_FREQ_PATH: &str = "/sys/kernel/debug/ged/hal/current_freqency";
/// GPU频率负载dump路径 - 详细频率信息
pub const GPU_FREQ_LOAD_PATH: &str = "/proc/gpufreq/gpufreq_var_dump";

// =============================================================================
// GPU频率控制路径常量
// =============================================================================

/// GPU频率表路径 - GPUFreq v2版本
pub const GPUFREQV2_TABLE: &str = "/proc/gpufreqv2/stack_working_opp_table";
/// GPU频率OPP控制路径 - GPUFreq v1版本
pub const GPUFREQ_OPP: &str = "/proc/gpufreq/gpufreq_opp_freq";
/// GPU频率OPP控制路径 - GPUFreq v2版本
pub const GPUFREQV2_OPP: &str = "/proc/gpufreqv2/fix_target_opp_index";
/// GPU电压控制路径 - GPUFreq v1版本
pub const GPUFREQ_VOLT: &str = "/proc/gpufreq/gpufreq_fixed_freq_volt";
/// GPU电压控制路径 - GPUFreq v2版本
pub const GPUFREQV2_VOLT: &str = "/proc/gpufreqv2/fix_custom_freq_volt";

// =============================================================================
// Mali GPU DVFS路径常量
// =============================================================================

/// Mali GPU DVFS使能控制路径
pub const MALI_DVFS_ENABLE: &str = "/proc/mali/dvfs_enable";
/// Mali GPU利用率路径 - 标准接口
pub const PROC_MALI_LOAD: &str = "/proc/mali/utilization";
/// MTK Mali GPU利用率路径 - MTK定制接口
pub const PROC_MTK_LOAD: &str = "/proc/mtk_mali/utilization";
/// Mali DVFS利用率路径 - 调试接口
pub const DEBUG_DVFS_LOAD: &str = "/sys/kernel/debug/mali0/dvfs_utilization";
/// Mali DVFS利用率路径 - 旧版调试接口
pub const DEBUG_DVFS_LOAD_OLD: &str = "/proc/mali/dvfs_utilization";

// =============================================================================
// DDR内存频率控制路径常量
// =============================================================================

/// DVFSRC v1驱动强制VCORE DVFS OPP路径
pub const DVFSRC_V1_PATH: &str =
    "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_force_vcore_dvfs_opp";
/// DVFSRC v1驱动OPP表路径
pub const DVFSRC_V1_OPP_TABLE: &str =
    "/sys/devices/platform/10012000.dvfsrc/helio-dvfsrc/dvfsrc_opp_table";

/// DVFSRC v2驱动强制VCORE DVFS OPP路径（SOC平台）
pub const DVFSRC_V2_PATH_1: &str = "/sys/devices/platform/soc/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_force_vcore_dvfs_opp";
/// DVFSRC v2驱动强制VCORE DVFS OPP路径（直接平台）
pub const DVFSRC_V2_PATH_2: &str = "/sys/devices/platform/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_force_vcore_dvfs_opp";
/// DVFSRC v2驱动OPP表路径（SOC平台）
pub const DVFSRC_V2_OPP_TABLE_1: &str =
    "/sys/devices/platform/soc/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_opp_table";
/// DVFSRC v2驱动OPP表路径（直接平台）
pub const DVFSRC_V2_OPP_TABLE_2: &str =
    "/sys/devices/platform/1c00f000.dvfsrc/1c00f000.dvfsrc:dvfsrc-helper/dvfsrc_opp_table";

// =============================================================================
// DDR频率档位常量定义
// =============================================================================

/// v1驱动自动模式 - 系统自动选择最优内存频率
pub const DDR_AUTO_MODE_V1: i64 = -1;
/// v2驱动自动模式 - 系统自动选择最优内存频率
pub const DDR_AUTO_MODE_V2: i64 = 999;
/// 最高内存频率档位（第一档） - 最高性能模式
pub const DDR_HIGHEST_FREQ: i64 = 0;
/// 第二档内存频率 - 高性能模式  
pub const DDR_SECOND_FREQ: i64 = 1;
/// 第三档内存频率 - 平衡模式
pub const DDR_THIRD_FREQ: i64 = 2;
/// 第四档内存频率 - 节能模式
pub const DDR_FOURTH_FREQ: i64 = 3;
/// 第五档内存频率 - 最低功耗模式
pub const DDR_FIFTH_FREQ: i64 = 4;
