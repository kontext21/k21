use axum::{
    routing::{get, post},
    Router,
    response::IntoResponse,
    http::StatusCode,
    Json,
};
use k21_screen::common::{mp4::utils::process_mp4_frames, utils::init_logger_exe};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use serde::{Deserialize, Serialize};

// Add this function to initialize the logger
#[tokio::main]
async fn main() {
    // Initialize the logger
    init_logger_exe();
    
    log::info!("Starting server...");
    
    let app = Router::new()
        .route("/ping", get(|| async { "pong" }))
        .route("/process-video-path", post(process_video_path));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    log::info!("Listening on http://{}", addr);

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
    
        
