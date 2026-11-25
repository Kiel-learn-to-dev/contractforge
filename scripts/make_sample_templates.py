#!/usr/bin/env python3
"""Generate the neutral sample DOCX templates shipped with the public repo.

The repository deliberately ignores ``ContractForge/assets/default_templates/*.docx``
because a real installation's templates carry that organization's letterhead and
wording. But one of them is not user content: the acceptance form (08A) is
rendered on demand by ``form08a_service`` and the feature simply breaks on a
fresh clone if no file is present.

This script builds a fictional, structurally-equivalent stand-in and writes it as
``sample_form_08a.docx``, which .gitignore explicitly un-ignores. Run it whenever
the placeholder set changes:

    python scripts/make_sample_templates.py

It is a generator, not a runtime dependency — the app only reads the .docx.
"""

from __future__ import annotations

import sys
from pathlib import Path

from docx import Document
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.shared import Cm, Pt
from docx.oxml import OxmlElement
from docx.oxml.ns import qn

OUT = (Path(__file__).resolve().parent.parent
       / "ContractForge" / "assets" / "default_templates" / "sample_form_08a.docx")
OUT_SERVICE = (Path(__file__).resolve().parent.parent
               / "ContractForge" / "assets" / "default_templates" / "sample_service_contract.docx")


def _para(doc, text="", *, bold=False, italic=False, align=None, size=11):
    p = doc.add_paragraph()
    if align is not None:
        p.alignment = align
    run = p.add_run(text)
    run.bold = bold
    run.italic = italic
    run.font.size = Pt(size)
    return p


def build_form_08a() -> Document:
    doc = Document()
    style = doc.styles["Normal"]
    style.font.name = "Times New Roman"
    style.font.size = Pt(12)

    # ── Header block ────────────────────────────────────────────────────────
    head = doc.add_table(rows=1, cols=3)
    head.alignment = WD_TABLE_ALIGNMENT.CENTER
    head.rows[0].cells[0].text = "{{BUDGET_UNIT_NAME}}"
    head.rows[0].cells[1].text = ""
    head.rows[0].cells[2].text = "Mẫu số 08a"

    _para(doc)
    _para(doc, "BẢNG XÁC ĐỊNH GIÁ TRỊ KHỐI LƯỢNG CÔNG VIỆC HOÀN THÀNH",
          bold=True, align=WD_ALIGN_PARAGRAPH.CENTER, size=13)
    _para(doc, "THEO HỢP ĐỒNG", bold=True, align=WD_ALIGN_PARAGRAPH.CENTER, size=13)
    _para(doc)

    _para(doc, "Đơn vị sử dụng ngân sách: {{BUDGET_UNIT_NAME}}")
    _para(doc, "Mã đơn vị quan hệ ngân sách: {{BUDGET_UNIT_CODE}}")
    _para(doc, "Căn cứ hợp đồng số: {{CONTRACT_NUMBER}} ngày {{SIGN_DATE}}")
    _para(doc, "Bên A: {{PARTY_A_NAME}}")
    _para(doc)

    # ── Work table ──────────────────────────────────────────────────────────
    table = doc.add_table(rows=4, cols=6)
    table.style = "Table Grid"
    headers = ["STT", "Nội dung công việc", "Đơn vị tính",
               "Số lượng", "Đơn giá", "Thành tiền"]
    for cell, text in zip(table.rows[0].cells, headers):
        cell.text = text
        for p in cell.paragraphs:
            p.alignment = WD_ALIGN_PARAGRAPH.CENTER
            for run in p.runs:
                run.bold = True

    row = table.rows[1].cells
    row[0].text = "1"
    row[1].text = "{{WORK_CONTENT}}"
    row[2].text = "Tháng"
    row[3].text = "{{QUANTITY}}"
    row[4].text = "{{UNIT_PRICE_VAT}}"
    row[5].text = "{{TOTAL_AMOUNT}}"

    total = table.rows[2].cells
    total[0].text = ""
    total[1].text = "Tổng cộng"
    total[5].text = "{{TOTAL_AMOUNT}}"

    note = table.rows[3].cells
    note[1].text = "(Giá trên đã bao gồm thuế giá trị gia tăng)"

    _para(doc)
    _para(doc,
          "Ngày {{ACCEPTANCE_DATE_DAY}} tháng {{ACCEPTANCE_DATE_MONTH}} "
          "năm {{ACCEPTANCE_DATE_YEAR}}",
          italic=True, align=WD_ALIGN_PARAGRAPH.RIGHT)
    _para(doc)

    # ── Signature block ─────────────────────────────────────────────────────
    sign = doc.add_table(rows=2, cols=2)
    sign.alignment = WD_TABLE_ALIGNMENT.CENTER
    sign.rows[0].cells[0].text = "{{PARTY_A_TITLE_UPPER}}"
    sign.rows[0].cells[1].text = "{{PARTY_B_TITLE_UPPER}}"
    sign.rows[1].cells[0].text = "{{PARTY_A_REPRESENTATIVE}}"
    sign.rows[1].cells[1].text = "{{PARTY_B_REPRESENTATIVE}}"
    for row_ in sign.rows:
        for cell in row_.cells:
            for p in cell.paragraphs:
                p.alignment = WD_ALIGN_PARAGRAPH.CENTER

    return doc

def build_service_contract() -> Document:
    """Hợp đồng dịch vụ hư cấu: bảng giá trị và bảng chức năng lặp theo dòng.

    Câu chữ được viết mới, không chép từ mẫu của tổ chức nào.
    """
    doc = Document()
    style = doc.styles["Normal"]
    style.font.name = "Times New Roman"
    style.font.size = Pt(12)

    _para(doc, "HỢP ĐỒNG DỊCH VỤ MẪU", bold=True, align=WD_ALIGN_PARAGRAPH.CENTER, size=13)
    _para(doc, "Số: {{CONTRACT_NUMBER}}", align=WD_ALIGN_PARAGRAPH.CENTER)
    _para(doc)
    _para(doc, "Bên A (Khách hàng): {{PARTY_A_NAME}}")
    _para(doc, "Địa chỉ: {{PARTY_A_ADDRESS}}")
    _para(doc, "Điện thoại: {{PARTY_A_PHONE}}    Người liên hệ: {{PARTY_A_CONTACT_NAME}}")
    _para(doc, "Bên B (Nhà cung cấp): {{PARTY_B_NAME}}")
    _para(doc, "Địa chỉ: {{PARTY_B_ADDRESS}}")
    _para(doc, "Đại diện: {{PARTY_B_REPRESENTATIVE}}    Chức vụ: {{PARTY_B_TITLE}}")
    _para(doc)
    _para(doc, "1. Giá trị hợp đồng", bold=True)

    value_head = ["TT", "Dịch vụ", "ĐVT", "Số lượng", "Đơn giá",
                  "Thành tiền trước thuế", "Thuế suất", "Tiền thuế GTGT", "Thành tiền sau thuế"]
    value_row = ["{{ITEM_NO}}", "{{ITEM_NAME}}", "{{ITEM_UNIT}}", "{{ITEM_QTY}}",
                 "{{ITEM_PRICE}}", "{{ITEM_AMOUNT_PRE_VAT}}", "{{ITEM_VAT_RATE}}",
                 "{{ITEM_VAT_AMOUNT}}", "{{ITEM_AMOUNT_TOTAL}}"]
    value_total = ["Tổng cộng", "", "", "", "", "{{TOTAL_PRE_VAT}}", "", "{{TOTAL_VAT}}",
                   "{{TOTAL_AMOUNT}}"]
    value = doc.add_table(rows=3, cols=9)
    value.style = "Table Grid"
    for table_row, values in zip(value.rows, [value_head, value_row, value_total]):
        for cell, text in zip(table_row.cells, values):
            cell.text = text

    _para(doc, "Bằng chữ: {{AMOUNT_TEXT}}", italic=True)
    _para(doc, "Cách tính: {{PRICING_DESCRIPTION}}", size=10)
    _para(doc)
    _para(doc, "2. Chức năng", bold=True)

    feature = doc.add_table(rows=2, cols=3)
    feature.style = "Table Grid"
    for table_row, values in zip(feature.rows, [["STT", "Chức năng", "Mô tả"],
                                                ["{{FEATURE_NO}}", "{{FEATURE_NAME}}",
                                                 "{{FEATURE_DESC}}"]]):
        for cell, text in zip(table_row.cells, values):
            cell.text = text

    _para(doc)
    _para(doc, "3. Thời hạn từ {{START_DATE}} đến {{END_DATE}}. "
               "Chu kỳ thanh toán: {{PAYMENT_CYCLE_MONTHS}} tháng.")
    _para(doc)

    sign = doc.add_table(rows=2, cols=2)
    for cell, text in zip(sign.rows[0].cells, ["ĐẠI DIỆN BÊN A", "ĐẠI DIỆN BÊN B"]):
        cell.text = text
    for cell, text in zip(sign.rows[1].cells,
                          ["{{PARTY_A_REPRESENTATIVE}}", "{{PARTY_B_REPRESENTATIVE}}"]):
        cell.text = text
    return doc


# ─── Yêu cầu báo giá: văn bản gửi đối tác để họ tự điền thông tin và đơn giá ──────
#
# Cả ba mẫu dùng chung quy tắc: KHÔNG có placeholder nào về tiền. Cột đơn giá, thuế và
# thành tiền là ô trống để đối tác tự điền. Xem QUOTE_REQUEST_PLAN.md.

DEFAULT_DIR = OUT.parent
REQUEST_FOOTER = "Đơn giá do đơn vị báo giá tự xác định và chịu trách nhiệm."
FILL = "……………………………………………"


def _request_doc() -> Document:
    doc = Document()
    style = doc.styles["Normal"]
    style.font.name = "Times New Roman"
    style.font.size = Pt(12)
    style.paragraph_format.space_after = Pt(4)
    style.paragraph_format.line_spacing = 1.05
    section = doc.sections[0]
    section.page_width, section.page_height = Cm(21), Cm(29.7)
    section.left_margin = section.right_margin = Cm(2)
    section.top_margin = section.bottom_margin = Cm(2)
    doc.sections[0].footer.paragraphs[0].text = REQUEST_FOOTER
    return doc


def _request_table_layout(table, widths, *, header_rows=0, keep_together=False):
    """Large descriptions get room; headers repeat and individual rows stay intact."""
    table.autofit = False
    for column, width in zip(table.columns, widths):
        column.width = Cm(width)
    for index, row in enumerate(table.rows):
        props = row._tr.get_or_add_trPr()
        props.append(OxmlElement("w:cantSplit"))
        if index < header_rows:
            props.append(OxmlElement("w:tblHeader"))
        for cell, width in zip(row.cells, widths):
            cell.width = Cm(width)
            for paragraph in cell.paragraphs:
                paragraph.paragraph_format.space_after = Pt(2)
                paragraph.paragraph_format.keep_with_next = (
                    index < header_rows or (keep_together and index < len(table.rows) - 1))
                for run in paragraph.runs:
                    run.font.size = Pt(11)


def _request_heading(doc, text):
    paragraph = _para(doc, text, bold=True)
    paragraph.paragraph_format.keep_with_next = True
    return paragraph


def _bold_cells(row, align=WD_ALIGN_PARAGRAPH.CENTER) -> None:
    for cell in row.cells:
        for p in cell.paragraphs:
            p.alignment = align
            for run in p.runs:
                run.bold = True


def _items_table(doc) -> None:
    """Bảng hạng mục: 4 cột đầu do app điền, 3 cột cuối để trống cho đối tác."""
    table = doc.add_table(rows=3, cols=7)
    table.style = "Table Grid"
    head = ["STT", "Tên hàng hóa, dịch vụ", "ĐVT", "Số lượng",
            "Đơn giá (VNĐ, chưa VAT)", "Thuế GTGT (%)", "Thành tiền (VNĐ, sau VAT)"]
    for cell, text in zip(table.rows[0].cells, head):
        cell.text = text
    _bold_cells(table.rows[0])
    for cell, text in zip(table.rows[1].cells,
                          ["{{ITEM_NO}}", "{{ITEM_NAME}}", "{{ITEM_UNIT}}", "{{ITEM_QTY}}"]):
        cell.text = text
    # Dòng mẫu được nhân bản theo số hạng mục: chiều cao tối thiểu để có chỗ điền tay.
    table.rows[1].height = Cm(0.9)
    table.rows[2].cells[1].text = "Cộng"
    table.rows[2].height = Cm(0.9)
    for p in table.rows[2].cells[1].paragraphs:
        for run in p.runs:
            run.bold = True
    _request_table_layout(table, [1.3, 4.5, 1.6, 1.7, 2.9, 1.7, 3.3], header_rows=1)


def _features_table(doc) -> None:
    table = doc.add_table(rows=3, cols=3)
    table.style = "Table Grid"
    table.rows[0].cells[0].merge(table.rows[0].cells[2]).text = "Yêu cầu chức năng"
    _bold_cells(table.rows[0])
    for cell, text in zip(table.rows[1].cells, ["STT", "Chức năng", "Mô tả"]):
        cell.text = text
    _bold_cells(table.rows[1])
    for cell, text in zip(table.rows[2].cells,
                          ["{{FEATURE_NO}}", "{{FEATURE_NAME}}", "{{FEATURE_DESC}}"]):
        cell.text = text
    # The merged title spans the full width of the two data rows.
    _request_table_layout(table, [1.3, 4, 11.7], header_rows=2)
    table.rows[0].cells[0].width = Cm(17)


def _partner_block(doc) -> None:
    """Khối thông tin đơn vị báo giá: toàn bộ để trống."""
    labels = ["Tên đơn vị báo giá", "Mã số thuế", "Địa chỉ",
              "Người đại diện, chức vụ", "Điện thoại, email",
              "Thời hạn hiệu lực của báo giá"]
    table = doc.add_table(rows=len(labels), cols=2)
    table.style = "Table Grid"
    for row, label in zip(table.rows, labels):
        row.cells[0].text = label
        row.cells[0].width = Cm(6)
        row.height = Cm(0.9)
        for p in row.cells[0].paragraphs:
            for run in p.runs:
                run.bold = True
    _request_table_layout(table, [6, 11], keep_together=True)
    for cell in table.rows[-1].cells:
        for paragraph in cell.paragraphs:
            paragraph.paragraph_format.keep_with_next = True


def _partner_signature(doc) -> None:
    date_line = _para(doc, f"{FILL}, ngày ….. tháng ….. năm ………", italic=True,
                     align=WD_ALIGN_PARAGRAPH.RIGHT)
    title = _para(doc, "ĐẠI DIỆN ĐƠN VỊ BÁO GIÁ", bold=True, align=WD_ALIGN_PARAGRAPH.RIGHT)
    date_line.paragraph_format.keep_with_next = True
    title.paragraph_format.keep_with_next = True
    signature = _para(doc, "(Ký tên, đóng dấu)", italic=True, align=WD_ALIGN_PARAGRAPH.RIGHT)
    signature.paragraph_format.space_after = Pt(36)


def _partner_tail(doc, contact=None):
    """Keep the contact and signature with the partner's fill-in block."""
    _partner_block(doc)
    if contact:
        paragraph = _para(doc, contact)
        paragraph.paragraph_format.keep_with_next = True
    gap = _para(doc)
    gap.paragraph_format.keep_with_next = True
    _partner_signature(doc)


def _letter_head(doc, title: str) -> None:
    head = doc.add_table(rows=1, cols=2)
    head.alignment = WD_TABLE_ALIGNMENT.CENTER
    left, right = head.rows[0].cells
    # Không dùng PARTY_A_NAME ở ô đầu của bảng: bộ nhân dòng bên A của renderer sẽ nhận nhầm
    # đây là bảng bên A và chèn thêm dòng "Mã số thuế" vào đầu thư.
    left.text = "{{REQUEST_ISSUER_NAME}}"
    right.text = "CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM\nĐộc lập – Tự do – Hạnh phúc"
    for cell in (left, right):
        for p in cell.paragraphs:
            p.alignment = WD_ALIGN_PARAGRAPH.CENTER
            for run in p.runs:
                run.bold = True
                run.font.size = Pt(11)
    _para(doc)
    _para(doc, "{{REQUEST_PLACE}}, ngày {{REQUEST_DATE_DAY}} tháng {{REQUEST_DATE_MONTH}} "
               "năm {{REQUEST_DATE_YEAR}}", italic=True, align=WD_ALIGN_PARAGRAPH.RIGHT)
    _para(doc, title, bold=True, align=WD_ALIGN_PARAGRAPH.CENTER, size=14)
    _para(doc, "V/v: {{REQUEST_SUBJECT}}", align=WD_ALIGN_PARAGRAPH.CENTER)
    _para(doc)
    _para(doc, "Kính gửi: {{REQUEST_ADDRESSEE}}")


def build_request_letter() -> Document:
    """Thư mời báo giá chính thức."""
    doc = _request_doc()
    _letter_head(doc, "THƯ MỜI BÁO GIÁ")
    _para(doc, "{{PARTY_A_NAME}} (địa chỉ: {{PARTY_A_ADDRESS}}; mã số thuế: {{PARTY_A_TAX_CODE}}) "
               "có nhu cầu mua sắm trực tiếp hàng hóa, dịch vụ dưới đây. Kính đề nghị Quý đơn vị "
               "gửi báo giá theo nội dung sau.")
    _para(doc, "Phạm vi: {{UNIT_COUNT}} đơn vị sử dụng, thời gian {{MONTH_COUNT}} tháng.")
    _para(doc, "Đơn vị sử dụng: {{PRICING_UNITS}}")
    _para(doc, "{{REQUEST_NOTE}}")
    _request_heading(doc, "1. Hạng mục đề nghị báo giá")
    _items_table(doc)
    _para(doc, "Bằng chữ: " + FILL, italic=True)
    _para(doc)
    _request_heading(doc, "2. Thông tin đơn vị báo giá")
    _partner_tail(doc, "Báo giá xin gửi về địa chỉ trên trước ngày {{REQUEST_DEADLINE}}. "
                      "Liên hệ: {{PARTY_A_CONTACT_NAME}}, điện thoại {{PARTY_A_PHONE}}, "
                      "email {{PARTY_A_EMAIL}}.")
    return doc


def build_request_form() -> Document:
    """Phiếu khảo sát giá gọn, một trang."""
    doc = _request_doc()
    _para(doc, "PHIẾU KHẢO SÁT GIÁ", bold=True, align=WD_ALIGN_PARAGRAPH.CENTER, size=14)
    _para(doc, "{{REQUEST_SUBJECT}}", align=WD_ALIGN_PARAGRAPH.CENTER)
    _para(doc, "Đơn vị khảo sát: {{PARTY_A_NAME}} – {{PARTY_A_ADDRESS}} – MST: {{PARTY_A_TAX_CODE}}")
    _para(doc, "Ngày lập: {{REQUEST_DATE}}    Hạn nhận phiếu: {{REQUEST_DEADLINE}}    "
               "Liên hệ: {{PARTY_A_CONTACT_NAME}} {{PARTY_A_PHONE}}")
    _para(doc, "Gửi: {{REQUEST_ADDRESSEE}}")
    _para(doc)
    _items_table(doc)
    _para(doc, "{{REQUEST_NOTE}}", size=10)
    _request_heading(doc, "Thông tin đơn vị báo giá")
    _partner_tail(doc)
    return doc


def build_request_detailed() -> Document:
    """Thư mời kèm yêu cầu chức năng và các điều kiện đối tác phải nêu."""
    doc = _request_doc()
    _letter_head(doc, "YÊU CẦU BÁO GIÁ")
    _para(doc, "{{PARTY_A_NAME}} có nhu cầu mua sắm trực tiếp hàng hóa, dịch vụ dưới đây. "
               "Kính đề nghị Quý đơn vị gửi báo giá đáp ứng đủ các yêu cầu sau.")
    _para(doc, "Phạm vi: {{UNIT_COUNT}} đơn vị sử dụng, thời gian {{MONTH_COUNT}} tháng. "
               "Đơn vị sử dụng: {{PRICING_UNITS}}")
    _para(doc, "{{REQUEST_NOTE}}")
    _request_heading(doc, "1. Hạng mục đề nghị báo giá")
    _items_table(doc)
    _para(doc)
    _features_table(doc)
    _para(doc)
    _request_heading(doc, "2. Điều kiện báo giá (đơn vị báo giá tự ghi)")
    for label in ("Thời gian triển khai", "Hỗ trợ, bảo hành", "Điều kiện thanh toán"):
        _para(doc, f"– {label}: {FILL}")
    _para(doc)
    _request_heading(doc, "3. Thông tin đơn vị báo giá")
    _partner_tail(doc, "Báo giá xin gửi trước ngày {{REQUEST_DEADLINE}}. Liên hệ: {{PARTY_A_CONTACT_NAME}}, "
                      "điện thoại {{PARTY_A_PHONE}}, email {{PARTY_A_EMAIL}}.")
    return doc


REQUEST_BUILDERS = {
    "sample_request_letter.docx": build_request_letter,
    "sample_request_form.docx": build_request_form,
    "sample_request_detailed.docx": build_request_detailed,
}


# ─── Báo giá nhập tay: năm bố cục dùng chung bộ placeholder ───────────────────
#
# Giá trị phát hành và giá do người tạo báo giá nhập. Không gắn với giá danh mục
# sản phẩm hoặc thông tin bên B trong phần cài đặt.

QUOTE_BUILDERS = {
    "sample_quote_compact.docx": "compact",
    "sample_quote_formal.docx": "formal",
    "sample_quote_landscape.docx": "landscape",
    "sample_quote_features.docx": "features",
    "sample_quote_terms.docx": "terms",
}

QUOTE_LINE_FIELDS = (
    "ITEM_NO", "ITEM_NAME", "ITEM_UNIT", "ITEM_QTY", "ITEM_PERIODS",
    "ITEM_PRICE", "ITEM_VAT_RATE", "ITEM_AMOUNT_PRE_VAT", "ITEM_VAT_AMOUNT",
    "ITEM_AMOUNT_TOTAL",
)


def _quote_doc(*, landscape=False, compact=False):
    doc = Document()
    section = doc.sections[0]
    if landscape:
        section.page_width, section.page_height = Cm(29.7), Cm(21)
        section.left_margin = section.right_margin = Cm(1.5)
        section.top_margin = section.bottom_margin = Cm(1.5)
    else:
        section.page_width, section.page_height = Cm(21), Cm(29.7)
        section.left_margin = section.right_margin = Cm(1.7 if compact else 2)
        section.top_margin = section.bottom_margin = Cm(1.5 if compact else 1.8)
    normal = doc.styles["Normal"]
    normal.font.name = "Times New Roman"
    normal.font.size = Pt(10 if compact else 10.5)
    normal.paragraph_format.space_after = Pt(3 if compact else 4)
    normal.paragraph_format.line_spacing = 1.0 if compact else 1.05
    section.footer.paragraphs[0].text = "{{QUOTE_ISSUER_NAME}} · {{QUOTE_ISSUER_PHONE}} · {{QUOTE_ISSUER_EMAIL}}"
    return doc


def _quote_para(doc, text="", *, bold=False, italic=False, align=None, size=10.5,
                keep_next=False, space_after=4):
    paragraph = doc.add_paragraph()
    if align is not None:
        paragraph.alignment = align
    paragraph.paragraph_format.space_after = Pt(space_after)
    paragraph.paragraph_format.keep_with_next = keep_next
    run = paragraph.add_run(text)
    run.bold = bold
    run.italic = italic
    run.font.name = "Times New Roman"
    run.font.size = Pt(size)
    return paragraph


def _quote_heading(doc, text):
    return _quote_para(doc, text, bold=True, keep_next=True, space_after=3)


def _quote_issuer_and_recipient(doc, *, compact=False, formal=False):
    if formal:
        head = doc.add_table(rows=1, cols=2)
        head.alignment = WD_TABLE_ALIGNMENT.CENTER
        head.autofit = False
        head.columns[0].width = Cm(12)
        head.columns[1].width = Cm(5)
        left, right = head.rows[0].cells
        left.width, right.width = Cm(12), Cm(5)
        left.text = "{{QUOTE_ISSUER_NAME}}\n{{QUOTE_ISSUER_ADDRESS}}\nMST: {{QUOTE_ISSUER_TAX_CODE}}"
        right.text = "Số: {{QUOTE_NUMBER}}\nNgày: {{QUOTE_DATE}}\nHiệu lực: {{QUOTE_VALID_UNTIL}}"
        for cell in (left, right):
            for paragraph in cell.paragraphs:
                paragraph.paragraph_format.space_after = Pt(2)
                for run in paragraph.runs:
                    run.font.name = "Times New Roman"
                    run.font.size = Pt(9.5)
        _quote_para(doc, "BÁO GIÁ", bold=True, align=WD_ALIGN_PARAGRAPH.CENTER,
                    size=16, keep_next=True, space_after=2)
        _quote_para(doc, "{{QUOTE_SUBJECT}}", align=WD_ALIGN_PARAGRAPH.CENTER,
                    size=11, keep_next=True)
    else:
        _quote_para(doc, "BÁO GIÁ", bold=True, align=WD_ALIGN_PARAGRAPH.CENTER,
                    size=14 if compact else 16, keep_next=True, space_after=2)
        _quote_para(doc, "{{QUOTE_SUBJECT}}", align=WD_ALIGN_PARAGRAPH.CENTER,
                    size=10 if compact else 11, keep_next=True, space_after=4)
        _quote_para(doc, "Số: {{QUOTE_NUMBER}}    Ngày: {{QUOTE_DATE}}    Hiệu lực đến: {{QUOTE_VALID_UNTIL}}",
                    align=WD_ALIGN_PARAGRAPH.CENTER, size=9.5 if compact else 10,
                    keep_next=True, space_after=5)

    table = doc.add_table(rows=4, cols=2)
    table.style = "Table Grid"
    fields = [
        ("Đơn vị báo giá", "{{QUOTE_ISSUER_NAME}}"),
        ("Địa chỉ, MST", "{{QUOTE_ISSUER_ADDRESS}} · {{QUOTE_ISSUER_TAX_CODE}}"),
        ("Người đại diện", "{{QUOTE_ISSUER_REPRESENTATIVE}} · {{QUOTE_ISSUER_TITLE}}"),
        ("Liên hệ", "{{QUOTE_ISSUER_PHONE}} · {{QUOTE_ISSUER_EMAIL}}"),
    ]
    for row, (label, value) in zip(table.rows, fields):
        row.cells[0].text, row.cells[1].text = label, value
        row.cells[0].width, row.cells[1].width = Cm(4.1), Cm(12.9)
        for run in row.cells[0].paragraphs[0].runs:
            run.bold = True
    _quote_table_layout(table, [4.1, 12.9], compact=compact, repeat_rows=0)

    _quote_heading(doc, "Kính gửi")
    _quote_para(doc, "{{PARTY_A_NAME}} · {{PARTY_A_ADDRESS}} · MST {{PARTY_A_TAX_CODE}}")
    _quote_para(doc, "Người liên hệ: {{PARTY_A_CONTACT_NAME}} · Địa điểm báo giá: {{QUOTE_PLACE}}",
                size=9.5 if compact else 10, keep_next=True)


def _quote_table_layout(table, widths, *, compact=False, repeat_rows=1, row_keep=True,
                        font_size=None):
    table.autofit = False
    table.alignment = WD_TABLE_ALIGNMENT.CENTER
    font_size = font_size or (9.5 if compact else 10)
    margin_twips = str(round(.08 / 2.54 * 1440))
    for column, width in zip(table.columns, widths):
        column.width = Cm(width)
    for row_index, row in enumerate(table.rows):
        props = row._tr.get_or_add_trPr()
        if row_keep:
            props.append(OxmlElement("w:cantSplit"))
        if row_index < repeat_rows:
            props.append(OxmlElement("w:tblHeader"))
        for cell, width in zip(row.cells, widths):
            cell.width = Cm(width)
            tc_pr = cell._tc.get_or_add_tcPr()
            margins = tc_pr.find("w:tcMar", namespaces=tc_pr.nsmap)
            if margins is None:
                margins = OxmlElement("w:tcMar")
                tc_pr.append(margins)
            for side in ("top", "start", "bottom", "end"):
                margin = margins.find(f"w:{side}", namespaces=margins.nsmap)
                if margin is None:
                    margin = OxmlElement(f"w:{side}")
                    margins.append(margin)
                margin.set(qn("w:w"), margin_twips)
                margin.set(qn("w:type"), "dxa")
            for paragraph in cell.paragraphs:
                paragraph.paragraph_format.space_after = Pt(1 if compact else 2)
                for run in paragraph.runs:
                    run.font.name = "Times New Roman"
                    run.font.size = Pt(font_size)
                    if row_index < repeat_rows:
                        run.bold = True


def _quote_items_table(doc, *, landscape=False, compact=False, formal=False):
    widths = ([.9, 6.1, 1.45, 1.55, 1.45, 2.8, 1.25, 11.2] if landscape else
              [1.1, 4.3, 1.5, 1.2, 1.2, 2.3, 1.3, 4.1])
    # Totals are displayed together inside one cell to keep portrait layouts legible.
    table = doc.add_table(rows=3, cols=8)
    table.style = "Table Grid"
    headings = ["STT", "Hàng hóa, dịch vụ", "ĐVT", "SL/kỳ", "Số kỳ",
                "Đơn giá", "VAT", "Thành tiền (VNĐ)"]
    for cell, value in zip(table.rows[0].cells, headings):
        cell.text = value
    row_values = [
        "{{ITEM_NO}}", "{{ITEM_NAME}}", "{{ITEM_UNIT}}", "{{ITEM_QTY}}",
        "{{ITEM_PERIODS}}", "{{ITEM_PRICE}}", "{{ITEM_VAT_RATE}}",
        "Trước thuế: {{ITEM_AMOUNT_PRE_VAT}}\nThuế: {{ITEM_VAT_AMOUNT}}\nSau thuế: {{ITEM_AMOUNT_TOTAL}}",
    ]
    for cell, value in zip(table.rows[1].cells, row_values):
        cell.text = value
    for cell, value in zip(table.rows[2].cells,
                           ["", "CỘNG", "", "", "", "", "",
                            "Trước thuế: {{TOTAL_PRE_VAT}}\nThuế: {{TOTAL_VAT}}\nSau thuế: {{TOTAL_AMOUNT}}"]):
        cell.text = value
    table.rows[2].cells[1].paragraphs[0].runs[0].bold = True
    for cell in table.rows[0].cells:
        for paragraph in cell.paragraphs:
            paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
    for cell in table.rows[0].cells:
        for run in cell.paragraphs[0].runs:
            run.bold = True
    _quote_table_layout(table, widths, compact=compact, repeat_rows=1,
                        font_size=10.5 if landscape else None)
    return table


def _quote_conditions(doc, *, terms=False):
    if terms:
        _quote_heading(doc, "Điều kiện cung cấp")
        for label, field in (("Giao hàng, triển khai", "QUOTE_DELIVERY_TERMS"),
                             ("Hỗ trợ, bảo hành", "QUOTE_SUPPORT_TERMS"),
                             ("Thanh toán", "QUOTE_PAYMENT_TERMS")):
            table = doc.add_table(rows=1, cols=2)
            table.style = "Table Grid"
            table.cell(0, 0).text = label
            table.cell(0, 1).text = "{{" + field + "}}"
            table.cell(0, 0).width, table.cell(0, 1).width = Cm(4.2), Cm(12.8)
            for run in table.cell(0, 0).paragraphs[0].runs:
                run.bold = True
            _quote_table_layout(table, [4.2, 12.8], repeat_rows=0)
    else:
        _quote_para(doc, "Thanh toán: {{QUOTE_PAYMENT_TERMS}}", size=9.5)
        _quote_para(doc, "Giao hàng, triển khai: {{QUOTE_DELIVERY_TERMS}}", size=9.5)
        _quote_para(doc, "Hỗ trợ, bảo hành: {{QUOTE_SUPPORT_TERMS}}", size=9.5)
    _quote_para(doc, "Ghi chú: {{QUOTE_NOTE}}", size=9.5)


def _quote_signature(doc):
    _quote_para(doc, "Bằng chữ: {{AMOUNT_TEXT}}", italic=True, keep_next=True)
    _quote_para(doc, "Trân trọng báo giá. Đơn vị báo giá chịu trách nhiệm về nội dung và hiệu lực nêu trên.",
                keep_next=True)
    date_line = _quote_para(doc, "{{QUOTE_PLACE}}, ngày {{QUOTE_DATE}}", italic=True,
                            align=WD_ALIGN_PARAGRAPH.RIGHT, keep_next=True)
    _quote_para(doc, "ĐẠI DIỆN ĐƠN VỊ BÁO GIÁ", bold=True,
                align=WD_ALIGN_PARAGRAPH.RIGHT, keep_next=True)
    _quote_para(doc, "{{QUOTE_ISSUER_REPRESENTATIVE}} · {{QUOTE_ISSUER_TITLE}}\n(Ký, ghi rõ họ tên, đóng dấu)",
                align=WD_ALIGN_PARAGRAPH.RIGHT, size=9.5, space_after=24)
    date_line.paragraph_format.keep_with_next = True


def _quote_features_table(doc):
    table = doc.add_table(rows=3, cols=3)
    table.style = "Table Grid"
    title = table.rows[0].cells[0].merge(table.rows[0].cells[2])
    title.text = "Phạm vi chức năng"
    for run in title.paragraphs[0].runs:
        run.bold = True
    title.paragraphs[0].alignment = WD_ALIGN_PARAGRAPH.CENTER
    for cell, value in zip(table.rows[1].cells, ["STT", "Chức năng", "Mô tả"]):
        cell.text = value
    for cell, value in zip(table.rows[2].cells, ["{{FEATURE_NO}}", "{{FEATURE_NAME}}", "{{FEATURE_DESC}}"]):
        cell.text = value
    _quote_table_layout(table, [1.1, 4.3, 11.6], repeat_rows=2)
    title.width = Cm(17)
    return table


def build_quote_compact():
    """Báo giá gọn, bảng giá rõ, ưu tiên gói hạng mục ngắn."""
    doc = _quote_doc(compact=True)
    _quote_issuer_and_recipient(doc, compact=True)
    _quote_heading(doc, "Nội dung báo giá")
    _quote_items_table(doc, compact=True)
    _quote_conditions(doc)
    _quote_signature(doc)
    return doc


def build_quote_formal():
    """Báo giá trang trọng với thông tin phát hành ở đầu trang."""
    doc = _quote_doc()
    _quote_issuer_and_recipient(doc, formal=True)
    _quote_para(doc, "Kính gửi Quý khách hàng, chúng tôi trân trọng gửi nội dung chào giá dưới đây.",
                keep_next=True)
    _quote_heading(doc, "Danh mục chào giá")
    _quote_items_table(doc, formal=True)
    _quote_conditions(doc, terms=True)
    _quote_signature(doc)
    return doc


def build_quote_landscape():
    """Báo giá khổ ngang cho nhiều cột và nhiều dòng."""
    doc = _quote_doc(landscape=True)
    _quote_issuer_and_recipient(doc)
    _quote_heading(doc, "Chi tiết đơn giá và thuế")
    _quote_items_table(doc, landscape=True)
    _quote_conditions(doc, terms=True)
    _quote_signature(doc)
    return doc


def build_quote_features():
    """Báo giá có bảng phạm vi chức năng riêng, được bỏ khi danh sách trống."""
    doc = _quote_doc()
    _quote_issuer_and_recipient(doc, formal=True)
    _quote_heading(doc, "Danh mục chào giá")
    _quote_items_table(doc, formal=True)
    _quote_features_table(doc)
    _quote_conditions(doc)
    _quote_signature(doc)
    return doc


def build_quote_terms():
    """Báo giá kèm điều kiện triển khai, hỗ trợ, bảo hành và thanh toán."""
    doc = _quote_doc()
    _quote_issuer_and_recipient(doc)
    _quote_heading(doc, "Hạng mục và giá chào")
    _quote_items_table(doc, formal=True)
    _quote_conditions(doc, terms=True)
    _quote_signature(doc)
    return doc


def build_quote_templates():
    """Return the five new quotation layouts, without rewriting request samples."""
    builders = {
        "sample_quote_compact.docx": build_quote_compact,
        "sample_quote_formal.docx": build_quote_formal,
        "sample_quote_landscape.docx": build_quote_landscape,
        "sample_quote_features.docx": build_quote_features,
        "sample_quote_terms.docx": build_quote_terms,
    }
    DEFAULT_DIR.mkdir(parents=True, exist_ok=True)
    for filename, builder in builders.items():
        path = DEFAULT_DIR / filename
        builder().save(path)
    return builders


def main() -> int:
    OUT.parent.mkdir(parents=True, exist_ok=True)
    build_form_08a().save(OUT)
    print(f"Đã ghi {OUT} ({OUT.stat().st_size} bytes)")
    build_service_contract().save(OUT_SERVICE)
    print(f"Đã ghi {OUT_SERVICE} ({OUT_SERVICE.stat().st_size} bytes)")
    for filename, builder in REQUEST_BUILDERS.items():
        path = DEFAULT_DIR / filename
        builder().save(path)
        print(f"Đã ghi {path} ({path.stat().st_size} bytes)")
    for filename, builder in build_quote_templates().items():
        path = DEFAULT_DIR / filename
        print(f"Đã ghi {path} ({path.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
