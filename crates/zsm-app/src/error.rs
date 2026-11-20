#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Store(#[from] zsm_core::Error),

    #[error("No mailbox is linked yet")]
    NoAccount,

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Gmail(#[from] zsm_gmail::Error),
}

impl Error {
    pub fn for_person(&self) -> String {
        match self {
            Self::Gmail(error) => error.for_person(),
            other => other.to_string(),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
