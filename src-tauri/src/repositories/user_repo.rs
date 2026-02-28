use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar_path: Option<String>,
    pub bio: Option<String>,
    pub locale: String,
    pub timezone: String,
    pub is_active: i64,
    pub last_active_at: Option<String>,
    pub metadata: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    pub id: Option<String>,
    pub username: String,
    pub display_name: String,
}

#[derive(Clone)]
pub struct UserRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl UserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            read_pool: pool.clone(),
            write_pool: pool,
        }
    }

    pub fn with_pools(read_pool: SqlitePool, write_pool: SqlitePool) -> Self {
        Self {
            read_pool,
            write_pool,
        }
    }

    pub async fn create_user(&self, user: NewUser) -> Result<User, AppError> {
        let id = user.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        sqlx::query("INSERT INTO users (id, username, display_name) VALUES (?1, ?2, ?3)")
            .bind(&id)
            .bind(&user.username)
            .bind(&user.display_name)
            .execute(&self.write_pool)
            .await?;

        self.get_user(&id).await?.ok_or_else(|| AppError::NotFound {
            entity: "user".to_string(),
            id,
        })
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>, AppError> {
        let row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        let row = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?1")
            .bind(username)
            .fetch_optional(&self.read_pool)
            .await?;
        Ok(row)
    }

    pub async fn get_or_create_default_user(&self) -> Result<User, AppError> {
        if let Some(user) = self.get_user_by_username("default").await? {
            return Ok(user);
        }

        self.create_user(NewUser {
            id: Some("default".to_string()),
            username: "default".to_string(),
            display_name: "Default User".to_string(),
        })
        .await
    }

    pub async fn set_last_active(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE users SET last_active_at = datetime('now','utc') WHERE id = ?1")
            .bind(id)
            .execute(&self.write_pool)
            .await?;
        Ok(())
    }
}
