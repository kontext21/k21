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
use super::screen_record;

#[derive(Debug, Clone)]
pub struct OcrResult {
    pub timestamp: SystemTime,
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
    let config = ScreenCaptureConfig {
        fps: 1.0,
        video_chunk_duration: 1,
        stdout: false,
        save_screenshot: false,
        save_video: false,
        max_frames: Some(1),
    };
    run_screen_capture_and_do_ocr(config).await
}

pub async fn run_screen_capture_and_do_ocr(config: ScreenCaptureConfig) -> Vec<OcrResult> {
    log::info!("Starting capture at {} fps", config.fps);
    let monitor_id = get_primary_monitor_id();
    
    // delete old screenshots
    cleanup_old_screenshots();

    let (screenshot_tx, mut screenshot_rx) = channel(512);
    
    // Create shared OCR results list
    let ocr_results = Arc::new(Mutex::new(Vec::<OcrResult>::new()));

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
        config.max_frames.unwrap(), 
        ocr_results.clone(),
    ).await;
    
    // Wait for screenshot capture to complete
    screenshot_task.await.unwrap();
    
    // Wait for all OCR tasks to complete
    for task in ocr_tasks {
        if let Err(e) = task.await {
            log::error!("OCR task failed: {}", e);
        }
    }
    
    let results = ocr_results.lock().unwrap();
    log::info!("Collected {} OCR results", results.len());
    
    results.clone()
}

async fn process_screenshots_with_ocr(
    screenshot_rx: &mut tokio::sync::mpsc::Receiver<(u64, DynamicImage)>,
    max_frames: u64,
    ocr_results: Arc<Mutex<Vec<OcrResult>>>,
) -> Vec<tokio::task::JoinHandle<()>> {
    let mut frame_count = 0;
    let mut tasks = Vec::new();
    
    while frame_count < max_frames {
        if let Some((frame_number, image)) = screenshot_rx.recv().await {
            log::info!("Processing frame {} with OCR", frame_number);
            
            // Clone Arc for the task
            let results_arc = ocr_results.clone();
            let should_collect = true;
                        
            // Process OCR in a separate task to avoid blocking
            let task = tokio::task::spawn(async move {
                // Use the OCR module from k21/src/ocr
                match crate::ocr::process_ocr(&image).await {
                    Ok(text) => {
                        if !text.is_empty() {
                            log::info!("OCR result for frame {}: {}", frame_number, text);
                            
                            // Store the OCR result if collection is enabled
                            if should_collect {
                                let result = OcrResult {
                                    timestamp: SystemTime::now(),
                                    frame_number,
                                    text,
                                };
                                
                                // Lock the mutex and add the result
                                if let Ok(mut results) = results_arc.lock() {
                                    results.push(result);
                                } else {
                                    log::error!("Failed to lock OCR results mutex");
                                }
                            }
                        } else {
                            log::debug!("No text detected in frame {}", frame_number);
                        }
                    },
                    Err(e) => log::error!("OCR error on frame {}: {}", frame_number, e),
                }
                
                log::debug!("OCR completed for frame {}", frame_number);
            });
            
            tasks.push(task);
            frame_count += 1;
        } else {
            break;
        }
    }
    
    tasks
}

pub async fn run_screen_capture(config: ScreenCaptureConfig) {
    log::info!("Starting capture at {} fps", config.fps);

    // setup ctrl-c handler
    // let running = setup_ctrl_c_handler();

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
            while max_frames.is_none() || frame_counter <= max_frames.unwrap() {
                
                let capture_start = Instant::now();
                let image = get_screenshot(monitor_id).await.unwrap();
                if let Err(e) = screenshot_tx.send((frame_counter, image)).await {
                    log::error!("Error: {}", e.to_string());
                    break;
                }
                let capture_duration = capture_start.elapsed();
                frame_counter += 1;

                if let Some(diff) = interval.checked_sub(capture_duration) {
                    log::info!("sleeping for {:?}", diff);
                    tokio::time::sleep(diff).await;
                } else {
                    log::warn!(
                        "Capture took longer than expected: {:?}, will not sleep",
                        capture_duration
                    );
                }
            }
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