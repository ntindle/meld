use thiserror::Error;

#[derive(Error, Debug)]
pub enum MeldError {
    #[error("sessions root not found: {0}")]
    RootNotFound(String),

    #[error("another meld instance is running (lock file: {0})")]
    Locked(String),

    #[error("no snapshot available to restore")]
    NoSnapshot,

    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = anyhow::Result<T>;
