use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Gmail could not be reached: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Gmail said no ({status}): {message}")]
    Api { status: u16, reason: String, message: String },

    #[error("Gmail asked us to slow down")]
    RateLimited { retry_after: Option<Duration> },

    #[error("Google sign-in needs to be done again: {0}")]
    NeedsRelink(String),

    #[error("Google did not give ZSortMail the Gmail permission: {0}")]
    MissingPermission(String),

    #[error("{0}")]
    Auth(String),

    #[error("Gmail no longer has our place in the history")]
    HistoryGone,

    #[error("Gmail sent something unexpected: {0}")]
    Unexpected(String),

    #[error(transparent)]
    Store(#[from] zsm_core::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub const MISSING_PERMISSION: &str = "Google did not give ZSortMail permission to use Gmail. Link the account again and keep the Gmail box ticked on Google's screen.";

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Network(_) | Self::RateLimited { .. } => true,
            Self::Api { status, .. } => *status >= 500 || *status == 408 || *status == 429,
            _ => false,
        }
    }

    pub fn for_person(&self) -> String {
        match self {
            Self::Network(_) => "Gmail did not answer. Trying again shortly. Your changes are saved.".into(),
            Self::RateLimited { .. } => "Gmail asked us to slow down. Trying again shortly.".into(),
            Self::NeedsRelink(_) => "Your Google sign-in has expired. Link the account again to keep syncing.".into(),
            Self::MissingPermission(_) => MISSING_PERMISSION.into(),
            Self::Api { message, .. } => format!("Gmail said: {message}"),
            other => other.to_string(),
        }
    }
}
