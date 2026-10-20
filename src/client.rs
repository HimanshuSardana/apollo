use serde::{Deserialize, Serialize};

pub const ENDPOINT: &str = "https://opencode.ai/inference/go/openai/v1/chat/completions";
pub const MODEL: &str = "deepseek-v4.1-flash";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            reasoning_content: None,
        }
    }

    pub fn assistant(content: impl Into<String>, reasoning: Option<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            reasoning_content: reasoning,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    reasoning_content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: AssistantMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: Option<ErrorBody>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ErrorBody {
    Text(String),
    Detailed { message: Option<String> },
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|p| {
                p.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| p.as_str().map(|s| s.to_string()))
            })
            .collect::<Vec<_>>()
            .join(""),
        other => other.to_string(),
    }
}

#[derive(Debug)]
pub struct ApiError(pub String);

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ApiError {}

pub struct Client {
    http: reqwest::Client,
    api_key: String,
    session_id: String,
}

impl Client {
    pub fn new() -> Result<Self, ApiError> {
        let api_key = std::env::var("OPENCODE_API_KEY")
            .map_err(|_| {
                ApiError("OPENCODE_API_KEY is not set (export it before running apollo)".to_string())
            })?;
        if api_key.trim().is_empty() {
            return Err(ApiError("OPENCODE_API_KEY is empty".to_string()));
        }
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            session_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    pub async fn complete(
        &self,
        history: &[ChatMessage],
    ) -> Result<(String, Option<String>), ApiError> {
        let body = ChatRequest {
            model: MODEL,
            messages: history,
            stream: false,
        };

        let res = self
            .http
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .header("x-opencode-session", &self.session_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError(format!("request failed: {e}")))?;

        let status = res.status();
        let text = res
            .text()
            .await
            .map_err(|e| ApiError(format!("failed to read response body: {e}")))?;

        if !status.is_success() {
            let detail = serde_json::from_str::<ErrorEnvelope>(&text)
                .ok()
                .and_then(|env| env.error)
                .map(|b| match b {
                    ErrorBody::Text(t) => t,
                    ErrorBody::Detailed { message } => {
                        message.unwrap_or_else(|| "unknown error".to_string())
                    }
                })
                .unwrap_or_else(|| text.trim().to_string());
            return Err(ApiError(format!("api error (http {status}): {detail}")));
        }

        let parsed: ChatResponse = serde_json::from_str(&text)
            .map_err(|e| ApiError(format!("failed to parse response: {e}\nbody: {text}")))?;
        let message = parsed
            .choices
            .first()
            .map(|c| &c.message)
            .ok_or_else(|| ApiError("api returned no choices".to_string()))?;

        let content = message
            .content
            .as_ref()
            .map(value_to_string)
            .unwrap_or_default();
        let reasoning = message
            .reasoning_content
            .as_ref()
            .map(value_to_string)
            .filter(|s| !s.trim().is_empty());

        Ok((content, reasoning))
    }
}
