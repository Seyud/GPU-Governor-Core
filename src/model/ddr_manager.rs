use std::{cell::Cell, fs};

use anyhow::Result;
use log::{debug, warn};

use crate::{datasource::file_path::*, utils::file_helper::FileHelper};

/// DDR频率管理器 - 负责内存频率控制
#[derive(Clone)]
pub struct DdrManager {
    /// 是否固定内存频率
    pub ddr_freq_fixed: bool,
    /// 当前固定的内存频率
    pub ddr_freq: i64,
    /// v2 driver支持的内存频率列表
    pub ddr_v2_supported_freqs: Vec<i64>,
    /// 是否使用v2驱动
    pub gpuv2: bool,
    /// 最近一次写入的DDR OPP值缓存
    last_written_ddr_opp: Cell<Option<i64>>,
}

impl DdrManager {
    pub fn new() -> Self {
        Self {
            ddr_freq_fixed: false,
            ddr_freq: 0,
            ddr_v2_supported_freqs: Vec::new(),
            gpuv2: false,
            last_written_ddr_opp: Cell::new(None),
        }
    }

    /// 设置DDR频率
    pub fn set_ddr_freq(&mut self, freq: i64) -> Result<()> {
        // 如果频率是999，表示不固定内存频率，让系统自己选择
        if freq == 999 {
            self.ddr_freq = if self.gpuv2 {
                DDR_AUTO_MODE_V2
            } else {
                DDR_AUTO_MODE_V1
            };
            self.ddr_freq_fixed = false;
            self.last_written_ddr_opp.set(None);
            debug!("DDR frequency not fixed (auto mode)");
            return self.write_ddr_freq();
        }

        // 如果频率是DDR_HIGHEST_FREQ，表示使用最高内存频率和电压
        if freq == DDR_HIGHEST_FREQ {
            self.ddr_freq = freq;
            self.ddr_freq_fixed = true;
            debug!("Setting DDR to highest frequency and voltage (OPP value: {DDR_HIGHEST_FREQ})");
            return self.write_ddr_freq();
        }

        // 如果频率小于0，表示不固定内存频率
        if freq < 0 {
            self.ddr_freq = if self.gpuv2 {
                DDR_AUTO_MODE_V2
            } else {
                DDR_AUTO_MODE_V1
            };
            self.ddr_freq_fixed = false;
            self.last_written_ddr_opp.set(None);
            debug!("DDR frequency not fixed");
            return self.write_ddr_freq();
        }

        // 如果freq值小于100，则认为是直接指定的DDR_OPP值
        if freq < 100 {
            self.ddr_freq = freq;
            self.ddr_freq_fixed = true;

            let opp_description = match freq {
                DDR_HIGHEST_FREQ => "Highest Frequency and Voltage",
                DDR_SECOND_FREQ => "Second Level Frequency and Voltage",
                DDR_THIRD_FREQ => "Third Level Frequency and Voltage",
                DDR_FOURTH_FREQ => "Fourth Level Frequency and Voltage",
                DDR_FIFTH_FREQ => "Fifth Level Frequency and Voltage",
                _ => "Custom Level",
            };

            debug!("Using direct DDR_OPP value: {freq} ({opp_description})");
        } else {
            // 如果是实际频率值，需要转换为DDR_OPP值
            // 这里简化处理，使用最高频率
            self.ddr_freq = DDR_HIGHEST_FREQ;
            self.ddr_freq_fixed = true;
            debug!("Using highest DDR frequency for target freq: {freq}");
        }

        self.write_ddr_freq()
    }

    /// 写入DDR频率
    pub fn write_ddr_freq(&self) -> Result<()> {
        if !self.ddr_freq_fixed {
            self.last_written_ddr_opp.set(None);
            // 如果不固定内存频率，根据驱动类型写入不同的自动模式值
            if self.gpuv2 {
                // v2 driver，使用DDR_AUTO_MODE_V2（999）表示自动模式
                let paths = [DVFSRC_V2_PATH_1, DVFSRC_V2_PATH_2];

                let mut path_written = false;
                for path in &paths {
                    if fs::exists(path)? {
                        let auto_mode_str = DDR_AUTO_MODE_V2.to_string();
                        debug!("Writing {auto_mode_str} to v2 DDR path: {path}");
                        if FileHelper::write_string_safe(path, &auto_mode_str) {
                            path_written = true;
                            break;
                        }
                    }
                }

                if !path_written {
                    debug!(
                        "Failed to write DDR_AUTO_MODE_V2 to any v2 driver path (continuing execution)"
                    );
                }
            } else {
                // v1 driver，使用DDR_AUTO_MODE_V1（-1）表示自动模式
                if fs::exists(DVFSRC_V1_PATH)? {
                    let auto_mode_str = DDR_AUTO_MODE_V1.to_string();
                    debug!("Writing {auto_mode_str} to v1 DDR path: {DVFSRC_V1_PATH}");
                    FileHelper::write_string_safe(DVFSRC_V1_PATH, &auto_mode_str);
                } else {
                    debug!("V1 DDR path does not exist: {DVFSRC_V1_PATH} (continuing execution)");
                }
            }

            return Ok(());
        }

        // 如果固定内存频率，直接使用DDR_OPP值
        let ddr_opp = self.ddr_freq;
        let freq_str = ddr_opp.to_string();

        if self.gpuv2 {
            // v2 driver
            let paths = [DVFSRC_V2_PATH_1, DVFSRC_V2_PATH_2];

            let mut path_written = false;
            for path in &paths {
                if fs::exists(path)? {
                    debug!("Writing {freq_str} to v2 DDR path: {path}");
                    if FileHelper::write_string_safe(path, &freq_str) {
                        path_written = true;
                        break;
                    }
                }
            }

            if !path_written {
                debug!(
                    "Failed to write DDR frequency to any v2 driver path (continuing execution)"
                );
            }
        } else {
            // v1 driver
            if fs::exists(DVFSRC_V1_PATH)? {
                debug!("Writing {freq_str} to v1 DDR path: {DVFSRC_V1_PATH}");
                FileHelper::write_string_safe(DVFSRC_V1_PATH, &freq_str);
            } else {
                debug!("V1 DDR path does not exist: {DVFSRC_V1_PATH} (continuing execution)");
            }
        }

        if self.last_written_ddr_opp.get() != Some(ddr_opp) {
            let opp_description = match ddr_opp {
                DDR_HIGHEST_FREQ => "Highest Frequency and Voltage",
                DDR_SECOND_FREQ => "Second Level Frequency and Voltage",
                DDR_THIRD_FREQ => "Third Level Frequency and Voltage",
                DDR_FOURTH_FREQ => "Fourth Level Frequency and Voltage",
                DDR_FIFTH_FREQ => "Fifth Level Frequency and Voltage",
                _ => "Custom Level",
            };

            debug!("Set DDR frequency with OPP value: {ddr_opp} ({opp_description})");
            self.last_written_ddr_opp.set(Some(ddr_opp));
        }
        Ok(())
    }

    /// 获取DDR频率表
    pub fn get_ddr_freq_table(&self) -> Result<Vec<(i64, String)>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let mut freq_table = Vec::new();

        // 添加自动模式
        if self.gpuv2 {
            freq_table.push((DDR_AUTO_MODE_V2, "Auto Mode".to_string()));
        } else {
            freq_table.push((DDR_AUTO_MODE_V1, "Auto Mode".to_string()));
        }

        // 添加预设的DDR_OPP值
        freq_table.push((
            DDR_HIGHEST_FREQ,
            "Highest Frequency and Voltage".to_string(),
        ));
        freq_table.push((
            DDR_SECOND_FREQ,
            "Second Level Frequency and Voltage".to_string(),
        ));
        freq_table.push((
            DDR_THIRD_FREQ,
            "Third Level Frequency and Voltage".to_string(),
        ));
        freq_table.push((
            DDR_FOURTH_FREQ,
            "Fourth Level Frequency and Voltage".to_string(),
        ));
        freq_table.push((
            DDR_FIFTH_FREQ,
            "Fifth Level Frequency and Voltage".to_string(),
        ));

        // 尝试读取系统内存频率表
        if self.gpuv2 {
            // v2 driver
            let opp_tables = [DVFSRC_V2_OPP_TABLE_1, DVFSRC_V2_OPP_TABLE_2];

            for opp_table in &opp_tables {
                if fs::exists(opp_table)? {
                    debug!("Reading v2 DDR OPP table: {opp_table}");

                    match File::open(opp_table) {
                        Ok(file) => {
                            let reader = BufReader::new(file);

                            for line in reader.lines().map_while(Result::ok) {
                                if line.contains("[OPP")
                                    && let Some(opp_str) = line.get(4..6)
                                    && let Ok(opp) = opp_str.parse::<i64>()
                                {
                                    freq_table
                                        .push((opp, format!("OPP{:02}: {}", opp, line.trim())));
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to open v2 DDR OPP table: {opp_table}: {e}");
                        }
                    }
                }
            }
        } else {
            // v1 driver
            if fs::exists(DVFSRC_V1_OPP_TABLE)? {
                debug!("Reading v1 DDR OPP table: {DVFSRC_V1_OPP_TABLE}");

                match File::open(DVFSRC_V1_OPP_TABLE) {
                    Ok(file) => {
                        let reader = BufReader::new(file);

                        for line in reader.lines().map_while(Result::ok) {
                            if line.contains("[OPP") {
                                let parts: Vec<&str> = line.split(',').collect();
                                if parts.len() >= 2 {
                                    let opp_part = parts[0].trim();
                                    let ddr_part = parts[1].trim();

                                    if opp_part.starts_with("[OPP")
                                        && opp_part.len() >= 6
                                        && ddr_part.starts_with("ddr:")
                                        && let Ok(opp) = opp_part[4..6].parse::<i64>()
                                    {
                                        let ddr_desc = ddr_part.trim_start_matches("ddr:").trim();
                                        freq_table.push((opp, format!("OPP{opp:02}: {ddr_desc}")));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to open v1 DDR OPP table: {DVFSRC_V1_OPP_TABLE}: {e}");
                    }
                }
            }
        }

        Ok(freq_table)
    }

    /// 读取v2 driver设备的内存频率表
    pub fn read_ddr_v2_freq_table(&self) -> Result<Vec<i64>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let mut freq_list = Vec::new();

        // 检查v2 driver的内存频率表文件
        let paths = [DVFSRC_V2_OPP_TABLE_1, DVFSRC_V2_OPP_TABLE_2];
        let mut found_path = None;

        for path in &paths {
            if fs::exists(path).unwrap_or(false) {
                found_path = Some(*path);
                debug!("Found V2 driver DDR OPP table file: {path}");
                break;
            }
        }

        if let Some(path) = found_path {
            let file = File::open(path)?;
            let reader = BufReader::new(file);

            for line in reader.lines().map_while(Result::ok) {
                if line.contains("[OPP")
                    && line.len() >= 6
                    && let Ok(opp) = line[4..6].parse::<i64>()
                {
                    freq_list.push(opp);
                    debug!("Found V2 driver DDR OPP value: {opp}");
                }
            }

            // 按升序排序
            freq_list.sort();
            debug!(
                "Read {} DDR OPP values from V2 driver table",
                freq_list.len()
            );
        } else {
            warn!("No V2 driver DDR OPP table file found");
        }

        Ok(freq_list)
    }

    // Getter和Setter方法 - 手动实现
    pub fn is_ddr_freq_fixed(&self) -> bool {
        self.ddr_freq_fixed
    }

    pub fn get_ddr_freq(&self) -> i64 {
        self.ddr_freq
    }

    pub fn get_ddr_v2_supported_freqs(&self) -> Vec<i64> {
        self.ddr_v2_supported_freqs.clone()
    }

    pub fn set_ddr_v2_supported_freqs(&mut self, ddr_v2_supported_freqs: Vec<i64>) {
        self.ddr_v2_supported_freqs = ddr_v2_supported_freqs;
    }
}

impl Default for DdrManager {
    fn default() -> Self {
        Self::new()
    }
}
