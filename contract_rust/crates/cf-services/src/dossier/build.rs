use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::SystemTime,
};

use cf_db::paths::DataPaths;
use chrono::NaiveDateTime;

use super::plan::DossierPlan;
use crate::{Ctx, ServiceError};

pub const DOSSIER_MARKER: &str = ".cf_dossier";
const MARKER_HEADER: &str = "ContractForge — bộ hồ sơ đấu nối (sinh tự động)";
const MARKER_VERSION: &str = "version=2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildResult {
    pub dest_dir: PathBuf,
    pub copied: usize,
    pub removed: usize,
    pub missing_required: usize,
    pub linked: usize,
    pub errors: Vec<String>,
}

impl BuildResult {
    pub fn duplicated_bytes_saved(&self) -> bool {
        self.linked > 0 && self.copied == self.linked
    }
}

pub fn clear_managed_files(root: &Path, dest_dir: &Path) -> Result<usize, ServiceError> {
    let root = fs::canonicalize(root)?;
    let name = dest_dir.file_name().ok_or_else(|| {
        ServiceError::Invalid(format!(
            "Đường dẫn thư mục hồ sơ không hợp lệ: {}",
            dest_dir.display()
        ))
    })?;
    let parent = dest_dir.parent().ok_or_else(|| {
        ServiceError::Invalid(format!(
            "Đường dẫn thư mục hồ sơ không hợp lệ: {}",
            dest_dir.display()
        ))
    })?;
    if fs::canonicalize(parent)? != root {
        return Err(ServiceError::Invalid(format!(
            "Từ chối dọn thư mục ngoài {}: {}",
            root.display(),
            dest_dir.display()
        )));
    }
    let dest = root.join(name);
    if !dest.exists() {
        return Ok(0);
    }
    if fs::canonicalize(&dest)? != dest {
        return Err(ServiceError::Invalid(format!(
            "Từ chối dọn thư mục liên kết ngoài: {}",
            dest_dir.display()
        )));
    }
    let marker = dest.join(DOSSIER_MARKER);
    if !marker.is_file() {
        return Ok(0);
    }
    let marker_text = fs::read_to_string(&marker).unwrap_or_default();
    if marker_text.lines().next() != Some(MARKER_HEADER) {
        return Err(ServiceError::Invalid(format!(
            "Từ chối dọn thư mục có dấu không hợp lệ: {}",
            dest.display()
        )));
    }
    let managed = managed_names(&marker_text);
    let legacy_marker = !marker_text.lines().any(|line| line == MARKER_VERSION);
    let mut removed = 0;
    for entry in fs::read_dir(&dest)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name_text) = name.to_str() else {
            continue;
        };
        if name_text == DOSSIER_MARKER {
            continue;
        }
        let is_managed = if legacy_marker {
            true
        } else {
            managed.iter().any(|candidate| candidate == name_text)
        };
        if is_managed && entry.file_type()?.is_file() && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

fn managed_names(marker_text: &str) -> Vec<String> {
    marker_text
        .lines()
        .skip_while(|line| *line != MARKER_VERSION)
        .skip(1)
        .filter(|line| {
            !line.is_empty()
                && Path::new(line).file_name().and_then(|name| name.to_str()) == Some(*line)
        })
        .map(str::to_owned)
        .collect()
}

fn write_marker(directory: &Path, names: &[String]) -> Result<(), std::io::Error> {
    let mut content = format!("{MARKER_HEADER}\n{MARKER_VERSION}\n");
    for name in names {
        content.push_str(name);
        content.push('\n');
    }
    fs::write(directory.join(DOSSIER_MARKER), content)
}

fn link_or_copy(source: &Path, destination: &Path) -> Result<bool, std::io::Error> {
    match fs::hard_link(source, destination) {
        Ok(()) => Ok(true),
        Err(link_error) if destination.exists() => Err(link_error),
        Err(_) => {
            let mut input = File::open(source)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            std::io::copy(&mut input, &mut output)?;
            output.flush()?;
            Ok(false)
        }
    }
}

pub fn newest_source_mtime(plan: &DossierPlan) -> Option<SystemTime> {
    plan.slots
        .iter()
        .filter_map(|slot| slot.source_path.as_ref())
        .filter_map(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .max()
}

pub fn is_stale(plan: &DossierPlan) -> bool {
    let stamp = plan.dest_dir.join("_checklist.txt");
    let Ok(stamp_time) = fs::metadata(stamp).and_then(|metadata| metadata.modified()) else {
        return false;
    };
    newest_source_mtime(plan).is_some_and(|source_time| source_time > stamp_time)
}

pub fn build_checklist_text(plan: &DossierPlan, timestamp: NaiveDateTime) -> String {
    let name = plan
        .customer
        .short_name
        .as_deref()
        .unwrap_or(&plan.customer.legal_name);
    let mut lines = vec![
        "BỘ HỒ SƠ ĐẤU NỐI".to_owned(),
        "=".repeat(60),
        format!("Khách hàng : {name} ({})", plan.customer.code),
        format!("Tạo lúc    : {}", timestamp.format("%d/%m/%Y %H:%M")),
    ];
    if plan.contracts.is_empty() {
        lines.push("Hợp đồng   : — chưa có hợp đồng nào —".into());
    } else {
        for (index, contract) in plan.contracts.iter().enumerate() {
            let prefix = if index == 0 {
                "Hợp đồng   : "
            } else {
                "             "
            };
            let product_code = contract
                .product_id
                .and_then(|id| {
                    plan.product_labels
                        .get(&id)
                        .map(|product| product.0.as_str())
                })
                .unwrap_or("—");
            lines.push(format!(
                "{prefix}{}  [{product_code}]",
                contract.contract_number
            ));
        }
    }
    lines.push(String::new());
    for slot in &plan.slots {
        if slot.ok() {
            lines.push(format!(
                "  [x] {}{}{}",
                slot.dest_name,
                " ".repeat(
                    42usize
                        .saturating_sub(slot.dest_name.chars().count())
                        .max(1)
                ),
                format_size(slot.size)
            ));
        } else {
            let tag = if slot.slot.required {
                "THIẾU"
            } else {
                "không bắt buộc"
            };
            lines.push(format!(
                "  [ ] {:02} {} — {tag}: {}",
                slot.order, slot.slot.label, slot.note
            ));
        }
    }
    let missing = plan.missing_required();
    lines.push(String::new());
    if missing.is_empty() {
        lines.push("Đủ toàn bộ mục bắt buộc.".into());
    } else {
        lines.push(format!(
            "Thiếu {} mục bắt buộc: {}",
            missing.len(),
            missing
                .iter()
                .map(|slot| slot.slot.label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.push(String::new());
    lines.push("File ở đây liên kết thẳng tới file gốc trong ứng dụng nên không tốn".into());
    lines.push("thêm dung lượng. Xoá thư mục này không làm mất dữ liệu gốc.".into());
    lines.join("\n") + "\n"
}

fn format_size(size: u64) -> String {
    if size >= 1024 * 1024 {
        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
    } else if size >= 1024 {
        format!("{} KB", size / 1024)
    } else {
        format!("{size} B")
    }
}

pub fn materialize(ctx: &Ctx, plan: &DossierPlan) -> Result<BuildResult, ServiceError> {
    fs::create_dir_all(&ctx.paths().dossier)?;
    let root = fs::canonicalize(&ctx.paths().dossier)?;
    let parent = plan.dest_dir.parent().ok_or_else(|| {
        ServiceError::Invalid(format!(
            "Đường dẫn thư mục hồ sơ không hợp lệ: {}",
            plan.dest_dir.display()
        ))
    })?;
    let name = plan.dest_dir.file_name().ok_or_else(|| {
        ServiceError::Invalid(format!(
            "Đường dẫn thư mục hồ sơ không hợp lệ: {}",
            plan.dest_dir.display()
        ))
    })?;
    if fs::canonicalize(parent)? != root {
        return Err(ServiceError::Invalid(format!(
            "Từ chối ghi ngoài thư mục hồ sơ {}",
            root.display()
        )));
    }
    let dest_dir = root.join(name);
    if dest_dir.exists() && fs::canonicalize(&dest_dir)? != dest_dir {
        return Err(ServiceError::Invalid(format!(
            "Từ chối ghi qua liên kết thư mục: {}",
            dest_dir.display()
        )));
    }
    let foreign_folder = dest_dir.exists()
        && !dest_dir.join(DOSSIER_MARKER).exists()
        && fs::read_dir(&dest_dir)?.next().is_some();
    let removed = clear_managed_files(&ctx.paths().dossier, &plan.dest_dir)?;
    fs::create_dir_all(&dest_dir)?;
    let mut result = BuildResult {
        dest_dir: dest_dir.clone(),
        copied: 0,
        removed,
        missing_required: plan.missing_required().len(),
        linked: 0,
        errors: Vec::new(),
    };
    if foreign_folder {
        result.errors.push("Thư mục đã có sẵn file không do ứng dụng tạo — giữ nguyên, chưa dọn. Kiểm tra lại trước khi nộp.".into());
    }
    let mut managed = Vec::new();
    write_marker(&dest_dir, &managed)?;

    for slot in &plan.slots {
        let Some(source) = slot.source_path.as_deref() else {
            continue;
        };
        let destination = dest_dir.join(&slot.dest_name);
        match link_or_copy(source, &destination) {
            Ok(linked) => {
                result.copied += 1;
                result.linked += usize::from(linked);
                managed.push(slot.dest_name.clone());
                if let Err(error) = write_marker(&dest_dir, &managed) {
                    result.errors.push(format!("{DOSSIER_MARKER}: {error}"));
                }
            }
            Err(error) => result.errors.push(format!("{}: {error}", slot.slot.label)),
        }
    }

    let checklist = "_checklist.txt";
    let checklist_path = dest_dir.join(checklist);
    let mut body = vec![0xEF, 0xBB, 0xBF];
    body.extend_from_slice(build_checklist_text(plan, ctx.clock().now_local()).as_bytes());
    let checklist_result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&checklist_path)
        .and_then(|mut file| file.write_all(&body));
    match checklist_result {
        Ok(()) => {
            managed.push(checklist.into());
            if let Err(error) = write_marker(&dest_dir, &managed) {
                result.errors.push(format!("{DOSSIER_MARKER}: {error}"));
            }
        }
        Err(error) => result.errors.push(format!("{checklist}: {error}")),
    }
    Ok(result)
}

pub fn open_in_file_manager(paths: &DataPaths, target: &Path) -> Result<(), ServiceError> {
    let root = fs::canonicalize(&paths.dossier)?;
    let resolved = fs::canonicalize(target)?;
    if !resolved.starts_with(&root) {
        return Err(ServiceError::Invalid(format!(
            "Từ chối mở đường dẫn ngoài {}: {}",
            root.display(),
            target.display()
        )));
    }
    if !resolved.is_dir() {
        return Err(ServiceError::Invalid(format!(
            "Thư mục không tồn tại: {}",
            resolved.display()
        )));
    }
    open::that(resolved)?;
    Ok(())
}
