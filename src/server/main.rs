use axum::{
    routing::{get, post},
    Router,
    response::IntoResponse,
    http::StatusCode,
    Json,
    extract::DefaultBodyLimit,
};
use k21::{common::get_results_from_state, process::ProcessorConfig};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::{Arc, Mutex};
use k21::common::{ImageData, ImageDataCollection};

use k21::logger::init_logger_exe;

#[tokio::main]
async fn main() {
    // Initialize the logger
    init_logger_exe();

    log::info!("Starting server...");

    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        // .route("/health", get(|| async { "healthy" }))
        // .route("/process-video-path", post(process_video_path))
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
struct VideoBase64Request {
    base64_data: String,
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
        &ProcessorConfig::default(),
        state_clone
    ).await {
        Ok(_) => {
            let result = get_results_from_state(state).await.unwrap();
            (
                StatusCode::OK,
                Json(ProcessVideoResponse {
                    message: format!("Successfully processed {} video frames", result.len()),
                    success: true,
                    result: result
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


    
        
