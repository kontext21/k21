use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessingType {
    Vision,
    OCR,
}

impl std::fmt::Display for ProcessingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingType::Vision => write!(f, "Vision"),
            ProcessingType::OCR => write!(f, "OCR"),
        }
    }
}

impl From<&str> for ProcessingType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "Vision" => ProcessingType::Vision,
            "OCR" => ProcessingType::OCR,
            _ => ProcessingType::OCR, // default case
        }
    }
}

impl From<String> for ProcessingType {
    fn from(s: String) -> Self {
        ProcessingType::from(s.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    timestamp: String,
    frame_number: u64,
    content: String,
    processing_type: ProcessingType,
}

impl ImageData {
    pub fn new(timestamp: String, frame_number: u64, content: String, processing_type: ProcessingType) -> Self {
        Self { timestamp, frame_number, content, processing_type }
    }

    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }

    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn processing_type(&self) -> &ProcessingType {
        &self.processing_type
    }
}

pub type ImageDataCollection = Vec<ImageData>;

