use crate::image2text::process_image_vision;
use crate::image2text::OcrConfig;
use crate::image_utils::image_to_base64;
use crate::mp4_pr::mp4_for_each_frame;
use crate::image2text::process_ocr;
use crate::common::get_current_timestamp_str;
use crate::image_utils::should_process_frame_rgb;
use crate::capture::ScreenCaptureConfig;
use crate::capture::spawn_screenshot_task;
use crate::common::ImageData;
use crate::common::ProcessingType;
use tokio::sync::broadcast::channel;
use crate::common::ImageDataCollection;
use crate::capture::handle_captured_frames;
use anyhow::Result;
use std::{sync::{Arc, Mutex}, path::PathBuf};
use image::DynamicImage;

use tokio::sync::watch;

use super::ProcessorConfig;


const THRESHOLD: f32 = 0.05;

async fn load_image_from_path(path: &std::path::PathBuf) -> Result<DynamicImage> {
    image::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to load image from {}: {}", path.display(), e))
}

async fn perform_ocr_and_return_frame_data(image: &DynamicImage) -> Result<ImageData> {
    let text = process_ocr(image, &OcrConfig::default()).await?;
    let image_data = ImageData::new(get_current_timestamp_str(), 0, text, ProcessingType::OCR);
    Ok(image_data)
}

pub async fn perform_ocr_on_image_from_path(path: &str) -> Result<ImageData> {
    let path_buf: PathBuf = std::path::PathBuf::from(path);
    let image: DynamicImage = load_image_from_path(&path_buf).await?;
    perform_ocr_and_return_frame_data(&image).await
}

pub async fn perform_ocr_on_video_path(path: &str) -> Result<ImageDataCollection> {
    let path_buf: PathBuf = std::path::PathBuf::from(path);
    let results: ImageDataCollection = mp4_for_each_frame(&path_buf, None).await?;
    Ok(results)
}

pub async fn run_live_screen_capture_ocr(screen_capture_config: &ScreenCaptureConfig, processor_config: &ProcessorConfig) -> ImageDataCollection {
    log::debug!("Starting capture at {} fps", screen_capture_config.fps);

    let ocr_results = Arc::new(Mutex::new(ImageDataCollection::new()));

    // channel for screenshot capture task
    let (screenshot_tx, mut screenshot_rx) = channel(512);
    let mut screenshot_rx_clone = screenshot_rx.resubscribe();

    // channel for closing the capture task
    let (close_tx, close_rx) = watch::channel(false);
    let close_rx_clone = close_rx.clone();


    let screenshot_task = spawn_screenshot_task(
        screen_capture_config,
        screenshot_tx,
        close_tx
    );

    let ocr_tasks = process_screenshots_with_method(
        &processor_config,
        &mut screenshot_rx, 
        close_rx,
        ocr_results.clone(),
    );

    let handle_captured_frames_task = handle_captured_frames(
        screen_capture_config,
        false,
        &mut screenshot_rx_clone,
        close_rx_clone,
    );

    let (_, ocr_result) = tokio::join!(
        handle_captured_frames_task,
        ocr_tasks
    );

    if let Err(e) = screenshot_task.await {
        log::error!("Screenshot task failed: {:?}", e);
    }

    for (i, task) in ocr_result.into_iter().enumerate() {
        if let Err(e) = task.await {
            log::error!("OCR task {} failed: {:?}", i, e);
        }
    }

    let results = {
        let guard = ocr_results.lock().unwrap();
        guard.clone()
    };

    log::debug!("Collected {} OCR results", results.len());

    results
}

async fn process_image_with_selected_method(
    processor_config: &ProcessorConfig,
    image: &DynamicImage,
    frame_number: u64,
    results_arc: &Arc<Mutex<Vec<ImageData>>>
) {
    let processing_type = &processor_config.processing_type;
    
    let result = match processing_type {
        ProcessingType::OCR => {
            let ocr_config = processor_config.ocr_config.as_ref().unwrap();
            match process_ocr(image, ocr_config).await {
                Ok(text) if !text.is_empty() => Some(text),
                Ok(_) => {
                    log::debug!("No text detected in frame {}", frame_number);
                    None
                },
                Err(e) => {
                    log::error!("OCR error on frame {}: {}", frame_number, e);
                    None
                }
            }
        },
        ProcessingType::Vision => {
            let vision_config = processor_config.vision_config.as_ref().unwrap();
            let result = process_image_vision(
                image_to_base64(image).unwrap(), 
                &vision_config
            ).await;
            Some(result)
        }
    };

    if let Some(text) = result {
        let timestamp = get_current_timestamp_str();
        let processing_type_clone = processing_type.clone();
        let image_data = ImageData::new(timestamp, frame_number, text, processing_type_clone);
        
        if let Ok(mut results) = results_arc.lock() {
            results.push(image_data);
        } else {
            log::error!("Failed to lock results mutex");
        }
    }
}

async fn process_screenshots_with_method(
    processor_config: &ProcessorConfig,
    screenshot_rx: &mut tokio::sync::broadcast::Receiver<(u64, DynamicImage)>,
    mut close_rx: tokio::sync::watch::Receiver<bool>,
    ocr_results: Arc<Mutex<ImageDataCollection>>
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut tasks = Vec::new();
    let mut previous_image: Option<DynamicImage> = None;

    loop {
        tokio::select! {
            Ok((frame_number, image)) = screenshot_rx.recv() => {
                log::debug!("Processing frame {} with {:?}", frame_number, processor_config.processing_type);

                let results_arc = ocr_results.clone();
                let current_rgb = image.to_rgb8();
                let previous_rgb = previous_image.as_ref().map(|img| img.to_rgb8());

                let should_process = should_process_frame_rgb(
                    &current_rgb,
                    previous_rgb.as_ref(),
                    THRESHOLD
                );

                if !should_process {
                    log::debug!("Images similar, skipping frame {}", frame_number);
                    continue;
                }

                let image_clone = image.clone();
                let processor_config = processor_config.clone();

                let task = tokio::task::spawn(async move {
                    process_image_with_selected_method(
                        &processor_config,
                        &image_clone,
                        frame_number,
                        &results_arc
                    ).await;
                });
                
                tasks.push(task);
                previous_image = Some(image.clone());
            }
            Ok(_) = close_rx.changed() => {
                if *close_rx.borrow() {
                    log::debug!("Screenshot channel closed, stopping processing");
                    break;
                }
            }
        }
    }

    tasks
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use tempfile::tempdir;

//     #[tokio::test]
//     async fn test_live_screen_capture_ocr() -> Result<()> {
//         // Create a temporary directory for screenshots
//         let temp_dir = tempdir()?;
//         let temp_path = temp_dir.path().to_string_lossy().to_string();

//         // Setup test configuration
//         let config = ScreenCaptureConfig {
//             fps: 1.0,
//             video_chunk_duration_in_seconds: 1,
//             save_screenshot: true,  // Enable screenshot saving
//             save_video: false,
//             record_length_in_seconds: 2,
//             output_dir_screenshot: Some(temp_path),  // Use temp directory
//             output_dir_video: None,
//         };

//         let processor_config = ProcessorConfig {
//             processing_type: ProcessingType::OCR,
//             vision_config: VisionConfig::new(),
//         };
//         // Run OCR capture
//         let results = run_live_screen_capture_ocr(&config, &processor_config).await;

//         // Print results for debugging
//         println!("Total OCR results: {}", results.len());
        
//         // Verify screenshots were saved
//         let entries = std::fs::read_dir(temp_dir.path())?
//             .filter_map(|e| e.ok())
//             .collect::<Vec<_>>();
        
//         println!("Screenshots saved: {}", entries.len());

//         // Verify results
//         assert!(!results.is_empty(), "Should have captured some OCR results");
//         assert!(!entries.is_empty(), "Should have saved some screenshots");
        
//         // Verify each result
//         for result in results {
//             assert!(!result.timestamp().is_empty(), "Timestamp should not be empty");
//             assert!(result.frame_number() > 0, "Frame number should be positive");
//             assert_eq!(result.processing_type(), &ProcessingType::OCR);
//         }

//         // temp_dir will be automatically cleaned up when it goes out of scope
//         Ok(())
//     }
// }