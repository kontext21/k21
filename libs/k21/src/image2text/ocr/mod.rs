// OCR module structure
#[cfg(target_os = "macos")]
mod ocr_mac;
#[cfg(target_os = "windows")]
mod ocr_win;

mod ocr_tesseract;

mod types;
pub use types::{OcrConfig, OcrModel};

use anyhow::Result;
use image::DynamicImage;

pub async fn process_ocr(img: &DynamicImage, config: &OcrConfig) -> Result<String> {
    match config.ocr_model {
        OcrModel::Tesseract => {
            use self::ocr_tesseract::perform_ocr_tesseract;
            Ok(perform_ocr_tesseract(img, config))
        },
        OcrModel::Default | OcrModel::Native => {
            #[cfg(target_os = "macos")]
            {
                use self::ocr_mac::process_ocr_macosx;
                Ok(process_ocr_macosx(img, config).await)
            }
            #[cfg(target_os = "windows")]
            {
                use self::ocr_win::process_ocr_windows;
                process_ocr_windows(img, config).await
            }
            #[cfg(target_os = "linux")]
            {
                use self::ocr_tesseract::perform_ocr_tesseract;
                Ok(perform_ocr_tesseract(img, config))
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            {
                unimplemented!()
            }
        }
    }
} 