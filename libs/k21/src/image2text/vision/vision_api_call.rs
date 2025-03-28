use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderMap, HeaderValue};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use anyhow::Result;
use crate::common::{get_current_timestamp_str, ImageData, ProcessingType};

use super::VisionConfig;

const DEFAULT_PROMPT: &str = "What is in this image?";

#[derive(Deserialize, Serialize)]
struct Message {
    role: String,
    content: Vec<Content>,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Content {
    Text { r#type: String, text: String },
    Image { image_url: ImageUrl },
}

#[derive(Deserialize, Serialize)]
struct ImageUrl {
    url: String,
}

// OpenRouter Response
#[derive(Deserialize)]
struct VisionModelResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageResponse,
}

#[derive(Deserialize)]
struct MessageResponse {
    content: String,
}

async fn image_path_to_base64(image_path: &str) -> String {
    // Check if it's a URL or a file path
    if image_path.starts_with("http://") || image_path.starts_with("https://") {
        // For URLs, download the image asynchronously
        let response = reqwest::get(image_path).await.expect("Failed to download image");
        let bytes = response.bytes().await.expect("Failed to read image bytes");
        STANDARD.encode(bytes)
    } else {
        // For file paths, read the file (this is still blocking but wrapped in tokio::fs)
        let buffer = tokio::fs::read(image_path).await.expect("Failed to read image file");
        STANDARD.encode(buffer)
    }
}

async fn call_vision_model(url: &str, api_key: &str, model: &str, base64_str: &String, prompt: &str) -> String {
    let client = reqwest::Client::new();
    
    // Create headers
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap());
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    // JSON payload
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    { "type": "text", "text": prompt },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{}", base64_str)
                        }
                    }
                ]
            }
        ]
    });

    // Send request
    let response = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .expect("Failed to send request");

    // Get the response text
    let response_text = response.text().await.expect("Failed to get response text");
    
    // Try to parse the response
    match serde_json::from_str::<VisionModelResponse>(&response_text) {
        Ok(parsed_response) => {
            if let Some(choice) = parsed_response.choices.get(0) {
                return choice.message.content.clone();
            }
            "No content in response".to_string()
        },
        Err(e) => {
            format!("Failed to parse response: {}. Raw response: {}", e, response_text)
        }
    }
}

pub async fn process_image_vision_from_path(image_path: &String, vision_config: &VisionConfig) -> Result<ImageData> {
    let image_base64 = image_path_to_base64(image_path).await;
    let vision_res = process_image_vision(image_base64, vision_config).await;
    let image_data = ImageData::new(get_current_timestamp_str(), 0, vision_res, ProcessingType::Vision);
    Ok(image_data)
}

pub async fn process_image_vision(image_base64: String, vision_config: &VisionConfig) -> String {
    let (url, api_key, model, prompt) = vision_config.unpack()
        .expect("Failed to unpack vision config, some fields are missing");

    let final_prompt = if let Some(prompt) = prompt {
        prompt
    } else {
        DEFAULT_PROMPT
    };

    call_vision_model(url, api_key, model, &image_base64, &final_prompt).await
}