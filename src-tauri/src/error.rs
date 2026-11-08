use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{code}: {message}")]
    Custom { code: String, message: String },

    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
}

impl AppError {
    pub fn custom(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Custom {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct ErrorPayload {
    code: String,
    message: String,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (code, message) = match self {
            AppError::Db(e) => ("db_error".to_string(), e.to_string()),
            AppError::Io(e) => ("io_error".to_string(), e.to_string()),
            AppError::Network(e) => ("network_error".to_string(), e.to_string()),
            AppError::Json(e) => ("json_error".to_string(), e.to_string()),
            AppError::Custom { code, message } => (code.clone(), message.clone()),
            AppError::Anyhow(e) => ("internal_error".to_string(), e.to_string()),
        };
        ErrorPayload { code, message }.serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
