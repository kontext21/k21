use image::DynamicImage;
use crate::{mp4_pr::utils::{FrameData, mp4_for_each_frame}, image2text::ocr::process_ocr};
use anyhow::Result;  // Import Result from anyhow

pub fn process_image(image: &DynamicImage) -> Result<DynamicImage> {
    Ok(image.clone())
}

pub fn load_image_from_path(path: &std::path::PathBuf) -> Result<DynamicImage> {
    image::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to load image from {}: {}", path.display(), e))
}

pub async fn perform_ocr_on_image(image: &DynamicImage) -> Result<FrameData> {
    let text = process_ocr(image).await?;
    let frame_data = FrameData {
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        ocr_text: text,
    };
    Ok(frame_data)
}

pub async fn perform_ocr_on_image_from_path(path: &str) -> Result<FrameData> {
    let path_buf = std::path::PathBuf::from(path);
    let image = load_image_from_path(&path_buf).unwrap();
    perform_ocr_on_image(&image).await
}

pub async fn perform_ocr_on_video_from_path(path: &str) -> Result<FrameData> {
    let path_buf = std::path::PathBuf::from(path);
    let image = load_image_from_path(&path_buf).unwrap();
    perform_ocr_on_image(&image).await
}

pub async fn perform_ocr_on_video_path(path: &str) -> Result<Vec<FrameData>> {
    let path_buf = std::path::PathBuf::from(path);
    let results = mp4_for_each_frame(&path_buf, None).await?;
    Ok(results)
}

#[tokio::test]
async fn test_perform_ocr_on_video_path() -> Result<()> {
    // Arrange
    let test_video_path = std::env::current_dir()
        .unwrap()
        .join("test-output.mp4")
        .to_str()
        .unwrap()
        .to_string();
    
    // Act
    let results = perform_ocr_on_video_path(&test_video_path).await?;
    
    // Assert
    assert!(!results.is_empty(), "OCR results should not be empty");
    
    // Check that each frame has some data
    for (i, frame) in results.iter().enumerate() {
        println!("Frame {}: timestamp={}, ocr_text={}", 
                 i, frame.timestamp, frame.ocr_text);
        
        // Basic validation that we have timestamps
        assert!(!frame.timestamp.is_empty(), "Frame timestamp should not be empty");
    }
    
    Ok(())
}
