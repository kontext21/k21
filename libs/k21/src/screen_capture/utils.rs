use anyhow::Result;
use glob::glob;
use image::DynamicImage;
use std::fs;
use std::path::Path;
// use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc::channel;
use xcap::Monitor;
use crate::image_sc::utils::images_differ;

use super::screen_record;
use chrono;

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub timestamp: String,
    pub frame_number: u64,
    pub text: String,
}

pub struct ScreenCaptureConfig {
    pub fps: f32,
    pub video_chunk_duration: u64,
    pub stdout: bool,
    pub save_screenshot: bool,
    pub save_video: bool,
    pub max_frames: Option<u64>,
    pub record_length_in_seconds: u64,
}

impl ScreenCaptureConfig {
    /// Computes the maximum number of frames based on fps and recording length
    pub fn compute_max_frames(&self) -> u64 {
        match self.max_frames {
            Some(frames) => frames,
            None => (self.fps as f64 * self.record_length_in_seconds as f64).ceil() as u64
        }
    }
}

pub async fn get_screenshot(monitor_id: u32) -> Result<DynamicImage> {
    let image = std::thread::spawn(move || -> Result<DynamicImage> {
        let monitor = Monitor::all()
            .unwrap()
            .into_iter()
            .find(|m| m.id() == monitor_id)
            .ok_or_else(|| anyhow::anyhow!("Monitor not found"))?;
        let image = monitor
            .capture_image()
            .map_err(anyhow::Error::from)
            .map(DynamicImage::ImageRgba8)?;
        Ok(image)
    })
    .join()
    .unwrap()?;
    Ok(image)
}

pub async fn run_screen_capture_and_do_ocr_default() -> Vec<OcrResult> {
    // Reduce logging frequency to avoid stdout contention
    log::debug!("Starting default screen capture with OCR");
    
    let config = ScreenCaptureConfig {
        fps: 1.0,
        video_chunk_duration: 1,
        stdout: false,
        save_screenshot: false,
        save_video: false,
        max_frames: None,
        record_length_in_seconds: 1,
    };
    config.compute_max_frames(); //ugly fix for now
    
    run_screen_capture_and_do_ocr(config).await
}

pub async fn run_screen_capture_and_do_ocr(config: ScreenCaptureConfig) -> Vec<OcrResult> {
    log::debug!("Starting capture at {} fps", config.fps);
    let monitor_id = get_primary_monitor_id();
    
    // delete old screenshots
    // cleanup_old_screenshots();

    // Create shared OCR results list
    let ocr_results = Arc::new(Mutex::new(Vec::<OcrResult>::new()));

    // Start screenshot capture task with a bounded channel to prevent overwhelming
    let (screenshot_tx, mut screenshot_rx) = channel(32); // Reduced buffer size
    
    // Start screenshot capture task
    let screenshot_task = spawn_screenshot_task(
        config.fps,
        config.max_frames,
        monitor_id,
        screenshot_tx,
    );

    // Process screenshots with OCR
    let ocr_tasks = process_screenshots_with_ocr(
        &mut screenshot_rx, 
        config.max_frames.unwrap_or(1), // Provide default if None
        ocr_results.clone(),
    ).await;
    
    // Wait for screenshot capture to complete
    if let Err(e) = screenshot_task.await {
        log::error!("Screenshot task failed: {:?}", e);
    }
    
    // Wait for all OCR tasks to complete
    for (i, task) in ocr_tasks.into_iter().enumerate() {
        if let Err(e) = task.await {
            log::error!("OCR task {} failed: {:?}", i, e);
        }
    }
    
    // Use a scope to ensure the mutex is released
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
            
            // Check if images are similar before proceeding
            let should_process = if let Some(prev_img) = &previous_image {
                images_differ(&image.to_rgb8(), &prev_img.to_rgb8(), 0.1)
            } else {
                true
            };
            
            if !should_process {
                log::debug!("Images similar, skipping OCR for frame {}", frame_number);
                continue;
            }
            
            // Clone image for the OCR task
            let image_clone = image.clone();
            
            // Process OCR in a separate task to avoid blocking
            let task = tokio::task::spawn(async move {
                // Use the OCR module from k21/src/ocr
                match crate::ocr::process_ocr(&image_clone).await {
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

pub async fn run_screen_capture(config: ScreenCaptureConfig) {
    log::info!("Starting capture at {} fps", config.fps);

    // get primary monitor
    let monitor_id = get_primary_monitor_id();
    log::warn!("Monitor ID: {}", monitor_id);

    // delete old screenshots
    cleanup_old_screenshots();

    let (screenshot_tx, mut screenshot_rx) = channel(512);

    // Start screenshot capture task
    let screenshot_task = spawn_screenshot_task(
        config.fps,
        config.max_frames,
        monitor_id,
        screenshot_tx,
    );

    let mut screen_record = screen_record::ScreenRecorder::new(monitor_id);
    let total_fps_in_chunk = config.fps as u64 * config.video_chunk_duration;
    let mut chunk_number = 0;

    process_captured_frames(
        &config,
        &mut screenshot_rx,
        &mut screen_record,
        total_fps_in_chunk,
        &mut chunk_number,
    ).await;

    log::info!("Exiting...");
    screenshot_task.await.unwrap();
    if config.save_video {
        save_video_chunk(&mut screen_record, &mut chunk_number, config.fps);
    }
}

// fn setup_ctrl_c_handler() -> Arc<AtomicBool> {
//     let running = Arc::new(AtomicBool::new(true));
//     let r = running.clone();

//     ctrlc::set_handler(move || {
//         log::warn!("Ctrl-C received, stopping...");
//         r.store(false, Ordering::SeqCst);
//     })
//     .expect("Error setting Ctrl-C handler");

//     running
// }

fn get_primary_monitor_id() -> u32 {
    Monitor::all()
        .unwrap()
        .iter()
        .find(|m| m.is_primary())
        .unwrap()
        .id()
}

fn cleanup_old_screenshots() {
    for entry in glob("screenshot-*.png").unwrap().filter_map(Result::ok) {
        if fs::remove_file(&entry).is_ok() {
            //log::info!("Removed file {}", entry.display());
        }
    }
}

fn spawn_screenshot_task(
    fps: f32,
    max_frames: Option<u64>,
    monitor_id: u32,
    screenshot_tx: tokio::sync::mpsc::Sender<(u64, DynamicImage)>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn({
        let interval = Duration::from_secs_f32(1.0 / fps);
        async move {
            let mut frame_counter: u64 = 1;
            while max_frames.map_or(true, |max| frame_counter <= max) {
                
                let capture_start = Instant::now();
                
                match get_screenshot(monitor_id).await {
                    Ok(image) => {
                        // Use try_send to avoid blocking if receiver is slow
                        if let Err(e) = screenshot_tx.send((frame_counter, image)).await {
                            log::error!("Failed to send screenshot: {}", e);
                            break;
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to capture screenshot: {}", e);
                        // Continue to next iteration instead of breaking
                        tokio::time::sleep(interval).await;
                        continue;
                    }
                }
                
                let capture_duration = capture_start.elapsed();
                frame_counter += 1;

                if let Some(diff) = interval.checked_sub(capture_duration) {
                    log::debug!("Sleeping for {:?}", diff);
                    tokio::time::sleep(diff).await;
                } else {
                    log::warn!(
                        "Capture took longer than expected: {:?}, will not sleep",
                        capture_duration
                    );
                }
            }
            
            log::debug!("Screenshot task completed after {} frames", frame_counter - 1);
        }
    })
}

async fn process_captured_frames(
    config: &ScreenCaptureConfig,
    screenshot_rx: &mut tokio::sync::mpsc::Receiver<(u64, DynamicImage)>,
    screen_record: &mut screen_record::ScreenRecorder,
    total_fps_in_chunk: u64,
    chunk_number: &mut u64,
) {
    let mut exit_condition: bool = true;
    
    while exit_condition {
        if let Some((frame_number, image)) = screenshot_rx.recv().await {
            log::info!("frame_number {}", frame_number);
            
            // Check if we've reached max frames
            if let Some(max_frames) = config.max_frames {
                if frame_number >= max_frames {
                    log::info!("Reached maximum frame count ({}), stopping capture", max_frames);
                    exit_condition = false;
                }
            }
            
            if config.stdout {
                send_frame_to_stdout(frame_number, &image).await;
            }

            // record the frame
            if config.save_video {
                screen_record.frame(&image);
                log::info!("frame {}", frame_number);

                if frame_number % total_fps_in_chunk == 0 {
                    log::info!(
                        "frame {}, total_fps_in_chunk {}",
                        frame_number,
                        total_fps_in_chunk
                    );
                    save_video_chunk(screen_record, chunk_number, config.fps);
                }
            }

            // save screenshot to disk
            if config.save_screenshot {
                save_screenshot(frame_number, image.clone());
            }
        }
    }
}

async fn send_frame_to_stdout(frame_number: u64, image: &DynamicImage) {
    let rgb = image.to_rgb8();
    let data = rgb.as_raw();
    let mut stdout = io::stdout();

    log::info!("Sending frame {}, len {}", frame_number, data.len());

    // send frame & size of raw image data
    stdout.write_all(&frame_number.to_le_bytes()).await.unwrap(); // Send frame number
    stdout.write_all(&rgb.width().to_le_bytes()).await.unwrap(); // Send width
    stdout.write_all(&rgb.height().to_le_bytes()).await.unwrap(); // Send height
    stdout.write_all(&data.len().to_le_bytes()).await.unwrap(); // Send data size
    stdout.write_all(&data).await.unwrap(); // Send frame data
    stdout.flush().await.unwrap(); // Ensure it's sent
}

fn save_video_chunk(screen_record: &mut screen_record::ScreenRecorder, chunk_number: &mut u64, fps: f32) {
    // save video chunk to disk with unique name
    let path = format!("output-{}.mp4", chunk_number);
    screen_record.save(Path::new(&path), fps);
    *chunk_number += 1;
}

fn save_screenshot(frame_number: u64, image: DynamicImage) {
    tokio::task::spawn(async move {
        let path = format!("screenshot-{}.png", frame_number);
        let _ = image.save_with_format(&path, image::ImageFormat::Png);
        log::info!("Saved screenshot to {}", path);
    });
} 