"""lifecycle.py — luật vòng đời hợp đồng, một nguồn duy nhất.

Trước module này, luật vòng đời nằm rải rác và **không khớp nhau**:

* `contract_service.update_status()` kiểm tra `VALID_TRANSITIONS`, nhưng
  `bulk_update_status()` gán thẳng trạng thái — bỏ qua toàn bộ kiểm tra,
  không đòi chứng từ, không ghi lịch sử.
* Sáu nơi khác nhau tự định nghĩa "hợp đồng đang hiệu lực gồm những trạng thái nào",
  mỗi nơi thiếu một trạng thái khác nhau, nên dashboard, danh sách và báo cáo Excel
  đếm ra ba con số khác nhau trên cùng một bộ dữ liệu.

Mọi thứ đó nay nằm ở đây (OPEN_SOURCE_DESKTOP_PLAN.md Task 9 & 10).

Ghi chú về `ExpiringSoon`
-------------------------
`ExpiringSoon` từng là một trạng thái **được ghi vào DB**: bản quét tự động đổi
`Signed → ExpiringSoon` khi hợp đồng còn ≤30 ngày. Nhưng `ExpiringSoon` không có
lối đi tới `Invoiced`, nên hợp đồng đã ký vào vùng 30 ngày cuối bị **khoá vĩnh viễn,
không bao giờ xuất hoá đơn được nữa**.

Nay "sắp hết hạn" là thông tin **dẫn xuất từ `end_date`** — xem `expiry_bucket()`.
Không code nào ghi `ExpiringSoon` nữa; hằng số enum được giữ lại chỉ để đọc được
các bản ghi cũ, và `seed.run_migrations()` chuyển chúng về trạng thái nghiệp vụ thật.
"""

from __future__ import annotations

from datetime import date
from typing import Optional

from app.models.contract import ContractStatus

# ─── Ngưỡng cảnh báo hết hạn ─────────────────────────────────────────────────
# Dùng chung cho dashboard, danh sách hợp đồng, và nhãn trên trang chi tiết.
EXPIRY_CRITICAL_DAYS = 7     # đỏ — cần xử lý ngay
EXPIRY_WARNING_DAYS = 14     # cam
EXPIRING_SOON_DAYS = 30      # vàng — "sắp hết hạn"
EXPIRING_WARN_DAYS = 60      # phạm vi cảnh báo rộng trên dashboard


# ─── Các tập trạng thái ──────────────────────────────────────────────────────

#: Đã ký và vòng đời còn đang chạy. Dùng cho quét hết hạn, danh sách sắp hết hạn,
#: và tổng giá trị "đang hiệu lực".
ACTIVE_LIFECYCLE_STATUSES: tuple[ContractStatus, ...] = (
    ContractStatus.Signed,
    ContractStatus.Invoiced,
    ContractStatus.PaidActive,
    ContractStatus.Active,        # legacy — dữ liệu cũ trước khi tách PaidActive
    ContractStatus.ExpiringSoon,  # legacy — không còn được ghi mới
)

#: Đang thực sự cung cấp dịch vụ — hẹp hơn tập trên vì loại `Signed`
#: (đã ký nhưng chưa xuất hoá đơn, chưa tính là đang chạy).
IN_SERVICE_STATUSES: tuple[ContractStatus, ...] = (
    ContractStatus.Invoiced,
    ContractStatus.PaidActive,
    ContractStatus.Active,        # legacy
    ContractStatus.ExpiringSoon,  # legacy
)

#: Đã ký nhưng chưa thu được tiền — dùng để tách doanh thu trên dashboard.
UNPAID_STATUSES: tuple[ContractStatus, ...] = (
    ContractStatus.Signed,
    ContractStatus.Active,        # legacy
    ContractStatus.ExpiringSoon,  # legacy
)

#: Không còn được ghi mới; chỉ tồn tại để đọc dữ liệu cũ.
DEPRECATED_STATUSES: frozenset[ContractStatus] = frozenset({ContractStatus.ExpiringSoon})

# Bản chuỗi, cho vài truy vấn cũ so sánh trực tiếp với giá trị cột.
ACTIVE_LIFECYCLE_STATUS_VALUES: tuple[str, ...] = tuple(
    s.value for s in ACTIVE_LIFECYCLE_STATUSES
)


# ─── Bảng chuyển trạng thái ──────────────────────────────────────────────────
# `ExpiringSoon` không còn là đích đến của bất kỳ bước chuyển nào. Nó vẫn có mặt
# ở vế trái để dữ liệu cũ chưa kịp di trú vẫn đi tiếp được — với đúng những lối
# đi mà `Signed` có, tức là vẫn xuất hoá đơn được.
VALID_TRANSITIONS: dict[ContractStatus, set[ContractStatus]] = {
    ContractStatus.Draft:        {ContractStatus.Generated},
    ContractStatus.Generated:    {ContractStatus.Sent, ContractStatus.Signed},
    ContractStatus.Sent:         {ContractStatus.Signed},
    ContractStatus.Signed:       {ContractStatus.Invoiced, ContractStatus.Terminated},
    ContractStatus.Invoiced:     {ContractStatus.PaidActive, ContractStatus.Terminated},
    ContractStatus.PaidActive:   {ContractStatus.Expired, ContractStatus.Terminated},
    ContractStatus.Active:       {ContractStatus.Invoiced, ContractStatus.PaidActive,
                                  ContractStatus.Expired, ContractStatus.Terminated},
    ContractStatus.ExpiringSoon: {ContractStatus.Invoiced, ContractStatus.PaidActive,
                                  ContractStatus.Expired, ContractStatus.Terminated},
    ContractStatus.Expired:      {ContractStatus.Terminated},
    ContractStatus.Terminated:   set(),
}


# ─── Chứng từ bắt buộc ───────────────────────────────────────────────────────
# Trạng thái → (tên cột phải có giá trị, mô tả cho người dùng, nhãn trạng thái).
REQUIRED_EVIDENCE: dict[ContractStatus, tuple[str, str, str]] = {
    ContractStatus.Invoiced: (
        "invoice_pdf_path", "file hóa đơn PDF", "Đã xuất hóa đơn",
    ),
    ContractStatus.PaidActive: (
        "payment_slip_path", "file ủy nhiệm chi", "Đã thanh toán",
    ),
}


# ─── Nhãn hiển thị ───────────────────────────────────────────────────────────
# Trạng thái → (biến thể màu Bootstrap, nhãn tiếng Việt).
# Trước đây có bốn bảng nhãn rời rạc — router hợp đồng, router dashboard (hai
# bảng), và một bảng viết thẳng trong dashboard/expiring.html. Bảng trong template
# thiếu Invoiced lẫn PaidActive nên hợp đồng ở hai trạng thái đó hiện ra dấu "?".
STATUS_LABELS: dict[str, tuple[str, str]] = {
    ContractStatus.Draft.value:        ("secondary", "Nháp"),
    ContractStatus.Generated.value:    ("info",      "Đã sinh file"),
    ContractStatus.Sent.value:         ("primary",   "Đã gửi"),
    ContractStatus.Signed.value:       ("success",   "Đã ký"),
    ContractStatus.Invoiced.value:     ("info",      "Đã xuất hóa đơn"),
    ContractStatus.PaidActive.value:   ("success",   "Đã thanh toán"),
    ContractStatus.Active.value:       ("success",   "Hiệu lực"),       # legacy
    ContractStatus.ExpiringSoon.value: ("warning",   "Sắp hết hạn"),    # legacy
    ContractStatus.Expired.value:      ("danger",    "Đã hết hạn"),
    ContractStatus.Terminated.value:   ("dark",      "Thanh lý"),
}


def status_label(status) -> str:
    """Nhãn tiếng Việt của một trạng thái; trả lại chính giá trị nếu chưa có nhãn."""
    key = status.value if isinstance(status, ContractStatus) else str(status)
    return STATUS_LABELS.get(key, ("secondary", key))[1]


def parse_status(value) -> ContractStatus:
    """Ép chuỗi về `ContractStatus`, báo lỗi tiếng Việt nếu không hợp lệ."""
    if isinstance(value, ContractStatus):
        return value
    try:
        return ContractStatus(value)
    except ValueError:
        raise ValueError(f"Trạng thái không hợp lệ: {value!r}") from None


def validate_transition(contract, new_status) -> ContractStatus:
    """Kiểm tra một bước chuyển trạng thái. Trả về trạng thái đích đã chuẩn hoá.

    Raises:
        ValueError: nếu bước chuyển không được phép, hoặc thiếu chứng từ bắt buộc.
            Thông điệp lỗi hiển thị thẳng cho người dùng nên viết bằng tiếng Việt.
    """
    new_st = parse_status(new_status)

    allowed = VALID_TRANSITIONS.get(contract.status, set())
    if new_st not in allowed:
        allowed_names = [s.value for s in sorted(allowed, key=lambda s: s.value)]
        raise ValueError(
            f"Không thể chuyển từ {contract.status.value} sang {new_st.value}. "
            f"Cho phép: {allowed_names or 'không có'}"
        )

    evidence = REQUIRED_EVIDENCE.get(new_st)
    if evidence:
        column, description, label = evidence
        if not getattr(contract, column, None):
            raise ValueError(
                f"Vui lòng upload {description} trước khi chuyển sang '{label}'."
            )

    return new_st


# ─── Hết hạn: thông tin dẫn xuất, không phải trạng thái ──────────────────────

def days_to_expiry(end_date: Optional[date], today: Optional[date] = None) -> Optional[int]:
    """Số ngày còn lại tới `end_date`. Âm nếu đã quá hạn, None nếu không có hạn."""
    if not end_date:
        return None
    return (end_date - (today or date.today())).days


def expiry_bucket(end_date: Optional[date], today: Optional[date] = None) -> Optional[str]:
    """Phân loại mức khẩn của hạn hợp đồng.

    Trả về ``"expired"`` | ``"critical"`` | ``"warning"`` | ``"info"`` | ``None``.
    ``None`` nghĩa là không có hạn, hoặc còn quá xa để cần cảnh báo.

    Ranh giới cố ý: hết hạn **đúng hôm nay** (0 ngày) tính là đã hết hạn — hợp đồng
    không còn hiệu lực trong ngày cuối cùng đó nữa.
    """
    days = days_to_expiry(end_date, today)
    if days is None:
        return None
    if days <= 0:
        return "expired"
    if days <= EXPIRY_CRITICAL_DAYS:
        return "critical"
    if days <= EXPIRY_WARNING_DAYS:
        return "warning"
    if days <= EXPIRING_SOON_DAYS:
        return "info"
    return None


def is_expiring_soon(end_date: Optional[date], today: Optional[date] = None) -> bool:
    """True khi hợp đồng còn hiệu lực nhưng sẽ hết hạn trong ≤30 ngày."""
    days = days_to_expiry(end_date, today)
    return days is not None and 0 < days <= EXPIRING_SOON_DAYS
