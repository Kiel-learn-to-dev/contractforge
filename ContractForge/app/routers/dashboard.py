"""
Router: /dashboard — Dashboard chính thức và API endpoint.

Routes:
  GET  /dashboard                  → Trang dashboard đầy đủ
  GET  /dashboard/expiring         → Trang danh sách hợp đồng sắp hết hạn
  GET  /dashboard/expiring/export  → Download CSV danh sách sắp hết hạn
  GET  /api/stats                  → JSON stats cho AJAX refresh
"""
import csv
import io
import json
from datetime import date
from decimal import Decimal
from fastapi import APIRouter, Request
from fastapi.responses import HTMLResponse, JSONResponse, StreamingResponse
from fastapi.templating import Jinja2Templates

from sqlalchemy import or_, func as sqlfunc
from app.database import SessionLocal
from app.paths import TEMPLATES_DIR
from app.models.contract import Contract, ContractStatus
from app.models.customer import Customer
from app.services.dashboard_service import (
    get_dashboard_data, get_expiring_contracts, get_action_items,
    get_monthly_expiry_forecast, get_monthly_stats, get_top_customers,
    EXPIRING_SOON_DAYS, EXPIRING_WARN_DAYS,
)
from app.services import lifecycle

router = APIRouter(tags=["dashboard"])
templates = Jinja2Templates(directory=str(TEMPLATES_DIR))


# ─── Helpers ─────────────────────────────────────────────────────────────────

def _fmt_currency(v) -> str:
    if not v:
        return "0"
    try:
        n = int(Decimal(str(v)))
        return f"{n:,}".replace(",", ".")
    except Exception:
        return str(v)


# ─── Main dashboard ──────────────────────────────────────────────────────────

@router.get("/dashboard", response_class=HTMLResponse)
def dashboard(request: Request):
    db = SessionLocal()
    try:
        data             = get_dashboard_data(db)
        action_items     = get_action_items(db)
        monthly_forecast = get_monthly_expiry_forecast(db)
        monthly_stats    = get_monthly_stats(db)
        top_customers    = get_top_customers(db)
    finally:
        db.close()

    # Serialize chart data — tránh vấn đề Decimal không JSON-serializable
    monthly_forecast_json = json.dumps(
        [{"label": m["label"], "count": m["count"]} for m in monthly_forecast]
    )
    by_product_json = json.dumps(
        [{"name": p["name"][:30], "count": p["count"]} for p in data["by_product"]]
    )

    # Format total_value for display
    data["summary"]["total_value_fmt"]    = _fmt_currency(data["summary"]["total_value_active"])
    data["summary"]["value_paid_fmt"]     = _fmt_currency(data["summary"]["value_paid"])
    data["summary"]["value_invoiced_fmt"] = _fmt_currency(data["summary"]["value_invoiced"])
    data["summary"]["value_unpaid_fmt"]   = _fmt_currency(data["summary"]["value_unpaid"])

    # Tính % cho status chart (tránh div/0)
    total = data["summary"]["total_contracts"] or 1
    data["status_pct"] = {
        k: round(v / total * 100, 1)
        for k, v in data["status_counts"].items()
        if v > 0
    }

    # Format monthly stats value
    mv = monthly_stats["new_value"]
    monthly_stats["new_value_fmt"] = _fmt_currency(mv) if mv else "0"

    return templates.TemplateResponse(request, "dashboard/index.html", {
        "page_title":            "Dashboard",
        "action_items":          action_items,
        "monthly_forecast_json": monthly_forecast_json,
        "by_product_json":       by_product_json,
        "monthly_stats":         monthly_stats,
        "top_customers":         top_customers,
        **data,
    })


# ─── Expiring contracts page ──────────────────────────────────────────────────

@router.get("/dashboard/expiring", response_class=HTMLResponse)
def dashboard_expiring(request: Request, days: int = 60):
    days = max(1, min(days, 365))   # clamp 1–365
    db = SessionLocal()
    try:
        contracts = get_expiring_contracts(db, days=days)
    finally:
        db.close()

    return templates.TemplateResponse(request, "dashboard/expiring.html", {
        "page_title": "Dashboard",
        "contracts": contracts,
        "days":      days,
        "today":     date.today(),
        "status_labels": lifecycle.STATUS_LABELS,
    })


# ─── Export CSV ──────────────────────────────────────────────────────────────

@router.get("/dashboard/expiring/export")
def export_expiring(days: int = 60):
    """Download CSV danh sách hợp đồng sắp hết hạn — mở được bằng Excel."""
    days = max(1, min(days, 365))
    db = SessionLocal()
    try:
        contracts = get_expiring_contracts(db, days=days)
    finally:
        db.close()

    buf = io.StringIO()
    buf.write("﻿")  # UTF-8 BOM — Excel Windows nhận đúng tiếng Việt
    writer = csv.writer(buf)
    writer.writerow(["Số hợp đồng", "Khách hàng", "Ngày hết hạn",
                     "Còn lại (ngày)", "Trạng thái", "Giá trị (VNĐ)"])
    for item in contracts:
        writer.writerow([
            item["contract_number"],
            item["customer_name"],
            item["end_date"].strftime("%d/%m/%Y"),
            item["days_left"],
            lifecycle.status_label(item["status"]),
            int(item["total_amount"]) if item["total_amount"] else "",
        ])

    content = buf.getvalue()
    filename = f"sap_het_han_{days}ngay.csv"
    return StreamingResponse(
        iter([content]),
        media_type="text/csv; charset=utf-8-sig",
        headers={"Content-Disposition": f'attachment; filename="{filename}"'},
    )


# ─── Global Search API ────────────────────────────────────────────────────────

# Nhãn RÚT GỌN, chỉ dùng cho ô tìm kiếm nhanh — mỗi dòng kết quả rất hẹp nên
# "Xuất HĐ" vừa chỗ còn "Đã xuất hóa đơn" thì không. Cố ý khác
# lifecycle.STATUS_LABELS (nhãn đầy đủ cho bảng biểu). Base.html tra màu theo
# đúng chuỗi này, nên đổi ở đây phải đổi cả STATUS_COLOR bên đó.
_STATUS_VI = {
    "Draft": "Nháp", "Generated": "Đã sinh file", "Sent": "Đã gửi",
    "Signed": "Đã ký", "Active": "Hiệu lực", "Invoiced": "Xuất HĐ",
    "PaidActive": "Đã thanh toán", "ExpiringSoon": "Sắp hết hạn",
    "Expired": "Hết hạn", "Terminated": "Thanh lý",
}

@router.get("/api/search")
def api_search(q: str = "", limit: int = 10):
    """Tìm kiếm nhanh hợp đồng và khách hàng."""
    q = q.strip()
    if len(q) < 2:
        return JSONResponse({"results": []})

    db = SessionLocal()
    try:
        pattern = f"%{q.lower()}%"

        # ── Tìm hợp đồng ─────────────────────────────────────────────────────
        contract_rows = (
            db.query(Contract, Customer)
            .outerjoin(Customer, Contract.customer_id == Customer.id)
            .filter(or_(
                sqlfunc.lower(Contract.contract_number).like(pattern),
                sqlfunc.lower(Customer.legal_name).like(pattern),
                sqlfunc.lower(Customer.short_name).like(pattern),
            ))
            .order_by(Contract.created_at.desc())
            .limit(limit)
            .all()
        )

        results = [
            {
                "type":   "contract",
                "id":     c.id,
                "label":  c.contract_number,
                "sub":    (cust.short_name or cust.legal_name or "—") if cust else "—",
                "url":    f"/contracts/{c.id}",
                "status": _STATUS_VI.get(c.status.value, c.status.value),
            }
            for c, cust in contract_rows
        ]

        # ── Tìm khách hàng (thêm vào nếu chưa xuất hiện từ HĐ) ──────────────
        cust_ids_in_results = {r["id"] for r in results if r["type"] == "contract"}
        cust_rows = (
            db.query(Customer)
            .filter(
                or_(
                    sqlfunc.lower(Customer.legal_name).like(pattern),
                    sqlfunc.lower(Customer.short_name).like(pattern),
                ),
                Customer.is_active == True,
                Customer.is_deleted == False,
            )
            .limit(5)
            .all()
        )
        for cust in cust_rows:
            results.append({
                "type":   "customer",
                "id":     cust.id,
                "label":  cust.short_name or cust.legal_name or "—",
                "sub":    "Khách hàng",
                "url":    f"/customers/{cust.id}",
                "status": None,
            })

    finally:
        db.close()

    return JSONResponse({"results": results[:limit]})


# ─── JSON API ─────────────────────────────────────────────────────────────────

@router.get("/api/stats")
def api_stats():
    """JSON endpoint — dùng cho AJAX refresh stat cards trên dashboard."""
    db = SessionLocal()
    try:
        data = get_dashboard_data(db)
    finally:
        db.close()

    s = data["summary"].copy()
    s["total_value_active"] = str(s["total_value_active"])

    return JSONResponse({
        "summary":       s,
        "status_counts": data["status_counts"],
        "expiring_30":   len(data["expiring_30"]),
        "expiring_60":   len(data["expiring_30"]) + len(data["expiring_31_60"]),
        "updated_at":    data["today"].isoformat(),
        # Giá trị tương ứng 1-1 với 6 stat cards — JS dùng data-stat attribute
        "cards": {
            "active":        s["active"] + s["invoiced"] + s["expiring_soon"],
            "signed":        s["signed"],
            "expiring_soon": s["expiring_soon"],
            "expired":       s["expired"],
            "customers":     s["total_customers"],
            "generated":     s["generated"],
        },
    })
