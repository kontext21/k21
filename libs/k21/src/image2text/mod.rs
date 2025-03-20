mod ocr;
pub use ocr::process_ocr;

mod vision;
pub use vision::vision_api_call::process_image_vision_from_path;