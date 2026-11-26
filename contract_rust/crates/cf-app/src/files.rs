//! Hộp thoại Mở/Lưu file (rfd) và mở file/thư mục bằng ứng dụng mặc định (open).
//! Ghi bytes ra đường dẫn người dùng đã chọn: đó là lựa chọn rõ ràng nên cho phép ghi đè.

use std::{
    io,
    path::{Path, PathBuf},
};

/// Một nhóm đuôi file cho hộp thoại: (tên hiển thị, các đuôi không có dấu chấm).
pub type Filter<'a> = (&'a str, &'a [&'a str]);

/// Hộp thoại Lưu. Trả None nếu người dùng hủy.
pub fn save_as(suggested_name: &str, filter: Filter<'_>) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_file_name(suggested_name)
        .add_filter(filter.0, filter.1)
        .save_file()
}

#[allow(dead_code)] // dùng từ T4.B05 (giấy tờ khách hàng)
/// Hộp thoại Mở một file. Trả None nếu người dùng hủy.
pub fn pick_file(filters: &[Filter<'_>]) -> Option<PathBuf> {
    apply_filters(rfd::FileDialog::new(), filters).pick_file()
}

#[allow(dead_code)] // dùng từ T4.G (kèm theo)
/// Hộp thoại Mở nhiều file. Danh sách rỗng nếu người dùng hủy.
pub fn pick_files(filters: &[Filter<'_>]) -> Vec<PathBuf> {
    apply_filters(rfd::FileDialog::new(), filters)
        .pick_files()
        .unwrap_or_default()
}

#[allow(dead_code)] // dùng từ T4.E (tải file hợp đồng)
/// Mở file bằng ứng dụng mặc định của hệ điều hành.
pub fn open_path(path: &Path) -> io::Result<()> {
    open::that(path)
}

/// Mở thư mục chứa `path` trong trình quản lý file. Nếu `path` không có thư mục cha thì mở chính nó.
pub fn open_folder(path: &Path) -> io::Result<()> {
    let folder = path.parent().unwrap_or(path);
    open::that(folder)
}

/// Ghi bytes ra đường dẫn đã chọn, ghi đè nếu đã có.
pub fn write_chosen(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, bytes)
}

fn apply_filters(mut dialog: rfd::FileDialog, filters: &[Filter<'_>]) -> rfd::FileDialog {
    for (name, extensions) in filters {
        dialog = dialog.add_filter(*name, extensions);
    }
    dialog
}

#[cfg(test)]
mod tests {
    use super::write_chosen;

    #[test]
    fn write_chosen_overwrites_existing_file() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("thu.txt");
        write_chosen(&path, b"cu")?;
        write_chosen(&path, b"moi")?;
        assert_eq!(std::fs::read(&path)?, b"moi");
        Ok(())
    }
}
