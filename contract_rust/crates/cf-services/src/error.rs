use cf_core::error::CoreError;
use cf_db::DbError;
use cf_docx::DocxError;

/// Lỗi của tầng service. Biến thể mang `String` là câu tiếng Việt y hệt bản Python.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Python LookupError / HTTP 404.
    #[error("{0}")]
    NotFound(String),
    /// Python ValueError: hiện nguyên câu cho người dùng.
    #[error("{0}")]
    Invalid(String),
    /// Python FileNotFoundError.
    #[error("{0}")]
    FileMissing(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] DbError),
}

impl ServiceError {
    /// Câu hiện cho người dùng.
    pub fn user_message(&self) -> String {
        match self {
            Self::NotFound(message) | Self::Invalid(message) | Self::FileMissing(message) => {
                message.clone()
            }
            Self::Io(error) => error.to_string(),
            Self::Db(error) => error.to_string(),
        }
    }
}

impl From<CoreError> for ServiceError {
    fn from(error: CoreError) -> Self {
        match error {
            CoreError::Invalid(message) => Self::Invalid(message),
        }
    }
}

impl From<DocxError> for ServiceError {
    fn from(error: DocxError) -> Self {
        match error {
            DocxError::Io(error) => Self::Io(error),
            other => Self::Invalid(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ServiceError;

    #[test]
    fn invalid_user_message_is_the_text_as_is() {
        assert_eq!(ServiceError::Invalid("x".into()).user_message(), "x");
    }
}
