use crate::mp4_bitstream_converter::*;
use anyhow::{anyhow, Result};
use image::DynamicImage;
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

pub async fn mp4_for_each_frame<P, F>(path: P, f: F) -> Result<()>
where
    P: AsRef<Path>,
    F: Fn(u32, DynamicImage) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
{
    let mut mp4 = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut mp4)?;

    let mut mp4 = mp4::Mp4Reader::read_header(Cursor::new(&mp4), mp4.len() as u64)?;

    let track = mp4
        .tracks()
        .iter()
        .find(|(_, t)| t.media_type().unwrap() == mp4::MediaType::H264)
        .ok_or_else(|| anyhow!("Must exist"))?
        .1;
    let track_id = track.track_id();
    let decoder_options = DecoderConfig::new()
        .debug(true)
        .flush_after_decode(Flush::NoFlush);

    // mp4 spits out length-prefixed NAL units, but openh264 expects start codes
    // the mp4 stream also lacks parameter sets, so we need to add them
    // Mp4BitstreamConverter does this for us
    let mut bitstream_converter = Mp4BitstreamConverter::for_mp4_track(track)?;
    let mut decoder =
        Decoder::with_api_config(openh264::OpenH264API::from_source(), decoder_options).unwrap();

    let yuv_to_image = |yuv: DecodedYUV| -> Result<DynamicImage> {
        let (width, height) = yuv.dimensions();
        let mut rgb = vec![0; width * height * 3];
        yuv.write_rgb8(&mut rgb);
        Ok(DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(width as u32, height as u32, rgb)
                .ok_or(anyhow::format_err!("Failed to create RgbImage"))?,
        ))
    };

    let mut buffer = Vec::new();
    let mut frame_idx = 0u32;
    for i in 1..=track.sample_count() {
        let sample = mp4.read_sample(track_id, i)?;
        let sample = match sample {
            Some(sample) => sample,
            None => continue,
        };

        // convert the packet from mp4 representation to one that openh264 can decode
        bitstream_converter.convert_packet(&sample.bytes, &mut buffer);
        match decoder.decode(&buffer) {
            Ok(Some(yuv)) => {
                f(frame_idx, yuv_to_image(yuv)?).await;
                frame_idx += 1;
            }
            Ok(None) => {
                // decoder is not ready to provide an image
                continue;
            }
            Err(err) => {
                log::error!("error frame {i}: {err}");
            }
        }
    }

    for yuv in decoder.flush_remaining()? {
        f(frame_idx, yuv_to_image(yuv)?).await;
        frame_idx += 1;
    }
    Ok(())
}
