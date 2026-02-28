use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub id: String,
    pub user_id: Option<String>,
    pub namespace: String,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub is_encrypted: i64,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct SettingsRepo {
    read_pool: SqlitePool,
    write_pool: SqlitePool,
}

impl SettingsRepo {
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

    pub async fn upsert_setting(
        &self,
        user_id: Option<&str>,
        namespace: &str,
        key: &str,
        value: &str,
        value_type: &str,
        is_encrypted: bool,
    ) -> Result<Setting, AppError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO settings (id, user_id, namespace, key, value, value_type, is_encrypted)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(user_id, namespace, key)
            DO UPDATE SET value = excluded.value, value_type = excluded.value_type, is_encrypted = excluded.is_encrypted
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(namespace)
        .bind(key)
        .bind(value)
        .bind(value_type)
        .bind(if is_encrypted { 1 } else { 0 })
        .execute(&self.write_pool)
        .await?;

        self.get_setting(user_id, namespace, key)
            .await?
            .ok_or_else(|| AppError::NotFound {
                entity: "setting".to_string(),
                id: format!("{namespace}:{key}"),
            })
    }

    pub async fn get_setting(
        &self,
        user_id: Option<&str>,
        namespace: &str,
        key: &str,
    ) -> Result<Option<Setting>, AppError> {
        let row = if user_id.is_some() {
            sqlx::query_as::<_, Setting>(
                "SELECT * FROM settings WHERE user_id = ?1 AND namespace = ?2 AND key = ?3",
            )
            .bind(user_id)
            .bind(namespace)
            .bind(key)
            .fetch_optional(&self.read_pool)
            .await?
        } else {
            sqlx::query_as::<_, Setting>(
                "SELECT * FROM settings WHERE user_id IS NULL AND namespace = ?1 AND key = ?2",
            )
            .bind(namespace)
            .bind(key)
            .fetch_optional(&self.read_pool)
            .await?
        };

        Ok(row)
    }

    pub async fn list_namespace(
        &self,
        user_id: Option<&str>,
        namespace: &str,
    ) -> Result<Vec<Setting>, AppError> {
        let rows = if user_id.is_some() {
            sqlx::query_as::<_, Setting>(
                "SELECT * FROM settings WHERE user_id = ?1 AND namespace = ?2 ORDER BY key",
            )
            .bind(user_id)
            .bind(namespace)
            .fetch_all(&self.read_pool)
            .await?
        } else {
            sqlx::query_as::<_, Setting>(
                "SELECT * FROM settings WHERE user_id IS NULL AND namespace = ?1 ORDER BY key",
            )
            .bind(namespace)
            .fetch_all(&self.read_pool)
            .await?
        };

        Ok(rows)
    }
}
