"""
Router: /batch — Sinh hàng loạt hợp đồng.

GET  /batch                → form chọn template + KH + shared params
POST /batch                → chạy batch với shared params
POST /batch/preview        → xem trước danh sách (JSON — dùng trong form)
GET  /batch/import         → trang upload Excel mapping
POST /batch/import         → xử lý Excel → preview
POST /batch/run-from-excel → chạy batch từ kết quả Excel đã parse
GET  /batch/template.xlsx  → tải file Excel mẫu
GET  /batch/download/{job_zip} → tải ZIP đã sinh
"""

import io, os, json
import unicodedata
from datetime import date as _date, timedelta
from calendar import monthrange
from fastapi import APIRouter, Request, Form, UploadFile, File, HTTPException
from fastapi.responses import HTMLResponse, RedirectResponse, FileResponse, StreamingResponse
from fastapi.templating import Jinja2Templates
from typing import Optional

from app.database import SessionLocal
from app.paths import TEMPLATES_DIR
from app.models.customer import Customer
from app.models.contract_template import ContractTemplate
from app.models.product import Product
from app.services.customer_service import CustomerFilters, list_customers
from app.services.batch_generator import (
    run_batch, build_zip_bytes, import_excel_mapping,
    make_batch_template_xlsx, OUTPUT_DIR, BatchResult,
)

router = APIRouter(prefix="/batch", tags=["batch"])
tpl_env = Jinja2Templates(directory=str(TEMPLATES_DIR))

def _normalize_vi(s: str) -> str:
    """Xóa dấu tiếng Việt để tìm kiếm không dấu."""
    if not s:
        return ""
    s = unicodedata.normalize("NFD", s)
    s = "".join(c for c in s if unicodedata.category(c) != "Mn")
    return s.replace("đ", "d").replace("Đ", "D").lower()

tpl_env.env.filters["normalize_vi"] = _normalize_vi


def _flash(url, msg, t="success"):
    sep = "&" if "?" in url else "?"
    return RedirectResponse(f"{url}{sep}msg={msg}&msg_type={t}", status_code=303)


def _fv(request):
    return {
        "flash_msg":  request.query_params.get("msg", ""),
        "flash_type": request.query_params.get("msg_type", "success"),
    }


# ─── MAIN FORM ───────────────────────────────────────────────────────────────

@router.get("", response_class=HTMLResponse)
def batch_form(request: Request):
    db = SessionLocal()
    try:
        rows, _ = list_customers(db, CustomerFilters(active_only=True, per_page=500, sort_by='code', sort_dir='asc'))
        customers_raw = [c for c, *_ in rows]
        tpls = (db.query(ContractTemplate)
                  .filter(ContractTemplate.is_active == True)
                  .order_by(ContractTemplate.name).all())
        products = (db.query(Product)
                      .filter(Product.is_active == True)
                      .order_by(Product.name).all())

        # 1 query GROUP BY thay vì N query COUNT riêng — từ O(n) xuống O(1)
        from app.models.customer_unit import CustomerUnit
        from sqlalchemy import func as _func
        unit_counts = dict(
            db.query(CustomerUnit.customer_id, _func.count(CustomerUnit.id))
            .filter(CustomerUnit.is_active == True)
            .group_by(CustomerUnit.customer_id)
            .all()
        )
        customers_list = []
        for c in customers_raw:
            sub_count = unit_counts.get(c.id, 0)
            raw_text = f"{c.code} {c.short_name or ''} {c.legal_name or ''}"
            search_key = _normalize_vi(raw_text)
            customers_list.append({
                "id": c.id,
                "code": c.code,
                "label": c.short_name or c.legal_name,
                "units": 1 + sub_count,
                "search": search_key,
            })
    finally:
        db.close()

    return tpl_env.TemplateResponse(request, "batch/form.html", {
        "request": request, "page_title": "Sinh hàng loạt",
        # Template dùng `| tojson` — xem chú thích ở routers/dashboard.py.
        "customers_data": customers_list,
        "templates": tpls, "products": products,
        "prefill": None,
        "errors": [], **_fv(request),
    })


@router.post("")
async def batch_run(request: Request):
    """
    Step 3: nhận dữ liệu từ trang preview, chạy batch với per-row overrides.
    Mỗi hàng có: row_customer_id_{i}, row_contract_number_{i},
                 row_sign_date_{i}, row_start_date_{i}, row_end_date_{i},
                 row_acceptance_date_{i}, row_liquidation_date_{i}, row_unit_count_{i}
    selected_rows: comma-separated list of selected row indices
    """
    form = await request.form()
    data = dict(form)

    template_id_str = data.get("template_id", "")
    row_count_str   = data.get("row_count", "0")
    selected_str    = data.get("selected_rows", "")

    try:
        template_id = int(template_id_str)
        row_count   = int(row_count_str)
        selected_idxs = {int(x) for x in selected_str.split(",") if x.strip().isdigit()}
    except ValueError:
        raise HTTPException(400, "Dữ liệu form không hợp lệ")

    if not selected_idxs:
        return _flash("/batch", "Không có hàng nào được chọn", "warning")

    # Build per-customer overrides from row data
    customer_ids = []
    per_customer_overrides = {}

    for i in selected_idxs:
        cid_str = data.get(f"row_customer_id_{i}", "")
        if not cid_str:
            continue
        try:
            cid = int(cid_str)
        except ValueError:
            continue
        customer_ids.append(cid)
        per_customer_overrides[cid] = {
            "contract_number":   data.get(f"row_contract_number_{i}", ""),
            "sign_date":         data.get(f"row_sign_date_{i}", ""),
            "start_date":        data.get(f"row_start_date_{i}", ""),
            "end_date":          data.get(f"row_end_date_{i}", ""),
            "acceptance_date":   data.get(f"row_acceptance_date_{i}", ""),
            "liquidation_date":  data.get(f"row_liquidation_date_{i}", ""),
            "unit_count":        data.get(f"row_unit_count_{i}", ""),
        }

    if not customer_ids:
        return _flash("/batch", "Không có khách hàng nào hợp lệ", "warning")

    # Shared params (không có customer_ids, row_* fields, meta fields)
    skip = {"template_id","row_count","selected_rows","customer_ids"}
    skip.update({k for k in data if k.startswith("row_")})
    shared = {k: v for k, v in data.items() if k not in skip}

    db = SessionLocal()
    try:
        result = run_batch(
            db=db,
            template_id=template_id,
            customer_ids=customer_ids,
            shared_params=shared,
            per_customer_overrides=per_customer_overrides,
        )
    except (LookupError, FileNotFoundError) as e:
        return _flash("/batch", str(e), "danger")
    finally:
        db.close()

    return RedirectResponse(
        f"/batch/result?job_id={result.job_id}&s={result.succeeded}&f={result.failed}",
        status_code=303,
    )


# ─── BATCH PREVIEW ───────────────────────────────────────────────────────────

@router.post("/preview", response_class=HTMLResponse)
async def batch_preview(request: Request):
    """
    Step 2: nhận thông số chung + danh sách KH đã chọn,
    trả về trang xem trước với số HĐ + các ngày tự tính cho từng KH.
    """
    form = await request.form()
    data = dict(form)
    customer_ids_raw = form.getlist("customer_ids")

    errors = []
    if not data.get("template_id"):
        errors.append("Phải chọn mẫu hợp đồng.")
    if not customer_ids_raw:
        errors.append("Phải chọn ít nhất 1 khách hàng.")

    if errors:
        db = SessionLocal()
        try:
            rows, _ = list_customers(db, CustomerFilters(active_only=True, per_page=500, sort_by='code', sort_dir='asc'))
            customers = [c for c, *_ in rows]
            tpls = db.query(ContractTemplate).filter(ContractTemplate.is_active==True).all()
            products = db.query(Product).filter(Product.is_active==True).all()
        finally:
            db.close()
        return tpl_env.TemplateResponse(request, "batch/form.html", {
            "request": request, "page_title": "Sinh hàng loạt",
            "customers": customers, "templates": tpls, "products": products,
            "errors": errors, "prefill": data,
        }, status_code=422)

    from app.services.contract_service import (
        get_next_contract_seq, make_contract_slug,
        build_contract_number, contract_type_from_code,
    )

    try:
        cust_ids = [int(x) for x in customer_ids_raw if x.strip()]
        template_id = int(data["template_id"])
    except ValueError:
        raise HTTPException(400, "Dữ liệu form không hợp lệ")

    sign_date   = data.get("sign_date", "")
    start_date  = data.get("start_date", "") or sign_date
    month_count = int(data.get("month_count") or 12)
    year        = int(data.get("year") or _date.today().year)

    def add_months(d, months):
        m = d.month - 1 + months
        yr = d.year + m // 12
        mo = m % 12 + 1
        day = min(d.day, monthrange(yr, mo)[1])
        return _date(yr, mo, day)

    def calc_dates(start_str):
        if not start_str:
            return "", "", ""
        try:
            s = _date.fromisoformat(start_str)
        except ValueError:
            return "", "", ""
        end = add_months(s, month_count) - timedelta(days=1)
        acc = s + timedelta(days=2)
        return str(end), str(acc), str(end)  # end, acceptance, liquidation

    db = SessionLocal()
    try:
        tpl = db.query(ContractTemplate).filter(ContractTemplate.id == template_id).first()
        c_type = contract_type_from_code(tpl.code if tpl else "")

        preview_rows = []
        for cid in cust_ids:
            cust = db.query(Customer).filter(Customer.id == cid).first()
            if not cust:
                continue
            seq  = get_next_contract_seq(db, cid, contract_type=c_type, year=year)
            slug = make_contract_slug(cust.legal_name or "")
            num  = build_contract_number(seq, slug, c_type, year)
            end_d, acc_d, liq_d = calc_dates(start_date)
            total_units = cust.total_units
            preview_rows.append({
                "customer_id":   cid,
                "customer_name": cust.short_name or cust.legal_name,
                "customer_legal":cust.legal_name,
                "contract_number": num,
                "sign_date":  sign_date,
                "start_date": start_date,
                "end_date":   end_d,
                "acceptance_date": acc_d,
                "liquidation_date": liq_d,
                "unit_count": total_units,
            })
    finally:
        db.close()

    return tpl_env.TemplateResponse(request, "batch/preview.html", {
        "request": request, "page_title": "Sinh hàng loạt — Xem trước",
        "preview_rows": preview_rows,
        "shared_params": data,
        "template_id": template_id,
        "month_count": month_count,
        "sign_date": sign_date,
        "start_date": start_date,
        "errors": [],
    })


# ─── RESULT PAGE ─────────────────────────────────────────────────────────────

@router.get("/result", response_class=HTMLResponse)
def batch_result(request: Request, job_id: str = "", s: int = 0, f: int = 0):
    """
    Hiển thị kết quả sau khi chạy batch.
    Tìm file ZIP trong OUTPUT_DIR theo job_id.
    """
    zip_path = ""
    zip_name = ""
    if job_id:
        for fname in os.listdir(OUTPUT_DIR) if os.path.exists(OUTPUT_DIR) else []:
            if fname.startswith(f"batch_{job_id}") and fname.endswith(".zip"):
                zip_path = os.path.join(OUTPUT_DIR, fname)
                zip_name = fname
                break

    return tpl_env.TemplateResponse(request, "batch/result.html", {
        "request": request, "page_title": "Sinh hàng loạt",
        "job_id": job_id, "succeeded": s, "failed": f,
        "zip_path": zip_path, "zip_name": zip_name,
        **_fv(request),
    })


# ─── IMPORT EXCEL ────────────────────────────────────────────────────────────

@router.get("/import", response_class=HTMLResponse)
def import_form(request: Request):
    db = SessionLocal()
    try:
        tpls = db.query(ContractTemplate).filter(ContractTemplate.is_active==True).all()
    finally:
        db.close()
    return tpl_env.TemplateResponse(request, "batch/import.html", {
        "request": request, "page_title": "Sinh hàng loạt",
        "templates": tpls, "import_result": None, "errors": [],
        **_fv(request),
    })


@router.post("/import")
async def import_submit(
    request: Request,
    file: UploadFile = File(...),
    template_id: str = Form(...),
):
    if not file.filename.endswith((".xlsx", ".xls")):
        return tpl_env.TemplateResponse(request, "batch/import.html", {
            "request": request, "page_title": "Sinh hàng loạt",
            "templates": [], "import_result": None,
            "errors": ["Chỉ chấp nhận file .xlsx hoặc .xls"],
        }, status_code=422)

    file_bytes = await file.read()

    db = SessionLocal()
    try:
        # Build lookup: code → id  và  short_name → id
        all_customers = db.query(Customer).filter(Customer.is_active==True, Customer.is_deleted==False).all()
        lookup: dict[str, int] = {}
        for c in all_customers:
            if c.code: lookup[c.code] = c.id
            if c.short_name: lookup[c.short_name] = c.id
            if c.legal_name: lookup[c.legal_name] = c.id

        excel_result = import_excel_mapping(file_bytes, lookup)
        tpls = db.query(ContractTemplate).filter(ContractTemplate.is_active==True).all()
        sel_tpl = db.query(ContractTemplate).filter(
            ContractTemplate.id == int(template_id)
        ).first() if template_id else None
    finally:
        db.close()

    return tpl_env.TemplateResponse(request, "batch/import.html", {
        "request": request, "page_title": "Sinh hàng loạt",
        "templates": tpls, "sel_template_id": template_id,
        "import_result": excel_result,
        "import_rows_json": json.dumps(excel_result.rows),
        "errors": [],
    })


@router.post("/run-from-excel")
async def run_from_excel(request: Request):
    """
    Nhận rows JSON từ form hidden field + template_id,
    chạy batch với per-customer overrides từ Excel.
    """
    form = await request.form()
    template_id_str = form.get("template_id", "")
    rows_json = form.get("rows_json", "[]")
    prefix = form.get("contract_number_prefix", "").strip()

    try:
        template_id = int(template_id_str)
        rows = json.loads(rows_json)
    except (ValueError, json.JSONDecodeError) as e:
        raise HTTPException(400, f"Dữ liệu form lỗi: {e}")

    if not rows:
        return _flash("/batch/import", "Không có dữ liệu để sinh", "warning")

    customer_ids = []
    per_customer_overrides = {}
    for row in rows:
        cid = row.get("customer_id")
        if cid is None:
            continue
        cid = int(cid)
        customer_ids.append(cid)
        override = {k: v for k, v in row.items() if k != "customer_id"}
        if override:
            per_customer_overrides[cid] = override

    if not customer_ids:
        return _flash("/batch/import", "Không tìm thấy khách hàng nào trong dữ liệu Excel", "danger")

    db = SessionLocal()
    try:
        result = run_batch(
            db=db,
            template_id=template_id,
            customer_ids=customer_ids,
            shared_params={},
            per_customer_overrides=per_customer_overrides,
            contract_number_prefix=prefix,
        )
    except (LookupError, FileNotFoundError) as e:
        return _flash("/batch/import", str(e), "danger")
    finally:
        db.close()

    return RedirectResponse(
        f"/batch/result?job_id={result.job_id}&s={result.succeeded}&f={result.failed}",
        status_code=303,
    )


# ─── TEMPLATE DOWNLOAD ───────────────────────────────────────────────────────

@router.get("/template.xlsx")
def download_template():
    xlsx_bytes = make_batch_template_xlsx()
    return StreamingResponse(
        io.BytesIO(xlsx_bytes),
        media_type="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        headers={"Content-Disposition": "attachment; filename=mau_batch_import.xlsx"},
    )


# ─── ZIP DOWNLOAD ────────────────────────────────────────────────────────────

@router.get("/download/{zip_name}")
def download_zip(zip_name: str):
    """Tải file ZIP đã sinh."""
    # Security: chỉ cho phép tên file an toàn (no path traversal)
    if "/" in zip_name or "\\" in zip_name or ".." in zip_name:
        raise HTTPException(400, "Tên file không hợp lệ")
    if not zip_name.endswith(".zip"):
        raise HTTPException(400, "Chỉ hỗ trợ file .zip")

    path = os.path.join(OUTPUT_DIR, zip_name)
    if not os.path.exists(path):
        raise HTTPException(404, "File ZIP không tồn tại hoặc đã hết hạn")

    return FileResponse(
        path=path, filename=zip_name,
        media_type="application/zip",
    )
