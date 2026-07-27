"""
template_service.py — Business logic cho module Mẫu hợp đồng.

Chức năng:
  - list_templates / get_template / get_by_code
  - create_template: lưu file .docx + chạy analyzer + lưu DB
  - update_metadata: sửa tên, mô tả, contract_type
  - update_mapping: lưu field_mapping đã chỉnh sửa
  - reanalyze: chạy lại analyzer trên file đã có
  - toggle_active
  - generate_next_code
"""

import os
import re
import uuid
from datetime import datetime
from typing import Optional
from sqlalchemy.orm import Session

from app.models.contract_template import ContractTemplate
from app.services.template_analyzer import analyze, AnalysisResult, ALL_CANONICAL_FIELDS


# ─── Constants ───────────────────────────────────────────────────────────────

from app.paths import UPLOAD_TEMPLATES_DIR

UPLOAD_DIR = str(UPLOAD_TEMPLATES_DIR)
MAX_FILE_SIZE = 10 * 1024 * 1024   # 10 MB
ALLOWED_EXTENSIONS = {".docx"}

# Nhóm mẫu hợp đồng gợi ý trong form. Đây là nhãn phân loại tự do — người dùng
# gõ giá trị khác cũng được; danh sách này chỉ để bấm cho nhanh.
CONTRACT_TYPES = ["Hợp đồng", "Phụ lục", "Nghiệm thu", "Thanh lý", "Báo giá", "Khác"]


# ─── Helpers ─────────────────────────────────────────────────────────────────

def generate_next_code(db: Session) -> str:
    all_codes = db.query(ContractTemplate.code).filter(
        ContractTemplate.code.like("TPL-%")
    ).all()
    max_num = 0
    for (code,) in all_codes:
        m = re.match(r"^TPL-(\d+)$", code)
        if m:
            max_num = max(max_num, int(m.group(1)))
    return f"TPL-{max_num + 1:03d}"


def _safe_filename(original: str, template_id: int) -> str:
    """Tên file an toàn: {id}_{uuid4_short}_{sanitized_original}"""
    base = os.path.basename(original)
    safe = re.sub(r'[^\w\-_\.]', '_', base)
    short_uuid = uuid.uuid4().hex[:8]
    return f"{template_id}_{short_uuid}_{safe}"


# ─── CRUD ────────────────────────────────────────────────────────────────────

def list_templates(db: Session, active_only: bool = False) -> list[ContractTemplate]:
    q = db.query(ContractTemplate)
    if active_only:
        q = q.filter(ContractTemplate.is_active == True)
    return q.order_by(ContractTemplate.contract_type, ContractTemplate.name).all()


def get_template(db: Session, template_id: int) -> Optional[ContractTemplate]:
    return db.query(ContractTemplate).filter(ContractTemplate.id == template_id).first()


def get_by_code(db: Session, code: str) -> Optional[ContractTemplate]:
    return db.query(ContractTemplate).filter(ContractTemplate.code == code).first()


def create_template(
    db: Session,
    code: str,
    name: str,
    contract_type: str,
    file_bytes: bytes,
    original_filename: str,
    description: str = "",
    mode: str = "placeholder",
) -> tuple[ContractTemplate, AnalysisResult]:
    """
    Tạo template mới:
      1. Validate input
      2. Chạy analyzer trước khi lưu DB (fail fast nếu file lỗi)
      3. Lưu file vào disk
      4. Lưu record vào DB với kết quả phân tích
    Raises ValueError cho validation errors.
    """
    # Validate
    code = (code or "").strip()
    name = (name or "").strip()
    if not name:
        raise ValueError("Tên template không được để trống.")
    if not contract_type:
        raise ValueError("Loại hợp đồng không được để trống.")
    if not file_bytes:
        raise ValueError("File .docx không được để trống.")
    if len(file_bytes) > MAX_FILE_SIZE:
        raise ValueError(f"File quá lớn (giới hạn {MAX_FILE_SIZE // (1024*1024)} MB).")

    ext = os.path.splitext(original_filename)[1].lower()
    if ext not in ALLOWED_EXTENSIONS:
        raise ValueError(f"Chỉ chấp nhận file .docx (nhận được {ext!r}).")

    if code and get_by_code(db, code):
        raise ValueError(f"Mã template '{code}' đã tồn tại.")

    # Phân tích nội dung TRƯỚC khi commit bất cứ thứ gì
    result = analyze(file_bytes)
    if not result.ok:
        raise ValueError(f"Không phân tích được file: {result.error}")

    # Auto-generate code nếu trống
    if not code:
        code = generate_next_code(db)

    # Tạo record DB trước để có ID cho filename
    tpl = ContractTemplate(
        code=code,
        name=name,
        contract_type=contract_type,
        description=description,
        mode=mode,
        is_active=True,
    )
    db.add(tpl)
    db.flush()   # flush để có tpl.id, chưa commit

    # Lưu file — ghi TRƯỚC commit để fail-fast nếu disk lỗi
    os.makedirs(UPLOAD_DIR, exist_ok=True)
    filename = _safe_filename(original_filename, tpl.id)
    filepath = os.path.join(UPLOAD_DIR, filename)
    with open(filepath, "wb") as f:
        f.write(file_bytes)

    # Cập nhật record với kết quả phân tích và đường dẫn file
    tpl.file_path = filepath
    tpl.file_name = original_filename
    tpl.detected_placeholders = result.placeholders
    tpl.field_mapping = result.mapping
    try:
        db.commit()
    except Exception:
        # Commit fail → xóa file vừa ghi để tránh orphan
        try:
            os.remove(filepath)
        except OSError:
            pass
        raise
    db.refresh(tpl)
    return tpl, result


def update_metadata(
    db: Session,
    template_id: int,
    name: str,
    contract_type: str,
    description: str = "",
) -> ContractTemplate:
    tpl = get_template(db, template_id)
    if not tpl:
        raise LookupError(f"Không tìm thấy template id={template_id}")
    name = (name or "").strip()
    if not name:
        raise ValueError("Tên template không được để trống.")
    tpl.name = name
    tpl.contract_type = contract_type
    tpl.description = description
    db.commit()
    db.refresh(tpl)
    return tpl


def update_mapping(
    db: Session,
    template_id: int,
    mapping: dict[str, str],
) -> ContractTemplate:
    """
    Lưu field_mapping đã chỉnh sửa.
    Chỉ giữ lại các key có trong detected_placeholders — tránh garbage.
    """
    tpl = get_template(db, template_id)
    if not tpl:
        raise LookupError(f"Không tìm thấy template id={template_id}")

    # Sanitize: chỉ giữ key trong detected_placeholders, value phải là canonical
    allowed_keys = set(tpl.detected_placeholders)
    # Không enforce strict canonical validation — allow custom field names
    # nhưng strip whitespace và loại bỏ key không thuộc placeholder set
    cleaned = {
        k: v.strip()
        for k, v in mapping.items()
        if k in allowed_keys
    }
    # Giữ lại các placeholder không có trong mapping submission (set = "")
    for ph in tpl.detected_placeholders:
        if ph not in cleaned:
            cleaned[ph] = ""

    tpl.field_mapping = cleaned
    db.commit()
    db.refresh(tpl)
    return tpl


def reanalyze(db: Session, template_id: int) -> tuple[ContractTemplate, AnalysisResult]:
    """
    Chạy lại analyzer trên file đã lưu.
    Hữu ích khi file được replace hoặc cần refresh mapping.
    """
    tpl = get_template(db, template_id)
    if not tpl:
        raise LookupError(f"Không tìm thấy template id={template_id}")
    if not tpl.file_path or not os.path.exists(tpl.file_path):
        raise FileNotFoundError(f"File không tồn tại: {tpl.file_path!r}")

    result = analyze(tpl.file_path)
    if not result.ok:
        raise ValueError(f"Phân tích thất bại: {result.error}")

    # Merge: giữ mapping cũ cho các placeholder vẫn còn tồn tại
    old_mapping = tpl.field_mapping
    new_mapping = result.mapping
    merged = {
        ph: old_mapping.get(ph, new_mapping.get(ph, ""))
        for ph in result.placeholders
    }

    tpl.detected_placeholders = result.placeholders
    tpl.field_mapping = merged
    db.commit()
    db.refresh(tpl)
    return tpl, result


def toggle_active(db: Session, template_id: int) -> ContractTemplate:
    tpl = get_template(db, template_id)
    if not tpl:
        raise LookupError(f"Không tìm thấy template id={template_id}")
    tpl.is_active = not tpl.is_active
    db.commit()
    db.refresh(tpl)
    return tpl


def _bundled_template_files():
    """Discover optional local-only DOCX assets without assuming their names."""
    from app.paths import ASSETS_DIR

    return sorted((ASSETS_DIR / "default_templates").glob("*.docx"))


def _bundled_template_code(file_bytes: bytes) -> str:
    """Return a stable anonymous code derived from a template's content."""
    import hashlib

    return f"BUNDLED-{hashlib.sha256(file_bytes).hexdigest()[:12].upper()}"


def _template_kind(placeholders: list[str]) -> str:
    fields = set(placeholders)
    if any(field.startswith("QUOTE_DATE") for field in fields):
        return "Báo giá"
    if any(field.startswith("ACCEPTANCE_DATE") for field in fields):
        return "Nghiệm thu"
    if any(field.startswith("LIQUIDATION_DATE") for field in fields):
        return "Thanh lý"
    return "Hợp đồng"


def _existing_template_with_bytes(db: Session, digest: str):
    import hashlib

    for template in db.query(ContractTemplate).all():
        if not template.file_path or not os.path.isfile(template.file_path):
            continue
        try:
            with open(template.file_path, "rb") as stream:
                current = hashlib.sha256(stream.read()).hexdigest()
        except OSError:
            continue
        if current == digest:
            return template
    return None


def _import_bundled_template_if_missing(db: Session, source_path):
    """Copy a discovered local asset once; never update an existing DB record."""
    import hashlib

    file_bytes = source_path.read_bytes()
    digest = hashlib.sha256(file_bytes).hexdigest()
    code = _bundled_template_code(file_bytes)

    existing = get_by_code(db, code) or _existing_template_with_bytes(db, digest)
    if existing is not None:
        return existing

    result = analyze(file_bytes)
    if not result.ok:
        raise ValueError(result.error)

    template, _ = create_template(
        db=db,
        code=code,
        name=source_path.stem.replace("_", " "),
        contract_type=_template_kind(result.placeholders),
        file_bytes=file_bytes,
        original_filename=source_path.name,
        description="Mẫu DOCX được phát hiện từ thư mục assets cục bộ.",
    )
    return template


def seed_from_project_files(db: Session):
    """Import optional local-only DOCX assets once without overwriting user data."""
    from app.utils.safe_console import safe_print

    for source_path in _bundled_template_files():
        try:
            _import_bundled_template_if_missing(db, source_path)
        except Exception as exc:
            db.rollback()
            safe_print(f"  [seed] Không nhập được mẫu cục bộ {source_path.name}: {exc}")


def cleanup_orphan_template_files(db: Session) -> None:
    """
    Xóa các file template orphan trong uploads/templates/ — file không được
    tham chiếu bởi bất kỳ ContractTemplate nào trong DB.
    Nguyên nhân explosion: _safe_filename() tạo UUID mới mỗi lần startup,
    để lại file cũ trên disk. Hàm này dọn dẹp một lần, sau đó _upsert sẽ
    reuse file path nên không explosion nữa.
    """
    from app.utils.safe_console import safe_print

    if not os.path.isdir(UPLOAD_DIR):
        return

    # Tập hợp các file_path đang được sử dụng trong DB
    active_paths = set()
    for tpl in db.query(ContractTemplate).all():
        if tpl.file_path:
            active_paths.add(os.path.normpath(tpl.file_path))

    removed = 0
    for fname in os.listdir(UPLOAD_DIR):
        if not fname.endswith(".docx"):
            continue
        fpath = os.path.normpath(os.path.join(UPLOAD_DIR, fname))
        if fpath not in active_paths:
            try:
                os.remove(fpath)
                removed += 1
            except OSError:
                pass

    if removed:
        safe_print(f"  [cleanup] Đã xóa {removed} file template orphan.")
