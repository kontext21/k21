use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisionConfig {
    pub url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub prompt: Option<String>,
}

impl VisionConfig {
    pub fn new() -> Self {
        Self {
            url: None,
            api_key: None,
            model: None,
            prompt: None,
        }
    }

    pub fn unpack(&self) -> Result<(&str, &str, &str, Option<&str>)> {
        let url = self.url.as_deref()
            .ok_or_else(|| anyhow::anyhow!("URL is required for vision processing"))?;
        let api_key = self.api_key.as_deref()
            .ok_or_else(|| anyhow::anyhow!("API key is required for vision processing"))?;
        let model = self.model.as_deref()
            .ok_or_else(|| anyhow::anyhow!("Model is required for vision processing"))?;
        
        Ok((url, api_key, model, self.prompt.as_deref()))
    }

}