use crate::client::LLMClient;
use crate::types::{Chunk, Message, Role, ToolCall, ToolDef};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    type_: String,
    function: OllamaToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    tools: Vec<OllamaTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaMessage>,
    done: bool,
}

pub struct OllamaClient {
    http: HttpClient,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            http: HttpClient::new(),
            base_url,
            model,
        }
    }

    fn to_ollama_messages(messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "user".into(),
                    Role::Assistant => "assistant".into(),
                    Role::System => "system".into(),
                    Role::Tool => "tool".into(),
                };
                let tool_calls = if !m.tool_calls.is_empty() {
                    Some(
                        m.tool_calls
                            .iter()
                            .map(|tc| OllamaToolCall {
                                function: OllamaFunction {
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                },
                            })
                            .collect(),
                    )
                } else {
                    None
                };
                OllamaMessage {
                    role,
                    content: m.content.clone(),
                    tool_calls,
                }
            })
            .collect()
    }

    fn convert_tools(tools: &[ToolDef]) -> Vec<OllamaTool> {
        tools
            .iter()
            .map(|t| OllamaTool {
                type_: "function".into(),
                function: OllamaToolFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    }

    fn from_ollama_message(msg: &OllamaMessage) -> Message {
        let tool_calls = msg
            .tool_calls
            .as_ref()
            .map(|calls| {
                calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc.function.name.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                        result: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Message {
            role: Role::Assistant,
            content: msg.content.clone(),
            tool_calls,
            tool_results: vec![],
        }
    }
}

#[async_trait]
impl LLMClient for OllamaClient {
    async fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> anyhow::Result<Message> {
        let body = OllamaChatRequest {
            model: self.model.clone(),
            messages: Self::to_ollama_messages(messages),
            stream: false,
            tools: Self::convert_tools(tools),
        };
        let resp = self
            .http
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<OllamaChatResponse>()
            .await?;
        Ok(Self::from_ollama_message(&resp.message))
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
    ) -> anyhow::Result<mpsc::Receiver<Chunk>> {
        let (tx, rx) = mpsc::channel(64);
        let body = OllamaChatRequest {
            model: self.model.clone(),
            messages: Self::to_ollama_messages(messages),
            stream: true,
            tools: Self::convert_tools(tools),
        };
        let url = format!("{}/api/chat", self.base_url);
        let http = self.http.clone();
        tokio::spawn(async move {
            if let Ok(resp) = http.post(&url).json(&body).send().await {
                let mut stream = resp.bytes_stream();
                let mut buf = String::new();
                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(bytes) => {
                            buf.push_str(&String::from_utf8_lossy(&bytes));
                            while let Some(nl) = buf.find('\n') {
                                let line = buf[..nl].trim().to_string();
                                buf = buf[nl + 1..].to_string();
                                if line.is_empty() {
                                    continue;
                                }
                                if let Ok(chunk) =
                                    serde_json::from_str::<OllamaStreamChunk>(&line)
                                {
                                    if let Some(msg) = &chunk.message {
                                        let c = Chunk {
                                            content: Some(msg.content.clone()),
                                            tool_call: msg.tool_calls.as_ref().and_then(
                                                |calls| {
                                                    calls.first().map(|tc| {
                                                        (
                                                            tc.function.name.clone(),
                                                            tc.function.name.clone(),
                                                            tc.function.arguments.clone(),
                                                        )
                                                    })
                                                },
                                            ),
                                            done: chunk.done,
                                        };
                                        let _ = tx.send(c).await;
                                    } else if chunk.done {
                                        let _ = tx
                                            .send(Chunk {
                                                content: None,
                                                tool_call: None,
                                                done: true,
                                            })
                                            .await;
                                    }
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        Ok(rx)
    }
}
