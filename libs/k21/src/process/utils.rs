use crate::mp4_pr::utils::{FrameData, mp4_for_each_frame};
use crate::image2text::process_ocr;
use crate::common::get_current_timestamp_str;
use crate::image_utils::should_process_frame_rgb;
use crate::common::get_primary_monitor_id;
use crate::capture::ScreenCaptureConfig;
use crate::capture::spawn_screenshot_task;
use crate::capture::OcrResult;
use tokio::sync::mpsc::channel;


use anyhow::Result;
use std::{sync::{Arc, Mutex}, time::SystemTime, path::PathBuf};
use image::DynamicImage;

const THRESHOLD: f32 = 0.05;

async fn load_image_from_path(path: &std::path::PathBuf) -> Result<DynamicImage> {
    image::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to load image from {}: {}", path.display(), e))
}

async fn perform_ocr_and_return_frame_data(image: &DynamicImage) -> Result<FrameData> {
    let text = process_ocr(image).await?;
    let frame_data = FrameData {
        timestamp: get_current_timestamp_str(),
        ocr_text: text,
    };
    Ok(frame_data)
}

pub async fn perform_ocr_on_image_from_path(path: &str) -> Result<FrameData> {
    let path_buf: PathBuf = std::path::PathBuf::from(path);
    let image: DynamicImage = load_image_from_path(&path_buf).await?;
    perform_ocr_and_return_frame_data(&image).await
}

pub async fn perform_ocr_on_video_path(path: &str) -> Result<Vec<FrameData>> {
    let path_buf: PathBuf = std::path::PathBuf::from(path);
    let results: Vec<FrameData> = mp4_for_each_frame(&path_buf, None).await?;
    Ok(results)
}

pub async fn run_live_screen_capture_ocr(config: &ScreenCaptureConfig) -> Vec<OcrResult> {
    log::debug!("Starting capture at {} fps", config.fps);
    let monitor_id = get_primary_monitor_id();
    let total_frames = config.compute_total_frames();

    let ocr_results = Arc::new(Mutex::new(Vec::<OcrResult>::new()));

    let (screenshot_tx, mut screenshot_rx) = channel(32); // Reduced buffer size

    let screenshot_task = spawn_screenshot_task(
        config.fps,
        Some(total_frames),
        monitor_id,
        screenshot_tx,
    );

    let ocr_tasks = process_screenshots_with_ocr(
        &mut screenshot_rx, 
        total_frames,
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
    max_frames: u64,
    ocr_results: Arc<Mutex<Vec<OcrResult>>>,
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut frame_count = 0;
    let mut tasks = Vec::new();

    let mut previous_image: Option<DynamicImage> = None;

    while frame_count <= max_frames {
        if let Some((frame_number, image)) = screenshot_rx.recv().await {
            frame_count = frame_number;
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
                // Use the OCR module from k21/src/ocr
                match crate::image2text::process_ocr(&image_clone).await {
                    Ok(text) => {
                        if !text.is_empty() {
                            log::debug!("OCR result for frame {}: {}", frame_number, text);
                            
                            // Format current time as a human-readable string
                            let now = SystemTime::now();
                            let datetime = chrono::DateTime::<chrono::Local>::from(now);
                            let timestamp = datetime.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                            
                            let result = OcrResult {
                                timestamp,
                                frame_number,
                                text,
                            };
                            
                            // Use a scope to minimize lock duration
                            if let Ok(mut results) = results_arc.lock() {
                                results.push(result);
                            } else {
                                log::error!("Failed to lock OCR results mutex");
                            }
                        } else {
                            log::debug!("No text detected in frame {}", frame_number);
                        }
                    },
                    Err(e) => log::error!("OCR error on frame {}: {}", frame_number, e),
                }
            });
            
            tasks.push(task);
            previous_image = Some(image.clone());
        } else {
            log::debug!("Screenshot channel closed, stopping OCR processing");
            break;
        }
    }
    
    tasks
}
