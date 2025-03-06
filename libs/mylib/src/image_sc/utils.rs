use image::RgbImage;

pub fn images_differ(img1: &RgbImage, img2: &RgbImage, tolerance: f32) -> bool {
    if img1.dimensions() != img2.dimensions() {
        return true; // Different dimensions = always different
    }

    let diff_percentage = calculate_image_difference(img1, img2);
    diff_percentage > tolerance
}

/// Calculate the difference percentage between two images
fn calculate_image_difference(rgb1: &RgbImage, rgb2: &RgbImage) -> f32 {
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

pub fn calculate_threshold_exceeded_ratio(img1: &[u8], img2: &[u8], tolerance: f32) -> f32 {
    if img1.len() != img2.len() {
        return 1.0; // Different lengths = 100% different
    }

    let total_pixels = img1.len() as u64;
    let mut different_pixels = 0u64;

    let max_diff = (255.0 * tolerance) as u8;
    
    for (p1, p2) in img1.iter().zip(img2.iter()) {
        // Consider pixels different if luminance differs by more than max_diff pixels
        if p1.abs_diff(*p2) > max_diff {
            different_pixels += 1;
        }
    }
    
    different_pixels as f32 / total_pixels as f32
}
