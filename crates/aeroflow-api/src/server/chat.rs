use crate::server::{extract_user, ApiError, AppState, ApiResponse};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{sse::Event, Json, Sse},
    routing::{delete, get, post},
    Router,
};
use aeroflow_llm::{
    anthropic::AnthropicClient,
    client::LLMClient,
    gemini::GeminiClient,
    ollama::OllamaClient,
    prompts::build_system_prompt,
    tools::{execute_tool, get_all_tools, ToolExecutorState},
    types::*,
};
use aeroflow_skills::SkillsDb;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use uuid::Uuid;

pub fn chat_routes() -> Router<AppState> {
    Router::new()
        .route("/api/chat/stream", post(chat_stream_handler))
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/conversations", get(list_conversations_handler).post(create_conversation_handler))
        .route("/api/chat/conversations/{id}", get(get_conversation_handler))
        .route("/api/chat/conversations/{id}", delete(delete_conversation_handler))
        .route("/api/agent/models", get(list_models_handler))
        .route("/api/agent/config", get(get_agent_config_handler).put(set_agent_config_handler))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatQuery {
    case_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ChatBody {
    case_id: Uuid,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversation_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

#[derive(Debug, Serialize)]
struct ConversationListResponse {
    conversations: Vec<aeroflow_skills::Conversation>,
}

#[derive(Debug, Serialize)]
struct ConversationDetailResponse {
    conversation: aeroflow_skills::Conversation,
    messages: Vec<aeroflow_skills::ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ModelInfo {
    id: String,
    provider: String,
    name: String,
    available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentConfig {
    model: String,
}

fn create_llm_client(model: &str, _db: &SkillsDb) -> Option<Box<dyn LLMClient>> {
    if model.starts_with("ollama:") {
        let model_name = model.trim_start_matches("ollama:");
        let base_url = std::env::var("OLLAMA_API_URL").unwrap_or_else(|_| "http://localhost:11434".into());
        Some(Box::new(OllamaClient::new(base_url, model_name.to_string())))
    } else if model.starts_with("anthropic:") {
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok()?;
        let model_name = model.trim_start_matches("anthropic:");
        Some(Box::new(AnthropicClient::new(api_key, model_name.to_string())))
    } else if model.starts_with("gemini:") {
        let api_key = std::env::var("GEMINI_API_KEY").ok()?;
        let model_name = model.trim_start_matches("gemini:");
        Some(Box::new(GeminiClient::new(api_key, model_name.to_string())))
    } else {
        None
    }
}

async fn chat_stream_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChatBody>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let model = body.model.unwrap_or_else(|| "ollama:deepseek-r1:14b".into());
    let client = create_llm_client(&model, &state.db).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ApiError { success: false, error: format!("Unknown model: {}", model) }))
    })?;

    // Get or create conversation
    let conv_id = if let Some(cid) = body.conversation_id {
        if state.db.get_conversation(cid).await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
        })?.is_some() {
            cid
        } else {
            state.db.create_conversation(body.case_id, &model).await.map_err(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
            })?.id
        }
    } else {
        state.db.create_conversation(body.case_id, &model).await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
        })?.id
    };

    // Load previous messages
    let db_messages = state.db.get_messages(conv_id).await.unwrap_or_default();

    // Build case context
    let case_detail = get_case_detail_json(&state, body.case_id).await;
    let system_prompt = build_system_prompt(case_detail.as_ref());

    // Convert conversation to LLM messages
    let mut llm_messages: Vec<Message> = db_messages.iter().map(|m| {
        let role = match m.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            "tool" => Role::Tool,
            _ => Role::User,
        };
        Message {
            role,
            content: m.content.clone(),
            tool_calls: serde_json::from_value(m.tool_calls.clone()).unwrap_or_default(),
            tool_results: serde_json::from_value(m.tool_results.clone()).unwrap_or_default(),
        }
    }).collect();

    // Prepend system prompt
    llm_messages.insert(0, Message {
        role: Role::System,
        content: system_prompt,
        tool_calls: vec![],
        tool_results: vec![],
    });

    // Add user message
    llm_messages.push(Message {
        role: Role::User,
        content: body.message.clone(),
        tool_calls: vec![],
        tool_results: vec![],
    });

    // Save user message
    state.db.add_message(conv_id, "user", &body.message, &serde_json::json!([]), &serde_json::json!([])).await.ok();

    let tools = get_all_tools();
    let max_loops = std::env::var("AGENT_MAX_TOOL_LOOPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5u32);

    let (tx, rx) = mpsc::channel(256);
    let tx_clone = tx.clone();
    let _db_clone = state.db.clone();
    let conv_id_clone = conv_id;
    let tool_state = ToolExecutorState::new(state.db.clone()).with_orchestrator(
        state.orchestrator.clone(),
        PathBuf::from(&state.settings.workspace_dir),
    );

    // Spawn a task to handle the LLM conversation loop
    tokio::spawn(async move {
        let mut current_messages = llm_messages;
        let mut has_user_msg = true;
        let mut loop_count = 0;

        while has_user_msg && loop_count < max_loops {
            has_user_msg = false;
            loop_count += 1;

            match client.chat_stream(&current_messages, &tools).await {
                Ok(mut stream) => {
                    let mut assistant_content = String::new();
                    let mut assistant_tool_calls: Vec<ToolCall> = Vec::new();
                    let mut is_final = false;

                    while let Some(chunk) = stream.recv().await {
                        if chunk.done {
                            is_final = true;
                            break;
                        }
                        if let Some(content) = &chunk.content
                            && !content.is_empty() {
                                assistant_content.push_str(content);
                                let _ = tx_clone.send(SseEvent::Token { content: content.clone() }).await;
                            }
                        if let Some((id, name, args)) = &chunk.tool_call {
                            assistant_tool_calls.push(ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: args.clone(),
                                result: None,
                            });
                            let _ = tx_clone.send(SseEvent::ToolCall {
                                tool: name.clone(),
                                args: args.clone(),
                                id: id.clone(),
                            }).await;
                        }
                    }

                    if !is_final {
                        // Try non-streaming fallback to get remaining content
                        if let Ok(fallback_msg) = client.chat(&current_messages, &tools).await {
                            if fallback_msg.content.len() > assistant_content.len() {
                                let new_part = &fallback_msg.content[assistant_content.len()..];
                                if !new_part.is_empty() {
                                    assistant_content.push_str(new_part);
                                    let _ = tx_clone.send(SseEvent::Token { content: new_part.to_string() }).await;
                                }
                            }
                            assistant_tool_calls = fallback_msg.tool_calls;
                            for tc in &assistant_tool_calls {
                                let _ = tx_clone.send(SseEvent::ToolCall {
                                    tool: tc.name.clone(),
                                    args: tc.arguments.clone(),
                                    id: tc.id.clone(),
                                }).await;
                            }
                        }
                    }

                    // Save assistant message
                    state.db.add_message(
                        conv_id_clone,
                        "assistant",
                        &assistant_content,
                        &serde_json::to_value(&assistant_tool_calls).unwrap_or_default(),
                        &serde_json::json!([]),
                    ).await.ok();

                    // Execute tool calls
                    for tc in &assistant_tool_calls {
                        let result = execute_tool(&tc.name, &tc.arguments, &tool_state).await.unwrap_or_else(|e| {
                            serde_json::json!({"error": e.to_string()})
                        });
                        let _ = tx_clone.send(SseEvent::ToolResult {
                            tool: tc.name.clone(),
                            result: result.clone(),
                        }).await;

                        // Save tool result
                        state.db.add_message(
                            conv_id_clone,
                            "tool",
                            &serde_json::to_string(&result).unwrap_or_default(),
                            &serde_json::json!([]),
                            &serde_json::json!([{
                                "tool_call_id": tc.id,
                                "name": tc.name,
                                "content": serde_json::to_string(&result).unwrap_or_default()
                            }]),
                        ).await.ok();

                        // Add tool result to messages for next LLM iteration
                        current_messages.push(Message {
                            role: Role::Assistant,
                            content: assistant_content.clone(),
                            tool_calls: vec![tc.clone()],
                            tool_results: vec![],
                        });
                        current_messages.push(Message {
                            role: Role::Tool,
                            content: serde_json::to_string(&result).unwrap_or_default(),
                            tool_calls: vec![],
                            tool_results: vec![],
                        });

                        has_user_msg = true;
                    }

                    if !has_user_msg {
                        let _ = tx_clone.send(SseEvent::Done {
                            message_id: Uuid::new_v4().to_string(),
                        }).await;
                    }
                }
                Err(e) => {
                    let _ = tx_clone.send(SseEvent::Error { message: e.to_string() }).await;
                    break;
                }
            }
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|event| {
        Ok(Event::default().event("chat").data(serde_json::to_string(&event).unwrap_or_default()))
    });

    Ok(Sse::new(stream))
}

async fn chat_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChatBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let model = body.model.unwrap_or_else(|| "ollama:deepseek-r1:14b".into());
    let client = create_llm_client(&model, &state.db).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ApiError { success: false, error: format!("Unknown model: {}", model) }))
    })?;

    let conv_id = if let Some(cid) = body.conversation_id {
        if state.db.get_conversation(cid).await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
        })?.is_some() { cid } else {
            state.db.create_conversation(body.case_id, &model).await.map_err(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
            })?.id
        }
    } else {
        state.db.create_conversation(body.case_id, &model).await.map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
        })?.id
    };

    state.db.add_message(conv_id, "user", &body.message, &serde_json::json!([]), &serde_json::json!([])).await.ok();

    let case_detail = get_case_detail_json(&state, body.case_id).await;
    let system_prompt = build_system_prompt(case_detail.as_ref());
    let tools = get_all_tools();

    let mut llm_messages = vec![
        Message { role: Role::System, content: system_prompt, tool_calls: vec![], tool_results: vec![] },
        Message { role: Role::User, content: body.message, tool_calls: vec![], tool_results: vec![] },
    ];

    let max_loops = std::env::var("AGENT_MAX_TOOL_LOOPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5u32);
    let tool_state = ToolExecutorState::new(state.db.clone()).with_orchestrator(
        state.orchestrator.clone(),
        PathBuf::from(&state.settings.workspace_dir),
    );

    for _ in 0..max_loops {
        let msg = client.chat(&llm_messages, &tools).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: e.to_string() }))
        })?;

        if msg.tool_calls.is_empty() {
            state.db.add_message(conv_id, "assistant", &msg.content, &serde_json::json!([]), &serde_json::json!([])).await.ok();
            return Ok(Json(ApiResponse {
                success: true,
                data: serde_json::json!({
                    "conversation_id": conv_id,
                    "message": msg.content,
                    "tool_calls": []
                }),
            }));
        }

        state.db.add_message(conv_id, "assistant", &msg.content, &serde_json::to_value(&msg.tool_calls).unwrap_or_default(), &serde_json::json!([])).await.ok();
        llm_messages.push(Message {
            role: Role::Assistant,
            content: msg.content.clone(),
            tool_calls: msg.tool_calls.clone(),
            tool_results: vec![],
        });

        for tc in &msg.tool_calls {
            let result = execute_tool(&tc.name, &tc.arguments, &tool_state).await.unwrap_or_else(|e| {
                serde_json::json!({"error": e.to_string()})
            });
            state.db.add_message(conv_id, "tool", &serde_json::to_string(&result).unwrap_or_default(), &serde_json::json!([]), &serde_json::json!([{
                "tool_call_id": tc.id, "name": tc.name, "content": serde_json::to_string(&result).unwrap_or_default()
            }])).await.ok();

            llm_messages.push(Message {
                role: Role::Tool,
                content: serde_json::to_string(&result).unwrap_or_default(),
                tool_calls: vec![],
                tool_results: vec![],
            });
        }
    }

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({
            "conversation_id": conv_id,
            "message": "Max tool loops reached",
            "tool_calls": []
        }),
    }))
}

#[derive(Debug, Deserialize)]
struct CreateConversationBody {
    case_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

async fn create_conversation_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateConversationBody>,
) -> Result<Json<ApiResponse<aeroflow_skills::Conversation>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    let model = body.model.unwrap_or_else(|| "ollama:deepseek-r1:14b".into());
    let conv = state.db.create_conversation(body.case_id, &model).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    Ok(Json(ApiResponse { success: true, data: conv }))
}

async fn list_conversations_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ChatQuery>,
) -> Result<Json<ApiResponse<ConversationListResponse>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    let conversations = state.db.list_conversations(query.case_id).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    Ok(Json(ApiResponse { success: true, data: ConversationListResponse { conversations } }))
}

async fn get_conversation_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<ConversationDetailResponse>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    let conversation = state.db.get_conversation(id).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?.ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(ApiError { success: false, error: "Conversation not found".into() }))
    })?;
    let messages = state.db.get_messages(id).await.unwrap_or_default();
    Ok(Json(ApiResponse { success: true, data: ConversationDetailResponse { conversation, messages } }))
}

async fn delete_conversation_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    state.db.delete_conversation(id).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    Ok(Json(ApiResponse { success: true, data: serde_json::json!({"deleted": true}) }))
}

async fn list_models_handler() -> Json<ApiResponse<Vec<ModelInfo>>> {
    let models = vec![
        ModelInfo { id: "ollama:deepseek-r1:14b".into(), provider: "Ollama".into(), name: "DeepSeek R1 14B".into(), available: true },
        ModelInfo { id: "ollama:qwen2.5:14b".into(), provider: "Ollama".into(), name: "Qwen 2.5 14B".into(), available: true },
        ModelInfo { id: "anthropic:claude-sonnet-4-20250514".into(), provider: "Anthropic".into(), name: "Claude Sonnet 4".into(), available: std::env::var("ANTHROPIC_API_KEY").is_ok() },
        ModelInfo { id: "anthropic:claude-haiku-3-5-sonnet-20241022".into(), provider: "Anthropic".into(), name: "Claude Haiku 3.5".into(), available: std::env::var("ANTHROPIC_API_KEY").is_ok() },
        ModelInfo { id: "gemini:gemini-2.5-pro".into(), provider: "Gemini".into(), name: "Gemini 2.5 Pro".into(), available: std::env::var("GEMINI_API_KEY").is_ok() },
        ModelInfo { id: "gemini:gemini-2.5-flash".into(), provider: "Gemini".into(), name: "Gemini 2.5 Flash".into(), available: std::env::var("GEMINI_API_KEY").is_ok() },
    ];
    Json(ApiResponse { success: true, data: models })
}

async fn get_agent_config_handler() -> Json<ApiResponse<AgentConfig>> {
    let model = std::env::var("AGENT_DEFAULT_MODEL").unwrap_or_else(|_| "ollama:deepseek-r1:14b".into());
    Json(ApiResponse { success: true, data: AgentConfig { model } })
}

async fn set_agent_config_handler(
    Json(body): Json<AgentConfig>,
) -> Json<ApiResponse<AgentConfig>> {
    Json(ApiResponse { success: true, data: body })
}

async fn get_case_detail_json(state: &AppState, case_id: Uuid) -> Option<serde_json::Value> {
    let cases = state.db.list_cases(100).await.ok()?;
    let found = cases.into_iter().find(|c| c.id == case_id)?;
    let case_dir = std::path::PathBuf::from(&state.settings.workspace_dir)
        .join("cases")
        .join(&found.name);
    let manifest = std::fs::read_to_string(case_dir.join("manifest.json")).ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    Some(serde_json::json!({
        "id": found.id,
        "name": found.name,
        "status": found.status,
        "solver": found.solver,
        "flow_type": found.flow_type,
        "manifest": manifest,
    }))
}
