use std::{collections::HashMap, path::Path, thread, time::Duration};

use anyhow::{Context, Result};
use inotify::{EventMask, Inotify, WatchMask};

const WAIT_MOVE_US: u64 = 500 * 1000;
const RECREATE_DEFAULT_PERM: u32 = 0o666;

pub struct InotifyWatcher {
    inotify: Inotify,
    watches: HashMap<inotify::WatchDescriptor, String>,
}

impl InotifyWatcher {
    pub fn new() -> Result<Self> {
        let inotify = Inotify::init().with_context(|| "Failed to initialize inotify")?;

        Ok(Self {
            inotify,
            watches: HashMap::new(),
        })
    }

    pub fn add<P: AsRef<Path>>(&mut self, path: P, mask: WatchMask) -> Result<()> {
        let path_ref = path.as_ref();
        let path_str = path_ref
            .to_str()
            .with_context(|| format!("Invalid path: {}", path_ref.display()))?;

        // 将 DELETE_SELF 和 MOVE_SELF 添加到监控掩码中
        let mask = mask | WatchMask::DELETE_SELF | WatchMask::MOVE_SELF;

        let wd = self
            .inotify
            .watches()
            .add(path_ref, mask)
            .with_context(|| format!("Failed to add watch for: {}", path_ref.display()))?;

        self.watches.insert(wd, path_str.to_string());

        Ok(())
    }

    pub fn wait_and_handle(&mut self) -> Result<()> {
        let mut buffer = [0; 4096];
        let events = self
            .inotify
            .read_events_blocking(&mut buffer)
            .with_context(|| "Failed to read inotify events")?;

        // 转换事件类型
        let mut converted_events = Vec::new();
        for event in events {
            let converted_event = inotify::Event {
                wd: event.wd,
                mask: event.mask,
                cookie: event.cookie,
                name: None, // 简化处理，忽略名称
            };
            converted_events.push(converted_event);
        }

        self.handle_events(converted_events)
    }

    // 新增：非阻塞地检查事件
    pub fn check_events(&mut self) -> Result<Vec<inotify::Event<&'static [u8]>>> {
        let mut buffer = [0; 4096];
        let events = self
            .inotify
            .read_events(&mut buffer)
            .with_context(|| "Failed to read inotify events")?;

        // 收集事件到向量中
        let mut events_vec: Vec<inotify::Event<&'static [u8]>> = Vec::new();
        for event in events {
            // 转换事件类型
            let converted_event = inotify::Event {
                wd: event.wd,
                mask: event.mask,
                cookie: event.cookie,
                name: None, // 简化处理，忽略名称
            };
            events_vec.push(converted_event);
        }

        // 如果有事件，处理它们
        if !events_vec.is_empty() {
            self.handle_events(events_vec.iter().cloned())?;
        }

        Ok(events_vec)
    }

    // 提取共同的事件处理逻辑
    fn handle_events<I>(&mut self, events: I) -> Result<()>
    where
        I: IntoIterator<Item = inotify::Event<&'static [u8]>>,
    {
        // 收集所有需要更新的监控项
        let mut watches_to_update = Vec::new();

        for event in events {
            if let Some(path) = self.watches.get(&event.wd) {
                // 在删除后重新建立监控
                if event.mask.contains(EventMask::IGNORED)
                    || event.mask.contains(EventMask::DELETE_SELF)
                    || event.mask.contains(EventMask::MOVE_SELF)
                {
                    watches_to_update.push((event.wd, path.clone()));
                }
            }
        }

        // 更新监控
        for (wd, path) in watches_to_update {
            // 如果文件不存在，尝试重新创建
            try_path(&path)?;

            // 重新添加监控
            let mask = WatchMask::MODIFY
                | WatchMask::CLOSE_WRITE
                | WatchMask::DELETE_SELF
                | WatchMask::MOVE_SELF;

            let new_wd = self
                .inotify
                .watches()
                .add(&path, mask)
                .with_context(|| format!("Failed to re-add watch for: {path}"))?;

            // 更新监控映射表
            self.watches.remove(&wd);
            self.watches.insert(new_wd, path);
        }

        Ok(())
    }
}

fn try_path(path: &str) -> Result<()> {
    let path = Path::new(path);

    if !path.exists() {
        // 稍作等待，让文件系统操作完成
        thread::sleep(Duration::from_micros(WAIT_MOVE_US));

        // 设置权限
        unsafe {
            libc::chmod(
                path.to_str().unwrap_or("").as_ptr() as *const libc::c_char,
                RECREATE_DEFAULT_PERM,
            );
        }
    }

    Ok(())
}
