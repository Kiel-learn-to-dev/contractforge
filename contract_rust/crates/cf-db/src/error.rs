#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Schema(String),
    #[error("{0}")]
    Codec(String),
    #[error("Cơ sở dữ liệu đang được một tiến trình khác ghi.")]
    Locked,
}
