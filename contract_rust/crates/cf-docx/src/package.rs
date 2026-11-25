use std::io::{Cursor, Read, Seek, Write};

use zip::{ZipArchive, write::ZipWriter};

use crate::DocxError;

/// Gói DOCX đã được kiểm tra có thể mở như ZIP; entry được giữ theo thứ tự gốc.
#[derive(Debug, Clone)]
pub struct Package {
    bytes: Vec<u8>,
    entries: Vec<String>,
}

impl Package {
    /// Đọc các entry ZIP, giữ thứ tự ban đầu để render chép lại gói.
    pub fn read(bytes: &[u8]) -> Result<Self, DocxError> {
        let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec()))
            .map_err(|error| DocxError::Open(error.to_string()))?;
        let mut entries = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|error| DocxError::Open(error.to_string()))?;
            entries.push(
                entry
                    .name()
                    .map_err(|error| DocxError::Open(error.to_string()))?
                    .into_owned(),
            );
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            entries,
        })
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn entry_index(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|entry| entry == name)
    }

    pub fn read_entry(&self, index: usize) -> Result<Vec<u8>, DocxError> {
        let mut archive = ZipArchive::new(Cursor::new(self.bytes.as_slice()))?;
        let mut entry = archive.by_index(index)?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Chép entry không đổi trực tiếp từ dữ liệu ZIP đã nén, theo API raw-copy của zip 9.8.
    /// Source: https://docs.rs/zip/9.8.2/zip/write/struct.ZipWriter.html#method.raw_copy_file
    pub fn copy_raw<W: Write + Seek>(
        &self,
        index: usize,
        writer: &mut ZipWriter<W>,
    ) -> Result<(), DocxError> {
        let mut archive = ZipArchive::new(Cursor::new(self.bytes.as_slice()))?;
        let entry = archive.by_index_raw(index)?;
        writer.raw_copy_file(entry)?;
        Ok(())
    }
}

/// Nhận diện các phần XML có nội dung chữ cần render/analyze.
pub fn is_text_part(name: &str) -> bool {
    name == "word/document.xml"
        || name == "word/footnotes.xml"
        || name == "word/endnotes.xml"
        || ((name.starts_with("word/header") || name.starts_with("word/footer"))
            && name.ends_with(".xml"))
}
