use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Ghi file DOCX mới, tự thêm hậu tố nếu tên đã tồn tại và xóa file dở khi lỗi ghi.
pub fn write_new_file(
    directory: &Path,
    file_name: &str,
    bytes: &[u8],
) -> io::Result<(PathBuf, String)> {
    let requested = Path::new(file_name);
    let stem = requested
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document");
    let extension = requested
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    for index in 1_u64.. {
        let name = if index == 1 {
            file_name.to_owned()
        } else {
            format!("{stem}_{index}{extension}")
        };
        let path = directory.join(&name);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = file
            .write_all(bytes)
            .and_then(|_| file.flush())
            .and_then(|_| file.sync_all());
        if let Err(error) = result {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        return Ok((path, name));
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Không tìm được tên file chưa tồn tại.",
    ))
}
