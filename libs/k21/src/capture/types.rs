use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenCaptureConfig {
    pub fps: f32,
    pub video_chunk_duration_in_seconds: u64,
    pub stdout: bool,
    pub save_screenshot: bool,
    pub save_video: bool,
    pub record_length_in_seconds: u64,
    pub output_dir_video: Option<String>,
    pub output_dir_screenshot: Option<String>,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        Self {
            fps: 1.0,
            video_chunk_duration_in_seconds: 60,
            stdout: false,
            save_screenshot: false,
            save_video: false,
            record_length_in_seconds: 1,
            output_dir_video: None,
            output_dir_screenshot: None,
        }
    }
}

impl ScreenCaptureConfig {
    pub fn new(
        fps: f32,
        video_chunk_duration_in_seconds: u64,
        save_screenshot: bool,
        save_video: bool,
        record_length_in_seconds: u64,
        output_dir_video: Option<String>,
        output_dir_screenshot: Option<String>,
    ) -> Self {
        let config: ScreenCaptureConfig = Self {
            fps,
            video_chunk_duration_in_seconds,
            record_length_in_seconds,
            save_screenshot,
            save_video,
            output_dir_video,
            output_dir_screenshot,
            ..Default::default()
        };
        config
    }
}
