use anyhow::Result;

mod image;
use image::process_image;

mod types;
use types::UploadType;

mod video;
pub use video::process_mp4_from_base64_with_state;
pub use video::process_mp4_buffer_path;
pub use video::process_mp4;

use crate::{common::ImageDataCollection, process::ProcessorConfig};

fn get_upload_type(path: &str) -> Result<UploadType> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| anyhow::anyhow!("Invalid file extension"))?;

    match extension.to_lowercase().as_str() {
        // Image formats 
        "png" => Ok(UploadType::Image),
        // Video formats
        "mp4" => Ok(UploadType::Video),
        // Unknown format
        ext => Err(anyhow::anyhow!("Unsupported file extension: {}", ext))
    }
}

pub async fn process_upload(path: String, config: &ProcessorConfig) -> Result<ImageDataCollection> {
    let upload_type = get_upload_type(&path)?;
    match upload_type {
        UploadType::Image => process_image(path, config).await,
        UploadType::Video => process_mp4(path, config).await,
    }
}