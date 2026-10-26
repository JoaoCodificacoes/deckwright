use std::path::PathBuf;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("part {part}: {message}")]
    InvalidPart { part: String, message: String },
    #[error("limit exceeded: {0}")]
    Limit(String),
    #[error("relationship {id} in {part}: {message}")]
    Relationship {
        part: String,
        id: String,
        message: String,
    },
}
pub type Result<T> = std::result::Result<T, Error>;
