"""
dossier_service.py — "Bộ hồ sơ đấu nối": gom file của một khách hàng về một chỗ.

Bài toán thực tế
----------------
File của cùng một khách hàng nằm rải ở bốn nơi khác nhau: giấy tờ pháp nhân ở
``uploads/customer_docs/{id}/``, scan hợp đồng đã ký ở ``uploads/signed_scans/``,
hoá đơn ở ``uploads/invoice_docs/``, uỷ nhiệm chi ở ``uploads/payment_slips/``.
Mỗi lần đấu nối trên trang quản lý bán hàng phải mở từng thư mục đi tìm, mà
trang đó lại có nhiều ô upload riêng cho từng loại giấy tờ.

Service này copy chúng về ``data/dau-noi/{mã KH}_{tên}/`` và đánh số tên file
theo đúng thứ tự các ô — hộp thoại chọn file của trình duyệt nhớ thư mục vừa
dùng, nên từ ô thứ hai trở đi không phải điều hướng lại lần nào.

Ranh giới
---------
Chỉ ĐỌC dữ liệu gốc rồi COPY. Không sửa DB, không xoá file gốc. Thư mục đích
hoàn toàn sinh lại được: xoá đi rồi bấm tạo lại là ra y hệt.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

from app import paths
from app.models.contract import Contract
from app.models.customer import Customer
from app.models.customer_document import CustomerDocument
from app.models.product import Product
from app.services.lifecycle import ACTIVE_LIFECYCLE_STATUSES
from app.utils.file_naming import sanitize_for_filename

# Đánh dấu "thư mục này do ContractForge sinh ra". Chỉ những thư mục có file
# này mới được phép dọn khi tạo lại — xem _clear_managed_files().
DOSSIER_MARKER = ".cf_dossier"

# Số thứ tự bắt đầu cho các giấy tờ 'other' — tách hẳn khỏi dải số của các ô
# cố định để việc thêm một ô mới không làm xô lệch tên file đã quen mắt.
EXTRA_START = 90


# ─── Định nghĩa ô hồ sơ ───────────────────────────────────────────────────────

@dataclass(frozen=True)
class Slot:
    """Một ô upload trên trang quản lý bán hàng.

    ``source`` cho biết lấy file ở đâu:
      ``customer_doc:<doc_type>`` — giấy tờ pháp nhân của khách hàng
      ``contract:<field>``        — file gắn với hợp đồng được chọn
    """
    key: str
    label: str
    source: str
    filename: str
    required: bool = True


#: Ô giấy tờ pháp nhân — thuộc về khách hàng, không thuộc hợp đồng nào.
#: Thứ tự trong tuple chính là số ô 01, 02, 03.
CUSTOMER_SLOTS: tuple[Slot, ...] = (
    Slot("cccd_front",    "CCCD mặt trước",       "customer_doc:cccd_front",    "CCCD_mat_truoc"),
    Slot("cccd_back",     "CCCD mặt sau",         "customer_doc:cccd_back",     "CCCD_mat_sau"),
    Slot("establishment", "Quyết định thành lập", "customer_doc:establishment", "Quyet_dinh_thanh_lap"),
)

#: Ô hợp đồng bắt đầu từ số này, và MỖI SẢN PHẨM chiếm một khối cố định.
#:
#: Đánh số theo vị trí của sản phẩm trong danh mục chứ không theo những gì
#: khách hàng đang có. Khách chỉ mua một sản phẩm thì dải số của sản phẩm kia
#: bỏ trống — cố ý: số ô luôn ứng với đúng một ô trên trang bán hàng, không xê
#: dịch giữa khách hàng này với khách hàng khác.
CONTRACT_BLOCK_START = 4
SLOTS_PER_PRODUCT = 2          # [0] hợp đồng đã ký, [1] hoá đơn


@dataclass
class SlotResult:
    """Kết quả tra một ô: có file hay không, lấy ở đâu, sẽ copy thành tên gì."""
    order: int
    slot: Slot
    source_path: Optional[Path] = None
    dest_name: str = ""
    size: int = 0
    note: str = ""

    @property
    def ok(self) -> bool:
        return self.source_path is not None

    @property
    def missing_required(self) -> bool:
        return self.slot.required and not self.ok


@dataclass
class DossierPlan:
    """Toàn bộ những gì sẽ được tạo ra, tính trước khi ghi bất cứ thứ gì."""
    customer: Customer
    contracts: list[Contract]
    dest_dir: Path
    slots: list[SlotResult] = field(default_factory=list)

    @property
    def ready_count(self) -> int:
        return sum(1 for s in self.slots if s.ok)

    @property
    def missing_required(self) -> list[SlotResult]:
        return [s for s in self.slots if s.missing_required]

    @property
    def is_complete(self) -> bool:
        return not self.missing_required


# ─── Đường dẫn ────────────────────────────────────────────────────────────────

def resolve_stored_path(stored: Optional[str]) -> Optional[Path]:
    """Đổi path lưu trong DB thành path thật trên đĩa.

    Hiện DB lưu path tuyệt đối, nhưng bước 5 của kế hoạch sẽ chuyển sang lưu
    tương đối so với thư mục dữ liệu. Hiểu sẵn cả hai kiểu để lúc đó không phải
    sửa lại service này.
    """
    if not stored:
        return None
    p = Path(stored)
    if not p.is_absolute():
        p = paths.DATA_DIR / p
    return p


def dossier_dir_for(customer: Customer) -> Path:
    """Thư mục hồ sơ của một khách hàng — cố định, không gắn ngày tháng.

    Đấu nối là việc làm một lần cho mỗi khách hàng, nên tạo lại thì ghi đè chính
    chỗ cũ. Gắn ngày vào tên sẽ đẻ ra một thư mục mới mỗi lần bấm nhầm.
    """
    name = customer.short_name or customer.legal_name or ""
    slug = sanitize_for_filename(name, max_len=40) or "KHACH_HANG"
    code = sanitize_for_filename(customer.code or "", max_len=15) or f"ID{customer.id}"
    return paths.DOSSIER_DIR / f"{code}_{slug}"


# ─── Hợp đồng đưa vào hồ sơ ───────────────────────────────────────────────────

def contracts_for_dossier(db, customer_id: int) -> list[Contract]:
    """Mọi hợp đồng của khách hàng cần có mặt trong hồ sơ đấu nối.

    Lấy TẤT CẢ chứ không chọn một. Phần lớn khách hàng mang hai hợp đồng thuộc
    hai sản phẩm khác nhau và trang bán hàng có ô riêng cho từng loại — chọn một
    cái là bỏ sót cái kia.

    Chỉ nhận hợp đồng đã ký và còn trong vòng đời; bản nháp chưa ký thì chưa có
    gì để nộp. Hợp đồng đã có scan luôn được nhận kể cả khi trạng thái nằm ngoài
    tập đó, để dữ liệu cũ không bị bỏ rơi.
    """
    active = set(ACTIVE_LIFECYCLE_STATUSES)
    rows = (
        db.query(Contract)
        .filter(Contract.customer_id == customer_id)
        .order_by(Contract.contract_number, Contract.id)
        .all()
    )
    return [c for c in rows if c.status in active or c.signed_pdf_path]


def _product_positions(db) -> dict[Optional[int], int]:
    """Vị trí của từng sản phẩm trong danh mục → quyết định dải số của nó.

    Khoá theo id sản phẩm, đọc từ DB chứ không viết cứng: thêm sản phẩm mới là
    nó tự nhận dải số kế tiếp, không phải sửa mã nguồn.
    """
    ids = [r[0] for r in db.query(Product.id).order_by(Product.id).all()]
    return {pid: i for i, pid in enumerate(ids)}


# ─── Dựng kế hoạch ────────────────────────────────────────────────────────────

def _doc_source(doc: Optional[CustomerDocument]) -> tuple[Optional[Path], str]:
    if not doc:
        return None, "Chưa upload"
    p = resolve_stored_path(doc.file_path)
    if not p or not p.exists():
        return None, "File không còn trên đĩa"
    return p, ""


def _contract_source(contract: Contract, field_name: str) -> tuple[Optional[Path], str]:
    stored = getattr(contract, f"{field_name}_path", None)
    if not stored:
        return None, "Hợp đồng chưa có file này"
    p = resolve_stored_path(stored)
    if not p or not p.exists():
        return None, "File không còn trên đĩa"
    return p, ""


def _fill(result: SlotResult, src: Optional[Path], note: str) -> SlotResult:
    """Gắn nguồn vào một ô và đặt tên file đích."""
    if src:
        result.source_path = src
        result.dest_name = f"{result.order:02d}_{result.slot.filename}{src.suffix.lower()}"
        result.size = src.stat().st_size
    else:
        result.note = note
    return result


def build_plan(db, customer: Customer,
               contracts: Optional[list[Contract]] = None) -> DossierPlan:
    """Tính trước toàn bộ hồ sơ mà không ghi gì ra đĩa.

    Tách hẳn khỏi bước copy để màn hình xác nhận hiện được đủ/thiếu trước khi
    người dùng quyết định — phát hiện thiếu giấy tờ ở đây rẻ hơn nhiều so với
    phát hiện lúc đang đấu nối dở trên trang bán hàng.
    """
    doc_rows = (
        db.query(CustomerDocument)
        .filter(CustomerDocument.customer_id == customer.id)
        .order_by(CustomerDocument.id)
        .all()
    )
    fixed = {d.doc_type: d for d in doc_rows if d.doc_type != "other"}
    others = [d for d in doc_rows if d.doc_type == "other"]

    if contracts is None:
        contracts = contracts_for_dossier(db, customer.id)

    plan = DossierPlan(customer=customer, contracts=contracts,
                       dest_dir=dossier_dir_for(customer))

    # ── 01..03: giấy tờ pháp nhân ────────────────────────────────────────────
    for i, slot in enumerate(CUSTOMER_SLOTS, start=1):
        src, note = _doc_source(fixed.get(slot.source.partition(":")[2]))
        plan.slots.append(_fill(SlotResult(order=i, slot=slot), src, note))

    # ── 04 trở đi: hợp đồng + hoá đơn, mỗi sản phẩm một khối ─────────────────
    positions = _product_positions(db)
    seen_per_product: dict[Optional[int], int] = {}

    for c in contracts:
        pos = positions.get(c.product_id, len(positions))
        base = CONTRACT_BLOCK_START + pos * SLOTS_PER_PRODUCT

        # Khách hàng có thể có hai hợp đồng cùng một sản phẩm (ví dụ ký bổ sung
        # đợt hai). Cái thứ hai trở đi được gắn hậu tố để không đè tên cái đầu,
        # nhưng vẫn giữ nguyên số ô nên đứng liền nhau khi sắp xếp.
        n = seen_per_product.get(c.product_id, 0)
        seen_per_product[c.product_id] = n + 1
        suffix = "" if n == 0 else f"_{n + 1}"

        prod_code = sanitize_for_filename(
            (c.product.code if c.product else "") or "KHAC", max_len=20)
        prod_label = (c.product.name if c.product else "Không rõ sản phẩm")

        for offset, (field_name, stem, label, required) in enumerate((
            ("signed_pdf",  "Hop_dong", "Hợp đồng đã ký", True),
            ("invoice_pdf", "Hoa_don",  "Hoá đơn",        False),
        )):
            slot = Slot(
                key=f"contract_{c.id}_{field_name}",
                label=f"{label} — {prod_label} ({c.contract_number})",
                source=f"contract:{field_name}",
                filename=f"{stem}_{prod_code}{suffix}",
                required=required,
            )
            src, note = _contract_source(c, field_name)
            plan.slots.append(
                _fill(SlotResult(order=base + offset, slot=slot), src, note))

    # Giấy tờ "khác" không ứng với ô cố định nào trên trang bán hàng, nên xếp
    # sau cùng ở dải số riêng — có mặt để không phải quay lại tìm, nhưng không
    # chen vào thứ tự các ô.
    for j, doc in enumerate(others):
        order = EXTRA_START + j
        label = (doc.label or "Giấy tờ khác").strip()
        slot = Slot(key=f"other_{doc.id}", label=label,
                    source=f"customer_doc_id:{doc.id}",
                    filename=sanitize_for_filename(label, max_len=40) or "Giay_to_khac",
                    required=False)
        src, note = _doc_source(doc)
        plan.slots.append(_fill(SlotResult(order=order, slot=slot), src, note))

    plan.slots.sort(key=lambda s: (s.order, s.slot.filename))
    return plan


# ─── Ghi ra đĩa ───────────────────────────────────────────────────────────────

def _clear_managed_files(dest_dir: Path) -> int:
    """Xoá file do lần tạo trước để lại, trong đúng một thư mục do ta sinh ra.

    Cần thiết vì hồ sơ được tạo đè lên chỗ cũ: nếu một giấy tờ đã bị xoá khỏi
    hệ thống mà file cũ vẫn nằm lại, nó sẽ bị nộp nhầm ở lần đấu nối sau.

    Ba lớp chặn, vì đây là thao tác xoá:
      1. Thư mục phải nằm trong DOSSIER_DIR.
      2. Thư mục phải có file dấu DOSSIER_MARKER (do chính ta ghi).
      3. Chỉ xoá file nằm trực tiếp trong đó — không đệ quy, không đụng thư mục con.
    """
    if not dest_dir.exists():
        return 0

    root = paths.DOSSIER_DIR.resolve()
    try:
        dest_dir.resolve().relative_to(root)
    except ValueError:
        raise ValueError(f"Từ chối dọn thư mục ngoài {root}: {dest_dir}")

    if not (dest_dir / DOSSIER_MARKER).exists():
        # Thư mục trùng tên nhưng không phải do ta tạo — để nguyên, không xoá gì.
        return 0

    removed = 0
    for entry in dest_dir.iterdir():
        if entry.is_file() and entry.name != DOSSIER_MARKER:
            try:
                entry.unlink()
                removed += 1
            except OSError:
                pass
    return removed


def _link_or_copy(src: Path, dest: Path) -> str:
    """Đưa ``src`` vào thư mục hồ sơ dưới tên ``dest``. Trả về 'link' hoặc 'copy'.

    Ưu tiên hardlink: hai tên cùng trỏ vào một khối dữ liệu trên đĩa, nên thư
    mục hồ sơ không tốn thêm byte nào và không thể có chuyện hai bản lệch nội
    dung — chúng *là* một file. Xoá tên bên này không đụng tên bên kia, nên
    việc dọn thư mục hồ sơ vẫn an toàn như khi còn copy.

    Rơi về copy khi hệ thống file không cho: ổ FAT32/exFAT không có hardlink, và
    nguồn với đích phải cùng phân vùng. Cả hai đều thoả trong cấu hình bình
    thường (cùng nằm dưới thư mục dữ liệu) nhưng người dùng có thể trỏ
    CONTRACTFORGE_DATA_ROOT đi bất cứ đâu.
    """
    if dest.exists():
        dest.unlink()
    try:
        os.link(src, dest)
        return "link"
    except (OSError, AttributeError, NotImplementedError):
        shutil.copy2(src, dest)
        return "copy"


def newest_source_mtime(plan: "DossierPlan") -> float:
    """Thời điểm sửa gần nhất trong các file nguồn — để phát hiện hồ sơ đã cũ."""
    times = [s.source_path.stat().st_mtime for s in plan.slots
             if s.ok and s.source_path.exists()]
    return max(times) if times else 0.0


def is_stale(plan: "DossierPlan") -> bool:
    """True khi thư mục hồ sơ đã tạo cũ hơn dữ liệu nguồn.

    Hardlink tự cập nhật khi file gốc bị ghi đè tại chỗ, nhưng upload đổi đuôi
    file (jpg → pdf) sinh ra file mới rồi xoá file cũ — lúc đó liên kết cũ giữ
    lại nội dung cũ. Cảnh báo để người dùng bấm tạo lại thay vì nộp nhầm bản cũ.
    """
    stamp = plan.dest_dir / "_checklist.txt"
    if not stamp.exists():
        return False
    return newest_source_mtime(plan) > stamp.stat().st_mtime


def _format_size(n: int) -> str:
    if n >= 1024 * 1024:
        return f"{n / (1024 * 1024):.1f} MB"
    if n >= 1024:
        return f"{n // 1024} KB"
    return f"{n} B"


def build_checklist_text(plan: DossierPlan, ts: Optional[datetime] = None) -> str:
    """Nội dung ``_checklist.txt`` — bản kê những gì có và những gì thiếu."""
    ts = ts or datetime.now()
    cust = plan.customer
    name = cust.short_name or cust.legal_name or ""

    lines = [
        "BỘ HỒ SƠ ĐẤU NỐI",
        "=" * 60,
        f"Khách hàng : {name} ({cust.code})",
        f"Tạo lúc    : {ts.strftime('%d/%m/%Y %H:%M')}",
    ]
    if plan.contracts:
        for i, c in enumerate(plan.contracts):
            prefix = "Hợp đồng   : " if i == 0 else "             "
            prod = c.product.code if c.product else "—"
            lines.append(f"{prefix}{c.contract_number}  [{prod}]")
    else:
        lines.append("Hợp đồng   : — chưa có hợp đồng nào —")
    lines.append("")

    for s in plan.slots:
        if s.ok:
            lines.append(f"  [x] {s.dest_name}"
                         f"{' ' * max(1, 42 - len(s.dest_name))}{_format_size(s.size)}")
        else:
            tag = "THIẾU" if s.slot.required else "không bắt buộc"
            lines.append(f"  [ ] {s.order:02d} {s.slot.label} — {tag}: {s.note}")

    missing = plan.missing_required
    lines.append("")
    if missing:
        lines.append(f"Thiếu {len(missing)} mục bắt buộc: "
                     + ", ".join(m.slot.label for m in missing))
    else:
        lines.append("Đủ toàn bộ mục bắt buộc.")
    lines.append("")
    lines.append("File ở đây liên kết thẳng tới file gốc trong ứng dụng nên không tốn")
    lines.append("thêm dung lượng. Xoá thư mục này không làm mất dữ liệu gốc.")
    return "\n".join(lines) + "\n"


@dataclass
class BuildResult:
    dest_dir: Path
    copied: int
    removed: int
    missing_required: int
    linked: int = 0
    errors: list[str] = field(default_factory=list)

    @property
    def duplicated_bytes_saved(self) -> bool:
        """True khi mọi file đều đi qua hardlink — thư mục hồ sơ tốn 0 byte."""
        return self.linked > 0 and self.copied == self.linked


def materialize(plan: DossierPlan, ts: Optional[datetime] = None) -> BuildResult:
    """Tạo thư mục hồ sơ theo kế hoạch. Ghi đè kết quả lần trước."""
    dest = plan.dest_dir
    errors: list[str] = []

    # Dọn TRƯỚC khi đặt file dấu. Nếu làm ngược lại, một thư mục trùng tên do
    # người dùng tự tạo sẽ được ta gắn dấu rồi xoá sạch ngay trong cùng lượt —
    # đúng thứ mà lớp chặn "phải có file dấu" sinh ra để ngăn.
    pre_existing_foreign = (
        dest.exists()
        and not (dest / DOSSIER_MARKER).exists()
        and any(dest.iterdir())
    )
    removed = _clear_managed_files(dest)

    dest.mkdir(parents=True, exist_ok=True)
    marker = dest / DOSSIER_MARKER
    if not marker.exists():
        marker.write_text("ContractForge — bộ hồ sơ đấu nối (sinh tự động)\n",
                          encoding="utf-8")
    if pre_existing_foreign:
        errors.append("Thư mục đã có sẵn file không do ứng dụng tạo — "
                      "giữ nguyên, chưa dọn. Kiểm tra lại trước khi nộp.")

    copied = linked = 0
    for s in plan.slots:
        if not s.ok:
            continue
        try:
            if _link_or_copy(s.source_path, dest / s.dest_name) == "link":
                linked += 1
            copied += 1
        except OSError as e:
            errors.append(f"{s.slot.label}: {e}")

    try:
        (dest / "_checklist.txt").write_text(
            build_checklist_text(plan, ts), encoding="utf-8-sig")
    except OSError as e:
        errors.append(f"_checklist.txt: {e}")

    return BuildResult(dest_dir=dest, copied=copied, removed=removed, linked=linked,
                       missing_required=len(plan.missing_required), errors=errors)


# ─── Mở thư mục ───────────────────────────────────────────────────────────────

def open_in_file_manager(target: Path) -> None:
    """Mở thư mục bằng trình quản lý file của hệ điều hành.

    Chỉ nhận thư mục nằm trong DOSSIER_DIR. Ứng dụng chạy loopback không đăng
    nhập, nên đường dẫn tới đây phải được chặn cứng chứ không tin tham số vào.
    Ném OSError nếu không mở được để router hiển thị đường dẫn cho người dùng
    tự mở tay.
    """
    root = paths.DOSSIER_DIR.resolve()
    resolved = target.resolve()
    try:
        resolved.relative_to(root)
    except ValueError:
        raise ValueError(f"Từ chối mở đường dẫn ngoài {root}: {target}")
    if not resolved.is_dir():
        raise OSError(f"Thư mục không tồn tại: {resolved}")

    if sys.platform == "win32":
        os.startfile(str(resolved))  # noqa: S606 - đường dẫn đã kiểm tra ở trên
    elif sys.platform == "darwin":
        subprocess.run(["open", str(resolved)], check=True)
    else:
        subprocess.run(["xdg-open", str(resolved)], check=True)
