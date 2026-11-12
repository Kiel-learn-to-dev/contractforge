"""Task 3.1 — lặp dòng bảng trong renderer (ITEM_* và FEATURE_*).

Tài liệu được dựng trong bộ nhớ bằng python-docx. Dữ liệu hoàn toàn hư cấu.
"""

import io

from docx import Document

from app.services.docx_renderer import render_docx


def _bytes(build):
    doc = Document()
    build(doc)
    buf = io.BytesIO()
    doc.save(buf)
    return buf.getvalue()


def _set(cell, text, bold=False):
    cell.text = ""
    run = cell.paragraphs[0].add_run(text)
    run.bold = bold


def _rows(out_bytes, table_index=0):
    doc = Document(io.BytesIO(out_bytes))
    return [[c.text for c in r.cells] for r in doc.tables[table_index].rows]


def _items_table(doc, bold_template=False, split_template=False):
    t = doc.add_table(rows=3, cols=3)
    _set(t.rows[0].cells[0], "TT", bold=True)
    _set(t.rows[0].cells[1], "Dịch vụ")
    _set(t.rows[0].cells[2], "Thành tiền")
    if split_template:
        t.rows[1].cells[0].text = ""
        p = t.rows[1].cells[0].paragraphs[0]
        p.add_run("{{ITEM_")
        p.add_run("NO")
        p.add_run("}}")
    else:
        _set(t.rows[1].cells[0], "{{ITEM_NO}}", bold=bold_template)
    _set(t.rows[1].cells[1], "{{ITEM_NAME}}", bold=bold_template)
    _set(t.rows[1].cells[2], "{{ITEM_AMOUNT_TOTAL}}", bold=bold_template)
    _set(t.rows[2].cells[0], "Tổng")
    _set(t.rows[2].cells[1], "")
    _set(t.rows[2].cells[2], "{{TOTAL_AMOUNT}}")
    return t


def _items(n):
    return [
        {"ITEM_NO": str(i), "ITEM_NAME": f"Dịch vụ {i}", "ITEM_AMOUNT_TOTAL": f"{i}00"}
        for i in range(1, n + 1)
    ]


def test_template_row_is_repeated_in_order_and_total_row_stays_last():
    src = _bytes(lambda d: _items_table(d))
    out = render_docx(src, {"TOTAL_AMOUNT": "300"}, lists={"items": _items(2)})

    rows = _rows(out)
    assert len(rows) == 4
    assert rows[1] == ["1", "Dịch vụ 1", "100"]
    assert rows[2] == ["2", "Dịch vụ 2", "200"]
    assert rows[3][0] == "Tổng"
    assert rows[3][2] == "300"


def test_empty_list_removes_the_template_row_and_keeps_the_rest():
    src = _bytes(lambda d: _items_table(d))
    out = render_docx(src, {"TOTAL_AMOUNT": "0"}, lists={"items": []})

    rows = _rows(out)
    assert len(rows) == 2
    assert rows[0][0] == "TT"
    assert rows[1][0] == "Tổng"


def test_one_item_gives_three_rows():
    src = _bytes(lambda d: _items_table(d))
    out = render_docx(src, {"TOTAL_AMOUNT": "100"}, lists={"items": _items(1)})
    assert len(_rows(out)) == 3


def test_generated_rows_keep_bold_from_the_template_row():
    src = _bytes(lambda d: _items_table(d, bold_template=True))
    out = render_docx(src, {"TOTAL_AMOUNT": "300"}, lists={"items": _items(2)})

    doc = Document(io.BytesIO(out))
    for row in doc.tables[0].rows[1:3]:
        text_runs = [r for r in row.cells[1].paragraphs[0].runs if r.text]
        assert text_runs and all(r.bold is True for r in text_runs)


def test_placeholder_split_across_three_runs_in_template_row_is_replaced():
    src = _bytes(lambda d: _items_table(d, split_template=True))
    out = render_docx(src, {"TOTAL_AMOUNT": "300"}, lists={"items": _items(2)})

    rows = _rows(out)
    assert rows[1][0] == "1"
    assert rows[2][0] == "2"
    assert all("{{" not in cell for row in rows for cell in row)


def test_two_tables_each_receive_their_own_list():
    def build(doc):
        _items_table(doc)
        feat = doc.add_table(rows=2, cols=2)
        _set(feat.rows[0].cells[0], "Chức năng")
        _set(feat.rows[0].cells[1], "Mô tả")
        _set(feat.rows[1].cells[0], "{{FEATURE_NAME}}")
        _set(feat.rows[1].cells[1], "{{FEATURE_DESC}}")

    src = _bytes(build)
    out = render_docx(src, {"TOTAL_AMOUNT": "100"}, lists={
        "items": _items(1),
        "features": [{"FEATURE_NAME": "Đăng nhập", "FEATURE_DESC": "Mô tả A"},
                     {"FEATURE_NAME": "Xuất báo cáo", "FEATURE_DESC": "Mô tả B"}],
    })

    assert len(_rows(out, 0)) == 3  # 1 dòng ITEM + tiêu đề + dòng tổng
    feature_rows = _rows(out, 1)
    assert feature_rows == [["Chức năng", "Mô tả"],
                            ["Đăng nhập", "Mô tả A"],
                            ["Xuất báo cáo", "Mô tả B"]]


def test_lists_none_leaves_repeat_rows_untouched():
    src = _bytes(lambda d: _items_table(d))
    out = render_docx(src, {"TOTAL_AMOUNT": "300"})

    rows = _rows(out)
    assert len(rows) == 3
    assert rows[1][0] == "{{ITEM_NO}}"


def test_document_without_repeat_rows_renders_the_same_with_or_without_lists():
    def build(doc):
        doc.add_paragraph("Hợp đồng số {{CONTRACT_NUMBER}}")
        _set(doc.add_table(rows=1, cols=1).rows[0].cells[0], "Tổng {{TOTAL_AMOUNT}}")

    src = _bytes(build)
    ctx = {"CONTRACT_NUMBER": "01/2026", "TOTAL_AMOUNT": "100"}

    def text(out):
        doc = Document(io.BytesIO(out))
        return [p.text for p in doc.paragraphs] + [c.text for r in doc.tables[0].rows for c in r.cells]

    assert text(render_docx(src, ctx)) == text(render_docx(src, ctx, lists={"items": _items(3)}))
