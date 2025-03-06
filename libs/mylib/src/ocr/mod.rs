// OCR module structure
#[cfg(target_os = "macos")]
mod ocr_mac;
#[cfg(target_os = "windows")]
mod ocr_win;
#[cfg(target_os = "linux")]
mod ocr_linux;

use anyhow::Result;
use image::DynamicImage;

pub async fn process_ocr(img: &DynamicImage) -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        use self::ocr_mac::process_ocr_macosx;
        Ok(process_ocr_macosx(img).await)
    }
    #[cfg(target_os = "windows")]
    {
        use self::ocr_win::process_ocr_windows;
        process_ocr_windows(img).await
    }
    #[cfg(target_os = "linux")]
    {
        use self::ocr_linux::perform_ocr_tesseract;
       Ok(perform_ocr_tesseract(img))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        unimplemented!()
    }
} 