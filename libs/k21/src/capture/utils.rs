use anyhow::Result;
use image::DynamicImage;
use std::path::Path;

use std::time::{Duration, Instant};
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc::channel;
use xcap::Monitor;

use crate::common::get_primary_monitor_id;
use crate::common::to_verified_path;
use crate::capture::screen_record;

use super::ScreenCaptureConfig;

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

pub async fn capture(
    fps: Option<f32>,
    duration: Option<u64>,
    dump_video: Option<bool>,
    video_chunk_duration_in_seconds: Option<u64>,
    dump_screenshot: Option<bool>,
    output_dir_video: Option<&Path>,
    output_dir_screenshot: Option<&Path>,
) -> Result<()> {
    let config = ScreenCaptureConfig {
        fps: fps.unwrap_or(1.0),
        video_chunk_duration_in_seconds: video_chunk_duration_in_seconds.unwrap_or(60),
        output_dir_video: output_dir_video.map(|p| p.to_path_buf()),
        output_dir_screenshot: output_dir_screenshot.map(|p| p.to_path_buf()),
        save_screenshot: dump_screenshot.unwrap_or(false),
        save_video: dump_video.unwrap_or(false),
        record_length_in_seconds: duration.unwrap_or(1),
        ..Default::default()
    };

    let _ = run_screen_capture(config).await;
    Ok(())
}

pub async fn run_screen_capture(mut config: ScreenCaptureConfig) -> Result<()> {
    if config.save_video {
        config.output_dir_video = Some(match &config.output_dir_video {
            Some(path) => to_verified_path(path.to_str().unwrap())?,
            None => std::env::current_dir()?, 
        });
    }

    log::info!("Starting capture at {} fps", config.fps);
    let monitor_id = get_primary_monitor_id();
    log::info!("Monitor ID: {}", monitor_id);

    let (screenshot_tx, mut screenshot_rx) = channel(512);

    let total_frames = {
        let frames = config.compute_total_frames();
        if frames == 0 { None } else { Some(frames) }
    };

    // Start screenshot capture task
    let screenshot_task = spawn_screenshot_task(
        config.fps,
        total_frames,
        monitor_id,
        screenshot_tx,
    );

    let mut screen_record = screen_record::ScreenCapturer::new(monitor_id);
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
    Ok(())
}

pub fn spawn_screenshot_task(
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
    screen_record: &mut screen_record::ScreenCapturer,
    total_fps_in_chunk: u64,
    chunk_number: &mut u64,
) {
    let mut exit_condition: bool = true;
    let mut screenshot_count = 0;
    let total_frames = config.compute_total_frames();

    while exit_condition {
        if let Some((frame_number, image)) = screenshot_rx.recv().await {

            if config.record_length_in_seconds > 0 && frame_number >= total_frames {
                log::info!("Reached maximum frame count ({}), stopping capture", &total_frames);
                exit_condition = false;
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

fn save_video_chunk(screen_record: &mut screen_record::ScreenCapturer, chunk_number: &mut u64, fps: f32, output_dir_video: &Path) {
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