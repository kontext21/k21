use std::sync::{Arc, Mutex};
use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::STANDARD};

pub fn get_current_timestamp_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub async fn get_results_from_state<T: Clone>(state: Arc<Mutex<T>>) -> Result<T> {
    let results = {
        let guard = state.lock().unwrap();
        guard.clone()
    };

    Ok(results)
}

pub fn decode_base64(base64_data: &str) -> Result<Vec<u8>> {
    STANDARD.decode(base64_data).map_err(|err| {
        log::error!("Failed to decode base64 data: {}", err);
        anyhow::anyhow!("Failed to decode base64 data: {}", err)
    })
}
