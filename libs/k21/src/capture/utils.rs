use anyhow::Result;
use image::DynamicImage;

use std::time::{Duration, Instant};
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::broadcast::channel;

use crate::capture::screen_record;
use super::screen_record::get_screenshot;
use tokio::sync::watch;
use super::ScreenCaptureConfig;
use chrono::Local;

pub async fn capture(config: ScreenCaptureConfig) -> Result<()> {
    capture_with_stdout(config, false).await
}

pub async fn capture_with_stdout(mut config: ScreenCaptureConfig, stdout: bool) -> Result<()> {
    if config.get_save_video_to().is_some() {
        config.save_video_to = Some(match &config.get_save_video_to() {
            Some(path) => path.to_string(),
            None => std::env::current_dir()?.to_string_lossy().to_string(), 
        });
    }

    log::info!("Starting capture at {} fps", config.get_fps());

    let (screenshot_tx, mut screenshot_rx) = channel(512);
    let (close_tx, close_rx) = watch::channel(false);

    let screenshot_task = spawn_screenshot_task(
        &config,
        screenshot_tx,
        close_tx,
    );

    let _ = handle_captured_frames(
        &config,
        stdout,
        &mut screenshot_rx,
        close_rx,
    ).await;

    log::info!("Exiting...");
    let _ = screenshot_task.await;

    Ok(())
}

pub fn spawn_screenshot_task(
    config: &ScreenCaptureConfig,
    screenshot_tx: tokio::sync::broadcast::Sender<(u64, DynamicImage)>,
    close_tx: tokio::sync::watch::Sender<bool>
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn({
        let interval = Duration::from_secs_f32(1.0 / config.get_fps());
        let total_frames_to_process = config.get_duration() * config.get_fps() as u64;
        let live_capture = config.get_duration() == 0;

        async move {
            let mut frame_counter: u64 = 1;
            while live_capture || frame_counter <= total_frames_to_process {
                let capture_start = Instant::now();
                match get_screenshot().await {
                    Ok(image) => {
                        // Use try_send to avoid blocking if receiver is slow
                        if let Err(e) = screenshot_tx.send((frame_counter, image)) {
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
            let _ = close_tx.send(true);
            log::debug!("Screenshot task completed after {} frames", frame_counter - 1);
        }
    })
}

pub async fn handle_captured_frames(
    config: &ScreenCaptureConfig,
    stdout: bool,
    screenshot_rx: &mut tokio::sync::broadcast::Receiver<(u64, DynamicImage)>,
    close_rx: tokio::sync::watch::Receiver<bool>
) -> Result<()> {
    let screen_record = &mut screen_record::ScreenCapturer::new();
    let mut chunk_number = 0;

    // Handle frames
    save_or_send_captured_frames(
        config,
        stdout,
        screen_record,
        screenshot_rx,
        close_rx,
        &mut chunk_number,
    ).await;

    // Save final video chunk if needed
    if config.get_save_video_to().is_some() && !screen_record.is_buf_empty() {
        save_video_chunk(
            screen_record, 
            &mut chunk_number, 
            config.get_fps(), 
            config.get_save_video_to().as_ref().unwrap()
        );
    }
    
    Ok(())
}

async fn save_or_send_captured_frames(
    config: &ScreenCaptureConfig,
    stdout: bool,
    screen_record: &mut screen_record::ScreenCapturer,
    screenshot_rx: &mut tokio::sync::broadcast::Receiver<(u64, DynamicImage)>,
    mut close_rx: tokio::sync::watch::Receiver<bool>,
    chunk_number: &mut u64,
) {
    let total_fps_in_chunk = config
        .get_video_chunk_duration()
        .map(|duration| config.get_fps() as u64 * duration)
        .unwrap_or(u64::MAX);  // If no chunk duration specified, use max value to prevent chunks

    loop {
        tokio::select! {
            Ok((frame_number, image)) = screenshot_rx.recv() => {
                if stdout {
                    send_frame_to_stdout(frame_number, &image).await;
                }

                // record the frame
                if config.get_save_video_to().is_some() {
                    screen_record.frame(&image);
                    log::info!("frame {}", frame_number);

                    if frame_number % total_fps_in_chunk == 0 {
                        log::info!(
                            "frame {}, total_fps_in_chunk {}",
                            frame_number,
                            total_fps_in_chunk
                        );
                        save_video_chunk(screen_record, chunk_number, config.get_fps(), config.get_save_video_to().as_ref().unwrap());
                    }
                }

                // save screenshot to disk
                if config.get_save_screenshot_to().is_some() {
                    if let Some(output_dir) = &config.get_save_screenshot_to() {
                        save_screenshot(frame_number, image.clone(), output_dir);
                    } else {
                        log::warn!("Screenshot saving enabled but no output directory specified");
                    }
                }
            }

            Ok(_) = close_rx.changed() => {
                if *close_rx.borrow() {
                    log::debug!("Screenshot channel closed, stopping OCR processing");
                    break;
                }
            }
        }
    }
    
    if config.get_save_screenshot_to().is_some() {
        if let Some(output_dir) = &config.get_save_screenshot_to() {
            log::info!("Total screenshots saved in directory: {}", 
                    output_dir);
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

fn save_video_chunk(screen_record: &mut screen_record::ScreenCapturer, chunk_number: &mut u64, fps: f32, output_dir_video: &str) {

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let path = std::path::PathBuf::from(output_dir_video)
            .join(format!("output-{}-{}.mp4", timestamp, chunk_number));
    
    screen_record.save(&path, fps);
    *chunk_number += 1;
}

fn save_screenshot(frame_number: u64, image: DynamicImage, output_dir: &str) {

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let path = std::path::PathBuf::from(output_dir)
        .join(format!("screenshot-{}-{}.png", timestamp, frame_number));

    tokio::task::spawn(async move {
        match image.save_with_format(&path, image::ImageFormat::Png) {
            Ok(_) => log::info!("Saved screenshot to {}", path.display()),
            Err(e) => log::error!("Failed to save screenshot: {}", e),
        }
    });
}