use anyhow::Result;
use image::DynamicImage;

pub async fn process_ocr(img: &DynamicImage) -> Result<String> {
    // #[cfg(target_os = "macos")]
    // {
    //     use crate::ocr_mac::process_ocr_macosx;
    //     process_ocr_macosx(img).await
    // }
    #[cfg(target_os = "windows")]
    {
        use crate::ocr_win::process_ocr_windows;
        process_ocr_windows(img).await
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        unimplemented!()
    }
}
