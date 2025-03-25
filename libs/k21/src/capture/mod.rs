mod utils;
pub use utils::capture;
pub use utils::spawn_screenshot_task;

mod screen_record;
pub use screen_record::ScreenCapturer;


mod types;
pub use types::ScreenCaptureConfig;
