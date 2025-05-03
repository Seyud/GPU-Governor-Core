use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

// Global status map
static STATUS_MAP: Lazy<Mutex<HashMap<String, bool>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn write_status(node: &str, status: bool) {
    let mut map = STATUS_MAP.lock().unwrap();
    map.insert(node.to_string(), status);
}

pub fn get_status(dir: &str) -> bool {
    let map = STATUS_MAP.lock().unwrap();
    *map.get(dir).unwrap_or(&false)
}
