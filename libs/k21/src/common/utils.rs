use std::fmt::Debug;

/// Abstract Capturer Trait
trait Capturer: Debug {
    fn capture(&self) -> String;
    fn info(&self) -> String;
}

/// Abstract Processor Trait
trait Processor: Debug {
    fn process(&self, input: &str) -> String;
    fn info(&self) -> String;
}

/// Concrete ScreenCapturer Implementation
#[derive(Debug)]
struct ScreenCapturer;
impl Capturer for ScreenCapturer {
    fn capture(&self) -> String {
        "Captured screen image".to_string()
    }

    fn info(&self) -> String {
        "ScreenCapturer: Captures the screen".to_string()
    }
}

/// Concrete VideoCapturer Implementation
#[derive(Debug)]
struct VideoCapturer;
impl Capturer for VideoCapturer {
    fn capture(&self) -> String {
        "Captured video frame".to_string()
    }

    fn info(&self) -> String {
        "VideoCapturer: Captures video frames".to_string()
    }
}

/// Concrete OCRProcessor Implementation
#[derive(Debug)]
struct OCRProcessor;
impl Processor for OCRProcessor {
    fn process(&self, input: &str) -> String {
        format!("OCR extracted text from: {}", input)
    }

    fn info(&self) -> String {
        "OCRProcessor: Extracts text from images".to_string()
    }
}
