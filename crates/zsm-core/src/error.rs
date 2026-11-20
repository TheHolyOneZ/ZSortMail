#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("{0} was not found")]
    NotFound(&'static str),

    #[error("{0}")]
    Invalid(String),

    #[error("migration {name} failed: {source}")]
    Migration {
        name: &'static str,
        #[source]
        source: rusqlite::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
