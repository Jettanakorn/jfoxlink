use aeroflow_core::SkillId;
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
                if !stmt.is_empty() && !stmt.starts_with("--") {
                    if let Err(e) = sqlx::query(stmt).execute(pool).await {
                        tracing::warn!("Migration statement skipped: {}", e);
                    }
                }
            }
            sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
                .bind("001")
                .execute(pool).await?;
            tracing::info!("Applied migration 001");
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
