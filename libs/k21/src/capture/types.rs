use std::path::PathBuf;
pub struct ScreenCaptureConfig {
    pub fps: f32,
    pub video_chunk_duration_in_seconds: u64,
    pub stdout: bool, // deprecated ?
    pub save_screenshot: bool,
    pub save_video: bool,
    pub record_length_in_seconds: u64,
    pub output_dir_video: Option<PathBuf>,
    pub output_dir_screenshot: Option<PathBuf>,
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
    /// Creates a new ScreenCaptureConfig with the specified parameters
    pub fn new(
        fps: f32,
        record_length_in_seconds: u64,
        save_screenshot: bool,
        save_video: bool,
        output_dir_video: Option<PathBuf>,
        output_dir_screenshot: Option<PathBuf>,
        video_chunk_duration_in_seconds: Option<u64>,
    ) -> Self {
        let config: ScreenCaptureConfig = Self {
            fps,
            record_length_in_seconds,
            save_screenshot,
            save_video,
            output_dir_video,
            output_dir_screenshot,
            video_chunk_duration_in_seconds: video_chunk_duration_in_seconds.unwrap_or(60),
            ..Default::default()
        };
        config
    }

    pub fn compute_total_frames(&self) -> u64 {
        let fps_f64: f64 = self.fps as f64;
        let seconds_f64: f64 = self.record_length_in_seconds as f64;
        (fps_f64 * seconds_f64).ceil() as u64
    }
}
