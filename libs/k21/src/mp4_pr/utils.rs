use anyhow::{anyhow, Result};
use image::DynamicImage;
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use std::fs::File;
use std::io::{Cursor, Read};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::path::PathBuf;
use super::bitstream_converter::Mp4BitstreamConverter;
use crate::image_sc::utils::calculate_threshold_exceeded_ratio;
use crate::image2text::ocr::process_ocr;
use base64::{Engine as _, engine::general_purpose::STANDARD};

pub async fn from_file_path_to_mp4_reader(path: &PathBuf) -> Result<std::vec::Vec<u8>>
{
    // File reading timing
    let file_start = Instant::now();
    let mut mp4 = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut mp4)?;
    log::info!("File reading took: {:?}", file_start.elapsed());
    Ok(mp4)
}

pub async fn mp4_for_each_frame(path: &PathBuf, state: Option<Arc<Mutex<ProcessingState>>>) -> Result<Vec<FrameData>>
{
    let mp4_reader = from_file_path_to_mp4_reader(path).await?;
    mp4_for_each_frame_from_reader(&mp4_reader, state).await
}


pub async fn mp4_for_each_frame_from_reader(mp4_data: &[u8], state: Option<Arc<Mutex<ProcessingState>>>) -> Result<Vec<FrameData>>
{
    let total_start = Instant::now();
    let mut results = Vec::new();
    
    log::info!("Processing MP4 frames start of Reader");
    let data = mp4_data.as_ref();
    let mut mp4 = mp4::Mp4Reader::read_header(Cursor::new(data), data.len() as u64)?;

    // Get MP4 duration in seconds
    let duration = mp4.duration();
    let duration_seconds = duration.as_secs_f64();
    log::info!("MP4 duration: {:?}", duration_seconds);

    // Calculate frame step based on track sample count and duration
    let track = mp4
        .tracks()
        .iter()
        .find(|(_, t)| t.media_type().unwrap() == mp4::MediaType::H264)
        .ok_or_else(|| anyhow!("Must exist"))?
        .1;
    let track_id = track.track_id();
    let sample_count = track.sample_count();
    let step = if duration_seconds > 0.0 {
        (sample_count as f64 / duration_seconds).ceil() as usize
    } else {
        1 // Default to processing every frame if duration is zero
    };
    log::info!("Processing with step size: {}, total samples: {}", step, sample_count);

    let decoder_options = DecoderConfig::new()
        .debug(false)
        .flush_after_decode(Flush::NoFlush);

    let mut bitstream_converter = Mp4BitstreamConverter::for_mp4_track(track)?;
    let mut decoder =
        Decoder::with_api_config(openh264::OpenH264API::from_source(), decoder_options).unwrap();

    let mut buffer = Vec::new();
    let mut frame_idx = 0u32;
    let mut previous_image: Option<Vec<u8>> = None;

    for i in 1..=track.sample_count() {
        let sample = mp4.read_sample(track_id, i)?;

        let sample = match sample {
            Some(sample) => sample,
            None => continue,
        };

        bitstream_converter.convert_packet(&sample.bytes, &mut buffer);
        
        match decoder.decode(&buffer) {
            Ok(Some(yuv)) => {
                // Skip frames based on step size (early exit)
                if i % step as u32 != 0 {
                    continue;
                }
                log::info!("Processing frame {}", i);

                let current_luma = yuv_to_luma(&yuv)?;
                let current_luma_image = current_luma.as_slice();
                
                let (width, height) = yuv.dimensions();
                let current_dynamic_image = luma_to_image(current_luma_image, width as u32, height as u32)?;

                let should_process = if let Some(prev_image) = &previous_image {
                    let result = calculate_threshold_exceeded_ratio(
                        current_luma_image, prev_image.as_slice(), 0.05);
                    result > 0.05
                } else {
                    true // Always process the first frame
                };

                if should_process {
                    let result = process_frame_callback(frame_idx, current_dynamic_image.clone(), state.clone()).await;
                    results.push(result);
                    previous_image = Some(current_luma_image.to_vec());
                } else {
                    log::info!("Frame {} not processed", frame_idx);
                }
                frame_idx += 1;
            }
            Ok(None) => continue,
            Err(err) => {
                log::error!("error frame {i}: {err}");
            }
        }
    }


    for yuv in decoder.flush_remaining()? {
        log::info!("Flushing frame {frame_idx}");

        let current_luma = yuv_to_luma(&yuv)?;
        let current_luma_image = current_luma.as_slice();

        let (width, height) = yuv.dimensions();
        let current_dynamic_image = luma_to_image(current_luma_image, width as u32, height as u32)?;

        let should_process = if let Some(prev_image) = &previous_image {
            let result = calculate_threshold_exceeded_ratio(
                current_luma_image, prev_image.as_slice(), 0.05);
            result > 0.05
        } else {
            true // Always process the first frame
        };

        if should_process {
            let result = process_frame_callback(frame_idx, current_dynamic_image.clone(), state.clone()).await;
            results.push(result);
        }
        frame_idx += 1;

        previous_image = Some(current_luma_image.to_vec());
    }

    log::info!("Total execution time: {:?}", total_start.elapsed());

    Ok(results)
}

// Extract the image conversion functions to public methods
pub fn yuv_to_luma(yuv: &DecodedYUV) -> Result<Vec<u8>> {
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
}

pub fn luma_to_image(luma: &[u8], width: u32, height: u32) -> Result<DynamicImage> {
    let luma_img = image::GrayImage::from_raw(width, height, luma.to_vec())
        .ok_or(anyhow::format_err!("Failed to create GrayImage"))?;
    Ok(DynamicImage::ImageLuma8(luma_img))
}

pub async fn process_mp4_frames(mp4_path: &PathBuf) -> Result<Vec<FrameData>> {
    log::info!("Processing MP4 frames");
    let results = mp4_for_each_frame(mp4_path, None)
    .await?;

    Ok(results)
}

// Add Debug derive to FrameData
#[derive(Debug, Clone)]
pub struct FrameData {
    pub timestamp: String,
    pub ocr_text: String,
}

pub type ProcessingState = Vec<FrameData>;

pub async fn process_mp4_reader(mp4_reader: Vec<u8>, state: Option<Arc<Mutex<ProcessingState>>>) -> Result<Vec<FrameData>> {
    log::info!("Processing MP4 frames");
    let results = mp4_for_each_frame_from_reader(&mp4_reader, state.clone()).await?;
    Ok(results)
}

async fn process_frame_callback(frame_idx: u32, image: DynamicImage, state: Option<Arc<Mutex<ProcessingState>>>) -> FrameData {
{
    let state_clone = state.clone();
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    log::info!("Processing frame {}", frame_idx);
    let ocr_res = process_ocr(&image).await;
    let frame_data = FrameData {
        timestamp: timestamp.clone(),
        ocr_text: match &ocr_res {
            Ok(text) => text.clone(),
            Err(_) => String::from("OCR Error"),
        }
    };

    if let Ok(text) = ocr_res {
        log::info!("Frame {} OCR result: {}", frame_idx, text);
        if let Some(state) = &state_clone {
            let mut state = state.lock().unwrap();
            state.push(frame_data.clone());
        }
    } else {
        log::error!(
            "Frame {} Failed to process OCR: {}",
            frame_idx,
            ocr_res.unwrap_err()
        );
    }
    
    frame_data
    }
}

pub async fn process_mp4_from_base64_with_state(
    base64_data: &str,
    state: Arc<Mutex<ProcessingState>>
) -> Result<Vec<FrameData>> {
    log::info!("Processing MP4 from base64 data");

    // Decode base64 to binary data
    let mp4_data = match STANDARD.decode(base64_data) {
        Ok(data) => data,
        Err(err) => {
            log::error!("Failed to decode base64 data: {}", err);
            // Convert to anyhow::Error
            return Err(anyhow::anyhow!("Failed to decode base64 data: {}", err));
        }
    };
    
    log::info!("Successfully decoded base64 data, size: {} bytes", mp4_data.len());
    
    // Process the decoded MP4 data
    process_mp4_reader(mp4_data, Some(state)).await
}

pub async fn process_mp4_from_base64(base64_data: &str) -> Result<Vec<FrameData>> {
    log::info!("Processing MP4 from base64 data");
    
    // Decode base64 to binary data
    let mp4_data = match STANDARD.decode(base64_data) {
        Ok(data) => data,
        Err(err) => {
            log::error!("Failed to decode base64 data: {}", err);
            // Use a standard error type that implements Error + Send + Sync
            return Err(anyhow::anyhow!("Failed to decode base64 data: {}", err));
        }
    };
    
    log::info!("Successfully decoded base64 data, size: {} bytes", mp4_data.len());
    
    // Process the decoded MP4 data
    process_mp4_reader(mp4_data, None).await
}