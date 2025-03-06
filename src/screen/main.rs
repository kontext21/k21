use anyhow::{Error, Result};
use clap::Parser;
use glob::glob;
use image::DynamicImage;
use mylib::logger::utils::init_logger_exe;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc::channel;
use xcap::Monitor;

mod screen_record;

#[derive(Parser)]
#[command(version, about = "A CLI tool to handle screen refresh rates", long_about = None)]
struct Cli {
    #[arg(
        long,
        help = "Screen refresh rate in fps (frames per second)",
        default_value_t = 1.0
    )]
    fps: f32,
    #[arg(
        long,
        help = "Duration of each video chunk in seconds",
        default_value_t = 60
    )]
    video_chunk_duration: u64,
    #[arg(
        long,
        help = "Dump image to stdout (for processor)",
        default_value_t = false
    )]
    stdout: bool,
    #[arg(
        long,
        help = "Save screenshot to disk", 
        default_value_t = false
    )]
    save_screenshot: bool,
    #[arg(
        long, 
        help = "Save video to disk", 
        default_value_t = false
    )]
    save_video: bool,
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
            .map_err(Error::from)
            .map(DynamicImage::ImageRgba8)?;
        Ok(image)
    })
    .join()
    .unwrap()?;
    Ok(image)
}

#[tokio::main]
async fn main() {
    init_logger_exe();

    let cli = Cli::parse();
    log::info!("Starting capture at {} fps", cli.fps);

    // init tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _ = rt.enter();

    // setup ctrl-c handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        log::warn!("Ctrl-C received, stopping...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // get primary monitor
    let monitor_id = Monitor::all()
        .unwrap()
        .iter()
        .find(|m| m.is_primary())
        .unwrap()
        .id();

    log::warn!("Monitor ID: {}", monitor_id);

    // delete old screenshots
    for entry in glob("screenshot-*.png").unwrap().filter_map(Result::ok) {
        if fs::remove_file(&entry).is_ok() {
            //log::info!("Removed file {}", entry.display());
        }
    }

    let (screenshot_tx, mut screenshot_rx) = channel(512);

    // this task will capture screenshots at the specified rate
    // and send them to the main task
    let screenshot_task = tokio::task::spawn({
        let running = running.clone();
        let interval = Duration::from_secs_f32(1.0 / cli.fps);
        async move {
            let mut frame_counter: u64 = 1;
            while running.load(Ordering::SeqCst) {
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
    });

    let mut screen_record = screen_record::ScreenRecorder::new(monitor_id);
    let total_fps_in_chunk = cli.fps as u64 * cli.video_chunk_duration;
    let mut chunk_number = 0;

    let mut save_chunk = |screen_record: &mut screen_record::ScreenRecorder| {
        // save video chunk to disk with unique name
        let path = format!("output-{}.mp4", chunk_number);
        screen_record.save(Path::new(&path), cli.fps as u32);
        chunk_number += 1;
    };

    // main task
    while running.load(Ordering::SeqCst) {
        if let Some((frame_number, image)) = screenshot_rx.recv().await {
            // send screenshot to stdout (processor)
            if cli.stdout {
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

            // record the frame
            if cli.save_video {
                screen_record.frame(&image);

                log::info!("frame {}", frame_number);

                if frame_number % total_fps_in_chunk == 0 {
                    log::info!(
                        "frame {}, total_fps_in_chunk {}",
                        frame_number,
                        total_fps_in_chunk
                    );
                    save_chunk(&mut screen_record);
                }
            }

            // save screenshot to disk
            if cli.save_screenshot {
                tokio::task::spawn({
                    let image = image.clone();
                    async move {
                        let path = format!("screenshot-{}.png", frame_number);
                        let _ = image.save_with_format(&path, image::ImageFormat::Png);
                        log::info!("Saved screenshot to {}", path);
                    }
                });
            }
        }
    }
    log::info!("Exiting...");
    screenshot_task.await.unwrap();
    if cli.save_video {
        save_chunk(&mut screen_record);
    }
    running.store(false, Ordering::SeqCst);
    rt.shutdown_timeout(Duration::from_nanos(0));
}
