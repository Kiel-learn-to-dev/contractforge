"""gen_fixtures.py — sinh "fixture vàng" cho bản Rust từ chính code Python.

Mỗi file JSON trong contract_rust/fixtures/ là cặp (đầu vào → kết quả) do code
Python hiện hành tính ra. Test Rust đọc các file này và phải ra đúng y hệt.
Nhờ vậy "giữ nguyên nghiệp vụ" được kiểm bằng máy, không bằng trí nhớ.

QUY TẮC: chỉ dùng dữ liệu HƯ CẤU. Không đọc DB thật, không đọc data/uploads.
Script tự trỏ CONTRACTFORGE_DATA_ROOT vào một thư mục tạm trước khi import app.

Chạy (từ thư mục gốc repo):
    .venv\\Scripts\\python.exe contract_rust\\tools\\gen_fixtures.py
"""

from __future__ import annotations

import copy
import io
import json
import os
import sys
import tempfile
import zipfile
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
APP = ROOT / "ContractForge"
OUT = ROOT / "contract_rust" / "fixtures"
DOCX_OUT = OUT / "docx"

# Cô lập dữ liệu: KHÔNG BAO GIỜ trỏ vào data/ thật.
os.environ["CONTRACTFORGE_DATA_ROOT"] = tempfile.mkdtemp(prefix="cf_fixtures_")
sys.path.insert(0, str(APP))

import docx  # noqa: E402
from docx.oxml.ns import qn  # noqa: E402
from lxml import etree  # noqa: E402

from app.models.contract import ContractStatus  # noqa: E402
from app.services import lifecycle, numbering, pricing  # noqa: E402
from app.services import template_service as ts  # noqa: E402
from app.services import manual_quotation as mq  # noqa: E402
from app.services import contract_service as cs  # noqa: E402
from app.services.docx_renderer import (  # noqa: E402
    _fmt_currency, _fmt_number, _fmt_vat, apply_template_mapping,
    build_render_context, lists_from_lines, render_docx,
)
from app.services.form08a_service import build_08a_context, _work_content  # noqa: E402
from app.services.template_analyzer import analyze, CANONICAL_MAP, ROW_PLACEHOLDERS  # noqa: E402
from app.utils import amount_to_words_vi as atw  # noqa: E402
from app.utils import date_helpers as dh  # noqa: E402
from app.utils import file_naming as fn  # noqa: E402


# ─── helpers ────────────────────────────────────────────────────────────────

def s(v):
    """Chuẩn hoá về JSON: Decimal → chuỗi, date → ISO, None giữ nguyên."""
    if isinstance(v, Decimal):
        return str(v)
    if isinstance(v, (date, datetime)):
        return v.isoformat()
    if isinstance(v, dict):
        return {k: s(x) for k, x in v.items()}
    if isinstance(v, (list, tuple)):
        return [s(x) for x in v]
    return v


def call(fn_, *args, **kw):
    """Gọi hàm, trả {"ok": kết quả} hoặc {"err": tên lớp lỗi, "msg": thông điệp}."""
    try:
        return {"ok": s(fn_(*args, **kw))}
    except Exception as exc:  # noqa: BLE001 — fixture ghi lại đúng lỗi Python ném
        return {"err": type(exc).__name__, "msg": str(exc)}


def dump(name: str, data) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / f"{name}.json"
    path.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"  wrote {path.relative_to(ROOT)}")


# ─── 1. Số tiền bằng chữ ────────────────────────────────────────────────────

def gen_amount_to_words():
    nums = [0, 1, 5, 10, 11, 14, 15, 20, 21, 24, 25, 55, 100, 101, 105, 110, 111, 115, 121,
            125, 999, 1000, 1001, 1005, 1010, 1015, 10_000, 15_000, 100_000, 105_000,
            1_000_000, 1_000_005, 1_005_000, 21_000_000, 24_000_000, 22_856_736,
            100_000_000, 999_999_999, 1_000_000_000, 1_000_000_001, 2_500_000_000,
            1_000_000_000_000, 999_999_999_999, 999_999_999_999_999]
    cases = []
    for n in nums:
        cases.append({"input": str(n), "words": call(atw.amount_to_words, n),
                      "chan": call(atw.amount_to_words_chan, n)})
    for raw in ["1234.99", "1234.5", "-1", "abc", ""]:
        cases.append({"input": raw, "words": call(atw.amount_to_words, raw),
                      "chan": call(atw.amount_to_words_chan, raw)})
    dump("amount_to_words", {"source": "app/utils/amount_to_words_vi.py", "cases": cases})


# ─── 2. Slug, tên file ──────────────────────────────────────────────────────

def gen_names():
    names = ["Trạm Y tế xã Sông Xanh", "Trung tâm Y tế huyện Đồi Cát", "Phòng y tế Thành phố Mây",
             "Công ty TNHH An Bình", "Trạm y tế phường Hòa Đông", "Đơn vị Ước Mơ",
             "TRẠM Y TẾ THỊ TRẤN BẾN ĐÁ", "Bệnh viện  Quận   Năm", "Trạm y tế xã Ơn Ưu (cơ sở 2)", ""]
    slug = [{"input": n, "output": cs.make_contract_slug(n)} for n in names]
    san = []
    for n in names + ["01/ABC/2026", "Hợp đồng", "Phụ lục bổ sung", "KH-001", "a" * 60, "x--y__z"]:
        for ml in (10, 15, 30, 40):
            san.append({"input": n, "max_len": ml, "output": fn.sanitize_for_filename(n, max_len=ml)})
    ts_ = datetime(2026, 1, 15, 9, 30, 45)
    out = []
    for num, code, ctype in [("01/ABC/2026", "KH-001", "Hợp đồng"),
                             ("PL01/02/XYZ/2026", "KH-002", "Phụ lục bổ sung"),
                             ("", "", ""), ("03/TYTSONGXANH/2026", "KH-010", "Nghiệm thu")]:
        out.append({"contract_number": num, "customer_code": code, "contract_type": ctype,
                    "ts": ts_.isoformat(),
                    "output": fn.make_output_filename(num, code, ctype, ts=ts_)})
    dump("names", {"source": ["contract_service.make_contract_slug",
                              "utils/file_naming.py"],
                   "slug": slug, "sanitize": san, "output_filename": out})


# ─── 3. Ngày tháng ──────────────────────────────────────────────────────────

def gen_dates():
    starts = [date(2026, 1, 1), date(2026, 1, 15), date(2026, 1, 31), date(2024, 1, 31),
              date(2026, 2, 28), date(2024, 2, 29), date(2026, 3, 31), date(2026, 12, 31),
              date(2026, 8, 31)]
    months = [1, 2, 3, 11, 12, 13, 24, 36]
    add, end = [], []
    for st in starts:
        for m in months:
            add.append({"start": st.isoformat(), "months": m, "output": dh.add_months(st, m).isoformat()})
            end.append({"start": st.isoformat(), "months": m,
                        "output": dh.compute_end_date(st, m).isoformat()})
    between = []
    for st, en in [(date(2027, 1, 1), date(2027, 12, 31)), (date(2027, 1, 15), date(2027, 2, 14)),
                   (date(2026, 1, 31), date(2026, 2, 27)), (date(2026, 1, 31), date(2026, 2, 28)),
                   (date(2026, 1, 1), date(2027, 1, 1)), (date(2026, 3, 1), date(2026, 3, 31)),
                   (date(2026, 5, 10), date(2026, 5, 9)), (date(2024, 2, 29), date(2025, 2, 28))]:
        between.append({"start": st.isoformat(), "end": en.isoformat(),
                        "output": dh.months_between(st, en)})
    parse = [{"input": raw, "result": call(dh.parse_date_input, raw)}
             for raw in ["2026-01-15", "15/01/2026", "15-01-2026", "15/1/2026", "5/1/26", "05-01-26",
                         "", "   ", "2026/01/15", "31/02/2026", "abc", "15.01.2026"]]
    fmt = [{"input": d.isoformat(), "vn": dh.format_date_vn(d), "parts": dh.split_date_parts(d)}
           for d in [date(2026, 1, 5), date(2026, 12, 25), date(2000, 2, 29)]]
    dump("dates", {"source": "app/utils/date_helpers.py", "add_months": add,
                   "compute_end_date": end, "months_between": between,
                   "parse_date_input": parse, "format": fmt})


# ─── 4. Định dạng số và đọc số từ form ──────────────────────────────────────

def gen_formatting():
    vals = [None, 0, 1, 999, 1000, 24000000, Decimal("24000000.00"), Decimal("1234.5"),
            Decimal("1234.99"), Decimal("-1500"), "abc", Decimal("0.5"), Decimal("12.3400")]
    cur = [{"input": s(v), "output": _fmt_currency(v)} for v in vals]
    num = [{"input": s(v), "output": _fmt_number(v)} for v in vals]
    vat = [{"input": s(v), "output": _fmt_vat(v)}
           for v in [None, 0, 8, 10, Decimal("10.00"), Decimal("8.5"), Decimal("5.50"), "x"]]
    raw_inputs = ["", "  ", "1000", "1.000.000", "1,5", "82.5", "500.000", "1.234,56", "abc",
                  "12 000", "-5", "0", "007"]
    safe_dec = [{"input": r, "output": s(cs._safe_decimal(r))} for r in raw_inputs]
    safe_int = [{"input": r, "output": cs._safe_int(r)} for r in raw_inputs]
    dec_str = [{"input": s(v), "output": cs._dec_str(v)}
               for v in [None, Decimal("500000.00"), Decimal("0.50"), 12, Decimal("1E+3")]]
    fmt_money = [{"input": s(v), "output": call(pricing.fmt_money, v)}
                 for v in [0, 1234567, Decimal("1234.5"), Decimal("1234.49"), Decimal("-2.5")]]
    manual_number = []
    for raw, money in [("1.000.000", True), ("1.000.000", False), ("1.000", True), ("1.5", True),
                       ("1,5", False), ("12 345", True), ("82.5", False), ("", False), ("x", True)]:
        manual_number.append({"input": raw, "money": money,
                              "result": call(mq._number, raw, "Giá trị", money=money)})
    unit_price_text = [{"input": str(v), "output": mq._unit_price_text(v)}
                       for v in [Decimal("1000"), Decimal("1234567.50"), Decimal("0.125"), Decimal("82.5")]]
    dump("formatting", {
        "source": ["docx_renderer._fmt_currency/_fmt_number/_fmt_vat",
                   "contract_service._safe_decimal/_safe_int/_dec_str", "pricing.fmt_money",
                   "manual_quotation._number/_unit_price_text"],
        "fmt_currency": cur, "fmt_number": num, "fmt_vat": vat, "safe_decimal": safe_dec,
        "safe_int": safe_int, "dec_str": dec_str, "fmt_money": fmt_money,
        "manual_number": manual_number, "unit_price_text": unit_price_text})


# ─── 5. Tính tiền ───────────────────────────────────────────────────────────

def _line(l):
    return {k: s(getattr(l, k)) for k in ("description", "unit_label", "quantity", "period_count",
                                          "unit_price_pre_vat", "vat_rate", "amount_pre_vat",
                                          "vat_amount", "amount_total", "product_id", "note")}


def gen_pricing():
    rnd = [{"input": v, "output": call(pricing.round_money, v)}
           for v in ["0.5", "1.5", "2.5", "-0.5", "-1.5", "1.49", "1234.5000", "abc", ""]]
    norm = []
    for raw, mn, st in [("1", None, None), ("1500", 2000, 500), ("2100", 2000, 500), ("2500", 2000, 500),
                        ("3", None, 2), ("0", None, None), ("-1", None, None), ("2.5", None, None),
                        ("", None, None), ("7", 10, None)]:
        norm.append({"raw": raw, "min_quantity": mn, "step": st,
                     "result": call(pricing.normalize_quantity, raw, mn, st)})
    per = [{"months": m, "price_period_months": p, "result": call(pricing.period_count, m, p)}
           for m, p in [(12, 1), (12, 12), (6, 12), (13, 3), (0, 1), (12, 0), (12, None)]]
    line_inputs = [
        dict(description="Gói A", unit_label="cơ sở", quantity="2", months=12,
             unit_price_pre_vat="440909", vat_rate="10"),
        dict(description="Gói B", quantity="3", months=12, unit_price_pre_vat="500000", vat_rate="8"),
        dict(description="Gói C", quantity="1", months=7, unit_price_pre_vat="1000000",
             vat_rate="0", price_period_months=12),
        dict(description="Gói D", quantity="2000", months=12, unit_price_pre_vat="82.5", vat_rate="5"),
        dict(description="Ghi đè", quantity="1", months=12, unit_price_pre_vat="100",
             vat_rate="10", amount_pre_vat_override="999999", note="Giảm giá đặc biệt"),
        dict(description="Ghi đè thiếu lý do", quantity="1", months=12, unit_price_pre_vat="100",
             amount_pre_vat_override="5"),
        dict(description="Âm", quantity="1", months=12, unit_price_pre_vat="-1"),
        dict(description="VAT sai", quantity="1", months=12, unit_price_pre_vat="1", vat_rate="101"),
        dict(description="SL 0", quantity="0", months=12, unit_price_pre_vat="1"),
        dict(description="Nửa đồng", quantity="1", months=1, unit_price_pre_vat="0.5", vat_rate="10"),
        dict(description="Nửa đồng thuế", quantity="1", months=1, unit_price_pre_vat="5", vat_rate="10"),
    ]
    lines = []
    for inp in line_inputs:
        r = call(pricing.price_line, **inp)
        if "ok" in r:
            r = {"ok": _line(pricing.price_line(**inp))}
        lines.append({"input": s(inp), "result": r})
    good = [pricing.price_line(**line_inputs[i]) for i in (0, 1, 2)]
    totals = pricing.sum_lines(good)
    sums = {"lines": [_line(l) for l in good],
            "totals": {"pre_vat": s(totals.pre_vat), "vat": s(totals.vat), "total": s(totals.total),
                       "amount_text": totals.amount_text},
            "describe": pricing.describe(good, totals)}
    upv = [{"price": p, "vat": v, "output": s(pricing.unit_price_with_vat(p, v))}
           for p, v in [("440909", "10"), ("500000", "8"), ("100", "5"), ("15", "10"), ("1", "50")]]
    addon = []
    for st, en in [(date(2026, 3, 1), date(2026, 12, 31)), (date(2026, 3, 15), date(2026, 4, 14)),
                   (date(2026, 3, 15), date(2026, 4, 15)), (date(2026, 12, 31), date(2026, 12, 31)),
                   (date(2027, 1, 1), date(2026, 12, 31)), (date(2026, 1, 31), date(2026, 2, 28))]:
        addon.append({"start": st.isoformat(), "main_end": en.isoformat(),
                      "result": call(pricing.addon_months, st, en)})
    dump("pricing", {"source": "app/services/pricing.py", "round_money": rnd,
                     "normalize_quantity": norm, "period_count": per, "price_line": lines,
                     "sum_lines": sums, "unit_price_with_vat": upv, "addon_months": addon,
                     "empty_sum": call(pricing.sum_lines, [])})


# ─── 6. Vòng đời ────────────────────────────────────────────────────────────

def gen_lifecycle():
    matrix = []
    for frm in ContractStatus:
        for to in list(ContractStatus) + ["Bogus"]:
            for has_invoice in (False, True):
                for has_slip in (False, True):
                    c = SimpleNamespace(status=frm,
                                        invoice_pdf_path="x.pdf" if has_invoice else None,
                                        payment_slip_path="y.pdf" if has_slip else None)
                    tv = to.value if isinstance(to, ContractStatus) else to
                    matrix.append({"from": frm.value, "to": tv, "has_invoice": has_invoice,
                                   "has_slip": has_slip,
                                   "result": call(lifecycle.validate_transition, c, tv)})
    today = date(2026, 6, 15)
    buckets = []
    for days in [None, -10, -1, 0, 1, 6, 7, 8, 13, 14, 15, 29, 30, 31, 59, 60, 61, 365]:
        end = None if days is None else date.fromordinal(today.toordinal() + days)
        buckets.append({"days": days,
                        "end_date": end.isoformat() if end else None,
                        "today": today.isoformat(),
                        "days_to_expiry": lifecycle.days_to_expiry(end, today),
                        "bucket": lifecycle.expiry_bucket(end, today),
                        "is_expiring_soon": lifecycle.is_expiring_soon(end, today)})
    dump("lifecycle", {
        "source": "app/services/lifecycle.py",
        "sets": {
            "ACTIVE_LIFECYCLE_STATUSES": [x.value for x in lifecycle.ACTIVE_LIFECYCLE_STATUSES],
            "RENEWAL_COUNTS_STATUSES": [x.value for x in lifecycle.RENEWAL_COUNTS_STATUSES],
            "IN_SERVICE_STATUSES": [x.value for x in lifecycle.IN_SERVICE_STATUSES],
            "UNPAID_STATUSES": [x.value for x in lifecycle.UNPAID_STATUSES],
            "DEPRECATED_STATUSES": sorted(x.value for x in lifecycle.DEPRECATED_STATUSES),
        },
        "valid_transitions": {k.value: sorted(x.value for x in v)
                              for k, v in lifecycle.VALID_TRANSITIONS.items()},
        "status_labels": {k: list(v) for k, v in lifecycle.STATUS_LABELS.items()},
        "thresholds": {"EXPIRY_CRITICAL_DAYS": lifecycle.EXPIRY_CRITICAL_DAYS,
                       "EXPIRY_WARNING_DAYS": lifecycle.EXPIRY_WARNING_DAYS,
                       "EXPIRING_SOON_DAYS": lifecycle.EXPIRING_SOON_DAYS,
                       "EXPIRING_WARN_DAYS": lifecycle.EXPIRING_WARN_DAYS},
        "transition_matrix": matrix, "expiry": buckets})


# ─── 7. Đánh số ─────────────────────────────────────────────────────────────

def gen_numbering():
    fmts = ["{seq}/{slug}/{year}", "{seq}/ABC-{slug}/XYZ/{year}", "{slug}/{year}", "{seq}/{foo}",
            "", "   ", "HD{seq}", "{seq}-{seq}/{year}"]
    validate = [{"input": f, "result": call(numbering.validate_format, f)} for f in fmts]
    build = []
    for f in [None, "{seq}/{slug}/{year}", "{seq}/ABC-{slug}/XYZ/{year}", "HD{seq}", "{slug}/{year}"]:
        build.append({"seq": "01", "slug": "TYTSONGXANH", "format": f, "year": 2026,
                      "result": call(numbering.build_number, "01", "TYTSONGXANH", f, 2026)})
        build.append({"format": f, "year": 2026, "like": call(numbering.like_pattern, f, 2026)})
    next_seq = [{"used": sorted(u), "output": numbering.next_seq(u)}
                for u in [set(), {1}, {2}, {1, 2, 3}, {1, 3}, {1, 2, 4, 5}, set(range(1, 100))]]
    seq_of = [{"input": n, "output": numbering.seq_of(n)}
              for n in ["01/ABC/2026", "12/X", " 7 /Y", "ABC/01", "", "PL01/02/X", "100"]]
    infer = []
    for num, slug, year in [("01/ABC-TYTSONGXANH/XYZ/2026", "TYTSONGXANH", 2026),
                            ("03/TYTSONGXANH/2026", "TYTSONGXANH", 2026),
                            ("01/TYT2026/2026", "TYT2026", 2026),
                            ("AB/TYT/2026", "TYT", 2026), ("01/OTHER/2026", "TYT", 2026),
                            ("01/TYT/2025", "TYT", 2026), ("", "TYT", 2026), ("01/TYT/2026", "", 2026)]:
        infer.append({"number": num, "slug": slug, "year": year,
                      "output": numbering.infer_format(num, slug, year)})
    add_fmt = []
    for f in ["PL{seq}/{parent}", "{parent}/PL{seq}", "PL{seq}", "{seq}{parent}{x}", ""]:
        add_fmt.append({"format": f, "validate": call(numbering.validate_addendum_format, f),
                        "build": call(numbering.build_addendum_number, "01/ABC/2026", "02", f),
                        "parse": [{"number": n, "output": call(numbering.parse_addendum_seq, n, f or None)}
                                  for n in ["PL02/01/ABC/2026", "01/ABC/2026/PL03", "PLxx/01", "PL7"]]})
    dump("numbering", {"source": "app/services/numbering.py",
                       "DEFAULT_FORMAT": numbering.DEFAULT_FORMAT,
                       "DEFAULT_ADDENDUM_FORMAT": numbering.DEFAULT_ADDENDUM_FORMAT,
                       "validate_format": validate, "build_and_like": build, "next_seq": next_seq,
                       "seq_of": seq_of, "infer_format": infer, "addendum": add_fmt})


# ─── 8. Vai trò mẫu ─────────────────────────────────────────────────────────

def gen_template_rules():
    types = ts.CONTRACT_TYPES + [ts.LEGACY_APPENDIX_TYPE, "Tùy ý", ""]
    roles = [ts.ROLE_CONTRACT, ts.ROLE_ADDON, ts.ROLE_RENEWAL, ts.ROLE_ATTACHMENT,
             ts.ROLE_QUOTATION, ts.ROLE_QUOTE_REQUEST]
    has_role = [{"contract_type": t, "role": r, "output": ts.has_role(t, r)} for t in types for r in roles]
    check = []
    for t, tg, g, r in [("Hợp đồng", None, "G1", "contract"), ("Hợp đồng", "G1", "G1", "contract"),
                        ("Hợp đồng", "G1", "G2", "contract"), ("Hợp đồng", "G1", None, "contract"),
                        ("Nghiệm thu", None, None, "contract"), ("Nghiệm thu", "G1", "G1", "attachment"),
                        ("Phụ lục", None, "G1", "addon"), ("Phụ lục", None, "G1", "renewal"),
                        ("Báo giá", " G1 ", "G1", "quotation")]:
        tpl = SimpleNamespace(name="Mẫu thử", contract_type=t, product_group=tg)
        check.append({"contract_type": t, "template_group": tg, "group": g, "role": r,
                      "result": call(ts.check_template_for, tpl, g, r)})
    kind = []
    for phs, heading in [([], ""), (["QUOTE_DATE_DAY"], ""), (["REQUEST_DATE"], ""),
                         (["ACCEPTANCE_DATE"], ""), (["LIQUIDATION_DATE_DAY"], ""),
                         (["PARENT_CONTRACT_NUMBER"], ""),
                         (["PARENT_CONTRACT_NUMBER", "START_DATE", "END_DATE"], ""),
                         ([], "BIÊN BẢN BÀN GIAO VÀ NGHIỆM THU"), ([], "BIÊN BẢN NGHIỆM THU"),
                         (["QUOTE_DATE"], "BIÊN BẢN THANH LÝ")]:
        kind.append({"placeholders": phs, "heading": heading, "output": ts._template_kind(phs, heading)})
    pre = [{"contract_type": t, "product_group": g,
            "output": ts.preselected_attachment(SimpleNamespace(contract_type=t, product_group=g))}
           for t in ["Bàn giao", "Nghiệm thu", "Thanh lý", "Khác"] for g in [None, "", "G1"]]
    dump("template_rules", {"source": "app/services/template_service.py",
                            "CONTRACT_TYPES": ts.CONTRACT_TYPES, "has_role": has_role,
                            "check_template_for": check, "template_kind": kind,
                            "preselected_attachment": pre,
                            "role_of_doc_kind": {k: ts.role_of_doc_kind(k) for k in
                                                 ["contract", "addendum_addon", "addendum_renewal", "x"]}})


# ─── 9. Ngữ cảnh render ─────────────────────────────────────────────────────

def fake_customer(**kw):
    unit = lambda name, active=True, bill=True: SimpleNamespace(  # noqa: E731
        name=name, is_active=active, include_in_billing=bill, unit_type="Điểm trạm")
    base = dict(legal_name="Trạm Y tế xã Sông Xanh", short_name="TYT Sông Xanh", code="KH-001",
                address="Ấp 1", ward="xã Sông Xanh", province="Tỉnh Mẫu",
                representative_name="Nguyễn Văn A", representative_gender="Nam",
                representative_title="Trạm trưởng", tax_code="0000000001", bank_account="123456789",
                bank_name="Ngân hàng Mẫu", beneficiary_name="Trạm Y tế xã Sông Xanh",
                budget_relation_code="1000001", phone="0900000000", email="tyt@example.org",
                contact_name="Trần Thị B", establishment_decision="12/QĐ-UBND",
                establishment_date=date(2010, 5, 20), bill_main_station=True, record_count=None,
                sub_units=[unit("Điểm trạm Ấp 2"), unit("Điểm trạm Ấp 3", bill=False),
                           unit("Điểm trạm cũ", active=False)])
    base.update(kw)
    c = SimpleNamespace(**base)
    from app.models.customer import Customer
    c.full_address = lambda: Customer.full_address(c)
    c.unit_names_list = lambda: Customer.unit_names_list(c)
    c.total_units = Customer.total_units.fget(c)
    return c


def fake_contract(**kw):
    base = dict(contract_number="01/TYTSONGXANH/2026", doc_kind="contract", parent=None,
                product=SimpleNamespace(name="Dịch vụ Mẫu (DVM)", code="DVM"),
                party_b_representative=None, party_b_authorization_number="05/UQ",
                party_b_authorization_date=None, total_pre_vat=Decimal("10581816"),
                total_vat=Decimal("1058182"), payment_cycle_months=None, sign_location=None,
                sign_date=date(2026, 1, 5), start_date=date(2026, 1, 5), end_date=date(2027, 1, 4),
                month_count=12, unit_count=2, quantity=Decimal("24"),
                unit_price_pre_vat=Decimal("440909"), vat_rate=Decimal("10"),
                unit_price_vat=Decimal("485000"), total_amount=Decimal("11639998"),
                amount_text=None, pricing_description="Mô tả cách tính",
                acceptance_date=date(2026, 1, 7), liquidation_date=None, contract_id=1)
    base.update(kw)
    return SimpleNamespace(**base)


def gen_render_context():
    cases = []
    variants = {
        "basic": (fake_contract(), fake_customer()),
        "female_rep_no_ward": (fake_contract(amount_text="Chữ nhập tay", sign_location="  "),
                               fake_customer(representative_gender="Nữ", ward=None)),
        "empty_numbers": (fake_contract(total_amount=None, total_pre_vat=None, total_vat=None,
                                        quantity=None, unit_count=None, vat_rate=None,
                                        unit_price_vat=None, unit_price_pre_vat=None,
                                        sign_date=None, acceptance_date=None, month_count=None,
                                        liquidation_date=date(2027, 1, 4), product=None),
                          fake_customer(representative_gender=None, bill_main_station=False)),
        "addendum": (fake_contract(contract_number="PL02/01/TYTSONGXANH/2026", doc_kind="addendum_addon",
                                   parent=SimpleNamespace(contract_number="01/TYTSONGXANH/2026",
                                                          sign_date=date(2026, 1, 5))),
                     fake_customer()),
    }
    for name, (contract, customer) in variants.items():
        ctx = build_render_context(contract, customer, db=None)
        ctx08 = build_08a_context(None, contract, customer, contract.product)
        cases.append({"name": name, "contract": s(vars_no_fn(contract)),
                      "customer": s(vars_no_fn(customer)), "context": ctx, "context_08a": ctx08})
    # Ánh xạ placeholder tùy biến của mẫu
    tpl = SimpleNamespace(field_mapping={"SO_HD": "contract.number", "TEN_KH": "party_a.name",
                                         "LA": "khong.co", "CONTRACT_NUMBER": "contract.number"})
    mapped = apply_template_mapping({"CONTRACT_NUMBER": "01/X/2026", "PARTY_A_NAME": "A"}, tpl)
    # Dòng lặp
    lines = [pricing.price_line(description="Gói A", unit_label="cơ sở", quantity="2", months=12,
                                unit_price_pre_vat="440909", vat_rate="10"),
             pricing.price_line(description="Gói B", unit_label="", quantity="1.5", months=12,
                                unit_price_pre_vat="1000", vat_rate="8.5", price_period_months=12)]
    feats = [[{"no": 1, "name": "Quản lý hồ sơ", "description": "Mô tả 1"},
              {"name": "  "}, {"name": "Báo cáo", "description": None}],
             [{"name": "Báo cáo", "description": "trùng tên — bỏ"}, {"name": "Đồng bộ"}]]
    lists = lists_from_lines(lines, feats)
    dump("render_context", {
        "source": ["docx_renderer.build_render_context", "form08a_service.build_08a_context",
                   "docx_renderer.apply_template_mapping", "docx_renderer.lists_from_lines"],
        "note": "Bên B lấy từ AppSetting; fixture dùng db=None nên mọi khóa PARTY_B_* lấy giá trị "
                "mặc định rỗng, trừ khi hợp đồng có giá trị riêng.",
        "cases": cases, "canonical_map": CANONICAL_MAP, "row_placeholders": ROW_PLACEHOLDERS,
        "template_mapping": {"input": {"CONTRACT_NUMBER": "01/X/2026", "PARTY_A_NAME": "A"},
                             "field_mapping": tpl.field_mapping, "output": mapped},
        "lists_from_lines": {"lines": [_line(l) for l in lines], "feature_lists": feats,
                             "output": lists},
        "work_content": [{"product_name": n, "output": _work_content(None, SimpleNamespace(name=n))}
                         for n in ["Phần mềm Quản lý Kho (WMS)", "Dịch vụ (A) (B)", "", "  Tên  "]]})


def vars_no_fn(obj):
    out = {}
    for k, v in vars(obj).items():
        if callable(v):
            continue
        if isinstance(v, SimpleNamespace):
            out[k] = vars_no_fn(v)
        elif isinstance(v, list) and v and isinstance(v[0], SimpleNamespace):
            out[k] = [vars_no_fn(x) for x in v]
        else:
            out[k] = v
    return out


# ─── 10. DOCX ───────────────────────────────────────────────────────────────

W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"


def docx_texts(data: bytes) -> dict:
    """Văn bản theo đoạn của MỌI phần chứa chữ: document, header*, footer*.

    Quy ước so sánh mà test Rust cũng phải dùng: với mỗi phần XML, lấy mọi <w:p>
    theo thứ tự tài liệu (kể cả trong bảng lồng, textbox), nối text các <w:t>
    trong đoạn đó. Một đoạn → một chuỗi.
    """
    out = {}
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for name in sorted(z.namelist()):
            if not (name == "word/document.xml" or name.startswith("word/header")
                    or name.startswith("word/footer")) or not name.endswith(".xml"):
                continue
            root = etree.fromstring(z.read(name))
            paras = []
            tags = {f"{{{W}}}t": None, f"{{{W}}}br": "\n", f"{{{W}}}cr": "\n", f"{{{W}}}tab": "\t"}
            for p in root.iter(f"{{{W}}}p"):
                # chỉ lấy nút thuộc chính đoạn này, không thuộc đoạn lồng bên trong.
                # w:t → chữ; w:br, w:cr → "\n"; w:tab → "\t" (w:tab trong w:tabs của pPr bị bỏ).
                texts = []
                for node in p.iter(*tags.keys()):
                    owner = node.getparent()
                    if node.tag == f"{{{W}}}tab" and owner.tag != f"{{{W}}}r":
                        continue
                    while owner is not None and owner.tag != f"{{{W}}}p":
                        owner = owner.getparent()
                    if owner is p:
                        texts.append(node.text or "" if tags[node.tag] is None else tags[node.tag])
                paras.append("".join(texts))
            out[name] = paras
    return out


def table_shape(data: bytes) -> list:
    """Số dòng của từng bảng cấp 1 trong thân tài liệu — để kiểm nhân dòng / bỏ bảng."""
    d = docx.Document(io.BytesIO(data))
    return [len(t.rows) for t in d.tables]


def make_synthetic_docx(path: Path) -> None:
    """Mẫu tổng hợp đủ các bẫy: split-run, bảng Bên A, dòng lặp, header, bảng lồng."""
    d = docx.Document()
    d.add_paragraph("Số hợp đồng: {{CONTRACT_NUMBER}}")
    p = d.add_paragraph("Bên A: ")
    for chunk in ["{{", "PARTY_A", "_NAME", "}}", " — ký ngày {{SIGN_DATE}}"]:
        r = p.add_run(chunk)
        r.bold = chunk == "PARTY_A"
    d.add_paragraph("Khóa lạ giữ nguyên: {{KHONG_CO_KHOA}} và {{lower_case}} và {{ CONTRACT_NUMBER }}")
    d.add_paragraph("Hai khóa liền nhau: {{MONTH_COUNT}}{{MONTH_COUNT}} tháng")
    d.add_paragraph("Nhiều dòng: {{PRICING_DESCRIPTION}}")
    d.add_paragraph("Ký tự đặc biệt: {{NOTES_XML}}")

    # Bảng Bên A: ô đầu chứa PARTY_A_NAME, có dòng Đại diện và Chức vụ, 3 cột.
    t = d.add_table(rows=3, cols=3)
    rows = [("{{PARTY_A_NAME}}", "", ""), ("Đại diện", ":", "{{PARTY_A_REPRESENTATIVE}}"),
            ("Chức vụ", ":", "{{PARTY_A_TITLE}}")]
    for i, vals in enumerate(rows):
        for j, v in enumerate(vals):
            t.cell(i, j).text = v

    # Bảng dòng lặp hỗn hợp: tiêu đề + dòng ITEM + dòng tổng có khóa thường.
    t2 = d.add_table(rows=3, cols=4)
    for j, v in enumerate(["STT", "Tên", "SL", "Thành tiền"]):
        t2.cell(0, j).text = v
    for j, v in enumerate(["{{ITEM_NO}}", "{{ITEM_NAME}}", "{{ITEM_QTY}}", "{{ITEM_AMOUNT_TOTAL}}"]):
        t2.cell(1, j).text = v
    t2.cell(2, 0).text = "Tổng"
    t2.cell(2, 3).text = "{{TOTAL_AMOUNT}}"

    # Bảng chỉ có chức năng (bảng "thuần" features).
    t3 = d.add_table(rows=1, cols=2)
    t3.cell(0, 0).text = "{{FEATURE_NO}}"
    t3.cell(0, 1).text = "{{FEATURE_NAME}}"

    # Bảng lồng: python-docx KHÔNG duyệt bảng lồng.
    outer = d.add_table(rows=1, cols=1)
    inner = outer.cell(0, 0).add_table(rows=1, cols=1)
    inner.cell(0, 0).text = "Lồng: {{CONTRACT_NUMBER}}"

    sec = d.sections[0]
    sec.header.paragraphs[0].text = "Đầu trang {{CONTRACT_NUMBER}}"
    sec.footer.paragraphs[0].text = "Chân trang {{PARTY_A_NAME}}"
    d.save(str(path))


def gen_docx():
    DOCX_OUT.mkdir(parents=True, exist_ok=True)
    synth = DOCX_OUT / "synthetic_all_traps.docx"
    make_synthetic_docx(synth)
    src_bytes = synth.read_bytes()

    ctx = {"CONTRACT_NUMBER": "01/TYTSONGXANH/2026", "PARTY_A_NAME": "Trạm Y tế xã Sông Xanh",
           "SIGN_DATE": "05/01/2026", "MONTH_COUNT": "12", "PARTY_A_REPRESENTATIVE": "(Ông) Nguyễn Văn A",
           "PARTY_A_TITLE": "Trạm trưởng", "TOTAL_AMOUNT": "11.639.998",
           "PARTY_A_TAX_CODE": "0000000001", "PARTY_A_BANK_ACCOUNT": "", "PARTY_A_BANK_NAME": "Ngân hàng Mẫu",
           "PARTY_A_BENEFICIARY": "  ", "PARTY_A_BUDGET_CODE": "1000001", "NONE_VALUE": None,
           "PRICING_DESCRIPTION": "Dòng 1\nDòng 2\tsau tab", "NOTES_XML": "A & B < C > D \"E\""}
    lists_full = {"items": [{"ITEM_NO": "1", "ITEM_NAME": "Gói A", "ITEM_QTY": "24", "ITEM_AMOUNT_TOTAL": "1.000"},
                            {"ITEM_NO": "2", "ITEM_NAME": "Gói B", "ITEM_QTY": "12", "ITEM_AMOUNT_TOTAL": "2.000"}],
                  "features": [{"FEATURE_NO": "1", "FEATURE_NAME": "Quản lý"}]}
    lists_empty = {"items": [], "features": []}
    scenarios = [
        ("lists_none", dict(lists=None)),
        ("lists_full", dict(lists=lists_full)),
        ("lists_empty", dict(lists=lists_empty)),
        ("lists_empty_omit_features", dict(lists=lists_empty, omit_empty_tables={"features"})),
        ("no_party_a_optional", dict(lists=lists_full, ctx_override={
            "PARTY_A_TAX_CODE": "", "PARTY_A_BANK_NAME": "", "PARTY_A_BUDGET_CODE": ""})),
    ]
    results = []
    for name, opt in scenarios:
        c = dict(ctx)
        c.update(opt.pop("ctx_override", {}))
        out = render_docx(src_bytes, c, **opt)
        (DOCX_OUT / f"expected_{name}.docx").write_bytes(out)
        results.append({"name": name, "context": c, "lists": s(opt.get("lists")),
                        "omit_empty_tables": sorted(opt.get("omit_empty_tables", ())),
                        "expected_texts": docx_texts(out), "top_level_table_rows": table_shape(out)})

    # Mẫu mẫu (sample_*.docx) có trong repo — dữ liệu hư cấu.
    samples = []
    assets = APP / "assets" / "default_templates"
    contract, customer = fake_contract(), fake_customer()
    lines = [pricing.price_line(description="Gói A", unit_label="cơ sở", quantity="2", months=12,
                                unit_price_pre_vat="440909", vat_rate="10")]
    lists = lists_from_lines(lines, [[{"name": "Quản lý hồ sơ", "description": "Mô tả"}]])
    for fname, ctx_s, lists_s in [
        ("sample_service_contract.docx", build_render_context(contract, customer, db=None), lists),
        ("sample_form_08a.docx", build_08a_context(None, contract, customer, contract.product), None),
    ]:
        path = assets / fname
        if not path.exists():
            continue
        out = render_docx(str(path), ctx_s, lists=lists_s)
        samples.append({"template": f"ContractForge/assets/default_templates/{fname}",
                        "context": ctx_s, "lists": lists_s, "expected_texts": docx_texts(out),
                        "top_level_table_rows": table_shape(out)})

    # Báo giá tự nhập (sample_quote_*.docx nằm cạnh mã nguồn).
    values = {"layout": "features", "issuer_name": "Công ty Mẫu", "issuer_address": "Số 1 Đường Mẫu",
              "recipient_name": "Trạm Y tế xã Sông Xanh", "quote_date": "05/01/2026",
              "valid_until": "2026-02-05", "quote_number": "BG-01",
              "items_json": json.dumps([{"name": "Gói A", "unit": "gói", "quantity": "2", "periods": "12",
                                         "price": "1.000.000", "vat": "10"},
                                        {"name": "Gói B", "unit": "", "quantity": "1,5", "periods": "1",
                                         "price": "82,5", "vat": "8"}])}
    manual = []
    try:
        mctx, mlists = mq.build_document(dict(values), [{"name": "Chức năng 1", "description": "Mô tả"}])
        path = mq.template_path("features")
        if path.exists():
            out = render_docx(str(path), mctx, lists=mlists, omit_empty_tables={"features"})
            manual.append({"values": values, "context": mctx, "lists": mlists,
                           "expected_texts": docx_texts(out)})
    except Exception as exc:  # noqa: BLE001
        manual.append({"values": values, "error": f"{type(exc).__name__}: {exc}"})

    # Phân tích mẫu
    analysis = []
    for path in [synth] + sorted(assets.glob("sample_*.docx")):
        r = analyze(str(path))
        analysis.append({"file": path.name, "placeholders": r.placeholders, "mapping": r.mapping,
                         "split_run_warnings": r.split_run_warnings, "source_counts": r.source_counts,
                         "repeat_groups": r.repeat_groups, "row_warnings": r.row_warnings, "error": r.error})

    dump("docx_render", {
        "source": "app/services/docx_renderer.py render_docx",
        "text_extraction_rule": docx_texts.__doc__.strip(),
        "synthetic_template": "contract_rust/fixtures/docx/synthetic_all_traps.docx",
        "scenarios": results, "samples": samples, "manual_quotation": manual})
    gen_intentional_differences(results)
    dump("template_analysis", {"source": "app/services/template_analyzer.py analyze",
                               "results": analysis})


def gen_schema():
    """DDL của một DB CÀI MỚI do chính bản Python tạo (trong thư mục tạm), và
    danh sách cột bắt buộc mà bản Rust kiểm khi mở DB có sẵn."""
    import sqlite3
    from app.database import SessionLocal, init_db, engine
    from app.models.seed import run_migrations, run_all_seeds
    init_db()
    db = SessionLocal()
    try:
        run_migrations(db)
        run_all_seeds(db)
    finally:
        db.close()
    path = engine.url.database
    con = sqlite3.connect(path)
    rows = con.execute("SELECT type, name, sql FROM sqlite_master WHERE sql IS NOT NULL "
                       "AND name NOT LIKE 'sqlite_%' ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, name").fetchall()
    ddl = ";\n\n".join(r[2].strip() for r in rows) + ";\n"
    (OUT / "schema_baseline.sql").write_text(
        "-- DDL của DB cài mới do bản Python tạo (init_db + run_migrations). Sinh bởi tools/gen_fixtures.py.\n"
        "-- Bản Rust chạy nguyên văn file này khi tạo DB mới.\n\n" + ddl, encoding="utf-8")
    cols = {}
    for (t,) in con.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'"):
        cols[t] = [r[1] for r in con.execute(f"PRAGMA table_info({t})")]
    seed = [dict(zip(("code", "name", "product_type", "default_price", "default_vat_rate",
                      "default_duration_months", "kind", "price_period_months", "quantity_source",
                      "is_active", "sort_order"), r))
            for r in con.execute("SELECT code, name, product_type, default_price, default_vat_rate, "
                                 "default_duration_months, kind, price_period_months, quantity_source, "
                                 "is_active, sort_order FROM products ORDER BY id")]
    con.close()
    dump("schema", {"source": "app/models/*.py + seed.run_migrations + seed.run_all_seeds",
                    "required_columns": cols, "seed_products": seed})
    print(f"  wrote {(OUT / 'schema_baseline.sql').relative_to(ROOT)}")


# ─── 11. Khác biệt có chủ ý (KB) — sinh bằng code, không gõ tay ─────────────
# Mỗi KB: "python_result" = gọi code Python hiện hành; "rust_expected" = bản cài
# THAM CHIẾU của hành vi mới viết ngay dưới đây. Test Rust so với rust_expected.

def ref_amount_to_words(n: int, suffix: str = "đồng") -> str:
    """KB-01: như amount_to_words nhưng phần tỷ ≥ 1000 đọc đệ quy."""
    if n == 0:
        return f"Không {suffix}"

    def groups(x: int) -> list[str]:
        ty, trieu = x // 1_000_000_000, (x % 1_000_000_000) // 1_000_000
        nghin, dv = (x % 1_000_000) // 1_000, x % 1_000
        parts = []
        if ty:
            parts.append(", ".join(groups(ty)) + " tỷ" if ty >= 1000 else f"{atw._say_three_digits(ty)} tỷ")
        if trieu:
            parts.append(f"{atw._say_three_digits(trieu)} triệu")
        if nghin:
            parts.append(f"{atw._say_three_digits(nghin)} nghìn")
        if dv:
            parts.append(atw._say_three_digits(dv))
        return parts

    result = ", ".join(groups(n)) + (f" {suffix}" if suffix else "")
    return result[0].upper() + result[1:]


def ref_format_regex(fmt: str, year: int):
    """KB-02: khuôn → regex khớp nguyên chuỗi."""
    import re
    out, i = "", 0
    for m in re.finditer(r"\{(seq|slug|year)\}", fmt):
        out += re.escape(fmt[i:m.start()])
        out += {"seq": r"(?P<seq>\d+)", "slug": r"[A-Za-z0-9]*", "year": str(year)}[m.group(1)]
        i = m.end()
    out += re.escape(fmt[i:])
    return re.compile(out)


def ref_next_number(fmt: str, slug: str, year: int, existing: list[str]) -> str:
    rx = ref_format_regex(fmt, year)
    used = {int(m.group("seq")) for n in existing if (m := rx.fullmatch(n))}
    return numbering.build_number(numbering.next_seq(used), slug, fmt, year)


def ref_windows_safe(name: str) -> str:
    import re
    return re.sub(r'[\\/:*?"<>|]', "-", name)


def python_next_number(fmt, slug, year, existing):
    """Gọi đúng code Python (LIKE + seq_of) trên một DB tạm."""
    from app.database import SessionLocal
    from app.models.customer import Customer
    from app.models.contract import Contract
    db = SessionLocal()
    try:
        cust = Customer(code=f"KB-{abs(hash((fmt, tuple(existing)))) % 10**8}", legal_name="Khách KB",
                        customer_type="Khác")
        db.add(cust)
        db.flush()
        for num in existing:
            db.add(Contract(contract_number=num, customer_id=cust.id))
        db.flush()
        seq = cs.get_next_contract_seq(db, cust.id, number_format=fmt, year=year)
        return numbering.build_number(seq, slug, fmt, year)
    finally:
        db.rollback()
        db.close()


def gen_intentional_differences(docx_scenarios):
    kb01 = []
    for n in [1_000_000_000_000, 1_000_000_000_001, 2_500_000_000_000, 999_999_999_999_999]:
        kb01.append({"input": str(n), "python_result": call(atw.amount_to_words, n),
                     "rust_expected": {"words": ref_amount_to_words(n),
                                       "chan": ref_amount_to_words(n) + " chẵn"}})
    # Kiểm chéo: với số < 1000 tỷ, bản tham chiếu phải trùng Python.
    for n in [0, 5, 15, 21, 105, 1_000_005, 22_856_736, 999_999_999_999]:
        assert ref_amount_to_words(n) == atw.amount_to_words(n), n
    kb02 = []
    for case, fmt, slug, existing in [
        ("seq không đứng đầu", "HD{seq}", "TYTSONGXANH", ["HD01", "HD02"]),
        ("khuôn khác không đếm chung", "{seq}/{slug}/{year}", "TYTSONGXANH", ["01/ABC-TYTSONGXANH/XYZ/2026"]),
        ("cùng khuôn vẫn lấp khoảng trống", "{seq}/{slug}/{year}", "TYTSONGXANH",
         ["01/TYTSONGXANH/2026", "03/TYTSONGXANH/2026", "02/TYTSONGXANH/2025"]),
        ("slug cũ (khách đổi tên) vẫn được đếm", "{seq}/{slug}/{year}", "TYTMOI", ["01/TYTCU/2026"]),
        ("khuôn có tiền tố cố định", "{seq}/ABC-{slug}/XYZ/{year}", "TYTSONGXANH",
         ["01/ABC-TYTSONGXANH/XYZ/2026", "01/TYTSONGXANH/2026"]),
    ]:
        kb02.append({"case": case, "format": fmt, "year": 2026, "slug": slug,
                     "existing_numbers_of_customer": existing,
                     "python_result": python_next_number(fmt, slug, 2026, existing),
                     "rust_expected": ref_next_number(fmt, slug, 2026, existing)})
    kb05 = []
    for sc in docx_scenarios:
        rows = sc["top_level_table_rows"]
        if 0 in rows:
            kb05.append({"scenario": sc["name"], "python_top_level_table_rows": rows,
                         "rust_expected": [r for r in rows if r != 0]})
    kb19 = []
    for layout, qd in [("features", "05/01/2026"), ("compact", "2026-01-05")]:
        py = f"BaoGia_{layout}_{qd}.docx"  # đúng biểu thức trong routers/manual_quotation.py
        kb19.append({"layout": layout, "quote_date": qd, "python_result": py,
                     "rust_expected": ref_windows_safe(py)})
    dump("intentional_differences", {
        "note": "Những chỗ bản Rust CỐ Ý ra kết quả khác fixture Python. SINH BỞI tools/gen_fixtures.py "
                "(python_result = code Python thật; rust_expected = bản cài tham chiếu trong script). "
                "Test Rust dùng 'rust_expected'. Mô tả: docs/06_KHAC_BIET_CO_CHU_Y.md",
        "KB-01_amount_to_words": kb01,
        "KB-02_numbering_next_seq": kb02,
        "KB-03_docx_nested_and_all_parts": {
            "applies_to": "docx_render.json → scenarios[*].expected_texts['word/document.xml']",
            "replace_paragraph": {"from": "Lồng: {{CONTRACT_NUMBER}}", "to": "Lồng: 01/TYTSONGXANH/2026"}},
        "KB-05_docx_empty_table_removed": kb05,
        "KB-19_manual_quotation_filename": kb19,
    })


def main():
    print("Sinh fixture vàng vào", OUT.relative_to(ROOT))
    gen_schema()
    gen_amount_to_words()
    gen_names()
    gen_dates()
    gen_formatting()
    gen_pricing()
    gen_lifecycle()
    gen_numbering()
    gen_template_rules()
    gen_render_context()
    gen_docx()
    print("Xong.")


if __name__ == "__main__":
    main()
