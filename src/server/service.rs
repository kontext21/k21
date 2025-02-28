use axum::{
    routing::get,
    Router,
    Json,
    http::StatusCode,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub status: String,
    pub data: Option<T>,
    pub message: Option<String>,
}

// You can add more shared functionality here as needed 