use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "type", content = "details")]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Model inference error: {0}")]
    Inference(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("MCP error: {mcp_id} - {message}")]
    McpError { mcp_id: String, message: String },

    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Encryption error: {0}")]
    Crypto(String),

    #[error("Hardware detection error: {0}")]
    Hardware(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Rate limited: {0}")]
    RateLimit(String),
}

impl AppError {
    /// Wrap an existing error with additional context message.
    pub fn context(self, msg: impl Into<String>) -> Self {
        let ctx = msg.into();
        match self {
            Self::Database(e) => Self::Database(format!("{ctx}: {e}")),
            Self::Inference(e) => Self::Inference(format!("{ctx}: {e}")),
            Self::Embedding(e) => Self::Embedding(format!("{ctx}: {e}")),
            Self::Io(e) => Self::Io(format!("{ctx}: {e}")),
            Self::Internal(e) => Self::Internal(format!("{ctx}: {e}")),
            Self::Config(e) => Self::Config(format!("{ctx}: {e}")),
            Self::Crypto(e) => Self::Crypto(format!("{ctx}: {e}")),
            Self::Hardware(e) => Self::Hardware(format!("{ctx}: {e}")),
            Self::Timeout(e) => Self::Timeout(format!("{ctx}: {e}")),
            Self::RateLimit(e) => Self::RateLimit(format!("{ctx}: {e}")),
            other => other, // Structured variants pass through unchanged
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        match &value {
            sqlx::Error::PoolTimedOut => {
                Self::Timeout(format!("Database connection pool timed out: {value}"))
            }
            sqlx::Error::ColumnNotFound(col) => {
                Self::Database(format!("Column '{col}' not found: {value}"))
            }
            sqlx::Error::RowNotFound => {
                Self::NotFound {
                    entity: "row".to_string(),
                    id: "unknown".to_string(),
                }
            }
            _ => Self::Database(value.to_string()),
        }
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(value: sqlx::migrate::MigrateError) -> Self {
        Self::Database(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(value: keyring::Error) -> Self {
        Self::Crypto(value.to_string())
    }
}

impl From<base64::DecodeError> for AppError {
    fn from(value: base64::DecodeError) -> Self {
        Self::Crypto(value.to_string())
    }
}

impl From<aes_gcm::Error> for AppError {
    fn from(_: aes_gcm::Error) -> Self {
        Self::Crypto("Invalid encrypted payload or key".to_string())
    }
}

impl From<flume::RecvError> for AppError {
    fn from(value: flume::RecvError) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<flume::SendError<crate::services::background_service::BackgroundTask>> for AppError {
    fn from(value: flume::SendError<crate::services::background_service::BackgroundTask>) -> Self {
        Self::Internal(value.to_string())
    }
}
