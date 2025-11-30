use std::{
    collections::HashMap,
    ffi::{CString, OsStr},
    path::Path,
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use inotify::{EventMask, Inotify, WatchMask};

const WAIT_MOVE_US: u64 = 500 * 1000;
const RECREATE_DEFAULT_PERM: u32 = 0o666;

#[derive(Debug, Clone)]
pub struct SimpleEvent {
    pub wd: inotify::WatchDescriptor,
    pub mask: EventMask,
    #[allow(dead_code)]
    pub cookie: u32,
    pub name: Option<String>,
}

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

    pub fn wait_and_handle(&mut self) -> Result<Vec<SimpleEvent>> {
        let mut buffer = [0; 4096];
        let events = self
            .inotify
            .read_events_blocking(&mut buffer)
            .with_context(|| "Failed to read inotify events")?;

        self.process_events(events)
    }

    // 新增：非阻塞地检查事件
    pub fn check_events(&mut self) -> Result<Vec<SimpleEvent>> {
        let mut buffer = [0; 4096];
        let events = self
            .inotify
            .read_events(&mut buffer)
            .with_context(|| "Failed to read inotify events")?;

        self.process_events(events)
    }

    fn process_events<'a, I>(&mut self, events: I) -> Result<Vec<SimpleEvent>>
    where
        I: IntoIterator<Item = inotify::Event<&'a OsStr>>,
    {
        let mut simple_events = Vec::new();
        let mut raw_events = Vec::new();

        for event in events {
            let name = event.name.map(|n| n.to_string_lossy().into_owned());

            let simple_event = SimpleEvent {
                wd: event.wd,
                mask: event.mask,
                cookie: event.cookie,
                name,
            };

            simple_events.push(simple_event.clone());
            raw_events.push(simple_event);
        }

        self.handle_events(&raw_events)?;

        Ok(simple_events)
    }

    // 提取共同的事件处理逻辑
    fn handle_events(&mut self, events: &[SimpleEvent]) -> Result<()> {
        // 收集所有需要更新的监控项
        let mut watches_to_update = Vec::new();

        for event in events {
            if let Some(path) = self.watches.get(&event.wd) {
                // 在删除后重新建立监控
                if event.mask.contains(EventMask::IGNORED)
                    || event.mask.contains(EventMask::DELETE_SELF)
                    || event.mask.contains(EventMask::MOVE_SELF)
                {
                    watches_to_update.push((event.wd.clone(), path.clone()));
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
    if !Path::new(path).exists() {
        // 稍作等待，让文件系统操作完成
        thread::sleep(Duration::from_micros(WAIT_MOVE_US));

        // 设置权限
        if let Ok(c_path) = CString::new(path) {
            unsafe {
                libc::chmod(c_path.as_ptr(), RECREATE_DEFAULT_PERM);
            }
        }
    }

    Ok(())
}
