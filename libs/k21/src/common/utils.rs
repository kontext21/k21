pub fn get_current_timestamp_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}