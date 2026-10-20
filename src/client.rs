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

#[derive(Debug)]
pub enum StreamEvent {
    Reasoning(String),
    Content(String),
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    reasoning_content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<StreamDelta>,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
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

fn error_detail(text: &str) -> String {
    serde_json::from_str::<ErrorEnvelope>(text)
        .ok()
        .and_then(|env| env.error)
        .map(|b| match b {
            ErrorBody::Text(t) => t,
            ErrorBody::Detailed { message } => {
                message.unwrap_or_else(|| "unknown error".to_string())
            }
        })
        .unwrap_or_else(|| text.trim().to_string())
}

fn handle_sse_line(
    line: &[u8],
    on_event: &mut impl FnMut(StreamEvent),
    content: &mut String,
    reasoning: &mut String,
) -> Result<bool, ApiError> {
    let line = String::from_utf8_lossy(line);
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(false);
    }
    let payload = line.strip_prefix("data:").unwrap_or(line).trim();
    if payload == "[DONE]" {
        return Ok(true);
    }
    let value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| ApiError(format!("failed to parse stream chunk: {e}")))?;
    if value.get("error").is_some() {
        return Err(ApiError(format!(
            "api error (stream): {}",
            error_detail(payload)
        )));
    }
    let chunk: StreamChunk = serde_json::from_value(value)
        .map_err(|e| ApiError(format!("failed to parse stream chunk: {e}")))?;
    if let Some(delta) = chunk.choices.first().and_then(|c| c.delta.as_ref()) {
        if let Some(text) = delta
            .reasoning_content
            .as_ref()
            .map(value_to_string)
            .filter(|s| !s.is_empty())
        {
            reasoning.push_str(&text);
            on_event(StreamEvent::Reasoning(text));
        }
        if let Some(text) = delta
            .content
            .as_ref()
            .map(value_to_string)
            .filter(|s| !s.is_empty())
        {
            content.push_str(&text);
            on_event(StreamEvent::Content(text));
        }
    }
    Ok(false)
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
        let api_key = std::env::var("OPENCODE_API_KEY").map_err(|_| {
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

    pub async fn complete_stream(
        &self,
        history: &[ChatMessage],
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<(String, Option<String>), ApiError> {
        Self::complete_stream_inner(
            &self.http,
            &self.api_key,
            &self.session_id,
            history,
            &mut on_event,
        )
        .await
    }

    async fn complete_stream_inner(
        http: &reqwest::Client,
        api_key: &str,
        session_id: &str,
        history: &[ChatMessage],
        on_event: &mut impl FnMut(StreamEvent),
    ) -> Result<(String, Option<String>), ApiError> {
        let body = ChatRequest {
            model: MODEL,
            messages: history,
            stream: true,
        };

        let res = http
            .post(ENDPOINT)
            .bearer_auth(api_key)
            .header("x-opencode-session", session_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError(format!("request failed: {e}")))?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(ApiError(format!(
                "api error (http {status}): {}",
                error_detail(&text)
            )));
        }

        let mut res = res;
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut buf: Vec<u8> = Vec::new();
        let mut done = false;
        while !done {
            match res
                .chunk()
                .await
                .map_err(|e| ApiError(format!("stream read failed: {e}")))?
            {
                Some(bytes) => {
                    buf.extend_from_slice(&bytes);
                    while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = buf.drain(..=pos).collect();
                        done = handle_sse_line(&line, on_event, &mut content, &mut reasoning)?;
                        if done {
                            break;
                        }
                    }
                }
                None => break,
            }
        }
        if !done && !buf.is_empty() {
            let tail: Vec<u8> = std::mem::take(&mut buf);
            handle_sse_line(&tail, on_event, &mut content, &mut reasoning)?;
        }

        let reasoning = if reasoning.trim().is_empty() {
            None
        } else {
            Some(reasoning)
        };
        Ok((content, reasoning))
    }
}
