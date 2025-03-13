use anyhow::Result;
use glob::glob;
use image::DynamicImage;
use std::fs;
use std::path::{Path, PathBuf};
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
    pub video_chunk_duration_in_seconds: u64,
    pub stdout: bool,
    pub save_screenshot: bool,
    pub save_video: bool,
    pub max_frames: Option<u64>,
    pub record_length_in_seconds: u64,
    pub output_dir_video: Option<PathBuf>,
    pub output_dir_screenshot: Option<PathBuf>,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        Self {
            fps: 1.0,
            video_chunk_duration_in_seconds: 60,
            stdout: false,
            save_screenshot: false,
            save_video: false,
            max_frames: None,
            record_length_in_seconds: 1,
            output_dir_video: None,
            output_dir_screenshot: None,
        }
    }
}

impl ScreenCaptureConfig {
    /// Computes the maximum number of frames based on fps and recording length
    /// and updates the max_frames field
    pub fn compute_max_frames(&mut self) {
        if self.max_frames.is_none() {
            self.max_frames = Some(((self.fps as f64) * (self.record_length_in_seconds as f64)).ceil() as u64);
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

    let mut config = ScreenCaptureConfig {
        max_frames: Some(1),
        ..Default::default()
    };
    config.compute_max_frames(); //ugly fix for now

    run_screen_capture_and_do_ocr(config).await
}

pub async fn run_screen_capture_and_do_ocr(mut config: ScreenCaptureConfig) -> Vec<OcrResult> {
    log::debug!("Starting capture at {} fps", config.fps);
    config.compute_max_frames(); //ugly fix for now
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

pub async fn record_screen_capture_images(
    fps: Option<f32>,
    duration: Option<u64>,
    output_dir_screenshot: Option<&String>,
) -> Result<()> {
    // Convert relative path to absolute path if provided
    let absolute_path = match output_dir_screenshot {
        Some(path) => to_verified_path(path)?,
        None => return Err(anyhow::anyhow!("No output directory provided for video recording")),
    };

    record(fps, duration, None, None, Some(true), None, Some(&absolute_path)).await;
    Ok(())
}

pub async fn record_screen_capture_video(
    fps: Option<f32>,
    duration: Option<u64>,
    video_chunk_duration_in_seconds: Option<u64>,
    output_dir_video: Option<&String>,
) -> Result<()> {


    let absolute_path = match output_dir_video {
        Some(path) => to_verified_path(path)?,
        None => return Err(anyhow::anyhow!("No output directory provided for video recording")),
    };

    log::info!("Absolute path: {}", absolute_path.display());

    record(fps, duration, Some(true), video_chunk_duration_in_seconds, None, Some(&absolute_path), None).await;
    Ok(())
}

pub async fn record(
    fps: Option<f32>,
    duration: Option<u64>,
    dump_video: Option<bool>,
    video_chunk_duration_in_seconds: Option<u64>,
    dump_screenshot: Option<bool>,
    output_dir_video: Option<&Path>,
    output_dir_screenshot: Option<&Path>,
) -> () {
    let mut config = ScreenCaptureConfig {
        fps: fps.unwrap_or(1.0),
        video_chunk_duration_in_seconds: video_chunk_duration_in_seconds.unwrap_or(60),
        output_dir_video: output_dir_video.map(|p| p.to_path_buf()),
        output_dir_screenshot: output_dir_screenshot.map(|p| p.to_path_buf()),
        save_screenshot: dump_screenshot.unwrap_or(false),
        save_video: dump_video.unwrap_or(false),
        record_length_in_seconds: duration.unwrap_or(1),
        ..Default::default()
    };
    config.compute_max_frames(); //ugly fix for now

    run_screen_capture(config).await;
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
    let total_fps_in_chunk = config.fps as u64 * config.video_chunk_duration_in_seconds;
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
        save_video_chunk(&mut screen_record, &mut chunk_number, config.fps, config.output_dir_video.as_ref().unwrap());
    }
}

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
    let mut screenshot_count = 0;

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
                    save_video_chunk(screen_record, chunk_number, config.fps, config.output_dir_video.as_ref().unwrap());
                }
            }

            // save screenshot to disk
            if config.save_screenshot {
                if let Some(output_dir) = &config.output_dir_screenshot {
                    save_screenshot(frame_number, image.clone(), output_dir);
                    screenshot_count += 1;
                    log::info!("Saved screenshot #{} to directory: {}", 
                              screenshot_count, output_dir.display());
                } else {
                    log::warn!("Screenshot saving enabled but no output directory specified");
                }
            }
        }
    }
    
    if config.save_screenshot {
        if let Some(output_dir) = &config.output_dir_screenshot {
            log::info!("Total screenshots saved: {} in directory: {}", 
                      screenshot_count, output_dir.display());
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

fn save_video_chunk(screen_record: &mut screen_record::ScreenRecorder, chunk_number: &mut u64, fps: f32, output_dir_video: &Path) {
    // save video chunk to disk with unique name using the provided output directory
    let path = output_dir_video.join(format!("output-{}.mp4", chunk_number));
    screen_record.save(&path, fps);
    *chunk_number += 1;
}

fn save_screenshot(frame_number: u64, image: DynamicImage, output_dir: &Path) {
    let output_dir = output_dir.to_owned();
    tokio::task::spawn(async move {
        let path = output_dir.join(format!("screenshot-{}.png", frame_number));
        match image.save_with_format(&path, image::ImageFormat::Png) {
            Ok(_) => log::info!("Saved screenshot to {}", path.display()),
            Err(e) => log::error!("Failed to save screenshot: {}", e),
        }
    });
}

pub fn to_absolute_path(path: &String) -> Result<PathBuf> {
    let path_buf = PathBuf::from(path);

    if path_buf.is_file() {
        return Err(anyhow::anyhow!("Path is a file, expected a directory: {}", path_buf.display()));
    }
    
    if path_buf.is_absolute() {
        return Ok(path_buf);
    }

    if path_buf.is_dir() {
        match std::env::current_dir() {
            Ok(current_dir) => {
                return Ok(current_dir.join(path_buf));
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to get current directory: {}", e));
            }
        }
    }

    let has_parent_refs = path.contains("../") || path.contains("..\\") || path == ".." || path.ends_with("/..");

    // Convert relative path to absolute
    match std::env::current_dir() {
        Ok(current_dir) => {
            let absolute_path = if has_parent_refs {
                // Use canonicalize to resolve parent directory references
                match current_dir.join(&path_buf).canonicalize() {
                    Ok(canonical_path) => canonical_path,
                    Err(e) => {
                        log::warn!("Failed to canonicalize path with parent refs: {}, using simple join", e);
                        current_dir.join(path_buf)
                    }
                }
            } else {
                // Simple join for paths without parent references
                current_dir.join(path_buf)
            };
            Ok(absolute_path)
        },
        Err(e) => {
            log::warn!("Failed to get current directory: {}, using path as is", e);
            Ok(path_buf)
        }
    }
}

pub fn ensure_path_exists(path: PathBuf) -> Result<PathBuf> {
    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow::anyhow!("Path does not exist: {}", path.display()))
    }
}

pub fn to_verified_path(path: &String) -> Result<PathBuf> {
    let absolute_path = to_absolute_path(path)?;
    ensure_path_exists(absolute_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_screen_capture_and_do_ocr_default() {
        // Initialize logger for tests if needed
        let _ = env_logger::try_init();

        // Run the default screen capture with OCR
        let results = run_screen_capture_and_do_ocr_default().await;

        // Verify basic properties of the results
        assert!(!results.is_empty(), "Should capture at least one frame");

        // Check the first result's properties
        if let Some(first_result) = results.first() {
            // Verify timestamp format (should be like "2024-03-21 10:30:45.123")
            assert!(first_result.timestamp.len() >= 19, "Timestamp should be properly formatted");

            // Verify frame number starts at 1
            assert!(first_result.frame_number >= 1, "Frame number should start at 1");

            // // Text might be empty if no text was detected, but should be a valid string
            // assert!(first_result.text.is_string(), "Text should be a valid string");
        }
    }

    #[tokio::test]
    async fn test_record_screen_capture_video() {
        // Create a temporary directory for test output
        let temp_path = "./test-video".to_string();
        
        // Record a very short video (0.5 second) at 2 fps
        let result = record_screen_capture_video(
            Some(1.0),           // fps
            Some(13),            // duration in seconds
            Some(5),             // chunk duration
            Some(&temp_path),    // output directory
        ).await;
        
        // Handle the result
        assert!(result.is_ok(), "Video recording should succeed");
        
        // Rest of the test...
    }

    #[tokio::test]
    async fn test_record_screen_capture_images() {
        // Create a temporary directory for test output
        let temp_path = "/Users/ferzu/k21/libs/k21/".to_string();
        
        // Record a very short video (0.5 second) at 2 fps
        let result = record_screen_capture_images(
            Some(1.0),           // fps
            Some(10),            // duration in seconds
            Some(&temp_path),    // output directory
        ).await;
        
        // Handle the result
        assert!(result.is_ok(), "Image recording should succeed");
    
        // Verify that 10 images were created
        let path = to_verified_path(&temp_path).unwrap();
        let screenshot_pattern = path.join("screenshot-*.png");
        let screenshot_count = glob(screenshot_pattern.to_str().unwrap())
            .expect("Failed to read screenshot pattern")
            .count();
        
        assert_eq!(screenshot_count, 10, "Expected 10 screenshots to be created");
    }

    #[tokio::test]
    async fn test_record_screen_capture_images_nonexistent_dir() {
        // Use a path that definitely doesn't exist
        let nonexistent_path = "/path/that/definitely/does/not/exist/12345abcde".to_string();
        
        // Attempt to record with a nonexistent output directory
        let result = record_screen_capture_images(
            Some(1.0),                // fps
            Some(10),                 // duration in seconds
            Some(&nonexistent_path),   // Nonexistent output directory
        ).await;
        
        // Verify that the operation failed with an error about the directory
        assert!(result.is_err(), "Recording should fail with nonexistent output directory");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("directory") || error_msg.contains("path"),
            "Error should mention directory or path issues: {}", error_msg
        );
    }
}