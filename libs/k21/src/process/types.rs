use crate::{common::ProcessingType, image2text::OcrConfig};
use crate::image2text::VisionConfig;

#[derive(Clone)]
pub struct ProcessorConfig {
    pub processing_type: ProcessingType,
    pub vision_config: Option<VisionConfig>,
    pub ocr_config: Option<OcrConfig>,
}

impl ProcessorConfig {
    pub fn new(processing_type: ProcessingType, vision_config: Option<VisionConfig>, ocr_config: Option<OcrConfig>) -> Self {
        Self {
            processing_type,
            vision_config,
            ocr_config
        }
    }

    pub fn default() -> Self {
        Self {
            processing_type: ProcessingType::OCR,
            vision_config: None,
            ocr_config: Some(OcrConfig::default()),
        }
    }
}