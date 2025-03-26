use crate::mp4_pr::mp4_for_each_frame;
use crate::image2text::process_ocr;
use crate::common::get_current_timestamp_str;
use crate::image_utils::should_process_frame_rgb;
use crate::capture::ScreenCaptureConfig;
use crate::capture::spawn_screenshot_task;
use crate::common::ImageData;
use crate::common::ProcessingType;
use tokio::sync::mpsc::channel;
use crate::common::ImageDataCollection;

use anyhow::Result;
use std::{sync::{Arc, Mutex}, path::PathBuf};
use image::DynamicImage;

const THRESHOLD: f32 = 0.05;

async fn load_image_from_path(path: &std::path::PathBuf) -> Result<DynamicImage> {
    image::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to load image from {}: {}", path.display(), e))
}

async fn perform_ocr_and_return_frame_data(image: &DynamicImage) -> Result<ImageData> {
    let text = process_ocr(image).await?;
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

pub async fn run_live_screen_capture_ocr(config: &ScreenCaptureConfig) -> ImageDataCollection {
    log::debug!("Starting capture at {} fps", config.fps);

    let ocr_results = Arc::new(Mutex::new(ImageDataCollection::new()));

    // channel for screenshot capture task
    let (screenshot_tx, mut screenshot_rx) = channel(32);

    // channel for closing the capture task
    let (close_tx, close_rx) = tokio::sync::oneshot::channel();

    let screenshot_task = spawn_screenshot_task(
        config,
        screenshot_tx,
        close_tx
    );

    let ocr_tasks = process_screenshots_with_ocr(
        &mut screenshot_rx, 
        close_rx,
        ocr_results.clone(),
    ).await;

    if let Err(e) = screenshot_task.await {
        log::error!("Screenshot task failed: {:?}", e);
    }

    for (i, task) in ocr_tasks.into_iter().enumerate() {
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

async fn process_screenshots_with_ocr(
    screenshot_rx: &mut tokio::sync::mpsc::Receiver<(u64, DynamicImage)>,
    mut close_rx: tokio::sync::oneshot::Receiver<()>,
    ocr_results: Arc<Mutex<ImageDataCollection>>
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut tasks = Vec::new();

    let mut previous_image: Option<DynamicImage> = None;

    loop {
        tokio::select! {
        Some((frame_number, image)) = screenshot_rx.recv() => {
            log::debug!("Processing frame {} with OCR", frame_number);

            // Clone Arc for the task
            let results_arc = ocr_results.clone();

            // Convert and store the RGB image
            let current_rgb = image.to_rgb8();
            let previous_rgb = previous_image.as_ref().map(|img| img.to_rgb8());

            // Check if images are similar before proceeding
            let should_process = should_process_frame_rgb(
                &current_rgb,
                previous_rgb.as_ref(),  // Get reference to the RGB image
                THRESHOLD
            );

            if !should_process {
                log::debug!("Images similar, skipping OCR for frame {}", frame_number);
                continue;
            }

            // Clone image for the OCR task
            let image_clone = image.clone();

            // Process OCR in a separate task to avoid blocking
            let task = tokio::task::spawn(async move {
                process_ocr_frame(&image_clone, frame_number, &results_arc).await;
            });
            
            tasks.push(task);
            previous_image = Some(image.clone());
        }
        _ = &mut close_rx => {
            log::debug!("Screenshot channel closed, stopping OCR processing");
            break;
        }
        }
    }

    tasks
}

async fn process_ocr_frame(
    image: &DynamicImage,
    frame_number: u64,
    results_arc: &Arc<Mutex<Vec<ImageData>>>
) {
    match crate::image2text::process_ocr(image).await {
        Ok(text) if !text.is_empty() => {
            let timestamp = get_current_timestamp_str();
            let result = ImageData::new(timestamp, frame_number, text, ProcessingType::OCR);
            
            if let Ok(mut results) = results_arc.lock() {
                results.push(result);
            } else {
                log::error!("Failed to lock OCR results mutex");
            }
        }
        Ok(_) => log::debug!("No text detected in frame {}", frame_number),
        Err(e) => log::error!("OCR error on frame {}: {}", frame_number, e),
    }
}