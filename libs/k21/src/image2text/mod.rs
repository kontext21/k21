mod ocr;
pub use ocr::{process_ocr, OcrConfig, OcrModel};

mod vision;
pub use vision::{process_image_vision_from_path, process_image_vision};
pub use vision::VisionConfig;