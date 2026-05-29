use aeroflow_core::SkillId;
use serde::Serialize;
use sqlx::postgres::PgPool;
use sqlx::Row;
use uuid::Uuid;

use crate::fingerprint::GeometryFingerprint;

#[derive(Clone)]
pub struct SkillsDb {
    pool: PgPool,
}

impl SkillsDb {
    pub async fn connect(database_url: &str) -> Result<Self, anyhow::Error> {
        let pool = PgPool::connect(database_url).await?;
        Self::run_migrations(&pool).await?;
        tracing::info!("Skills database connected and migrated");
        Ok(Self { pool })
    }

    pub async fn new_with_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn run_migrations(pool: &PgPool) -> Result<(), anyhow::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version TEXT PRIMARY KEY, applied_at TIMESTAMPTZ DEFAULT NOW())"
        ).execute(pool).await?;

        let exists = sqlx::query_scalar::<_, String>(
            "SELECT version FROM schema_migrations WHERE version = $1"
        )
        .bind("001")
        .fetch_optional(pool).await?;

        if exists.is_none() {
            let schema = include_str!("../../../db/migrations/001_initial_schema.sql");
            for statement in schema.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--")
                    && let Err(e) = sqlx::query(stmt).execute(pool).await {
                        tracing::warn!("Migration statement skipped: {}", e);
                    }
            }
            sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
                .bind("001")
                .execute(pool).await?;
            tracing::info!("Applied migration 001");
        }

        let exists_002 = sqlx::query_scalar::<_, String>(
            "SELECT version FROM schema_migrations WHERE version = $1"
        )
        .bind("002")
        .fetch_optional(pool).await?;

        if exists_002.is_none() {
            let schema = include_str!("../../../db/migrations/002_chat.sql");
            for statement in schema.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--")
                    && let Err(e) = sqlx::query(stmt).execute(pool).await {
                        tracing::warn!("Migration 002 statement skipped: {}", e);
                    }
            }
            sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
                .bind("002")
                .execute(pool).await?;
            tracing::info!("Applied migration 002");
        }

        let exists_003 = sqlx::query_scalar::<_, String>(
            "SELECT version FROM schema_migrations WHERE version = $1"
        )
        .bind("003")
        .fetch_optional(pool).await?;

        if exists_003.is_none() {
            let schema = include_str!("../../../db/migrations/003_agent_loop.sql");
            for statement in schema.split(';') {
                let stmt = statement.trim();
                if !stmt.is_empty() && !stmt.starts_with("--")
                    && let Err(e) = sqlx::query(stmt).execute(pool).await {
                        tracing::warn!("Migration 003 statement skipped: {}", e);
                    }
            }
            sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
                .bind("003")
                .execute(pool).await?;
            tracing::info!("Applied migration 003");
        }

        Ok(())
    }

    pub async fn list_skills(&self) -> Result<Vec<SkillSummary>, anyhow::Error> {
        let rows = sqlx::query(
            "SELECT id, name, version, confidence, n_trials, reward_score FROM skills WHERE active = true ORDER BY confidence DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let skills = rows.iter().map(|r| {
            SkillSummary {
                id: r.get("id"),
                name: r.get("name"),
                version: r.get("version"),
                confidence: r.get("confidence"),
                n_trials: r.get("n_trials"),
                reward_score: r.get("reward_score"),
            }
        }).collect();

        Ok(skills)
    }

    pub async fn insert_skill(
        &self,
        name: &str,
        geometry_id: SkillId,
        flow_regime_key: &str,
        parameters: &serde_json::Value,
    ) -> Result<SkillId, anyhow::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO skills (name, geometry_id, flow_regime_key, parameters)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#
        )
        .bind(name)
        .bind(geometry_id)
        .bind(flow_regime_key)
        .bind(parameters)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn get_trials(&self, skill_id: SkillId, limit: i64) -> Result<Vec<TrialSummary>, anyhow::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, parameters, reward, converged, runtime_s, peak_memory_gb, created_at
            FROM parameter_trials
            WHERE skill_id = $1 AND deprecated = false
            ORDER BY reward DESC
            LIMIT $2
            "#
        )
        .bind(skill_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| TrialSummary {
            id: r.get("id"),
            parameters: r.get("parameters"),
            reward: r.get("reward"),
            converged: r.get("converged"),
            runtime_s: r.get("runtime_s"),
            peak_memory_gb: r.get("peak_memory_gb"),
            created_at: r.get("created_at"),
        }).collect())
    }

    pub async fn insert_trial(
        &self,
        skill_id: SkillId,
        case_id: Option<SkillId>,
        parameters: &serde_json::Value,
        reward: f64,
        converged: bool,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO parameter_trials (skill_id, case_id, parameters, reward, converged)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(skill_id)
        .bind(case_id)
        .bind(parameters)
        .bind(reward)
        .bind(converged)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_skill(&self, id: SkillId) -> Result<Option<SkillDetail>, anyhow::Error> {
        let rows = sqlx::query(
            r#"
            SELECT s.id, s.name, s.version, s.flow_regime_key,
                   s.reward_score, s.confidence, s.n_trials,
                   s.parameters, s.gp_model,
                   g.sha256_hash, g.bounding_box, g.surface_area, g.volume
            FROM skills s
            JOIN geometries g ON s.geometry_id = g.id
            WHERE s.id = $1 AND s.active = true
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(rows.map(|r| SkillDetail {
            id: r.get("id"),
            name: r.get("name"),
            version: r.get("version"),
            flow_regime_key: r.get("flow_regime_key"),
            reward_score: r.get("reward_score"),
            confidence: r.get("confidence"),
            n_trials: r.get("n_trials"),
            parameters: r.get("parameters"),
            gp_model: r.get("gp_model"),
            sha256_hash: r.get("sha256_hash"),
            bounding_box: r.get("bounding_box"),
            surface_area: r.get("surface_area"),
            volume: r.get("volume"),
        }))
    }

    pub async fn find_geometry_by_hash(&self, hash: &[u8]) -> Result<Option<Uuid>, anyhow::Error> {
        let row = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM geometries WHERE sha256_hash = $1"
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn insert_geometry(&self, fp: &GeometryFingerprint) -> Result<Uuid, anyhow::Error> {
        let bbox_json = serde_json::json!(fp.bbox);
        let row = sqlx::query(
            r#"
            INSERT INTO geometries (id, sha256_hash, voxel_hash_8, voxel_hash_32, voxel_hash_64,
                                    bounding_box, surface_area, volume, aspect_ratio, num_triangles)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (sha256_hash) DO UPDATE SET
                bounding_box = EXCLUDED.bounding_box,
                surface_area  = EXCLUDED.surface_area,
                volume        = EXCLUDED.volume,
                aspect_ratio  = EXCLUDED.aspect_ratio,
                num_triangles = EXCLUDED.num_triangles
            RETURNING id
            "#
        )
        .bind(fp.geometry_id)
        .bind(&fp.sha256_hash)
        .bind(&fp.voxel_signature.coarse_hash)
        .bind(&fp.voxel_signature.medium_hash)
        .bind(&fp.voxel_signature.fine_hash)
        .bind(&bbox_json)
        .bind(fp.surface_area)
        .bind(fp.volume)
        .bind(fp.aspect_ratio)
        .bind(fp.num_triangles as i32)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn create_case(
        &self,
        name: &str,
        user_id: Option<Uuid>,
        geometry_id: Option<Uuid>,
        solver: &str,
        flow_type: &str,
        case_path: &str,
    ) -> Result<Uuid, anyhow::Error> {
        let row = sqlx::query(
            r#"
            INSERT INTO cases (user_id, name, geometry_id, status, flow_type, solver, report_path)
            VALUES ($1, $2, $3, 'created', $4, $5, $6)
            RETURNING id
            "#
        )
        .bind(user_id)
        .bind(name)
        .bind(geometry_id)
        .bind(flow_type)
        .bind(solver)
        .bind(case_path)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get("id"))
    }

    pub async fn update_case_results(
        &self,
        case_id: Uuid,
        status: &str,
        forces: &serde_json::Value,
        mesh_stats: &serde_json::Value,
        solver_stats: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            UPDATE cases
            SET status = $1,
                results = $2,
                mesh_stats = $3,
                resource_usage = $4,
                completed_at = NOW()
            WHERE id = $5
            "#
        )
        .bind(status)
        .bind(forces)
        .bind(mesh_stats)
        .bind(solver_stats)
        .bind(case_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_case_name(&self, id: Uuid, name: &str) -> Result<(), anyhow::Error> {
        sqlx::query("UPDATE cases SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_cases(&self, limit: i64) -> Result<Vec<CaseSummary>, anyhow::Error> {
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.name, c.status, c.flow_type, c.solver,
                   c.created_at, c.completed_at
            FROM cases c
            ORDER BY c.created_at DESC
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| CaseSummary {
            id: r.get("id"),
            name: r.get("name"),
            status: r.get("status"),
            flow_type: r.get("flow_type"),
            solver: r.get("solver"),
            created_at: r.get("created_at"),
            completed_at: r.get("completed_at"),
        }).collect())
    }

    pub async fn delete_case(&self, id: Uuid) -> Result<(), anyhow::Error> {
        sqlx::query("DELETE FROM events WHERE case_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM cases WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update a skill's reward score and confidence after an agent loop completes
    pub async fn update_skill_score(&self, skill_id: SkillId, score: f64) -> Result<(), anyhow::Error> {
        sqlx::query(
            "UPDATE skills SET reward_score = $1, confidence = GREATEST(confidence, 0.5), n_trials = n_trials + 1, updated_at = NOW() WHERE id = $2"
        )
        .bind(score)
        .bind(skill_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Agent Loop iteration persistence ──

    pub async fn record_agent_iteration(
        &self,
        case_id: Uuid,
        iteration: i32,
        manifest: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO agent_iterations (case_id, iteration, manifest)
            VALUES ($1, $2, $3)
            ON CONFLICT (case_id, iteration) DO UPDATE SET
                manifest = EXCLUDED.manifest
            "#
        )
        .bind(case_id)
        .bind(iteration)
        .bind(manifest)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_agent_iteration_results(
        &self,
        case_id: Uuid,
        iteration: i32,
        forces: &serde_json::Value,
        mesh_quality: &serde_json::Value,
        convergence: &serde_json::Value,
        score: f64,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            UPDATE agent_iterations
            SET forces = $1, mesh_quality = $2, convergence = $3, score = $4
            WHERE case_id = $5 AND iteration = $6
            "#
        )
        .bind(forces)
        .bind(mesh_quality)
        .bind(convergence)
        .bind(score)
        .bind(case_id)
        .bind(iteration)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_agent_iterations(&self, case_id: Uuid) -> Result<Vec<serde_json::Value>, anyhow::Error> {
        let rows = sqlx::query(
            r#"
            SELECT iteration, manifest, forces, mesh_quality, convergence, score, created_at
            FROM agent_iterations
            WHERE case_id = $1
            ORDER BY iteration ASC
            "#
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| {
            serde_json::json!({
                "iteration": r.get::<i32, _>("iteration"),
                "manifest": r.get::<serde_json::Value, _>("manifest"),
                "forces": r.get::<Option<serde_json::Value>, _>("forces"),
                "mesh_quality": r.get::<Option<serde_json::Value>, _>("mesh_quality"),
                "convergence": r.get::<Option<serde_json::Value>, _>("convergence"),
                "score": r.get::<Option<f64>, _>("score"),
                "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            })
        }).collect())
    }

    pub async fn record_agent_fix(
        &self,
        case_id: Uuid,
        iteration: i32,
        diagnosis: &str,
        fix_action: &str,
        details: &str,
        manifest: &serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO agent_fixes (case_id, iteration, diagnosis, fix_action, details, manifest)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(case_id)
        .bind(iteration)
        .bind(diagnosis)
        .bind(fix_action)
        .bind(details)
        .bind(manifest)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_agent_fixes(&self, case_id: Uuid) -> Result<Vec<serde_json::Value>, anyhow::Error> {
        let rows = sqlx::query(
            r#"
            SELECT iteration, diagnosis, fix_action, details, manifest, created_at
            FROM agent_fixes
            WHERE case_id = $1
            ORDER BY created_at ASC
            "#
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| {
            serde_json::json!({
                "iteration": r.get::<i32, _>("iteration"),
                "diagnosis": r.get::<String, _>("diagnosis"),
                "fix_action": r.get::<String, _>("fix_action"),
                "details": r.get::<String, _>("details"),
                "manifest": r.get::<serde_json::Value, _>("manifest"),
                "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            })
        }).collect())
    }
}

#[derive(Debug)]
pub struct CaseSummary {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub flow_type: Option<String>,
    pub solver: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug)]
pub struct SkillSummary {
    pub id: Uuid,
    pub name: String,
    pub version: i32,
    pub confidence: f64,
    pub n_trials: i32,
    pub reward_score: f64,
}

#[derive(Debug)]
pub struct TrialSummary {
    pub id: SkillId,
    pub parameters: serde_json::Value,
    pub reward: f64,
    pub converged: bool,
    pub runtime_s: Option<f64>,
    pub peak_memory_gb: Option<f64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug)]
pub struct SkillDetail {
    pub id: Uuid,
    pub name: String,
    pub version: i32,
    pub flow_regime_key: String,
    pub reward_score: f64,
    pub confidence: f64,
    pub n_trials: i32,
    pub parameters: serde_json::Value,
    pub gp_model: Option<Vec<u8>>,
    pub sha256_hash: Vec<u8>,
    pub bounding_box: serde_json::Value,
    pub surface_area: Option<f64>,
    pub volume: Option<f64>,
}

// ── Conversations ──

#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub id: Uuid,
    pub case_id: Uuid,
    pub model: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub tool_calls: serde_json::Value,
    pub tool_results: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SkillsDb {
    pub async fn create_conversation(&self, case_id: Uuid, model: &str) -> Result<Conversation, anyhow::Error> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO conversations (id, case_id, model, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(id)
        .bind(case_id)
        .bind(model)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(Conversation { id, case_id, model: model.to_string(), created_at: now, updated_at: now })
    }

    pub async fn list_conversations(&self, case_id: Uuid) -> Result<Vec<Conversation>, anyhow::Error> {
        let rows = sqlx::query(
            "SELECT id, case_id, model, created_at, updated_at FROM conversations WHERE case_id = $1 ORDER BY updated_at DESC"
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| Conversation {
            id: r.get("id"),
            case_id: r.get("case_id"),
            model: r.get("model"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }).collect())
    }

    pub async fn get_conversation(&self, id: Uuid) -> Result<Option<Conversation>, anyhow::Error> {
        let row = sqlx::query(
            "SELECT id, case_id, model, created_at, updated_at FROM conversations WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Conversation {
            id: r.get("id"),
            case_id: r.get("case_id"),
            model: r.get("model"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    pub async fn delete_conversation(&self, id: Uuid) -> Result<(), anyhow::Error> {
        sqlx::query("DELETE FROM conversations WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_message(&self, conversation_id: Uuid, role: &str, content: &str, tool_calls: &serde_json::Value, tool_results: &serde_json::Value) -> Result<ChatMessage, anyhow::Error> {
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO chat_messages (id, conversation_id, role, content, tool_calls, tool_results, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
        .bind(id)
        .bind(conversation_id)
        .bind(role)
        .bind(content)
        .bind(tool_calls)
        .bind(tool_results)
        .bind(now)
        .execute(&self.pool)
        .await?;
        sqlx::query("UPDATE conversations SET updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(conversation_id)
            .execute(&self.pool)
            .await?;
        Ok(ChatMessage { id, conversation_id, role: role.to_string(), content: content.to_string(), tool_calls: tool_calls.clone(), tool_results: tool_results.clone(), created_at: now })
    }

    pub async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<ChatMessage>, anyhow::Error> {
        let rows = sqlx::query(
            "SELECT id, conversation_id, role, content, tool_calls, tool_results, created_at FROM chat_messages WHERE conversation_id = $1 ORDER BY created_at"
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| ChatMessage {
            id: r.get("id"),
            conversation_id: r.get("conversation_id"),
            role: r.get("role"),
            content: r.get("content"),
            tool_calls: r.get("tool_calls"),
            tool_results: r.get("tool_results"),
            created_at: r.get("created_at"),
        }).collect())
    }
}
