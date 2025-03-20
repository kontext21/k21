#[derive(Debug, Clone)]
pub enum ProcessingType {
    Vision,
    OCR,
}

struct ImageData {
    pub timestamp: String,
    pub content: String,
    pub processing_type: ProcessingType,
}

impl ImageData {
    pub fn new(timestamp: String, content: String, processing_type: ProcessingType) -> Self {
        Self { timestamp, content, processing_type }
    }
}


