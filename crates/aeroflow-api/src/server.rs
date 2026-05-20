use aeroflow_core::{
    metrics, CreateUserRequest, SettingsManager, SystemEvent, User, UserRole,
};
use aeroflow_pipeline::PipelineOrchestrator;
use aeroflow_skills::{SkillsDb, UserManager};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{sse::Event, Json, Sse},
    routing::{get, post},
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
use tracing::info;

// ── JWT ──

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

// ── App state ──

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

// ── Request / Response types ──

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
    #[allow(dead_code)]
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CreateCaseRequest {
    name: String,
    geometry_id: uuid::Uuid,
    solver: String,
    flow_type: String,
}

// ── Auth extraction ──

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

// ── Route handlers ──

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

    let json_cases: Vec<serde_json::Value> = cases
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "name": c.name,
                "status": c.status,
                "flow_type": c.flow_type,
                "solver": c.solver,
                "created_at": c.created_at,
                "completed_at": c.completed_at,
            })
        })
        .collect();

    Ok(Json(ApiResponse {
        success: true,
        data: json_cases,
    }))
}

async fn get_case(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;
    Ok(Json(ApiResponse {
        success: true,
        data: serde_json::json!({ "id": id, "status": "fetched" }),
    }))
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
    std::fs::create_dir_all(case_dir.join("constant")).ok();
    std::fs::create_dir_all(case_dir.join("system")).ok();

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
        "geometry_id": req.geometry_id.to_string(),
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

async fn run_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiError>)> {
    let _claims = extract_user(&headers)?;

    let mut orch = state.orchestrator.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                success: false,
                error: "Failed to acquire orchestrator lock".into(),
            }),
        )
    })?;

    let case_dir = PathBuf::from(&state.settings.workspace_dir)
        .join("cases")
        .join(id.to_string());

    let case_id = orch.register_case(&id.to_string());
    match orch.run_pipeline(case_id, &case_dir, "simpleFoam", None) {
        Ok(stage) => Ok(Json(ApiResponse {
            success: true,
            data: serde_json::json!({ "case_id": id, "status": format!("{:?}", stage) }),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                success: false,
                error: format!("Pipeline failed: {}", e),
            }),
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

// ── Server ──

pub struct WebApi;

impl WebApi {
    pub fn new() -> Self {
        Self
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
        )));

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
            .route("/api/cases", get(list_cases).post(create_case))
            .route("/api/cases/{id}", get(get_case))
            .route("/api/cases/{id}/run", post(run_case))
            .route("/api/users", get(list_users))
            .route("/api/events", get(events_handler));

        let app = Router::new()
            .merge(metrics_route)
            .merge(api_routes)
            .layer(cors)
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        info!("AeroFlow API server listening on http://0.0.0.0:{}", port);

        axum::serve(listener, app).await?;

        Ok(())
    }
}
