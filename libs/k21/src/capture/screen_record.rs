use image::DynamicImage;
use openh264::encoder::Encoder;
use std::path::Path;
use xcap::Monitor;
use anyhow::Result;
pub struct ScreenCapturer {
    encoder: Encoder,
    buf: Vec<u8>,
    frame_count: u32,
}

impl ScreenCapturer {
    pub fn new() -> Self {
        Self {
            encoder: Encoder::new().unwrap(),
            buf: Vec::new(),
            frame_count: 0,
        }
    }

    pub fn is_buf_empty(&self) -> bool {
        self.buf.len() == 0
    }

    pub fn frame(&mut self, image: &DynamicImage) {
        use openh264::formats::*;
        let frame = image.to_rgb8();
        // Convert RGB into YUV.
        let rgb_source = RgbSliceU8::new(
            frame.as_raw(),
            (frame.width() as usize, frame.height() as usize),
        );
        let yuv = YUVBuffer::from_rgb_source(rgb_source);

        // Forces the encoder to emit an intra frame (I-frame, "keyframe") for the next encoded frame
        self.encoder.force_intra_frame();

        // Encode YUV into H.264.
        let bitstream = self.encoder.encode(&yuv).unwrap();
        bitstream.write_vec(&mut self.buf);

        log::info!(
            "Encoded frame {}, buf size {}",
            self.frame_count,
            self.buf.len()
        );

        self.frame_count += 1;
    }

    pub fn save(&mut self, p: &Path, fps: f32) {
        use minimp4::Mp4Muxer;
        use std::io::{Cursor, Read, Seek, SeekFrom};

        let monitor = get_primary_monitor();

        let mut video_buffer = Cursor::new(Vec::new());
        let mut mp4muxer = Mp4Muxer::new(&mut video_buffer);
        mp4muxer.init_video(
            monitor.width() as i32,
            monitor.height() as i32,
            false,
            "Screen capturer",
        );

        mp4muxer.write_video_with_fps(&self.buf, fps as u32);
        mp4muxer.close();

        video_buffer.seek(SeekFrom::Start(0)).unwrap();
        let mut video_bytes = Vec::new();
        video_buffer.read_to_end(&mut video_bytes).unwrap();

        std::fs::write(p, &video_bytes).unwrap();

        log::info!("Saved {} frames to {}", self.frame_count, p.display());

        // reset
        self.encoder = Encoder::new().unwrap();
        self.buf.clear();
        self.frame_count = 0;
    }
}

fn get_monitor(monitor_id: u32) -> Monitor {
    Monitor::all()
        .unwrap()
        .into_iter()
        .find(|m| m.id() == monitor_id)
        .ok_or_else(|| anyhow::anyhow!("Monitor not found"))
        .unwrap()
}

fn get_primary_monitor_id() -> u32 {
    Monitor::all()
        .unwrap()
        .iter()
        .find(|m| m.is_primary())
        .unwrap()
        .id()
}

pub fn get_primary_monitor() -> Monitor {
    get_monitor(get_primary_monitor_id())
}

// pub async fn get_screenshot() -> Result<DynamicImage> {
//     let image = std::thread::spawn(move || -> Result<DynamicImage> {
//         let monitor = get_primary_monitor();
//         let image = monitor
//             .capture_image()
//             .map_err(anyhow::Error::from)
//             .map(DynamicImage::ImageRgba8)?;
//         Ok(image)
//     })
//     .join()
//     .unwrap()?;
//     Ok(image)
// }

pub async fn get_screenshot(quality: u8) -> Result<DynamicImage> {
    let quality = quality.clamp(1, 100);

    let image = std::thread::spawn(move || -> Result<DynamicImage> {
        let monitor = get_primary_monitor();
        let original = monitor
            .capture_image()
            .map_err(anyhow::Error::from)
            .map(DynamicImage::ImageRgba8)?;

        // Convert quality (1-100) to scale factor
        // 100 = original size (scale by 1.0)
        // 50 = half size (scale by 0.5)
        // 1 = smallest (scale by 0.01)
        let scale = quality as f32 / 100.0;
        
        let scaled = original.resize(
            (original.width() as f32 * scale) as u32,
            (original.height() as f32 * scale) as u32,
            image::imageops::FilterType::Nearest
        );

        Ok(scaled)
    })
    .join()
    .unwrap()?;

    Ok(image)
}