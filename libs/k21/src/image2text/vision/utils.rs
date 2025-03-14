// use axum::{routing::post, Json, Router};
// use image::DynamicImage;
// use serde::{Deserialize, Serialize};
// use reqwest::header::{HeaderMap, HeaderValue};
// use base64::{Engine as _, engine::general_purpose::STANDARD};

// #[derive(Deserialize)]
// struct UserRequest {
//     api_key: String,
//     model: String,
//     messages: Vec<Message>,
// }

// #[derive(Deserialize, Serialize)]
// struct Message {
//     role: String,
//     content: Vec<Content>,
// }

// #[derive(Deserialize, Serialize)]
// #[serde(untagged)]
// enum Content {
//     Text { r#type: String, text: String },
//     Image { image_url: ImageUrl },
// }

// #[derive(Deserialize, Serialize)]
// struct ImageUrl {
//     url: String,
// }

// // OpenRouter Response
// #[derive(Deserialize)]
// struct OpenRouterResponse {
//     choices: Vec<Choice>,
// }

// #[derive(Deserialize)]
// struct Choice {
//     message: MessageResponse,
// }

// #[derive(Deserialize)]
// struct MessageResponse {
//     content: String,
// }

// // Response to the user
// #[derive(Serialize)]
// struct ImageToTextResponse {
//     extracted_text: String,
// }

// // async fn image_to_base64(image_path: &str) -> String {
// //     // Check if it's a URL or a file path
// //     if image_path.starts_with("http://") || image_path.starts_with("https://") {
// //         // For URLs, download the image asynchronously
// //         let response = reqwest::get(image_path).await.expect("Failed to download image");
// //         let bytes = response.bytes().await.expect("Failed to read image bytes");
// //         STANDARD.encode(bytes)
// //     } else {
// //         // For file paths, read the file (this is still blocking but wrapped in tokio::fs)
// //         let buffer = tokio::fs::read(image_path).await.expect("Failed to read image file");
// //         STANDARD.encode(buffer)
// //     }
// // }

// fn dynamic_image_to_base64(image: &DynamicImage) -> String {
//     let mut buffer = Vec::new();
//     image.write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
//         .expect("Failed to encode image to PNG");
//     STANDARD.encode(&buffer)
// }

// async fn call_openrouter(api_key: &str, model: &str, image: &DynamicImage) -> String {
//     let client = reqwest::Client::new();
    
//     // Create headers
//     let mut headers = HeaderMap::new();
//     headers.insert("Authorization", HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap());
//     headers.insert("Content-Type", HeaderValue::from_static("application/json"));

//     let base64_str = dynamic_image_to_base64(image);
//     println!("Base64 string: {}", base64_str);
//     // JSON payload
//     let body = serde_json::json!({
//         "model": model,
//         "messages": [
//             {
//                 "role": "user",
//                 "content": [
//                     { "type": "text", "text": "What is in this image?" },
//                     { 
//                         "type": "image_url",
//                         "image_url": {
//                             "url": format!("data:image/png;base64,{}", base64_str)
//                         } 
//                     }
//                 ]
//             }
//         ]
//     });

//     // Send request
//     let response = client
//         .post("https://openrouter.ai/api/v1/chat/completions")
//         .headers(headers)
//         .json(&body)
//         .send()
//         .await
//         .expect("Failed to send request");

//     // Parse response
//     if let Ok(parsed_response) = response.json::<OpenRouterResponse>().await {
//         if let Some(choice) = parsed_response.choices.get(0) {
//             return choice.message.content.clone();
//         }
//     }

//     "Failed to extract text".to_string()
// }

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

// // async fn process_image_vision(image: DynamicImage, api_key: &str, model: &str, prompt: Option<&str>) -> String {
    
// //     let final_prompt = if let Some(prompt) = prompt {
// //         prompt
// //     } else {
// //         "What is in this image?"
// //     };
// // }
