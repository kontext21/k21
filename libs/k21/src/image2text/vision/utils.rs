use std::path::Path;

use axum::{routing::post, Json, Router};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderMap, HeaderValue};
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Deserialize)]
struct UserRequest {
    api_key: String,
    model: String,
    messages: Vec<Message>,
}

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
struct OpenRouterResponse {
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

// // Response to the user
// #[derive(Serialize)]
// struct ImageToTextResponse {
//     extracted_text: String,
// }

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

// fn dynamic_image_to_base64(image: &DynamicImage) -> String {
//     let mut buffer = Vec::new();
//     image.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
//         .expect("Failed to encode image to PNG");
//     STANDARD.encode(&buffer)
// }

async fn call_openrouter(url: &str, api_key: &str, model: &str, base64_str: &String, prompt: &str) -> String {
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
    match serde_json::from_str::<OpenRouterResponse>(&response_text) {
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

// async fn handle_request(Json(payload): Json<UserRequest>) -> String {
//     for message in &payload.messages {
//         for content in &message.content {
//             if let Content::Image { image_url, .. } = content {
//                 let extracted_text = call_openrouter(&payload.api_key, &payload.model, &image_url.url).await;
//                 return extracted_text;
//             }
//         }
//     }

//     "No image found".to_string()
// }

// async fn process_image_vision_from_DynamicImage(image: &DynamicImage, api_key: &str, model: &str, prompt: Option<&str>) -> String {
//     let base64_str = dynamic_image_to_base64(image);
//     process_image_vision(base64_str, api_key, model, prompt).await
// }

pub async fn process_image_vision_from_path(image_path: &String, url: &str, api_key: &str, model: &str, prompt: Option<&str>) -> String {
    let image_base64 = image_path_to_base64(image_path).await;
    process_image_vision(image_base64, url, api_key, model, prompt).await
}

async fn process_image_vision(image_base64: String, url: &str, api_key: &str, model: &str, prompt: Option<&str>) -> String {

    let final_prompt = if let Some(prompt) = prompt {
        prompt
    } else {
        "What is in this image?"
    };

    call_openrouter(url, api_key, model, &image_base64, &final_prompt).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_image_vision_from_path() {
        // Replace with a path to a test image that exists in your project
        let test_image_path: &str = "/Users/ferzu/rustTest/k21-node/__test__/screenshot-9.png";
        let url = "https://api.openai.com/v1/chat/completions";
        let key="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";
        let result = process_image_vision_from_path(&test_image_path.to_string(), url, key, "gpt-4-turbo", Some("What is in this image?")).await;

        // Basic check that we got some result
        assert!(!result.is_empty());
        println!("Vision API result: {}", result);
    }
}
