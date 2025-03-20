use image::{DynamicImage, RgbImage};
use openh264::decoder::DecodedYUV;
use openh264::formats::YUVSource;
use anyhow::Result;

const TOLERANCE: f32 = 0.05;

pub fn images_differ_rgb(img1: &RgbImage, img2: &RgbImage, tolerance: f32) -> bool {
    if img1.dimensions() != img2.dimensions() {
        return true; // Different dimensions = always different
    }

    let diff_percentage = calculate_image_difference_rgb(img1, img2);
    diff_percentage > tolerance
}

/// Calculate the difference percentage between two images
pub fn calculate_image_difference_rgb(rgb1: &RgbImage, rgb2: &RgbImage) -> f32 {
    if rgb1.dimensions() != rgb2.dimensions() {
        return 1.0; // Different dimensions = 100% different
    }

    let total_pixels = (rgb1.width() * rgb1.height()) as u64;
    let mut different_pixels = 0u64;

    for (p1, p2) in rgb1.pixels().zip(rgb2.pixels()) {
        // Consider pixels different if any RGB component differs by more than 10
        if (p1[0].abs_diff(p2[0]) > 10) ||
           (p1[1].abs_diff(p2[1]) > 10) ||
           (p1[2].abs_diff(p2[2]) > 10) {
            different_pixels += 1;
        }
    }

    different_pixels as f32 / total_pixels as f32
}

pub fn calculate_image_difference_luma(img1: &[u8], img2: &[u8]) -> f32 {
    if img1.len() != img2.len() {
        return 1.0; // Different lengths = 100% different
    }

    let total_pixels = img1.len() as u64;
    let mut different_pixels = 0u64;

    let max_diff = (255.0 * TOLERANCE) as u8;
    
    for (p1, p2) in img1.iter().zip(img2.iter()) {
        // Consider pixels different if luminance differs by more than max_diff pixels
        if p1.abs_diff(*p2) > max_diff {
            different_pixels += 1;
        }
    }
    
    different_pixels as f32 / total_pixels as f32
}

pub fn luma_to_image(luma: &[u8], width: u32, height: u32) -> Result<DynamicImage> {
    let luma_img = image::GrayImage::from_raw(width, height, luma.to_vec())
        .ok_or(anyhow::format_err!("Failed to create GrayImage"))?;
    Ok(DynamicImage::ImageLuma8(luma_img))
}

// Extract the image conversion functions to public methods
pub fn yuv_to_luma(yuv: &DecodedYUV) -> Result<Vec<u8>> {
    let (width, height) = yuv.dimensions();
    let stride = yuv.strides().0; // Get Y plane stride

    // Create a new buffer for the luma data with correct dimensions
    let mut luma_data = Vec::with_capacity(width * height);

    // Copy data from Y plane, accounting for stride if needed
    for y in 0..height {
        let row_start = y * stride;
        luma_data.extend_from_slice(&yuv.y()[row_start..row_start + width]);
    }

    Ok(luma_data)
}

pub fn convert_yuv_to_dynamic_image(yuv: &DecodedYUV) -> Result<(DynamicImage, Vec<u8>)> {
    let current_luma = yuv_to_luma(yuv)?;
    let current_luma_image = current_luma.as_slice();
    
    let (width, height) = yuv.dimensions();
    let dynamic_image = luma_to_image(current_luma_image, width as u32, height as u32)?;
    
    Ok((dynamic_image, current_luma))
}

pub fn should_process_frame_luma(current_luma: &[u8], previous_image: Option<&[u8]>, threshold: f32) -> bool {
    match previous_image {
        Some(prev_image) => {
            let ratio = calculate_image_difference_luma(current_luma, prev_image);
            ratio > threshold
        }
        None => true // Always process the first frame
    }
}

pub fn should_process_frame_rgb(current_image: &RgbImage, previous_image: Option<&RgbImage>, threshold: f32) -> bool {
    match previous_image {
        Some(prev_image) => {
            let ratio = calculate_image_difference_rgb(current_image, prev_image);
            ratio > threshold
        }
        None => true // Always process the first frame
    }
}