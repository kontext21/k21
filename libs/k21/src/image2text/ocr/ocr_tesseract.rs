use image::DynamicImage;
use rusty_tesseract::{Args, DataOutput, Image};
use std::collections::HashMap;

use super::types::OcrConfig;

pub fn perform_ocr_tesseract(
    image: &DynamicImage,
    config: &OcrConfig
) -> String {
    let language_string = "eng".to_string();

    let args = Args {
        lang: language_string,
        config_variables: HashMap::from([("tessedit_create_tsv".into(), "1".into())]),
        dpi: Some(config.dpi.unwrap_or(OcrConfig::get_default_dpi()) as i32),
        psm: Some(config.psm.unwrap_or(OcrConfig::get_default_psm()) as i32),
        oem: Some(config.oem.unwrap_or(OcrConfig::get_default_oem()) as i32)
    };

    let ocr_image = Image::from_dynamic_image(image).unwrap();

    // Extract data output
    let data_output = rusty_tesseract::image_to_data(&ocr_image, &args).unwrap();
    data_output_to_text(&data_output, config.bounding_boxes.unwrap_or(OcrConfig::get_default_bounding_boxes()))
}

fn data_output_to_text(data_output: &DataOutput, add_bounding_boxes: bool) -> String {
    let (width, height) = data_output.data.first()
        .map(|line| (line.width as f32, line.height as f32))
        .unwrap_or((1.0, 1.0));

    data_output.data.iter()
        .filter(|line| !line.text.is_empty())
        .map(|line| {
            if add_bounding_boxes {
                // Normalize top-left corner coordinates to 0-1 range
                let x = line.left as f32 / width;
                let y = line.top as f32 / height;
                
                // Format with coordinates, rounded to 2 decimal places
                format!("({:.2}, {:.2}) {}", x, y, line.text)
            } else {
                line.text.clone()
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}