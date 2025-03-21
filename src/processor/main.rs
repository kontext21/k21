use clap::Parser;
use image::{DynamicImage, RgbImage};
use k21::image_utils::images_differ_rgb;
use k21::mp4_pr::mp4_for_each_frame;
use k21::image2text::process_ocr;
use k21::logger::init_logger_exe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{self, AsyncReadExt, BufReader};

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

#[tokio::main]
async fn main() {
    init_logger_exe();
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
        let char_counter = Arc::new(AtomicI32::new(0));        
        let start_time = std::time::Instant::now();
        
        mp4_for_each_frame(&cli.mp4.unwrap(), None)
        .await
        .unwrap();
        
        let elapsed = start_time.elapsed();
        log::info!("Total characters: {}", char_counter.load(Ordering::SeqCst));
        log::info!("Time taken: {:.2?}", elapsed);
    } else if cli.stdin {
        let mut stdin: BufReader<io::Stdin> = BufReader::new(io::stdin());
        let mut previous_image: Option<RgbImage> = None; // Store previous frame
        
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
                let image = DynamicImage::ImageRgb8(rgb_image.clone());
                
                // Check image difference if we have a previous frame
                let should_process = if let Some(prev_img) = &previous_image {
                    let diff = images_differ_rgb(&rgb_image, prev_img, 0.05);
                    log::debug!("Images differ: {}", diff);
                    diff
                } else {
                    true // Always process first frame
                };

                if should_process {
                    let ocr_res = process_ocr(&image).await;
                    if let Ok(text) = ocr_res {
                        log::info!("OCR result: {}", text);
                    } else {
                        log::error!("Failed to process OCR: {}", ocr_res.unwrap_err());
                    }
                }

                previous_image = Some(rgb_image);
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
