//! AI Provider implementations

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Supported AI providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiProvider {
    #[default]
    Anthropic,
    OpenAI,
}

impl std::str::FromStr for AiProvider {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Ok(AiProvider::Anthropic),
            "openai" | "gpt" => Ok(AiProvider::OpenAI),
            _ => anyhow::bail!("Unknown AI provider: {}. Use 'anthropic' or 'openai'", s),
        }
    }
}

/// AI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// AI provider to use
    #[serde(default)]
    pub provider: String,
    /// API key (can use env var like ${ANTHROPIC_API_KEY})
    pub api_key: Option<String>,
    /// Model to use
    pub model: Option<String>,
    /// Enable automatic annotation
    #[serde(default)]
    pub auto_annotate: bool,
    /// Language for annotations
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "en".to_string()
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            api_key: None,
            model: None,
            auto_annotate: false,
            language: default_language(),
        }
    }
}

/// Request to Anthropic Claude API
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

/// Response from Anthropic Claude API
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

/// Request to OpenAI API
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

/// Response from OpenAI API
#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessageContent,
}

#[derive(Debug, Deserialize)]
struct OpenAIMessageContent {
    content: String,
}

/// AI client for making API calls
pub struct AiClient {
    provider: AiProvider,
    api_key: String,
    model: String,
    language: String,
    client: reqwest::Client,
}

impl AiClient {
    /// Create a new AI client from configuration
    pub fn new(config: &AiConfig) -> Result<Self> {
        let provider: AiProvider = config.provider.parse().unwrap_or_default();

        // Get API key from config or environment
        let api_key = config.api_key.clone()
            .or_else(|| {
                match provider {
                    AiProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
                    AiProvider::OpenAI => std::env::var("OPENAI_API_KEY").ok(),
                }
            })
            .context("API key not found. Set it in config or via environment variable")?;

        // Expand env vars in API key if needed
        let api_key = if api_key.starts_with("${") && api_key.ends_with("}") {
            let var_name = &api_key[2..api_key.len()-1];
            std::env::var(var_name).unwrap_or(api_key)
        } else {
            api_key
        };

        let model = config.model.clone().unwrap_or_else(|| {
            match provider {
                AiProvider::Anthropic => "claude-sonnet-4-20250514".to_string(),
                AiProvider::OpenAI => "gpt-4o".to_string(),
            }
        });

        Ok(Self {
            provider,
            api_key,
            model,
            language: config.language.clone(),
            client: reqwest::Client::new(),
        })
    }

    /// Generate annotation for test code
    pub async fn annotate_test(&self, test_name: &str, test_code: &str) -> Result<TestAnnotation> {
        let prompt = self.build_annotation_prompt(test_name, test_code);
        let response = self.call_api(&prompt).await?;

        // Parse the structured response
        parse_annotation_response(&response, test_name)
    }

    /// Generate annotations for multiple tests
    pub async fn annotate_tests(&self, tests: &[(String, String)]) -> Result<Vec<TestAnnotation>> {
        let mut annotations = Vec::new();

        for (name, code) in tests {
            match self.annotate_test(name, code).await {
                Ok(annotation) => annotations.push(annotation),
                Err(e) => {
                    eprintln!("Warning: Failed to annotate test '{}': {}", name, e);
                    // Add a placeholder annotation
                    annotations.push(TestAnnotation {
                        test_name: name.clone(),
                        description: "Annotation unavailable".to_string(),
                        purpose: None,
                        tested_function: None,
                        test_type: None,
                        tags: vec![],
                    });
                }
            }
        }

        Ok(annotations)
    }

    fn build_annotation_prompt(&self, test_name: &str, test_code: &str) -> String {
        let lang_instruction = match self.language.as_str() {
            "fr" => "Réponds en français.",
            "es" => "Responde en español.",
            "de" => "Antworte auf Deutsch.",
            _ => "Respond in English.",
        };

        format!(
            r#"Analyze this test and provide a structured annotation.

Test name: {}
Test code:
```
{}
```

{}

Provide a JSON response with this exact structure:
{{
  "description": "Brief description of what this test verifies (1-2 sentences)",
  "purpose": "Why this test exists and what scenario it covers",
  "tested_function": "Name of the main function/method being tested",
  "test_type": "unit|integration|e2e|performance|security",
  "tags": ["relevant", "tags", "for", "categorization"]
}}

Only respond with the JSON, no additional text."#,
            test_name, test_code, lang_instruction
        )
    }

    async fn call_api(&self, prompt: &str) -> Result<String> {
        match self.provider {
            AiProvider::Anthropic => self.call_anthropic(prompt).await,
            AiProvider::OpenAI => self.call_openai(prompt).await,
        }
    }

    async fn call_anthropic(&self, prompt: &str) -> Result<String> {
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to call Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        let result: AnthropicResponse = response.json().await
            .context("Failed to parse Anthropic response")?;

        result.content
            .first()
            .map(|c| c.text.clone())
            .context("Empty response from Anthropic")
    }

    async fn call_openai(&self, prompt: &str) -> Result<String> {
        let request = OpenAIRequest {
            model: self.model.clone(),
            max_tokens: 1024,
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to call OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error ({}): {}", status, error_text);
        }

        let result: OpenAIResponse = response.json().await
            .context("Failed to parse OpenAI response")?;

        result.choices
            .first()
            .map(|c| c.message.content.clone())
            .context("Empty response from OpenAI")
    }
}

/// Structured test annotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAnnotation {
    pub test_name: String,
    pub description: String,
    pub purpose: Option<String>,
    pub tested_function: Option<String>,
    pub test_type: Option<String>,
    pub tags: Vec<String>,
}

/// Parse the AI response into a structured annotation
fn parse_annotation_response(response: &str, test_name: &str) -> Result<TestAnnotation> {
    // Try to extract JSON from the response
    let json_str = if response.contains("{") {
        let start = response.find('{').unwrap();
        let end = response.rfind('}').map(|i| i + 1).unwrap_or(response.len());
        &response[start..end]
    } else {
        response
    };

    #[derive(Deserialize)]
    struct RawAnnotation {
        description: String,
        purpose: Option<String>,
        tested_function: Option<String>,
        test_type: Option<String>,
        tags: Option<Vec<String>>,
    }

    match serde_json::from_str::<RawAnnotation>(json_str) {
        Ok(raw) => Ok(TestAnnotation {
            test_name: test_name.to_string(),
            description: raw.description,
            purpose: raw.purpose,
            tested_function: raw.tested_function,
            test_type: raw.test_type,
            tags: raw.tags.unwrap_or_default(),
        }),
        Err(_) => {
            // If JSON parsing fails, use the raw response as description
            Ok(TestAnnotation {
                test_name: test_name.to_string(),
                description: response.trim().to_string(),
                purpose: None,
                tested_function: None,
                test_type: None,
                tags: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_annotation_response() {
        let response = r#"{
            "description": "Tests that user login works correctly",
            "purpose": "Verify authentication flow",
            "tested_function": "login",
            "test_type": "integration",
            "tags": ["auth", "login"]
        }"#;

        let annotation = parse_annotation_response(response, "test_login").unwrap();
        assert_eq!(annotation.description, "Tests that user login works correctly");
        assert_eq!(annotation.tested_function, Some("login".to_string()));
        assert_eq!(annotation.tags, vec!["auth", "login"]);
    }

    #[test]
    fn test_provider_parsing() {
        assert_eq!("anthropic".parse::<AiProvider>().unwrap(), AiProvider::Anthropic);
        assert_eq!("claude".parse::<AiProvider>().unwrap(), AiProvider::Anthropic);
        assert_eq!("openai".parse::<AiProvider>().unwrap(), AiProvider::OpenAI);
        assert_eq!("gpt".parse::<AiProvider>().unwrap(), AiProvider::OpenAI);
    }
}
