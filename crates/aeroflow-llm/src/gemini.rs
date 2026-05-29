use crate::client::LLMClient;
use crate::types::{Chunk, Message, Role, ToolCall, ToolDef};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    FunctionCall { function_call: GeminiFunctionCall },
    FunctionResponse { function_response: GeminiFunctionResponse },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiTool {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiStreamChunk {
    candidates: Option<Vec<GeminiStreamCandidate>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiStreamCandidate {
    content: Option<GeminiContent>,
    finish_reason: Option<String>,
}

pub struct GeminiClient {
    http: HttpClient,
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            http: HttpClient::new(),
            api_key,
            model,
        }
    }

    fn api_url(&self, stream: bool) -> String {
        if stream {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
                self.model, self.api_key
            )
        } else {
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                self.model, self.api_key
            )
        }
    }

    fn convert_messages(messages: &[Message], system: &mut Option<String>) -> Vec<GeminiContent> {
        let mut result = Vec::new();
        for msg in messages {
            match msg.role {
                Role::System => {
                    *system = Some(msg.content.clone());
                }
                Role::User => {
                    result.push(GeminiContent {
                        role: "user".into(),
                        parts: vec![GeminiPart::Text { text: msg.content.clone() }],
                    });
                }
                Role::Assistant => {
                    let mut parts = Vec::new();
                    if !msg.content.is_empty() {
                        parts.push(GeminiPart::Text { text: msg.content.clone() });
                    }
                    for tc in &msg.tool_calls {
                        parts.push(GeminiPart::FunctionCall {
                            function_call: GeminiFunctionCall {
                                name: tc.name.clone(),
                                args: tc.arguments.clone(),
                            },
                        });
                    }
                    result.push(GeminiContent {
                        role: "model".into(),
                        parts,
                    });
                }
                Role::Tool => {
                    for tr in &msg.tool_results {
                        result.push(GeminiContent {
                            role: "function".into(),
                            parts: vec![GeminiPart::FunctionResponse {
                                function_response: GeminiFunctionResponse {
                                    name: tr.name.clone(),
                                    response: serde_json::json!({"result": tr.content}),
                                },
                            }],
                        });
                    }
                }
            }
        }
        result
    }

    fn convert_tools(tools: &[ToolDef]) -> Vec<GeminiTool> {
        if tools.is_empty() {
            return vec![];
        }
        vec![GeminiTool {
            function_declarations: tools
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                })
                .collect(),
        }]
    }
}

#[async_trait]
impl LLMClient for GeminiClient {
    async fn chat(&self, messages: &[Message], tools: &[ToolDef]) -> anyhow::Result<Message> {
        let mut system = None;
        let contents = Self::convert_messages(messages, &mut system);
        let body = GeminiRequest {
            system_instruction: system.map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart::Text { text: s }],
            }),
            tools: {
                let t = Self::convert_tools(tools);
                if t.is_empty() { None } else { Some(t) }
            },
            contents,
        };

        let resp = self
            .http
            .post(self.api_url(false))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<GeminiResponse>()
            .await?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();
        if let Some(candidate) = resp.candidates.into_iter().next() {
            for part in candidate.content.parts {
                match part {
                    GeminiPart::Text { text } => content.push_str(&text),
                    GeminiPart::FunctionCall { function_call } => {
                        tool_calls.push(ToolCall {
                            id: format!("fc_{}", function_call.name),
                            name: function_call.name,
                            arguments: function_call.args,
                            result: None,
                        });
                    }
                    _ => {}
                }
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
        let contents = Self::convert_messages(messages, &mut system);
        let body = GeminiRequest {
            system_instruction: system.map(|s| GeminiSystemInstruction {
                parts: vec![GeminiPart::Text { text: s }],
            }),
            tools: {
                let t = Self::convert_tools(tools);
                if t.is_empty() { None } else { Some(t) }
            },
            contents,
        };

        let url = self.api_url(true);
        let http = self.http.clone();

        tokio::spawn(async move {
            match http
                .post(&url)
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
                                    if data.is_empty() {
                                        continue;
                                    }
                                    if let Ok(ev) = serde_json::from_str::<GeminiStreamChunk>(data)
                                        && let Some(candidates) = ev.candidates {
                                            for c in candidates {
                                                if let Some(finish) = &c.finish_reason
                                                    && finish != "STOP" && finish != "MAX_TOKENS" {
                                                        continue;
                                                    }
                                                if let Some(content) = c.content {
                                                    for part in content.parts {
                                                        match part {
                                                            GeminiPart::Text { text } => {
                                                                let _ = tx.send(Chunk {
                                                                    content: Some(text),
                                                                    tool_call: None,
                                                                    done: false,
                                                                }).await;
                                                            }
                                                            GeminiPart::FunctionCall { function_call } => {
                                                                let _ = tx.send(Chunk {
                                                                    content: None,
                                                                    tool_call: Some((
                                                                        format!("fc_{}", function_call.name),
                                                                        function_call.name,
                                                                        function_call.args,
                                                                    )),
                                                                    done: false,
                                                                }).await;
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                                if let Some(finish) = c.finish_reason
                                                    && (finish == "STOP" || finish == "MAX_TOKENS") {
                                                        let _ = tx.send(Chunk {
                                                            content: None,
                                                            tool_call: None,
                                                            done: true,
                                                        }).await;
                                                    }
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
