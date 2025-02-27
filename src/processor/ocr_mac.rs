#[cfg(target_os = "macos")]
use cidre::{
    cv::{PixelBuf, PixelFormat},
    ns,
    vn::{self, ImageRequestHandler, RecognizeTextRequest},
};
use image::{DynamicImage, GenericImageView};
use std::{ffi::c_void, ptr::null_mut};

#[no_mangle]
#[cfg(target_os = "macos")]
extern "C" fn release_callback(_refcon: *mut c_void, _data_ptr: *const *const c_void) {
    // Do nothing
}

#[cfg(target_os = "macos")]
pub async fn process_ocr_macosx(image: &DynamicImage) -> String {
    cidre::objc::ar_pool(|| {
        let (width, height) = image.dimensions();
        let rgb = image.grayscale().to_luma8();
        let raw_data = rgb.as_raw();

        let width = usize::try_from(width).unwrap();
        let height = usize::try_from(height).unwrap();

        let mut pixel_buf_out = None;

        let pixel_buf = unsafe {
            PixelBuf::create_with_bytes_in(
                width,
                height,
                PixelFormat::ONE_COMPONENT_8,
                raw_data.as_ptr() as *mut c_void,
                width,
                release_callback,
                null_mut(),
                None,
                &mut pixel_buf_out,
                None,
            )
            .to_result_unchecked(pixel_buf_out)
        }
        .unwrap();

        let handler = ImageRequestHandler::with_cv_pixel_buf(&pixel_buf, None).unwrap();
        let mut request = RecognizeTextRequest::new();
        request.set_uses_lang_correction(false);
        request.set_automatically_detects_lang(true);
        let requests = ns::Array::<vn::Request>::from_slice(&[&request]);
        let result = handler.perform(&requests);

        if result.is_err() {
            return "".to_string();
        }

        if let Some(results) = request.results() {
            if !results.is_empty() {
                let mut ocr_text: String = String::new();
                results.iter().for_each(|result| {
                    let observation_result = result.top_candidates(1).get(0).unwrap();
                    let text = observation_result.string();
                    let bounds = result.bounding_box();
                    // Vision's coordinate system has (0,0) at bottom-left, with y going up
                    // To get top-left, we use x and (1 - y) since y increases downward in typical coordinate systems
                    ocr_text.push_str(&format!("({:.2}, {:.2}) ", bounds.origin.x, 1.0 - bounds.origin.y));
                    ocr_text.push_str(text.to_string().as_str());
                    ocr_text.push(' ');
                });
                
                let ocr_text = ocr_text.trim_end().to_string();
                return ocr_text;
            }
        }

        String::from("")
    })
}