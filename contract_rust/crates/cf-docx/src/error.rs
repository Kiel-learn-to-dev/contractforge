use std::io;

#[derive(Debug, thiserror::Error)]
pub enum DocxError {
    #[error("Không mở được file .docx: {0}")]
    Open(String),
    #[error("XML không hợp lệ: {0}")]
    Xml(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<zip::result::ZipError> for DocxError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Open(error.to_string())
    }
}
