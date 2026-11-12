"""Task 3.3 — analyzer nhận biết placeholder mới và dòng lặp.

Dữ liệu hoàn toàn hư cấu. Mẫu Word dựng trong bộ nhớ.
"""

import io

import docx

from app.services.template_analyzer import (
    CANONICAL_MAP,
    ROW_PLACEHOLDERS,
    analyze,
    build_default_mapping,
)


def _bytes(build):
    doc = docx.Document()
    build(doc)
    buf = io.BytesIO()
    doc.save(buf)
    return buf.getvalue()


def _table_with(doc, cells_by_row):
    t = doc.add_table(rows=len(cells_by_row), cols=len(cells_by_row[0]))
    for r, row in enumerate(cells_by_row):
        for c, text in enumerate(row):
            t.rows[r].cells[c].text = text
    return t


def test_repeat_rows_in_tables_are_detected_without_warnings():
    def build(doc):
        _table_with(doc, [["TT", "Dịch vụ"], ["{{ITEM_NO}}", "{{ITEM_NAME}}"]])
        _table_with(doc, [["STT", "Chức năng"], ["{{FEATURE_NO}}", "{{FEATURE_NAME}}"]])

    result = analyze(_bytes(build))

    assert result.ok
    assert result.repeat_groups == ["features", "items"]
    assert result.row_warnings == []


def test_row_placeholder_outside_a_table_is_warned():
    def build(doc):
        doc.add_paragraph("Dịch vụ: {{ITEM_NAME}}")

    result = analyze(_bytes(build))

    assert result.repeat_groups == []
    assert len(result.row_warnings) == 1
    assert "{{ITEM_NAME}}" in result.row_warnings[0]
    assert "dòng bảng" in result.row_warnings[0]


def test_scalar_placeholders_added_in_task_3_2_are_all_mapped():
    scalars = [
        "TOTAL_PRE_VAT", "TOTAL_VAT", "PAYMENT_CYCLE_MONTHS",
        "PARTY_A_PHONE", "PARTY_A_EMAIL", "PARTY_A_CONTACT_NAME",
        "PARTY_A_ESTABLISHMENT_DECISION", "PARTY_A_ESTABLISHMENT_DATE",
        "PARTY_B_PHONE", "PARTY_B_CONTACT_NAME", "PARTY_B_CONTACT_EMAIL",
        "PRODUCT_NAME", "PARENT_CONTRACT_NUMBER", "PARENT_SIGN_DATE",
    ]
    for ph in scalars:
        assert CANONICAL_MAP.get(ph), ph


def test_row_placeholders_map_to_their_list_and_key():
    mapping = build_default_mapping(["ITEM_NAME", "FEATURE_DESC", "TOTAL_AMOUNT", "UNKNOWN_X"])

    assert mapping == {
        "ITEM_NAME": "items.ITEM_NAME",
        "FEATURE_DESC": "features.FEATURE_DESC",
        "TOTAL_AMOUNT": "contract.total_amount",
        "UNKNOWN_X": "",
    }
    assert ROW_PLACEHOLDERS["ITEM_AMOUNT_TOTAL"] == "items.ITEM_AMOUNT_TOTAL"


def test_appendix_kind_is_recognised_by_parent_contract_number():
    from app.services.template_service import CONTRACT_TYPES, _template_kind

    assert "Bàn giao" in CONTRACT_TYPES
    assert _template_kind(["CONTRACT_NUMBER", "PARENT_CONTRACT_NUMBER"]) == "Phụ lục bổ sung"
    assert _template_kind(["CONTRACT_NUMBER", "PARENT_CONTRACT_NUMBER", "START_DATE", "END_DATE"]) == "Phụ lục gia hạn"
    assert _template_kind(["CONTRACT_NUMBER", "TOTAL_AMOUNT"]) == "Hợp đồng"
    assert _template_kind(["ACCEPTANCE_DATE_DAY", "PARENT_CONTRACT_NUMBER"]) == "Nghiệm thu"


def test_template_detail_page_shows_the_row_capabilities(client, make_template, data_root):
    path = data_root / "rows_template.docx"
    path.write_bytes(_bytes(lambda d: _table_with(d, [["TT"], ["{{ITEM_NO}}"]])))
    tpl = make_template(file_path=str(path), detected_placeholders=["ITEM_NO"])

    resp = client.get(f"/templates/{tpl.id}")

    assert resp.status_code == 200
    assert "Có bảng giá trị" in resp.text
