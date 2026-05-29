use crate::client::LLMClient;
use crate::types::{Chunk, Message, Role, ToolCall, ToolDef};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text { text: String, #[serde(rename = "type")] type_: String },
    ToolUse { id: String, name: String, input: serde_json::Value, #[serde(rename = "type")] type_: String },
    ToolResult { tool_use_id: String, content: String, #[serde(rename = "type")] type_: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    type_: String,
    delta: Option<AnthropicDelta>,
    content_block: Option<AnthropicContentBlock>,
    index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicDelta {
    text: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    type_: String,
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
    text: Option<String>,
}

pub struct AnthropicClient {
    http: HttpClient,
    api_key: String,
    api_url: String,
    model: String,
}

impl AnthropicClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            http: HttpClient::new(),
            api_key,
            api_url: "https://api.anthropic.com/v1/messages".into(),
            model,
        }
    }

    fn convert_messages(messages: &[Message], system: &mut Option<String>) -> Vec<AnthropicMessage> {
        let mut result = Vec::new();
        for msg in messages {
            match msg.role {
                Role::System => {
                    *system = Some(msg.content.clone());
                }
                Role::User => {
                    result.push(AnthropicMessage {
                        role: "user".into(),
                        content: vec![AnthropicContent::Text {
                            text: msg.content.clone(),
                            type_: "text".into(),
                        }],
                    });
                }
                Role::Assistant => {
                    let mut content = Vec::new();
                    if !msg.content.is_empty() {
                        content.push(AnthropicContent::Text {
                            text: msg.content.clone(),
                            type_: "text".into(),
                        });
                    }
                    for tc in &msg.tool_calls {
                        content.push(AnthropicContent::ToolUse {
                            id: tc.id.clone(),
                            name: tc.name.clone(),
                            input: tc.arguments.clone(),
                            type_: "tool_use".into(),
                        });
                    }
                    result.push(AnthropicMessage {
                        role: "assistant".into(),
                        content,
                    });
                }
                Role::Tool => {
                    for tr in &msg.tool_results {
                        result.push(AnthropicMessage {
                            role: "user".into(),
                            content: vec![AnthropicContent::ToolResult {
                                tool_use_id: tr.tool_call_id.clone(),
                                content: tr.content.clone(),
                                type_: "tool_result".into(),
                            }],
                        });
                    }
                }
            }
        }
        result
    }

    fn convert_tools(tools: &[ToolDef]) -> Vec<AnthropicTool> {
        tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect()
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    async fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> anyhow::Result<Message> {
        let mut system = None;
        let anthropic_messages = Self::convert_messages(messages, &mut system);
        let mut body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            messages: anthropic_messages,
            system,
            tools: None,
            stream: false,
        };
        if !tools.is_empty() {
            body.tools = Some(Self::convert_tools(tools));
        }
        let resp = self
            .http
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<AnthropicResponse>()
            .await?;
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        for block in resp.content {
            match block {
                AnthropicContent::Text { text, .. } => content.push_str(&text),
                AnthropicContent::ToolUse { id, name, input, .. } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                        result: None,
                    });
                }
                _ => {}
            }
        }
        Ok(Message {
            role: Role::Assistant,
            content,
            tool_calls,
            tool_results: vec![],
        })
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
    ) -> anyhow::Result<mpsc::Receiver<Chunk>> {
        let (tx, rx) = mpsc::channel(64);
        let mut system = None;
        let anthropic_messages = Self::convert_messages(messages, &mut system);
        let mut body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 8192,
            messages: anthropic_messages,
            system,
            tools: None,
            stream: true,
        };
        if !tools.is_empty() {
            body.tools = Some(Self::convert_tools(tools));
        }
        let url = self.api_url.clone();
        let api_key = self.api_key.clone();
        let http = self.http.clone();
        tokio::spawn(async move {
            match http
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let mut stream = resp.bytes_stream();
                    let mut buf = String::new();
                    while let Some(chunk_result) = stream.next().await {
                        match chunk_result {
                            Ok(bytes) => {
                                buf.push_str(&String::from_utf8_lossy(&bytes));
                                while let Some(nl) = buf.find('\n') {
                                    let line = buf[..nl].trim().to_string();
                                    buf = buf[nl + 1..].to_string();
                                    if !line.starts_with("data: ") {
                                        continue;
                                    }
                                    let data = line[6..].trim();
                                    if data == "[DONE]" {
                                        let _ = tx.send(Chunk { content: None, tool_call: None, done: true }).await;
                                        break;
                                    }
                                    if let Ok(ev) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                                        match ev.type_.as_str() {
                                            "content_block_delta" => {
                                                if let Some(d) = &ev.delta
                                                    && let Some(text) = &d.text {
                                                        let _ = tx.send(Chunk {
                                                            content: Some(text.clone()),
                                                            tool_call: None,
                                                            done: false,
                                                        }).await;
                                                    }
                                            }
                                            "content_block_start" => {
                                                if let Some(cb) = &ev.content_block
                                                    && cb.type_ == "tool_use"
                                                        && let (Some(id), Some(name), Some(input)) = (&cb.id, &cb.name, &cb.input) {
                                                            let _ = tx.send(Chunk {
                                                                content: None,
                                                                tool_call: Some((id.clone(), name.clone(), input.clone())),
                                                                done: false,
                                                            }).await;
                                                        }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(Chunk {
                        content: Some(format!("Error: {}", e)),
                        tool_call: None,
                        done: true,
                    }).await;
                }
            }
        });
        Ok(rx)
    }
}
