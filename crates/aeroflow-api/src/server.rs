use aeroflow_core::{
    metrics, CreateUserRequest, SettingsManager, SystemEvent, User, UserRole,
};
use aeroflow_pipeline::PipelineOrchestrator;
use aeroflow_skills::{GeometryFingerprint, SkillsDb, UserManager};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{sse::Event, Json, Sse},
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    iat: usize,
    role: String,
}

const JWT_SECRET: &str = "aeroflow-jwt-secret-change-me";

fn encode_token(user_id: &str, role: &str) -> Result<String, anyhow::Error> {
    let now = Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp: now + 86400,
        iat: now,
        role: role.to_string(),
    };
    Ok(encode(&Header::default(), &claims, &EncodingKey::from_secret(JWT_SECRET.as_bytes()))?)
}

fn decode_token(token: &str) -> Result<Claims, anyhow::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

#[derive(Clone)]
pub struct AppState {
    pub db: SkillsDb,
    pub user_mgr: UserManager,
    pub event_tx: broadcast::Sender<SystemEvent>,
    pub orchestrator: Arc<Mutex<PipelineOrchestrator>>,
    pub settings: AeroflowSettings,
}

#[derive(Clone)]
pub struct AeroflowSettings {
    pub workspace_dir: String,
    pub allow_registration: bool,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    token: String,
    user: User,
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    name: String,
    email: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: T,
}

#[derive(Debug, Serialize)]
struct ApiError {
    success: bool,
    error: String,
}

#[derive(Debug, Deserialize)]
struct Pagination {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CreateCaseRequest {
    name: String,
    geometry_id: Option<uuid::Uuid>,
    solver: String,
    flow_type: String,
}

#[derive(Debug, Deserialize)]
struct UploadStlRequest {
    name: String,
    solver: String,
    flow_type: String,
    stl_data: String,
    stl_filename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateCaseRequest {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApproveRequest {
    stage: String,
    action: String,
    params: Option<serde_json::Value>,
}

fn extract_user(headers: &HeaderMap) -> Result<Claims, (StatusCode, Json<ApiError>)> {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    success: false,
                    error: "Missing or invalid Authorization header".into(),
                }),
            )
        })?;
    decode_token(auth).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                success: false,
                error: "Invalid or expired token".into(),
            }),
        )
    })
}

fn require_admin(claims: &Claims) -> Result<(), (StatusCode, Json<ApiError>)> {
    if claims.role != "admin" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                success: false,
                error: "Admin access required".into(),
            }),
        ));
    }
    Ok(())
}

async fn health(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": true,
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ApiError>)> {
    let user = state
        .user_mgr
        .authenticate(&req.email, &req.password)
        .await
        .map_err(|e| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    success: false,
                    error: format!("Authentication failed: {}", e),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    success: false,
                    error: "Invalid email or password".into(),
                }),
            )
        })?;

    let token = encode_token(&user.id.to_string(), &user.role.label()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                success: false,
                error: format!("Token generation failed: {}", e),
            }),
        )
    })?;

    Ok(Json(LoginResponse { token, user }))
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<User>>, (StatusCode, Json<ApiError>)> {
    if !state.settings.allow_registration {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                success: false,
                error: "Registration is disabled".into(),
            }),
        ));
    }

    let user_req = CreateUserRequest {
        name: req.name,
        email: req.email,
        password: req.password,
        role: UserRole::Engineer,
    };

    let user = state.user_mgr.create_user(&user_req).await.map_err(|e| {
        (
            StatusCode::CONFLICT,
            Json(ApiError {
                success: false,
                error: format!("Registration failed: {}", e),
            }),
        )
    })?;

    Ok(Json(ApiResponse {
        success: true,
        data: user,
    }))
}

async fn list_cases(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(pagination): Query<Pagination>,
) -> Result<Json<ApiResponse<Vec<serde_json::Value>>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    let limit = pagination.limit.unwrap_or(20);
    let cases = state.db.list_cases(limit).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                success: false,
                error: format!("Failed to list cases: {}", e),
            }),
        )
    })?;

    let mut json_cases: Vec<serde_json::Value> = Vec::new();
    for c in cases {
        let case_dir = PathBuf::from(&state.settings.workspace_dir)
            .join("cases")
            .join(&c.name);
        let has_report = case_dir.join("report/index.html").exists();
        json_cases.push(serde_json::json!({
            "id": c.id,
            "name": c.name,
            "status": c.status,
            "flow_type": c.flow_type,
            "solver": c.solver,
            "has_report": has_report,
            "created_at": c.created_at,
            "completed_at": c.completed_at,
        }));
    }

    Ok(Json(ApiResponse {
        success: true,
        data: json_cases,
    }))
}

async fn get_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let cases = state.db.list_cases(100).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: e.to_string() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);

            let manifest = std::fs::read_to_string(case_dir.join("manifest.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());

            let detail = serde_json::json!({
                "id": c.id,
                "name": c.name,
                "status": c.status,
                "flow_type": c.flow_type,
                "solver": c.solver,
                "manifest": manifest,
                "created_at": c.created_at,
                "completed_at": c.completed_at,
            });

            Ok(Json(ApiResponse { success: true, data: detail }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn get_case_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let cases = state.db.list_cases(100).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);
            let report_dir = case_dir.join("report");

            let manifest = std::fs::read_to_string(case_dir.join("manifest.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());

            let forces = read_forces(&case_dir);
            let mesh = read_mesh_quality(&case_dir);
            let iterations = read_iterations(&case_dir);
            let report_exists = report_dir.join("index.html").exists();
            let viz_images = if report_exists {
                std::fs::read_dir(report_dir.join("images"))
                    .ok()
                    .map(|entries| {
                        entries
                            .filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .filter(|n| n.ends_with(".png"))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            } else {
                Vec::<String>::new()
            };

            let stl_files: Vec<String> = std::fs::read_dir(case_dir.join("constant/triSurface"))
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .filter(|n| n.ends_with(".stl"))
                        .collect()
                })
                .unwrap_or_default();

            let detail = serde_json::json!({
                "id": c.id,
                "name": c.name,
                "status": c.status,
                "flow_type": c.flow_type,
                "solver": c.solver,
                "manifest": manifest,
                "forces": forces,
                "mesh_quality": mesh,
                "iterations": iterations,
                "has_report": report_exists,
                "viz_images": viz_images,
                "stl_files": stl_files,
                "created_at": c.created_at,
                "completed_at": c.completed_at,
            });

            Ok(Json(ApiResponse { success: true, data: detail }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn get_case_stl(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<(StatusCode, [(String, String); 1], Vec<u8>), (StatusCode, Json<ApiError>)> {
    let cases = state.db.list_cases(100).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);
            let tri_surface = case_dir.join("constant/triSurface");

            let stl_file = std::fs::read_dir(&tri_surface)
                .map_err(|_| {
                    (StatusCode::NOT_FOUND, Json(ApiError { success: false, error: "No triSurface directory".into() }))
                })?
                .filter_map(|e| e.ok())
                .find(|e| e.file_name().to_string_lossy().ends_with(".stl"))
                .ok_or_else(|| {
                    (StatusCode::NOT_FOUND, Json(ApiError { success: false, error: "No STL file found".into() }))
                })?;

            let data = std::fs::read(stl_file.path()).map_err(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "Failed to read STL".into() }))
            })?;

            Ok((StatusCode::OK, [("Content-Type".into(), "model/stl".into())], data))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn create_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCaseRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let claims = extract_user(&headers)?;
    let user_id: uuid::Uuid = claims.sub.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                success: false,
                error: "Invalid user ID in token".into(),
            }),
        )
    })?;

    let case_dir = PathBuf::from(&state.settings.workspace_dir)
        .join("cases")
        .join(&req.name);

    std::fs::create_dir_all(case_dir.join("0")).ok();
    std::fs::create_dir_all(case_dir.join("constant/triSurface")).ok();
    std::fs::create_dir_all(case_dir.join("system")).ok();
    std::fs::create_dir_all(case_dir.join("logs")).ok();

    let case_id = state
        .db
        .create_case(
            &req.name,
            Some(user_id),
            req.geometry_id,
            &req.solver,
            &req.flow_type,
            &case_dir.to_string_lossy(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError {
                    success: false,
                    error: format!("Failed to create case: {}", e),
                }),
            )
        })?;

    let manifest = serde_json::json!({
        "name": req.name,
        "case_id": case_id.to_string(),
        "geometry_id": req.geometry_id.map(|id| id.to_string()),
        "solver": req.solver,
        "flow_type": req.flow_type,
        "created_at": Utc::now().to_rfc3339(),
    });
    std::fs::write(case_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).ok();

    let _ = state.event_tx.send(SystemEvent::info(
        Some(case_id),
        "api",
        format!("Case '{}' created via API", req.name),
    ));

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": case_id, "name": req.name, "path": case_dir }),
    }))
}

async fn upload_stl(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<UploadStlRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let claims = extract_user(&headers)?;
    let user_id: uuid::Uuid = claims.sub.parse().map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(ApiError { success: false, error: "Invalid user ID in token".into() }))
    })?;

    // Decode base64 STL
    let stl_bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &req.stl_data)
        .map_err(|_| (StatusCode::BAD_REQUEST, Json(ApiError { success: false, error: "Invalid base64 STL data".into() })))?;

    let filename = req.stl_filename.unwrap_or_else(|| "geometry.stl".to_string());

    // Save to temp file
    let tmp_dir = PathBuf::from(&state.settings.workspace_dir).join("temp");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("Failed to create temp dir: {}", e) }))
    })?;
    let stl_path = tmp_dir.join(format!("{}_{}", &req.name, &filename));
    std::fs::write(&stl_path, &stl_bytes).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("Failed to save STL: {}", e) }))
    })?;

    // Compute geometry fingerprint
    let fingerprint = GeometryFingerprint::from_stl(&stl_path).map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(ApiError { success: false, error: format!("Failed to process STL: {}", e) }))
    })?;

    // Check for duplicate or insert
    let geometry_id = match state.db.find_geometry_by_hash(&fingerprint.sha256_hash).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("DB error: {}", e) }))
    })? {
        Some(gid) => gid,
        None => state.db.insert_geometry(&fingerprint).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("Failed to store geometry: {}", e) }))
        })?,
    };

    // Create case directory structure
    let case_dir = PathBuf::from(&state.settings.workspace_dir)
        .join("cases")
        .join(&req.name);
    std::fs::create_dir_all(case_dir.join("0")).ok();
    std::fs::create_dir_all(case_dir.join("constant/triSurface")).ok();
    std::fs::create_dir_all(case_dir.join("system")).ok();
    std::fs::create_dir_all(case_dir.join("logs")).ok();

    // Copy STL to case directory
    let stl_dest = case_dir.join("constant/triSurface/geometry.stl");
    std::fs::copy(&stl_path, &stl_dest).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("Failed to copy STL: {}", e) }))
    })?;

    // Clean up temp file
    std::fs::remove_file(&stl_path).ok();

    // Create case in DB
    let case_id = state.db.create_case(
        &req.name,
        Some(user_id),
        Some(geometry_id),
        &req.solver,
        &req.flow_type,
        &case_dir.to_string_lossy(),
    ).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("Failed to create case: {}", e) }))
    })?;

    // Write manifest
    let manifest = serde_json::json!({
        "name": req.name,
        "case_id": case_id.to_string(),
        "geometry_id": geometry_id.to_string(),
        "solver": req.solver,
        "flow_type": req.flow_type,
        "num_triangles": fingerprint.num_triangles,
        "created_at": Utc::now().to_rfc3339(),
    });
    std::fs::write(case_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).ok();

    let _ = state.event_tx.send(SystemEvent::info(
        Some(case_id),
        "api",
        format!("Case '{}' created via STL upload", req.name),
    ));

    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": case_id, "name": req.name, "geometry_id": geometry_id, "num_triangles": fingerprint.num_triangles }),
    }))
}

async fn update_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<UpdateCaseRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let cases = state.db.list_cases(100).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);

            if let Some(ref new_name) = req.name {
                let new_dir = PathBuf::from(&state.settings.workspace_dir)
                    .join("cases")
                    .join(new_name);
                std::fs::rename(&case_dir, &new_dir).ok();
                // Update manifest
                if let Ok(mut manifest) = std::fs::read_to_string(new_dir.join("manifest.json"))
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
                {
                    if let Some(obj) = manifest.as_object_mut() {
                        obj.insert("name".into(), serde_json::Value::String(new_name.clone()));
                    }
                    std::fs::write(new_dir.join("manifest.json"), serde_json::to_string_pretty(&manifest).unwrap()).ok();
                }
            }

            Ok(Json(ApiResponse {
                success: true,
                data: serde_json::json!({ "id": id, "updated": true }),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn delete_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let cases = state.db.list_cases(100).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            // Remove case directory
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);
            std::fs::remove_dir_all(&case_dir).ok();

            state.db.delete_case(id).await.map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: format!("Failed to delete case: {}", e) }))
            })?;

            let _ = state.event_tx.send(SystemEvent::info(
                Some(id),
                "api",
                format!("Case '{}' deleted", c.name),
            ));

            Ok(Json(ApiResponse {
                success: true,
                data: serde_json::json!({ "id": id, "deleted": true }),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn approve_stage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<ApproveRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let cases = state.db.list_cases(100).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);

            if let Some(ref params) = req.params {
                let params_path = case_dir.join(format!("approved_{}.json", req.stage.to_lowercase()));
                std::fs::write(&params_path, serde_json::to_string_pretty(params).unwrap()).ok();
            }

            let _ = state.event_tx.send(SystemEvent::info(
                Some(id),
                "api",
                format!("Stage '{}' {} for case '{}'", req.stage, req.action, c.name),
            ));

            Ok(Json(ApiResponse {
                success: true,
                data: serde_json::json!({
                    "case_id": id,
                    "stage": req.stage,
                    "action": req.action,
                    "approved": true,
                }),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn run_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let cases = state.db.list_cases(100).await.map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "DB error".into() }))
    })?;
    let found = cases.into_iter().find(|c| c.id == id);

    match found {
        Some(c) => {
            let case_dir = PathBuf::from(&state.settings.workspace_dir)
                .join("cases")
                .join(&c.name);

            let mut orch = state.orchestrator.lock().map_err(|_| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { success: false, error: "Lock error".into() }))
            })?;

            let cid = orch.register_case_with_id(&c.name, id);
            match orch.run_pipeline(cid, &case_dir, c.solver.as_deref().unwrap_or("simpleFoam"), None, None) {
                Ok(result) => Ok(Json(ApiResponse {
                    success: true,
                    data: serde_json::json!({ "case_id": id, "status": format!("{:?}", result.stage) }),
                })),
                Err(e) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError { success: false, error: format!("Pipeline failed: {}", e) }),
                )),
            }
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { success: false, error: "Case not found".into() }),
        )),
    }
}

async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<Vec<User>>>, (StatusCode, Json<ApiError>)> {
    let claims = extract_user(&headers)?;
    require_admin(&claims)?;

    let users = state.user_mgr.list_users().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                success: false,
                error: format!("Failed to list users: {}", e),
            }),
        )
    })?;

    Ok(Json(ApiResponse {
        success: true,
        data: users,
    }))
}

async fn metrics_handler() -> (StatusCode, String) {
    (StatusCode::OK, metrics::gather_metrics())
}

async fn events_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => Some(Ok(Event::default()
            .event("aeroflow-event")
            .data(serde_json::to_string(&event).unwrap_or_default()))),
        Err(_) => None,
    });
    Ok(Sse::new(stream))
}

fn read_forces(case_dir: &PathBuf) -> Option<serde_json::Value> {
    let path = case_dir.join("postProcessing/forceCoeffs/0/coefficient.dat");
    let data = std::fs::read_to_string(path).ok()?;
    let last_line = data.lines().filter(|l| !l.starts_with('#')).last()?;
    let parts: Vec<&str> = last_line.split_whitespace().collect();
    if parts.len() >= 6 {
        Some(serde_json::json!({
            "cd": parts[1].parse::<f64>().unwrap_or(0.0),
            "cl": parts[3].parse::<f64>().unwrap_or(0.0),
            "cm": parts[5].parse::<f64>().unwrap_or(0.0),
        }))
    } else {
        None
    }
}

fn read_mesh_quality(case_dir: &PathBuf) -> Option<serde_json::Value> {
    let log_path = case_dir.join("logs/checkMesh.log");
    let log = std::fs::read_to_string(log_path).ok()?;

    let n_cells = log.lines()
        .find(|l| l.contains("cells:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().split_whitespace().next())
        .and_then(|s| s.parse::<u64>().ok());

    let max_non_ortho = log.lines()
        .find(|l| l.contains("Max non-orthogonality"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|s| s.trim().split_whitespace().next())
        .and_then(|s| s.parse::<f64>().ok());

    Some(serde_json::json!({
        "n_cells": n_cells,
        "max_non_orthogonality": max_non_ortho,
    }))
}

fn read_iterations(case_dir: &PathBuf) -> Option<serde_json::Value> {
    let path = case_dir.join("postProcessing/forceCoeffs/0/coefficient.dat");
    let data = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = data.lines().filter(|l| !l.starts_with('#')).collect();
    let n_iter = lines.len();
    let last = lines.last()?;
    let parts: Vec<&str> = last.split_whitespace().collect();
    if parts.len() >= 2 {
        Some(serde_json::json!({
            "iterations": n_iter,
            "last_time": parts[0].parse::<f64>().unwrap_or(0.0),
        }))
    } else {
        None
    }
}

pub struct WebApi {
    frontend_path: Option<PathBuf>,
}

impl WebApi {
    pub fn new() -> Self {
        Self { frontend_path: None }
    }

    pub fn with_frontend(path: PathBuf) -> Self {
        Self { frontend_path: Some(path) }
    }

    pub async fn start(self) -> Result<(), anyhow::Error> {
        let settings = SettingsManager::load();
        let database_url = settings.settings.database_url.clone();
        let workspace_dir = settings.settings.workspace_dir.clone();
        let allow_reg = settings.settings.allow_registration;
        let port: u16 = std::env::var("AEROFLOW_API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let db = SkillsDb::connect(&database_url).await?;
        let user_mgr = UserManager::new(db.pool().clone());
        let (event_tx, _) = broadcast::channel(1024);

        let data_dir = PathBuf::from(&workspace_dir);
        let orchestrator = Arc::new(Mutex::new(PipelineOrchestrator::new(
            data_dir.clone(),
            settings.settings.max_concurrent_cases,
        ).with_db(db.clone())));

        let state = AppState {
            db,
            user_mgr,
            event_tx: event_tx.clone(),
            orchestrator,
            settings: AeroflowSettings {
                workspace_dir,
                allow_registration: allow_reg,
            },
        };

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let metrics_route = Router::new()
            .route("/metrics", get(metrics_handler));

        let api_routes = Router::new()
            .route("/api/health", get(health))
            .route("/api/auth/login", post(login))
            .route("/api/auth/register", post(register))
            .route("/api/cases/upload", post(upload_stl))
            .route("/api/cases", get(list_cases).post(create_case))
            .route("/api/cases/{id}", get(get_case).put(update_case).delete(delete_case))
            .route("/api/cases/{id}/detail", get(get_case_detail))
            .route("/api/cases/{id}/stl", get(get_case_stl))
            .route("/api/cases/{id}/run", post(run_case))
            .route("/api/cases/{id}/approve", post(approve_stage))
            .route("/api/users", get(list_users))
            .route("/api/events", get(events_handler));

        let mut app = Router::new()
            .merge(metrics_route)
            .merge(api_routes)
            .layer(cors)
            .with_state(state);

        if let Some(ref frontend) = self.frontend_path {
            let serve_dir = ServeDir::new(frontend).append_index_html_on_directories(true);
            app = app.fallback_service(serve_dir);
            info!("Serving frontend from {:?}", frontend);
        }

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("AeroFlow web UI ready at http://0.0.0.0:{}", port);
        println!("\n  ╔══════════════════════════════════════════╗");
        println!("  ║   AeroFlow Agent — Web Workspace        ║");
        println!("  ║   Open: http://localhost:{}          ║", port);
        println!("  ╚══════════════════════════════════════════╝\n");

        axum::serve(listener, app).await?;

        Ok(())
    }
}
