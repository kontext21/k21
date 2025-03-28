use serde::{Deserialize, Serialize};

const FPS_DEFAULT: f32 = 1.0;
const DURATION_DEFAULT: u64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenCaptureConfig {
    pub fps: Option<f32>,
    pub duration: Option<u64>,
    pub save_screenshot_to: Option<String>,
    pub save_video_to: Option<String>,
    pub video_chunk_duration: Option<u64>,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        Self {
            fps: None,
            duration: None,
            save_screenshot_to: None,
            save_video_to: None,
            video_chunk_duration: None,
        }
    }
}

impl ScreenCaptureConfig {
    pub fn new(
        fps: Option<f32>,
        duration: Option<u64>,
        save_screenshot_to: Option<String>,
        save_video_to: Option<String>,
        video_chunk_duration: Option<u64>,
    ) -> Self {
        let config: ScreenCaptureConfig = Self {
            fps,
            duration,
            save_screenshot_to,
            save_video_to,
            video_chunk_duration,
            ..Default::default()
        };
        config
    }

    pub fn get_fps(&self) -> f32 {
        self.fps.unwrap_or(FPS_DEFAULT)
    }

    pub fn get_duration(&self) -> u64 {
        self.duration.unwrap_or(DURATION_DEFAULT)
    }

    pub fn get_save_screenshot_to(&self) -> Option<String> {
        self.save_screenshot_to.clone()
    }

    pub fn get_save_video_to(&self) -> Option<String> {
        self.save_video_to.clone()
    }

    pub fn get_video_chunk_duration(&self) -> Option<u64> {
        self.video_chunk_duration.clone()
    }
}
