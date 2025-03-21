mod utils;
pub use utils::capture;
pub use utils::spawn_screenshot_task;
pub use utils::run_screen_capture;

mod screen_record;
pub use screen_record::ScreenCapturer;
pub use screen_record::get_primary_monitor_id;


mod types;
pub use types::ScreenCaptureConfig;
