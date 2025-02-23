use clap::Parser;
use image::DynamicImage;
use log::LevelFilter;
use mp4::mp4_for_each_frame;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{self, AsyncReadExt, BufReader};

mod mp4;
mod mp4_bitstream_converter;

mod ocr;
use crate::ocr::process_ocr;

#[cfg(target_os = "windows")]
mod ocr_win;

#[cfg(target_os = "macos")]
mod ocr_mac;

#[derive(Parser)]
#[command(version, about = "A CLI tool to OCR image/video", long_about = None)]
struct Cli {
    #[arg(
        long,
        help = "input file in image (png, jpeg, gif, webp, tiff, bmp, ico, hdr, etc) format"
    )]
    image: Option<PathBuf>,
    #[arg(long, help = "input file in MP4 format")]
    mp4: Option<PathBuf>,
    #[arg(long, help = "get image from stdin (from screen)")]
    stdin: bool,
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

#[tokio::main]
async fn main() {
    init_logger(
        env::current_exe()
            .unwrap()
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap(),
    );
    let cli = Cli::parse();

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

    if cli.image.is_some() {
        let path = cli.image.unwrap();
        let image = image::open(&path);
        if let Ok(image) = image {
            let ocr_res = process_ocr(&image).await;
            if let Ok(text) = ocr_res {
                log::info!("OCR result: {}", text);
            } else {
                log::error!("Failed to process OCR: {}", ocr_res.unwrap_err());
            }
        } else {
            log::error!("Failed to open image: {:?}", image.err());
        }
    } else if cli.mp4.is_some() {
        mp4_for_each_frame(&cli.mp4.unwrap(), |frame_idx, image| {
            Box::pin(async move {
                let ocr_res = process_ocr(&image).await;
                if let Ok(text) = ocr_res {
                    log::info!("Frame {} OCR result: {}", frame_idx, text);
                } else {
                    log::error!(
                        "Frame {} Failed to process OCR: {}",
                        frame_idx,
                        ocr_res.unwrap_err()
                    );
                }
            })
        })
        .await
        .unwrap();
    } else if cli.stdin {
        let mut stdin = BufReader::new(io::stdin()); // Buffered stdin
        loop {
            // Read the frame number (assume it's a u64, 8 bytes)
            let mut frame_number_bytes = [0u8; 8];
            if stdin.read_exact(&mut frame_number_bytes).await.is_err() {
                break; // Exit on EOF
            }
            let frame_number = u64::from_le_bytes(frame_number_bytes); // Convert bytes to u32

            // read width and height
            let mut width_bytes = [0u8; 4];
            if stdin.read_exact(&mut width_bytes).await.is_err() {
                break;
            }
            let width = u32::from_le_bytes(width_bytes);

            let mut height_bytes = [0u8; 4];
            if stdin.read_exact(&mut height_bytes).await.is_err() {
                break;
            }
            let height = u32::from_le_bytes(height_bytes);

            // Read the data size (assume it's a usize, 8 bytes)
            let mut size_bytes = [0u8; 8];
            if stdin.read_exact(&mut size_bytes).await.is_err() {
                break;
            }
            let data_size = usize::from_le_bytes(size_bytes);

            // Read the binary frame data (Vec<u8>)
            let mut buffer = vec![0u8; data_size];
            if stdin.read_exact(&mut buffer[..data_size]).await.is_err() {
                break;
            }

            log::info!("Received frame {}, len {}", frame_number, data_size);

            let rgb_image = image::RgbImage::from_raw(width, height, buffer);
            if let Some(rgb_image) = rgb_image {
                let image = DynamicImage::ImageRgb8(rgb_image);
                let ocr_res = process_ocr(&image).await;
                if let Ok(text) = ocr_res {
                    log::info!("OCR result: {}", text);
                } else {
                    log::error!("Failed to process OCR: {}", ocr_res.unwrap_err());
                }
            } else {
                log::error!("Failed to open image");
            }
        }
    }

    // main task
    // while running.load(Ordering::SeqCst) {

    // }
    log::info!("Exiting...");
    running.store(false, Ordering::SeqCst);
    rt.shutdown_timeout(Duration::from_nanos(0));
}
