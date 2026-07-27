"""
dashboard_service.py — Tất cả business logic cho Dashboard & nhắc hạn.

Chức năng chính:
  1. auto_update_contract_statuses() — Cập nhật tự động trạng thái:
       hợp đồng đang hiệu lực đã quá end_date → Expired
     Gọi mỗi khi load dashboard (hoặc startup).

     Bản quét này KHÔNG còn ghi ExpiringSoon. Trước đây nó đổi Signed → ExpiringSoon
     khi còn ≤30 ngày, mà ExpiringSoon không có lối tới Invoiced — hợp đồng đã ký
     vào vùng 30 ngày cuối bị khoá, không xuất hoá đơn được nữa. "Sắp hết hạn" nay
     tính từ end_date mỗi lần hiển thị (lifecycle.expiry_bucket).

  2. get_dashboard_data() — Một lần gọi, lấy đủ data cho toàn bộ dashboard:
       - stats: đếm theo status
       - expiring_30: sắp hết hạn ≤30 ngày
       - expiring_60: sắp hết hạn 31-60 ngày
       - by_product: phân bổ hợp đồng theo sản phẩm (Active+Signed)
       - by_status: phân bổ theo trạng thái (non-zero)
       - recent_activity: 8 ContractEvent gần nhất
       - total_value_active: tổng giá trị hợp đồng đang hiệu lực

  3. get_expiring_contracts(days) — Lấy danh sách hợp đồng sắp hết hạn.

  4. mark_as_expiring_soon() / mark_as_expired() — Internal helpers.
"""

from datetime import date, timedelta, datetime as _datetime
from decimal import Decimal
from typing import Optional

from sqlalchemy.orm import Session
from sqlalchemy import func

from app.models.contract import Contract, ContractStatus
from app.models.contract_event import ContractEvent
from app.models.contract_template import ContractTemplate
from app.models.customer import Customer
from app.models.product import Product
from app.services import lifecycle


# ─── Thresholds ──────────────────────────────────────────────────────────────
# Ngưỡng và tập trạng thái sống ở app/services/lifecycle.py để dashboard, danh
# sách hợp đồng và báo cáo Excel không thể đếm lệch nhau nữa. Re-export ở đây
# cho code cũ đang import từ module này.

EXPIRING_SOON_DAYS = lifecycle.EXPIRING_SOON_DAYS   # ≤30 ngày → cảnh báo "sắp hết hạn"
EXPIRING_WARN_DAYS = lifecycle.EXPIRING_WARN_DAYS   # ≤60 ngày → hiện trên dashboard

ACTIVE_LIFECYCLE_STATUSES = lifecycle.ACTIVE_LIFECYCLE_STATUSES


# ─── Status auto-update ───────────────────────────────────────────────────────

def auto_update_contract_statuses(db: Session) -> dict:
    """
    Quét hợp đồng đang hiệu lực và đánh dấu những cái đã quá hạn.

    Quy tắc duy nhất:
      - end_date ≤ today → Expired

    Đây là thay đổi trạng thái tự động **duy nhất** còn lại. Cảnh báo sắp hết hạn
    không còn ghi vào DB: nó được tính từ end_date mỗi lần hiển thị, nên một hợp
    đồng gần hết hạn vẫn giữ nguyên trạng thái nghiệp vụ thật (Signed / Invoiced /
    PaidActive) và vẫn đi tiếp bình thường trong quy trình thu tiền.

    Returns:
        dict với {newly_expired: int}
    """
    today = date.today()
    counts = {"newly_expired": 0}

    expired_now = (
        db.query(Contract)
        .filter(
            Contract.status.in_(ACTIVE_LIFECYCLE_STATUSES),
            Contract.end_date.isnot(None),
            Contract.end_date <= today,
        )
        .all()
    )

    for c in expired_now:
        c.status = ContractStatus.Expired
        _add_event(db, c.id, "expired",
                   f"Tự động cập nhật: hết hạn ngày {c.end_date}", "system")
        counts["newly_expired"] += 1

    if expired_now:
        db.commit()

    return counts


# ─── Main dashboard data fetcher ─────────────────────────────────────────────

def get_dashboard_data(db: Session) -> dict:
    """
    Lấy toàn bộ data cần thiết cho dashboard trong 1 lần gọi.
    Gọi auto_update trước khi đọc để đảm bảo trạng thái mới nhất.
    """
    # Luôn auto-update trước khi hiển thị
    update_counts = auto_update_contract_statuses(db)

    today = date.today()

    # ── 1. Đếm theo trạng thái (aggregate query, không load toàn bộ) ───────
    status_counts: dict[str, int] = {s.value: 0 for s in ContractStatus}
    rows = db.query(Contract.status, func.count(Contract.id)).group_by(Contract.status).all()
    for status, count in rows:
        status_counts[status.value] = count

    # ── 2. Hợp đồng sắp hết hạn ────────────────────────────────────────────
    expiring_30 = _get_expiring_list(db, today, days=30)
    expiring_60 = _get_expiring_list(db, today, days=60)
    # expiring_60 = tất cả ≤60 ngày, ta tách thành 31-60 ngày riêng
    expiring_31_60 = [r for r in expiring_60 if r["days_left"] > 30]

    # ── 3. Phân bổ theo sản phẩm (hợp đồng đang hiệu lực) ──────────────────
    by_product = _contracts_by_product(db)

    # ── 4. Tổng giá trị hợp đồng đang hiệu lực (tách 3 nhóm) ──────────────
    total_value_row = (
        db.query(func.sum(Contract.total_amount))
        .filter(Contract.status.in_(ACTIVE_LIFECYCLE_STATUSES))
        .scalar()
    )
    total_value_active = Decimal(str(total_value_row or 0))

    # Đã thanh toán (PaidActive)
    value_paid_row = (
        db.query(func.sum(Contract.total_amount))
        .filter(Contract.status == ContractStatus.PaidActive)
        .scalar()
    )
    value_paid = Decimal(str(value_paid_row or 0))

    # Đã xuất hóa đơn (Invoiced)
    value_invoiced_row = (
        db.query(func.sum(Contract.total_amount))
        .filter(Contract.status == ContractStatus.Invoiced)
        .scalar()
    )
    value_invoiced = Decimal(str(value_invoiced_row or 0))

    # Chưa thanh toán (Signed + legacy — chưa xuất hóa đơn hoặc chưa thu tiền)
    value_unpaid_row = (
        db.query(func.sum(Contract.total_amount))
        .filter(Contract.status.in_(lifecycle.UNPAID_STATUSES))
        .scalar()
    )
    value_unpaid = Decimal(str(value_unpaid_row or 0))

    # ── 5. Hoạt động gần đây (8 events mới nhất) ──────────────────────────
    recent_events = (
        db.query(ContractEvent, Contract, Customer)
        .join(Contract, ContractEvent.contract_id == Contract.id)
        .outerjoin(Customer, Contract.customer_id == Customer.id)
        .filter(ContractEvent.event_type.notin_(["expiring_soon_alert", "expired"]))
        .order_by(ContractEvent.occurred_at.desc())
        .limit(8)
        .all()
    )
    recent_activity = [
        {
            "event":         ev,
            "contract_number": c.contract_number,
            "contract_id":   c.id,
            "customer_name": (cust.short_name or cust.legal_name) if cust else "—",
            "event_type":    ev.event_type,
            "description":   ev.description,
            "occurred_at":   ev.occurred_at,
        }
        for ev, c, cust in recent_events
    ]

    # ── 6. Thống kê tổng hợp ──────────────────────────────────────────────
    # Đã sinh file = tất cả HĐ đã từng generate (có output_file_path), không phân biệt trạng thái
    count_generated_file = db.query(Contract).filter(Contract.output_file_path.isnot(None)).count()

    # Số hợp đồng đang trong vòng đời hiệu lực. MỘT con số có thẩm quyền, thay cho
    # kiểu cộng tay `active + invoiced + expiring_soon` rải rác trong template —
    # hai chỗ trong dashboard/index.html từng cộng theo hai công thức khác nhau.
    count_in_force = (
        db.query(func.count(Contract.id))
        .filter(Contract.status.in_(ACTIVE_LIFECYCLE_STATUSES))
        .scalar()
        or 0
    )
    # Hẹp hơn: đang thực sự chạy dịch vụ (không tính Signed đang chờ xuất hoá đơn).
    # Khớp đúng bộ lọc `?alert_filter=all_active` mà thẻ trên dashboard trỏ tới,
    # nên con số và danh sách mở ra luôn bằng nhau.
    count_in_service = (
        db.query(func.count(Contract.id))
        .filter(Contract.status.in_(lifecycle.IN_SERVICE_STATUSES))
        .scalar()
        or 0
    )

    summary = {
        "total_contracts":    sum(status_counts.values()),
        "in_force":           count_in_force,
        "in_service":         count_in_service,
        "active":             status_counts.get("Active", 0) + status_counts.get("PaidActive", 0),
        "paid_active":        status_counts.get("PaidActive", 0),
        "invoiced":           status_counts.get("Invoiced", 0),
        "signed":             status_counts["Signed"],
        # Lớp phủ tính từ end_date, KHÔNG phải một nhóm riêng: các hợp đồng này
        # cũng đã được đếm trong in_force. Đừng cộng nó vào các con số khác.
        "expiring_soon":      len(expiring_30),
        "expired":            status_counts["Expired"],
        "generated":          count_generated_file,
        "terminated":         status_counts["Terminated"],
        "draft":              status_counts["Draft"],
        "total_value_active": total_value_active,
        "value_paid":         value_paid,
        "value_invoiced":     value_invoiced,
        "value_unpaid":       value_unpaid,
        # Customers
        "total_customers":    db.query(Customer).filter(Customer.is_active == True, Customer.is_deleted == False).count(),
        # Templates
        "total_templates":    db.query(ContractTemplate).filter(
            ContractTemplate.is_active == True
        ).count(),
    }

    return {
        "summary":          summary,
        "status_counts":    status_counts,
        "expiring_30":      expiring_30,
        "expiring_31_60":   expiring_31_60,
        "by_product":       by_product,
        "recent_activity":  recent_activity,
        "update_counts":    update_counts,
        "today":            today,
    }


# ─── Targeted queries ─────────────────────────────────────────────────────────

def get_expiring_contracts(db: Session, days: int = 30) -> list[dict]:
    """
    Trả về list hợp đồng sắp hết hạn trong `days` ngày.
    Gọi auto_update trước để trạng thái chính xác.
    """
    auto_update_contract_statuses(db)
    return _get_expiring_list(db, date.today(), days=days)


# ─── Internal helpers ─────────────────────────────────────────────────────────

def _get_expiring_list(db: Session, today: date, days: int) -> list[dict]:
    """
    Lấy danh sách hợp đồng có end_date trong khoảng (today, today+days].
    Bao gồm mọi trạng thái còn trong vòng đời hiệu lực.
    Sắp xếp theo end_date ASC (gần hết hạn nhất lên trước).
    """
    cutoff = today + timedelta(days=days)
    active_set = ACTIVE_LIFECYCLE_STATUSES

    rows = (
        db.query(Contract, Customer)
        .outerjoin(Customer, Contract.customer_id == Customer.id)
        .filter(
            Contract.status.in_(active_set),
            Contract.end_date.isnot(None),
            Contract.end_date > today,
            Contract.end_date <= cutoff,
        )
        .order_by(Contract.end_date.asc())
        .all()
    )

    result = []
    for c, cust in rows:
        days_left = (c.end_date - today).days
        result.append({
            "contract":      c,
            "contract_id":   c.id,
            "contract_number": c.contract_number,
            "customer_name": (cust.short_name or cust.legal_name) if cust else "—",
            "customer_id":   cust.id if cust else None,
            "end_date":      c.end_date,
            "days_left":     days_left,
            "status":        c.status.value,
            "total_amount":  c.total_amount,
            # `expiry_bucket` phân loại theo ngưỡng cố định và trả None khi còn
            # >30 ngày. Nhưng ở đây mọi dòng đều nằm trong cửa sổ người dùng chọn
            # (có thể tới 365 ngày), nên đáng chú ý theo định nghĩa — quy về
            # "info". Không có bước này thì trang 60/365 ngày đếm thiếu nhóm
            # "Chú ý" vì các dòng xa hạn mang urgency None.
            "urgency":       lifecycle.expiry_bucket(c.end_date, today) or "info",
        })
    return result


def _contracts_by_product(db: Session) -> list[dict]:
    """
    Đếm hợp đồng đang hiệu lực (Active + Signed + ExpiringSoon) theo sản phẩm.
    Bao gồm cả hợp đồng không gắn sản phẩm (product_id = NULL).
    """
    active_set = ACTIVE_LIFECYCLE_STATUSES

    rows = (
        db.query(
            Product.name,
            Product.code,
            func.count(Contract.id).label("count"),
            func.sum(Contract.total_amount).label("total_value"),
        )
        .outerjoin(Contract, Contract.product_id == Product.id)
        .filter(Contract.status.in_(active_set))
        .group_by(Product.id, Product.name, Product.code)
        .order_by(func.count(Contract.id).desc())
        .all()
    )

    # Hợp đồng không có sản phẩm
    no_product_count = (
        db.query(func.count(Contract.id))
        .filter(Contract.status.in_(active_set), Contract.product_id.is_(None))
        .scalar()
        or 0
    )

    result = [
        {
            "name":        row.name or "—",
            "code":        row.code or "",
            "count":       row.count or 0,
            "total_value": Decimal(str(row.total_value or 0)),
        }
        for row in rows
    ]

    if no_product_count:
        result.append({
            "name":        "Không phân loại",
            "code":        "",
            "count":       no_product_count,
            "total_value": Decimal("0"),
        })

    return result


def get_monthly_stats(db: Session) -> dict:
    """
    Thống kê hoạt động trong tháng hiện tại và so sánh với tháng trước.
    Dùng UTC (nhất quán với trường created_at).
    """
    import calendar as _cal

    today = date.today()
    y, m = today.year, today.month

    # Biên tháng này (datetime để so với created_at)
    first_this = _datetime(y, m, 1)
    last_day   = _cal.monthrange(y, m)[1]
    last_this  = _datetime(y, m, last_day, 23, 59, 59)

    # Biên tháng trước
    prev_m = 12 if m == 1 else m - 1
    prev_y = y - 1 if m == 1 else y
    prev_last_day = _cal.monthrange(prev_y, prev_m)[1]
    first_prev = _datetime(prev_y, prev_m, 1)
    last_prev  = _datetime(prev_y, prev_m, prev_last_day, 23, 59, 59)

    def _n_created(start, end):
        return db.query(func.count(Contract.id)).filter(
            Contract.created_at >= start, Contract.created_at <= end,
        ).scalar() or 0

    def _n_signed(start_d, end_d):
        return db.query(func.count(Contract.id)).filter(
            Contract.sign_date >= start_d, Contract.sign_date <= end_d,
        ).scalar() or 0

    this_new     = _n_created(first_this, last_this)
    this_signed  = _n_signed(date(y, m, 1), date(y, m, last_day))
    this_expired = db.query(func.count(Contract.id)).filter(
        Contract.end_date >= date(y, m, 1),
        Contract.end_date <= date(y, m, last_day),
    ).scalar() or 0

    prev_new    = _n_created(first_prev, last_prev)
    prev_signed = _n_signed(date(prev_y, prev_m, 1), date(prev_y, prev_m, prev_last_day))

    new_value_raw = db.query(func.sum(Contract.total_amount)).filter(
        Contract.created_at >= first_this,
        Contract.created_at <= last_this,
        Contract.total_amount.isnot(None),
    ).scalar()
    new_value = Decimal(str(new_value_raw or 0))

    return {
        "month_label":   f"Tháng {m}/{y}",
        "new_contracts": this_new,
        "signed":        this_signed,
        "expired_count": this_expired,
        "new_value":     new_value,
        "vs_new":        this_new - prev_new,
        "vs_signed":     this_signed - prev_signed,
    }


def get_top_customers(db: Session, limit: int = 5) -> list[dict]:
    """Top khách hàng có nhiều HĐ đang hiệu lực nhất."""
    active_set = ACTIVE_LIFECYCLE_STATUSES
    rows = (
        db.query(
            Customer.id,
            Customer.short_name,
            Customer.legal_name,
            func.count(Contract.id).label("cnt"),
        )
        .join(Contract, Contract.customer_id == Customer.id)
        .filter(
            Contract.status.in_(active_set),
            Customer.is_active == True,
            Customer.is_deleted == False,
        )
        .group_by(Customer.id)
        .order_by(func.count(Contract.id).desc())
        .limit(limit)
        .all()
    )
    return [
        {
            "id":    row.id,
            "name":  (row.short_name or row.legal_name or "—"),
            "count": row.cnt,
        }
        for row in rows
    ]


def get_monthly_expiry_forecast(db: Session, months: int = 12) -> list[dict]:
    """
    Đếm HĐ sắp hết hạn theo từng tháng trong `months` tháng tới.
    Trả về list đủ `months` phần tử (kể cả tháng count=0) để chart hiển thị đầy đủ.
    """
    import calendar as _cal

    def _add_months(d: date, n: int) -> date:
        m = d.month - 1 + n
        y = d.year + m // 12
        m = m % 12 + 1
        return date(y, m, min(d.day, _cal.monthrange(y, m)[1]))

    today = date.today()
    cutoff = _add_months(today, months)

    active_set = ACTIVE_LIFECYCLE_STATUSES

    rows = (
        db.query(
            func.strftime('%Y-%m', Contract.end_date).label('ym'),
            func.count(Contract.id).label('cnt'),
        )
        .filter(
            Contract.status.in_(active_set),
            Contract.end_date.isnot(None),
            Contract.end_date > today,
            Contract.end_date <= cutoff,
        )
        .group_by(func.strftime('%Y-%m', Contract.end_date))
        .order_by(func.strftime('%Y-%m', Contract.end_date))
        .all()
    )

    counts = {row.ym: row.cnt for row in rows}

    result = []
    for i in range(1, months + 1):
        d = _add_months(today, i)
        ym = f"{d.year}-{d.month:02d}"
        result.append({
            "month": ym,
            "label": f"T{d.month}/{str(d.year)[2:]}",
            "count": counts.get(ym, 0),
        })
    return result


def get_action_items(db: Session) -> list[dict]:
    """
    Sinh danh sách việc cần xử lý dựa trên trạng thái hiện tại.
    Trả về list[dict] — mỗi item: level (high/medium/low), icon, message, link, count.
    Chỉ trả về item có điều kiện thỏa, list rỗng = không có việc tồn đọng.
    """
    from datetime import datetime as _dt
    today = date.today()
    items: list[dict] = []

    # ── 1. HĐ đang hiệu lực còn ≤7 ngày ─────────────────────────────────────
    # Lọc theo end_date, không theo trạng thái. Bản cũ lọc `status == ExpiringSoon`
    # nên sau khi bỏ trạng thái đó, mục việc gấp nhất sẽ luôn ra 0.
    critical_count = (
        db.query(func.count(Contract.id))
        .filter(
            Contract.status.in_(ACTIVE_LIFECYCLE_STATUSES),
            Contract.end_date.isnot(None),
            Contract.end_date > today,
            Contract.end_date <= today + timedelta(days=lifecycle.EXPIRY_CRITICAL_DAYS),
        )
        .scalar() or 0
    )
    if critical_count:
        items.append({
            "level":   "high",
            "icon":    "bi-alarm-fill",
            "message": f"{critical_count} hợp đồng hết hạn trong 7 ngày",
            "link":    "/dashboard/expiring?days=7",
            "count":   critical_count,
        })

    # ── 2. HĐ Draft chưa sinh file ───────────────────────────────────────────
    draft_no_file = (
        db.query(func.count(Contract.id))
        .filter(
            Contract.status == ContractStatus.Draft,
            Contract.output_file_path.is_(None),
        )
        .scalar() or 0
    )
    if draft_no_file:
        items.append({
            "level":   "medium",
            "icon":    "bi-file-earmark-x",
            "message": f"{draft_no_file} hợp đồng nháp chưa được sinh file",
            "link":    "/contracts?status=Draft",
            "count":   draft_no_file,
        })

    # ── 3. HĐ Generated > 14 ngày, chưa chuyển Sent ─────────────────────────
    stale_dt = _dt.utcnow() - timedelta(days=14)
    generated_stale = (
        db.query(func.count(Contract.id))
        .filter(
            Contract.status == ContractStatus.Generated,
            Contract.updated_at <= stale_dt,
        )
        .scalar() or 0
    )
    if generated_stale:
        items.append({
            "level":   "medium",
            "icon":    "bi-clock",
            "message": f"{generated_stale} hợp đồng đã sinh file, chưa gửi KH (>14 ngày)",
            "link":    "/contracts?status=Generated",
            "count":   generated_stale,
        })

    # ── 4. HĐ Signed > 7 ngày, chưa kích hoạt ───────────────────────────────
    signed_stale = (
        db.query(func.count(Contract.id))
        .filter(
            Contract.status == ContractStatus.Signed,
            Contract.sign_date.isnot(None),
            Contract.sign_date <= today - timedelta(days=7),
        )
        .scalar() or 0
    )
    if signed_stale:
        items.append({
            "level":   "medium",
            "icon":    "bi-pen",
            "message": f"{signed_stale} hợp đồng đã ký, chưa chuyển hiệu lực (>7 ngày)",
            "link":    "/contracts?status=Signed",
            "count":   signed_stale,
        })

    # ── 5. Không có mẫu hợp đồng nào đang hoạt động ─────────────────────────
    active_tpl = (
        db.query(func.count(ContractTemplate.id))
        .filter(ContractTemplate.is_active == True)
        .scalar() or 0
    )
    if active_tpl == 0:
        items.append({
            "level":   "low",
            "icon":    "bi-file-earmark-word",
            "message": "Chưa có mẫu hợp đồng nào đang hoạt động",
            "link":    "/templates/new",
            "count":   0,
        })

    return items


def _add_event(db: Session, contract_id: int, event_type: str, description: str, actor: str):
    ev = ContractEvent(
        contract_id=contract_id,
        event_type=event_type,
        description=description,
        actor=actor,
    )
    db.add(ev)
