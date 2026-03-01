use sqlx::SqlitePool;

use crate::error::AppError;

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
