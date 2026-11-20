use serde::Serialize;

#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum AppError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    NoAccount(String),
    #[error("{0}")]
    Internal(String),
}

impl From<zsm_app::Error> for AppError {
    fn from(err: zsm_app::Error) -> Self {
        match err {
            zsm_app::Error::Store(zsm_core::Error::NotFound(what)) => Self::NotFound(format!("{what} was not found")),
            zsm_app::Error::Store(zsm_core::Error::Invalid(message)) => Self::Invalid(message),
            zsm_app::Error::NoAccount => Self::NoAccount(err.to_string()),
            other => Self::Internal(other.to_string()),
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        Self::Internal(err.to_string())
    }
}
