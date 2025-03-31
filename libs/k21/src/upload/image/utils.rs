use image::DynamicImage;

use anyhow::Result;

use crate::{common::{get_current_timestamp_str, ImageData, ImageDataCollection}, process::{process_image_by_processing_type, ProcessorConfig}};

pub fn path_to_image(path: &str) -> Result<DynamicImage> {
    let image = image::open(path)?;
    Ok(image)
}

pub async fn process_image(path: String, config: &ProcessorConfig) -> Result<ImageDataCollection> {
    let image = path_to_image(&path)?;
    
    let result = process_image_by_processing_type(&image, config, 0).await;
    let image_data = ImageData::new(get_current_timestamp_str(), 0, result.unwrap(), config.processing_type.clone());
    
    let mut image_data_collection = ImageDataCollection::new();
    image_data_collection.push(image_data);
    Ok(image_data_collection)
}