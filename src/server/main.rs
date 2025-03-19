use axum::{
    routing::{get, post},
    Router,
    response::IntoResponse,
    http::StatusCode,
    Json,
    extract::DefaultBodyLimit,
};
use k21::mp4_pr::process_mp4_frames;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};
use mp4::Mp4Reader;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use k21::common::{ImageData, ImageDataCollection};
// Or import the entire module

// Add this function to initialize the logger
#[tokio::main]
async fn main() {
    // Initialize the logger
    init_logger_exe();

    log::info!("Starting server...");

    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        .route("/health", get(|| async { "healthy" }))
        .route("/process-video-path", post(process_video_path))
        .route("/upload", post(upload))
        .route("/process-video-base64", post(process_video_base64))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024)); // 1GB limit for testing

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = SocketAddr::from(([0, 0, 0, 0], port.parse().unwrap()));

    log::info!("Attempting to bind to port {}", port);

    let listener = TcpListener::bind(addr).await.unwrap();
    log::info!("Successfully bound to http://{}", addr);

    // Spawn a task to log server health every second
    tokio::spawn(async {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            log::info!("Server health check: OK");
        }
    });

    log::info!("Starting server...");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct VideoPathRequest {
    file_path: String,
}

#[derive(Serialize)]
struct ProcessResponse {
    message: String,
    success: bool,
}

async fn process_video_path(Json(payload): Json<VideoPathRequest>) -> impl IntoResponse {
    log::info!("0. Frames starting to be processed");
    let path = std::path::PathBuf::from(&payload.file_path);
    
    // Check if the file exists
    if !path.exists() {
        return (
            StatusCode::BAD_REQUEST, 
            Json(ProcessResponse {
                message: format!("File not found: {}", payload.file_path),
                success: false,
            })
        );
    }
    log::info!("1. Frames starting to be processed");

    // Process the MP4 file
    match process_mp4_frames(&path).await {
        Ok(_) => {
            log::info!("2. Frames processed");
            (
                StatusCode::OK,
                Json(ProcessResponse {
                    message: format!("Successfully processed video: {}", payload.file_path),
                    success: true,
                })
            )
        },
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ProcessResponse {
                message: format!("Error processing video: {}", err),
                success: false,
            })
        )
    }
}

#[derive(Deserialize)]
struct UploadRequest {
    files: UploadFiles,
}

#[derive(Deserialize)]
struct UploadFiles {
    video: String, // Base64 string
}

async fn upload(Json(payload): Json<UploadRequest>) -> impl IntoResponse {
    log::info!("Processing uploaded video data");
    let base64_data = payload.files.video;

    // Extract the Base64 data (remove the "data:video/mp4;base64," prefix)
    let base64_part = match base64_data.split(',').nth(1) {
        Some(part) => part,
        None => {
            log::error!("Invalid base64 format: missing data part");
            return (
                StatusCode::BAD_REQUEST,
                Json(ProcessResponse {
                    message: "Invalid base64 format".to_string(),
                    success: false,
                })
            );
        }
    };
    
    // Decode the base64 data
    let binary_data = match STANDARD.decode(base64_part) {
        Ok(data) => data,
        Err(err) => {
            log::error!("Failed to decode base64 data: {}", err);
            return (
                StatusCode::BAD_REQUEST,
                Json(ProcessResponse {
                    message: format!("Failed to decode base64 data: {}", err),
                    success: false,
                })
            );
        }
    };

    log::info!("Successfully decoded {} bytes of video data", binary_data.len());
    
    // Use an in-memory Cursor for the binary data
    let mut cursor = Cursor::new(binary_data);

    // Parse the MP4 file
    let cursor_len = cursor.get_ref().len() as u64;
    let mp4_reader = match Mp4Reader::read_header(&mut cursor, cursor_len) {
        Ok(reader) => reader,
        Err(err) => {
            log::error!("Failed to parse MP4 data: {:?}", err);
            return (
                StatusCode::BAD_REQUEST,
                Json(ProcessResponse {
                    message: "Invalid MP4 format".to_string(),
                    success: false,
                })
            );
        }
    };

    // Extract metadata
    let metadata = format!(
        "Tracks: {}, Duration: {:?}, Timescale: {}",
        mp4_reader.tracks().len(),
        mp4_reader.duration(),
        mp4_reader.timescale()
    );

    log::info!("Successfully processed MP4 data: {}", metadata);
    
    (
        StatusCode::OK,
        Json(ProcessResponse {
            message: metadata,
            success: true,
        })
    )
}

#[derive(Deserialize)]
struct VideoBase64Request {
    base64_data: String,
}

// Instead, import it from the utils module
use k21::logger::init_logger_exe;

// Add a helper function to log the state
pub fn log_processing_state(state: &ImageDataCollection) {
    log::info!("Processing state contains {} frames", state.len());
    
    for image_data in state.iter() {
        log::info!(
            "Frame {}: timestamp={}, text={}, processing_type={}",
            image_data.frame_number(),
            image_data.timestamp(),
            image_data.content(),
            image_data.processing_type()
        );
    }
}

// Add this new response type
#[derive(Serialize)]
struct ProcessVideoResponse {
    message: String,
    success: bool,
    result: Vec<ImageData>
}

async fn process_video_base64(Json(payload): Json<VideoBase64Request>) -> impl IntoResponse {

    log::info!("Received base64 data of length: {}", payload.base64_data.len());
    log::info!("Processing base64 video data for frame extraction");
    let base64_data = &payload.base64_data;

    // Extract the Base64 data (remove the "data:video/mp4;base64," prefix if present)
    let base64_part = if base64_data.contains(',') {
        base64_data.split(',').nth(1).unwrap_or(base64_data)
    } else {
        base64_data
    };
    
    // Decode the base64 data
    let binary_data = match STANDARD.decode(base64_part) {
        Ok(data) => data,
        Err(err) => {
            log::error!("Failed to decode base64 data: {}", err);
            return (
                StatusCode::BAD_REQUEST,
                Json(ProcessVideoResponse {
                    message: format!("Failed to decode base64 data: {}", err),
                    success: false,
                    result: Vec::new()  // Empty result for error case
                })
            );
        }
    };

    log::info!("Successfully decoded {} bytes of video data", binary_data.len());
    
    // Create shared state
    let state = Arc::new(Mutex::new(ImageDataCollection::new()));
    let state_clone = Arc::clone(&state);
    
    // Process the MP4 data with shared state
    match k21::mp4_pr::process_mp4_from_base64_with_state(
        base64_part, 
        state_clone
    ).await {
        Ok(_) => {
            let final_state = state.lock().unwrap();
            (
                StatusCode::OK,
                Json(ProcessVideoResponse {
                    message: format!("Successfully processed {} video frames", final_state.len()),
                    success: true,
                    result: final_state.to_vec()
                })
            )
        },
        Err(err) => {
            log::error!("Error processing MP4 frames: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ProcessVideoResponse {
                    message: format!("Error processing video frames: {}", err),
                    success: false,
                    result: Vec::new()
                })
            )
        }
    }
}


    
        
