// Standard library imports
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Result};
use openh264::decoder::{Decoder, DecoderConfig, Flush};

use super::bitstream_converter::Mp4BitstreamConverter;
use crate::common::{decode_base64, get_results_from_state, ImageDataCollection};
use crate::image_utils::convert_yuv_to_dynamic_image;
use crate::image_utils::should_process_frame_luma;

use crate::common::to_verified_path;
use crate::process::{process_image, ProcessorConfig};
// Module-level constant
const THRESHOLD_VALUE: f32 = 0.05;

pub async fn process_mp4_buffer_path(
    path: &PathBuf,
    config: &ProcessorConfig, 
    state: Arc<Mutex<ImageDataCollection>>
) -> Result<()>
{
    let mp4_data = from_file_path_to_mp4_reader(path).await?;
    process_mp4_buffer(&mp4_data, config, state).await?;
    Ok(())
}

pub async fn process_mp4(
    file_path: String,
    config: &ProcessorConfig,
) -> Result<ImageDataCollection> {
    let state = Arc::new(Mutex::new(ImageDataCollection::new()));
    let state_clone = state.clone();
    process_mp4_string_file_path(file_path, config, state_clone).await?;
    let results = get_results_from_state(state).await?;
    Ok(results)
}

pub async fn process_mp4_string_file_path(
    file_path: String,
    config: &ProcessorConfig,
    state: Arc<Mutex<ImageDataCollection>>
) -> Result<()> {
    let path = PathBuf::from(&file_path);
    // let path = to_verified_path(&file_path)?;
    process_mp4_buffer_path(&path, config, state).await?;
    Ok(())
}

pub async fn process_mp4_from_base64_with_state(
    base64_data: &str,
    config: &ProcessorConfig,
    state: Arc<Mutex<ImageDataCollection>>
) -> Result<()> {
    let mp4_data = decode_base64(base64_data)?;    
    process_mp4_buffer(&mp4_data, config, state.clone()).await?;
    Ok(())
}

pub async fn process_mp4_buffer(mp4_data: &[u8], config: &ProcessorConfig, state: Arc<Mutex<ImageDataCollection>>) -> Result<()>
{
    let total_start = Instant::now();
    
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

                let (current_dynamic_image, current_luma) = convert_yuv_to_dynamic_image(&yuv)?;
                
                if should_process_frame_luma(&current_luma, previous_image.as_deref(), THRESHOLD_VALUE) {
                    process_image(config, &current_dynamic_image, frame_idx as u64, state.clone()).await;
                    previous_image = Some(current_luma.to_vec());

                } else {
                    log::info!("Frame {} skipped - no significant changes", frame_idx);
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

        let (current_dynamic_image, current_luma) = convert_yuv_to_dynamic_image(&yuv)?;

        if should_process_frame_luma(&current_luma, previous_image.as_deref(), THRESHOLD_VALUE) {
            process_image(config, &current_dynamic_image, frame_idx as u64, state.clone()).await;
            previous_image = Some(current_luma.to_vec());
        } else {
            log::info!("Frame {} skipped - no significant changes", frame_idx);
        }
        frame_idx += 1;
    }

    log::info!("Total execution time: {:?}", total_start.elapsed());
    Ok(())
}


async fn from_file_path_to_mp4_reader(path: &PathBuf) -> Result<std::vec::Vec<u8>>
{
    let mut mp4 = Vec::new();
    let mut file = File::open(&path)?;
    file.read_to_end(&mut mp4)?;
    Ok(mp4)
}
