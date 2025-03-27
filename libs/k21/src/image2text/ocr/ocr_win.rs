use anyhow::Result;
use image::DynamicImage;

use super::types::OcrConfig;


#[cfg(target_os = "windows")]
pub async fn process_ocr_windows(img: &DynamicImage, config: &OcrConfig) -> Result<String> {
    use std::io::Cursor;
    use windows::{
        Graphics::Imaging::BitmapDecoder,
        Media::Ocr::OcrEngine,
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    };

    let (width, height) = img.dimensions();
    let mut img_buffer = Vec::new();
    img.write_to(&mut Cursor::new(&mut img_buffer), image::ImageFormat::Png)
        .map_err(|err| anyhow::anyhow!("Error processing image: {}", err))?;

    let inmem_stream = InMemoryRandomAccessStream::new()?;
    let data_handler = DataWriter::CreateDataWriter(&inmem_stream)?;
    data_handler.WriteBytes(&img_buffer)?;
    data_handler.StoreAsync()?.get()?;
    data_handler.FlushAsync()?.get()?;
    inmem_stream.Seek(0)?;

    let img_decoder =
        BitmapDecoder::CreateWithIdAsync(BitmapDecoder::PngDecoderId()?, &inmem_stream)?.get()?;
    let soft_bitmap = img_decoder.GetSoftwareBitmapAsync()?.get()?;
    let text_engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
    let extracted_text = text_engine.RecognizeAsync(&soft_bitmap)?.get()?;

    if config.bounding_boxes.unwrap_or(false) {
        let mut result = Vec::new();
        let lines = extracted_text.Lines()?;
        
        for line in lines {
            if let Ok(rect) = line.GetBoundingRect()? {
                // Normalize coordinates to 0-1 range
                let x = rect.X as f32 / width as f32;
                let y = rect.Y as f32 / height as f32;
                
                result.push(format!("({:.2}, {:.2}) {}", x, y, line.Text()?));
            } else {
                result.push(line.Text()?.to_string());
            }
        }
        Ok(result.join(" "))
    } else {
        Ok(extracted_text.Text()?.to_string())
    }
}
