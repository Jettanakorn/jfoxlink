use aeroflow_core::{CreateUserRequest, UpdateUserRequest, User, UserId, UserRole, Session};
use sha2::Digest;
use sqlx::postgres::PgPool;
use std::str::FromStr;
use sqlx::Row;

#[derive(Clone)]
pub struct UserManager {
    pool: PgPool,
}

impl UserManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_user(&self, req: &CreateUserRequest) -> Result<User, anyhow::Error> {
        let hash = Self::hash_password(&req.password)?;
        let row = sqlx::query(
            r#"
            INSERT INTO users (name, email, password_hash, role)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, email, role, password_hash, active, last_login,
                      quota_max_concurrent, quota_max_cores, quota_max_memory_gb, preferences, created_at
            "#
        )
        .bind(&req.name)
        .bind(&req.email)
        .bind(&hash)
        .bind(req.role.label())
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::row_to_user(&row))
    }

    pub async fn get_user(&self, id: UserId) -> Result<Option<User>, anyhow::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, name, email, role, password_hash, active, last_login,
                   quota_max_concurrent, quota_max_cores, quota_max_memory_gb, preferences, created_at
            FROM users WHERE id = $1
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Self::row_to_user(&r)))
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>, anyhow::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, name, email, role, password_hash, active, last_login,
                   quota_max_concurrent, quota_max_cores, quota_max_memory_gb, preferences, created_at
            FROM users WHERE email = $1
            "#
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Self::row_to_user(&r)))
    }

    pub async fn list_users(&self) -> Result<Vec<User>, anyhow::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, email, role, password_hash, active, last_login,
                   quota_max_concurrent, quota_max_cores, quota_max_memory_gb, preferences, created_at
            FROM users ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(Self::row_to_user).collect())
    }

    pub async fn update_user(&self, id: UserId, req: &UpdateUserRequest) -> Result<User, anyhow::Error> {
        let existing = self.get_user(id).await?
            .ok_or_else(|| anyhow::anyhow!("User not found: {}", id))?;

        let name = req.name.as_deref().unwrap_or(&existing.name);
        let email = req.email.as_deref().unwrap_or(&existing.email);
        let role = req.role.as_ref().map(|r| r.label()).unwrap_or_else(|| {
            UserRole::from_str(existing.role.label()).map(|r| r.label()).unwrap_or("engineer")
        });
        let active = req.active.unwrap_or(existing.active);
        let quota_max_concurrent = req.quota_max_concurrent.unwrap_or(existing.quota_max_concurrent);
        let quota_max_cores = req.quota_max_cores.unwrap_or(existing.quota_max_cores);
        let quota_max_memory_gb = req.quota_max_memory_gb.unwrap_or(existing.quota_max_memory_gb);
        let preferences = req.preferences.as_ref().unwrap_or(&existing.preferences);

        let hash = if let Some(pw) = &req.password {
            Self::hash_password(pw)?
        } else {
            existing.password_hash.unwrap_or_default()
        };

        let row = sqlx::query(
            r#"
            UPDATE users SET
                name = $1, email = $2, password_hash = $3, role = $4,
                active = $5, quota_max_concurrent = $6, quota_max_cores = $7,
                quota_max_memory_gb = $8, preferences = $9
            WHERE id = $10
            RETURNING id, name, email, role, password_hash, active, last_login,
                      quota_max_concurrent, quota_max_cores, quota_max_memory_gb, preferences, created_at
            "#
        )
        .bind(name)
        .bind(email)
        .bind(&hash)
        .bind(role)
        .bind(active)
        .bind(quota_max_concurrent)
        .bind(quota_max_cores)
        .bind(quota_max_memory_gb)
        .bind(preferences)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::row_to_user(&row))
    }

    pub async fn delete_user(&self, id: UserId) -> Result<(), anyhow::Error> {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn authenticate(&self, email: &str, password: &str) -> Result<Option<User>, anyhow::Error> {
        let user = self.get_user_by_email(email).await?;
        match user {
            Some(u) => {
                if let Some(hash) = &u.password_hash {
                    if Self::verify_password(password, hash)? {
                        // Update last_login
                        sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
                            .bind(u.id)
                            .execute(&self.pool).await?;
                        Ok(Some(u))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    pub async fn create_session(&self, user_id: UserId, ttl_hours: u32) -> Result<Session, anyhow::Error> {
        use chrono::{Duration, Utc};
        let token = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        Ok(Session {
            token,
            user_id,
            created_at: now,
            expires_at: now + Duration::hours(ttl_hours as i64),
        })
    }

    fn hash_password(password: &str) -> Result<String, anyhow::Error> {
        let digest = sha2::Sha256::digest(password.as_bytes());
        Ok(format!("sha256:{:x}", digest))
    }

    fn verify_password(password: &str, hash: &str) -> Result<bool, anyhow::Error> {
        if let Some(stored) = hash.strip_prefix("sha256:") {
            let digest = sha2::Sha256::digest(password.as_bytes());
            Ok(format!("{:x}", digest) == stored)
        } else {
            Ok(false)
        }
    }

    fn row_to_user(row: &sqlx::postgres::PgRow) -> User {
        use aeroflow_core::UserRole;
        let role_str: String = row.get("role");
        User {
            id: row.get("id"),
            name: row.get("name"),
            email: row.get("email"),
            role: UserRole::from_str(&role_str).unwrap_or(UserRole::Viewer),
            password_hash: row.get("password_hash"),
            active: row.get("active"),
            last_login: row.get("last_login"),
            quota_max_concurrent: row.get("quota_max_concurrent"),
            quota_max_cores: row.get("quota_max_cores"),
            quota_max_memory_gb: row.get("quota_max_memory_gb"),
            preferences: row.get("preferences"),
            created_at: row.get("created_at"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password_starts_with_sha256() {
        let hash = UserManager::hash_password("test123").unwrap();
        assert!(hash.starts_with("sha256:"));
    }

    #[test]
    fn test_hash_password_different_inputs_different_outputs() {
        let h1 = UserManager::hash_password("password1").unwrap();
        let h2 = UserManager::hash_password("password2").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "correct-horse-battery-staple";
        let hash = UserManager::hash_password(password).unwrap();
        assert!(UserManager::verify_password(password, &hash).unwrap());
    }

    #[test]
    fn test_verify_password_incorrect() {
        let hash = UserManager::hash_password("real-password").unwrap();
        assert!(!UserManager::verify_password("wrong-password", &hash).unwrap());
    }
}
