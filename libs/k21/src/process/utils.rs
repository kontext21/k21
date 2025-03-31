use crate::common::get_results_from_state;
use crate::image2text::process_image_vision;
use crate::image_utils::image_to_base64;
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
use std::sync::{Arc, Mutex};
use image::DynamicImage;

use tokio::sync::watch;

use super::ProcessorConfig;

const THRESHOLD: f32 = 0.05;

pub async fn capture_and_process_screen(screen_capture_config: &ScreenCaptureConfig, processor_config: &ProcessorConfig) -> ImageDataCollection {
    log::debug!("Starting capture at {} fps", screen_capture_config.get_fps());

    let results_arc = Arc::new(Mutex::new(ImageDataCollection::new()));

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

    let image2text_tasks = process_image2text_screenshots_task(
        &processor_config,
        &mut screenshot_rx, 
        close_rx,
        results_arc.clone(),
    );

    let handle_captured_frames_task = handle_captured_frames(
        screen_capture_config,
        false,
        &mut screenshot_rx_clone,
        close_rx_clone,
    );

    let (_, ocr_result) = tokio::join!(
        handle_captured_frames_task,
        image2text_tasks
    );

    if let Err(e) = screenshot_task.await {
        log::error!("Screenshot task failed: {:?}", e);
    }

    for (i, task) in ocr_result.into_iter().enumerate() {
        if let Err(e) = task.await {
            log::error!("Image2Text task {} failed: {:?}", i, e);
        }
    }

    let results = get_results_from_state(results_arc).await.unwrap();
    log::debug!("Collected {} Image2Text results", results.len());

    results
}

pub async fn process_image_by_processing_type(
    image: &DynamicImage,
    processor_config: &ProcessorConfig,
    frame_number: u64,
) -> Option<String> {
    match &processor_config.processing_type {
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
    }
}

pub async fn process_image(
    processor_config: &ProcessorConfig,
    image: &DynamicImage,
    frame_number: u64,
    results_arc: Arc<Mutex<ImageDataCollection>>
) {
    let processing_type = &processor_config.processing_type;
    
    let result = process_image_by_processing_type(image, processor_config, frame_number).await;

    if let Some(text) = result {
        let timestamp: String = get_current_timestamp_str();
        let processing_type_clone = processing_type.clone();
        let image_data: ImageData = ImageData::new(timestamp, frame_number, text, processing_type_clone);
        
        if let Ok(mut results) = results_arc.lock() {
            results.push(image_data);
        } else {
            log::error!("Failed to lock results mutex");
        }
    }
}

async fn process_image2text_screenshots_task(
    processor_config: &ProcessorConfig,
    screenshot_rx: &mut tokio::sync::broadcast::Receiver<(u64, DynamicImage)>,
    mut close_rx: tokio::sync::watch::Receiver<bool>,
    results_arc: Arc<Mutex<ImageDataCollection>>
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut tasks = Vec::new();
    let mut previous_image: Option<DynamicImage> = None;

    loop {
        tokio::select! {
            Ok((frame_number, image)) = screenshot_rx.recv() => {
                log::debug!("Processing frame {} with {:?}", frame_number, processor_config.processing_type);

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
                let results_arc_clone = results_arc.clone();

                let task = tokio::task::spawn(async move {
                    process_image(
                        &processor_config,
                        &image_clone,
                        frame_number,
                        results_arc_clone
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