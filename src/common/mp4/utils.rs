use anyhow::{anyhow, Result};
use image::DynamicImage;
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Instant;
use std::path::PathBuf;
use super::bitstream_converter::Mp4BitstreamConverter;
use crate::common::image_sc::utils::calculate_threshold_exceeded_ratio;
use crate::common::ocr::process_ocr;

pub async fn mp4_for_each_frame<P, F>(path: P, f: F) -> Result<()>
where
    P: AsRef<Path>,
    F: Fn(u32, DynamicImage) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
{
    let total_start = Instant::now();
    
    // File reading timing
    let file_start = Instant::now();
    let mut mp4 = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut mp4)?;
    log::info!("File reading took: {:?}", file_start.elapsed());

    // MP4 header parsing timing
    let header_start = Instant::now();
    let mut mp4 = mp4::Mp4Reader::read_header(Cursor::new(&mp4), mp4.len() as u64)?;
    log::info!("Header parsing took: {:?}", header_start.elapsed());

    // Track finding and decoder setup timing
    let setup_start = Instant::now();
    let track = mp4
        .tracks()
        .iter()
        .find(|(_, t)| t.media_type().unwrap() == mp4::MediaType::H264)
        .ok_or_else(|| anyhow!("Must exist"))?
        .1;
    let track_id = track.track_id();
    let decoder_options = DecoderConfig::new()
        .debug(false)
        .flush_after_decode(Flush::NoFlush);

    let mut bitstream_converter = Mp4BitstreamConverter::for_mp4_track(track)?;
    let mut decoder =
        Decoder::with_api_config(openh264::OpenH264API::from_source(), decoder_options).unwrap();
    log::info!("Setup and initialization took: {:?}", setup_start.elapsed());

    let yuv_to_luma = |yuv: &DecodedYUV| -> Result<Vec<u8>> {
        let (width, height) = yuv.dimensions();
        let stride = yuv.strides().0; // Get Y plane stride
        
        // Create a new buffer for the luma data with correct dimensions
        let mut luma_data = Vec::with_capacity(width * height);
        
        // Copy data from Y plane, accounting for stride if needed
        for y in 0..height {
            let row_start = y * stride;
            luma_data.extend_from_slice(&yuv.y()[row_start..row_start + width]);
        }
        
        Ok(luma_data)
    };
    
    // Add a new function to create DynamicImage directly from luma data
    let luma_to_image = |luma: &[u8], width: u32, height: u32| -> Result<DynamicImage> {
        let luma_img = image::GrayImage::from_raw(width, height, luma.to_vec())
            .ok_or(anyhow::format_err!("Failed to create GrayImage"))?;
        Ok(DynamicImage::ImageLuma8(luma_img))
    };

    let mut buffer = Vec::new();
    let mut frame_idx = 0u32;
    let frame_processing_start = Instant::now();
    let mut decode_time = std::time::Duration::new(0, 0);
    let mut convert_time = std::time::Duration::new(0, 0);
    let mut callback_time = std::time::Duration::new(0, 0);
    let mut read_sample_time = std::time::Duration::new(0, 0);
    let mut yuv_to_luma_time = std::time::Duration::new(0, 0);
    let mut luma_to_image_time = std::time::Duration::new(0, 0);
    let mut diff_check_time = std::time::Duration::new(0, 0);
    let mut previous_image: Option<Vec<u8>> = None;
    let mut image_diff_time = std::time::Duration::new(0, 0);

    for i in 1..=track.sample_count() {
        // Read sample timing
        let read_sample_start = Instant::now();
        let sample = mp4.read_sample(track_id, i)?;
        read_sample_time += read_sample_start.elapsed();

        let sample = match sample {
            Some(sample) => sample,
            None => continue,
        };

        // Conversion timing
        let convert_start = Instant::now();
        bitstream_converter.convert_packet(&sample.bytes, &mut buffer);
        convert_time += convert_start.elapsed();

        // Decoding timing
        let decode_start = Instant::now();
        match decoder.decode(&buffer) {
            Ok(Some(yuv)) => {
                decode_time += decode_start.elapsed();

                // Time YUV to luma conversion
                let yuv_to_luma_start = Instant::now();
                let current_luma = yuv_to_luma(&yuv)?;
                let current_luma_image = current_luma.as_slice();
                yuv_to_luma_time += yuv_to_luma_start.elapsed();
                
                // Time YUV to image conversion
                let luma_to_image_start = Instant::now();
                // Create DynamicImage directly from luma data instead of full YUV conversion
                let (width, height) = yuv.dimensions();
                let current_dynamic_image = luma_to_image(current_luma_image, width as u32, height as u32)?;
                // If RGB is needed, you can convert: current_dynamic_image.to_rgb8()
                luma_to_image_time += luma_to_image_start.elapsed();

                // Add timing for image difference check
                let diff_check_start = Instant::now();
                let should_process = if let Some(prev_image) = &previous_image {
                    // Time actual image difference calculation
                    let diff_start = Instant::now();
                    let result = calculate_threshold_exceeded_ratio(
                        current_luma_image, prev_image.as_slice(), 0.05);
                    image_diff_time += diff_start.elapsed();
                    result > 0.05
                } else {
                    true // Always process the first frame
                };
                diff_check_time += diff_check_start.elapsed();

                log::info!("YUV to luma conversion time: {:?}", yuv_to_luma_time);
                log::info!("Luma to image conversion time: {:?}", luma_to_image_time);
                log::info!("Image difference calculation time: {:?}", image_diff_time);

                let callback_start = Instant::now();
                if should_process {
                    // Callback timing
                    f(frame_idx, current_dynamic_image.clone()).await;
                } else {
                    log::info!("Frame {} not processed", frame_idx);
                }
                callback_time += callback_start.elapsed();
                frame_idx += 1;

                // Update previous image
                previous_image = Some(current_luma_image.to_vec());
            }
            Ok(None) => {
                decode_time += decode_start.elapsed();
                continue;
            }
            Err(err) => {
                decode_time += decode_start.elapsed();
                log::error!("error frame {i}: {err}");
            }
        }
    }

    // Flush timing
    let flush_start = Instant::now();
    for yuv in decoder.flush_remaining()? {
        log::info!("Flushing frame {frame_idx}");
        
        // Time YUV to luma conversion
        let yuv_to_luma_start = Instant::now();
        let current_luma = yuv_to_luma(&yuv)?;
        let current_luma_image = current_luma.as_slice();
        yuv_to_luma_time += yuv_to_luma_start.elapsed();
        
        // Time YUV to image conversion
        let luma_to_image_start = Instant::now();
        let (width, height) = yuv.dimensions();
        let current_dynamic_image = luma_to_image(current_luma_image, width as u32, height as u32)?;
        luma_to_image_time += luma_to_image_start.elapsed();

        // Add timing for image difference check
        let diff_check_start = Instant::now();
        let should_process = if let Some(prev_image) = &previous_image {
            // Time actual image difference calculation
            let diff_start = Instant::now();
            let result = calculate_threshold_exceeded_ratio(
                current_luma_image, prev_image.as_slice(), 0.05);
            image_diff_time += diff_start.elapsed();
            result > 0.05
        } else {
            true // Always process the first frame
        };
        diff_check_time += diff_check_start.elapsed();

        let callback_start = Instant::now();
        if should_process {
            // Callback timing
            f(frame_idx, current_dynamic_image.clone()).await;
        }
        callback_time += callback_start.elapsed();
        frame_idx += 1;

        // Update previous image
        previous_image = Some(current_luma_image.to_vec());
    }
    let flush_time = flush_start.elapsed();

    // Log final timings
    log::info!("Frame processing breakdown:");
    log::info!("  - Total read sample time: {:?}", read_sample_time);
    log::info!("  - Total conversion time: {:?}", convert_time);
    log::info!("  - Total decode time: {:?}", decode_time);
    log::info!("  - Total YUV to luma conversion time: {:?}", yuv_to_luma_time);
    log::info!("  - Total YUV to image conversion time: {:?}", luma_to_image_time);
    log::info!("  - Total diff check time: {:?}", diff_check_time);
    log::info!("  - Total callback time: {:?}", callback_time);
    log::info!("  - Flush time: {:?}", flush_time);
    log::info!("  - Total frame processing time: {:?}", frame_processing_start.elapsed());
    log::info!("Total execution time: {:?}", total_start.elapsed());
    
    Ok(())
}

pub async fn process_mp4_frames(mp4_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Processing MP4 frames");
    mp4_for_each_frame(mp4_path, move |frame_idx, image| {
        Box::pin(async move {
            log::info!("Processing frame {}", frame_idx);
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
    .await?;
    
    Ok(())
}
