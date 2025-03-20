use xcap::Monitor;

pub fn get_current_timestamp_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn get_primary_monitor_id() -> u32 {
    Monitor::all()
        .unwrap()
        .iter()
        .find(|m| m.is_primary())
        .unwrap()
        .id()
}