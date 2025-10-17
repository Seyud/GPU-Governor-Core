/// GPU Governor 常量定义
/// 将分散的常量集中管理，提高代码可维护性
pub const NOTES: &str = "Mediatek Mali GPU Governor";
pub const AUTHOR: &str = "Author: walika @CoolApk, Tools-cx-app @GitHub";
pub const SPECIAL: &str =
    "Special Thanks: HamJin @CoolApk, asto18089 @CoolApk and helloklf @GitHub";
pub const VERSION: &str = "Version: v2.10.3";

/// GPU 调频策略常量
pub mod strategy {
    pub const IDLE_THRESHOLD: i32 = 5;
    pub const FOREGROUND_APP_STARTUP_DELAY: u64 = 60; // seconds
}
