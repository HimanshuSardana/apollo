use serde::{Deserialize, Serialize};

pub const ENDPOINT: &str = "https://opencode.ai/inference/go/openai/v1/chat/completions";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>, reasoning: Option<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content.into()),
            reasoning_content: reasoning,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_tool_calls(
        content: String,
        reasoning: Option<String>,
        calls: Vec<ToolCall>,
    ) -> Self {
        let content = if content.trim().is_empty() {
            None
        } else {
            Some(content)
        };
        Self {
            role: "assistant".to_string(),
            content,
            reasoning_content: reasoning,
            tool_calls: Some(calls),
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<&'a serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaToolCall {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<DeltaFunction>,
}

#[derive(Debug, Default, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    reasoning_content: Option<serde_json::Value>,
    #[serde(default)]
    reasoning: Option<serde_json::Value>,
    #[serde(default)]
    tool_calls: Option<Vec<DeltaToolCall>>,
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

#[derive(Debug, Clone, Copy)]
pub enum StreamEvent<'a> {
    Reasoning(&'a str),
    Content(&'a str),
}

pub struct Completion {
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    args: String,
}

#[derive(Default)]
struct Acc {
    content: String,
    reasoning: String,
    calls: Vec<PartialCall>,
}

fn error_detail(text: &str) -> String {
    serde_json::from_str::<ErrorEnvelope>(text)
        .ok()
        .and_then(|env| env.error)
        .map(|b| match b {
            ErrorBody::Text(t) => t,
            ErrorBody::Detailed { message } => message.unwrap_or_else(|| "unknown error".to_string()),
        })
        .unwrap_or_else(|| text.trim().to_string())
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

fn absorb_tool_calls(acc: &mut Acc, deltas: Vec<DeltaToolCall>) {
    for d in deltas {
        let idx = d.index.unwrap_or(0);
        while acc.calls.len() <= idx {
            acc.calls.push(PartialCall::default());
        }
        let slot = &mut acc.calls[idx];
        if let Some(id) = d.id.filter(|s| !s.is_empty()) {
            slot.id = id;
        }
        if let Some(f) = d.function {
            if let Some(name) = f.name.filter(|s| !s.is_empty()) {
                slot.name.push_str(&name);
            }
            if let Some(args) = f.arguments {
                slot.args.push_str(&args);
            }
        }
    }
}

fn handle_sse_line(
    line: &[u8],
    on_event: &mut impl FnMut(StreamEvent),
    acc: &mut Acc,
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
        let thought = delta
            .reasoning_content
            .as_ref()
            .or(delta.reasoning.as_ref())
            .map(value_to_string)
            .filter(|s| !s.is_empty());
        if let Some(text) = thought {
            acc.reasoning.push_str(&text);
            on_event(StreamEvent::Reasoning(&text));
        }
        if let Some(text) = delta
            .content
            .as_ref()
            .map(value_to_string)
            .filter(|s| !s.is_empty())
        {
            acc.content.push_str(&text);
            on_event(StreamEvent::Content(&text));
        }
        if let Some(calls) = delta.tool_calls.as_ref() {
            absorb_tool_calls(acc, calls.iter().map(clone_delta).collect());
        }
    }
    Ok(false)
}

fn clone_delta(d: &DeltaToolCall) -> DeltaToolCall {
    DeltaToolCall {
        index: d.index,
        id: d.id.clone(),
        function: d.function.as_ref().map(|f| DeltaFunction {
            name: f.name.clone(),
            arguments: f.arguments.clone(),
        }),
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
    model: String,
    effort: Option<String>,
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
            model: crate::models::DEFAULT_MODEL.to_string(),
            effort: None,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn set_model(&mut self, id: &str) {
        self.model = id.to_string();
    }

    pub fn effort(&self) -> Option<&str> {
        self.effort.as_deref()
    }

    pub fn set_effort(&mut self, level: Option<&str>) {
        self.effort = match level {
            None | Some("auto") => None,
            Some(other) => Some(other.to_string()),
        };
    }

    pub async fn complete_stream(
        &self,
        history: &[ChatMessage],
        tools: Option<&serde_json::Value>,
        mut on_event: impl FnMut(StreamEvent),
    ) -> Result<Completion, ApiError> {
        let effort = self.effort.as_deref();
        let thinking = if effort == Some("off") && self.model.starts_with("deepseek") {
            Some(serde_json::json!({"type": "disabled"}))
        } else {
            None
        };
        let reasoning_effort = match effort {
            None | Some("off") => None,
            Some(level) => Some(level),
        };
        let body = ChatRequest {
            model: &self.model,
            messages: history,
            stream: true,
            tools,
            reasoning_effort,
            thinking: thinking.as_ref(),
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
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(ApiError(format!(
                "api error (http {status}): {}",
                error_detail(&text)
            )));
        }

        let mut res = res;
        let mut acc = Acc::default();
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
                        done = handle_sse_line(&line, &mut on_event, &mut acc)?;
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
            handle_sse_line(&tail, &mut on_event, &mut acc)?;
        }

        let tool_calls = acc
            .calls
            .into_iter()
            .filter(|c| !c.name.is_empty())
            .enumerate()
            .map(|(i, c)| ToolCall {
                id: if c.id.is_empty() {
                    format!("call_{i}")
                } else {
                    c.id
                },
                kind: "function".to_string(),
                function: FunctionCall {
                    name: c.name,
                    arguments: if c.args.trim().is_empty() {
                        "{}".to_string()
                    } else {
                        c.args
                    },
                },
            })
            .collect();

        let reasoning = if acc.reasoning.trim().is_empty() {
            None
        } else {
            Some(acc.reasoning)
        };

        Ok(Completion {
            content: acc.content,
            reasoning,
            tool_calls,
        })
    }
}
