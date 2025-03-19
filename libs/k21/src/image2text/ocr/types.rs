use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OcrModel {
    Tesseract,
    Native,
    Default,
}

impl std::fmt::Display for OcrModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrModel::Tesseract => write!(f, "Tesseract"),
            OcrModel::Native => write!(f, "Native"),
            OcrModel::Default => write!(f, "Default")
        }
    }
}


impl From<String> for OcrModel {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "tesseract" => OcrModel::Tesseract,
            "native" => OcrModel::Native,
            "default" => OcrModel::Default,
            _ => OcrModel::Default,
        }
    }
}

impl From<&str> for OcrModel {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "tesseract" => OcrModel::Tesseract,
            "native" => OcrModel::Native,
            "default" => OcrModel::Default,
            _ => OcrModel::Default,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OcrConfig {
    pub ocr_model: OcrModel,
    pub bounding_boxes: Option<bool>, // add normatlized coordinates of the text
    pub dpi: Option<u32>, // dots per inch
    pub psm: Option<u32>, // Page segmentation mode
    pub oem: Option<u32>, // OCR Engine Mode
}

impl OcrConfig {
    pub fn default() -> Self {
        Self {
            ocr_model: OcrModel::Default,
            bounding_boxes: Some(true),
            dpi: None,
            psm: None,
            oem: None
        }
    }

    pub fn new(ocr_model: OcrModel, bounding_boxes: Option<bool>, dpi: Option<u32>, psm: Option<u32>, oem: Option<u32>) -> Self {
        Self {
            ocr_model,
            bounding_boxes,
            dpi,
            psm,
            oem
        }
    }

    pub fn get_default_bounding_boxes() -> bool {
        true
    }

    pub fn get_default_dpi() -> u32 {
        600
    }

    pub fn get_default_psm() -> u32 {
        1
    }

    pub fn get_default_oem() -> u32 {
        1
    }
}