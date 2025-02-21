use anyhow::{Error, Result};
use clap::Parser;
use glob::glob;
use image::DynamicImage;
use log::LevelFilter;
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::channel;
use xcap::Monitor;
use std::path::Path;

mod ocr;
use crate::ocr::process_ocr;

#[cfg(target_os = "windows")]
mod ocr_win;

#[cfg(target_os = "macos")]
mod ocr_mac;

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
}

pub fn init_logger(name: impl Into<String>) {
    let crate_name = name.into().replace('-', "_");

    env_logger::builder()
        .parse_default_env()
        .filter(Some(&crate_name), LevelFilter::Trace)
        .format(move |f, rec| {
            let now = humantime::format_rfc3339_millis(std::time::SystemTime::now());
            let module = rec.module_path().unwrap_or("<unknown>");
            let line = rec.line().unwrap_or(u32::MIN);
            let level = rec.level();

            writeln!(
                f,
                "[{} {} {} {}:{}] {}",
                level,
                crate_name,
                now,
                module,
                line,
                rec.args()
            )
        })
        .init();
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
    init_logger(env!("CARGO_PKG_NAME"));
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
            let mut frame_counter: u64 = 0;
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

    // main task
    while running.load(Ordering::SeqCst) {
        if let Some((frame_number, image)) = screenshot_rx.recv().await {
            // record the frame
            screen_record.frame(&image);
            log::info!("Frame {}", frame_number);

            // save screenshot to disk
            tokio::task::spawn({
                let image = image.clone();
                async move {
                    let path = format!("screenshot-{}.png", frame_number);
                    let _ = image.save_with_format(&path, image::ImageFormat::Png);
                    log::info!("Saved screenshot to {}", path);
                }
            });

            // ocr it
            let ocr_start = Instant::now();
            let ocr_res = process_ocr(&image).await;
            if let Ok(text) = ocr_res {
                let ocr_duration = ocr_start.elapsed();
                log::info!("OCR took {:?}", ocr_duration);
                log::info!("OCR text: {}", text);
            } else {
                log::error!("Error processing OCR: {:?}", ocr_res.unwrap_err());
            }
        }
    }
    log::info!("Exiting...");
    screenshot_task.await.unwrap();
    screen_record.save(Path::new("output.mp4"), cli.fps as u32);
    running.store(false, Ordering::SeqCst);
    rt.shutdown_timeout(Duration::from_nanos(0));
}
