use k21::capture::{self, ScreenCaptureConfig};
use std::time::Duration;

#[cfg(test)]
mod capture_tests {
    use super::*;

    #[tokio::test]
    async fn test_screen_capture() {
        // Create a minimal capture configuration
        let config = ScreenCaptureConfig::new(
            Some(1.0),
            Some(1),
            None,
            None,
            None
        );

        // Test the capture function
        let result = capture::capture(config).await;
        assert!(result.is_ok(), "Screen capture should succeed");
    }

    #[tokio::test]
    async fn test_screen_capture_with_timeout() {
        let config = ScreenCaptureConfig::new(
            Some(1.0),
            Some(2),
            None,
            None,
            None
        );

        // Use tokio timeout to ensure the capture doesn't run too long
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            capture::capture(config)
        ).await;

        assert!(result.is_ok(), "Capture should complete within timeout");
        assert!(result.unwrap().is_ok(), "Screen capture should succeed");
    }
}

mod upload_tests {
    use k21::upload::process_upload;
    use k21::process::ProcessorConfig;

    #[tokio::test]
    async fn test_upload_png() {
        let config = ProcessorConfig::default();
        
        // Get current working directory and join with the file name
        let current_dir = std::env::current_dir()
            .expect("Failed to get current directory");
        let test_file_path = current_dir.join("tests").join("screenshot-1.png");

        println!("Current directory: {:?}", current_dir);
        println!("Full file path: {:?}", test_file_path);
        println!("File exists: {}", test_file_path.exists());
        
        let result = process_upload(test_file_path.to_string_lossy().to_string(), &config).await;

        match &result {
            Ok(results) => println!("PNG processing results: {} items", results.len()),
            Err(e) => println!("PNG upload failed with error: {:?}", e),
        }

        assert!(result.is_ok(), "PNG upload should succeed");
        if let Ok(results) = result {
            assert!(!results.is_empty(), "Should have processing results");
        }
    }

    #[tokio::test]
    async fn test_upload_mp4() {
        // Get current working directory and join withdd the file name
        let current_dir = std::env::current_dir()
            .expect("Failed to get current directory");
        let test_file_path = current_dir.join("tests").join("output-0.mp4");

        println!("Current directory: {:?}", current_dir);
        println!("Full file path: {:?}", test_file_path);
        println!("File exists: {}", test_file_path.exists());

        let config = ProcessorConfig::default();
        let result = process_upload(test_file_path.to_string_lossy().to_string(), &config).await;
        
        match &result {
            Ok(results) => println!("Upload succeeded with {} results", results.len()),
            Err(e) => println!("Upload failed with error: {:?}", e),
        }
        
        assert!(result.is_ok(), "MP4 upload should succeed");
    }
}