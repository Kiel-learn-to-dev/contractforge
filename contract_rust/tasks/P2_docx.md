# Đợt 2 — `cf-docx`: engine sinh docx

Blueprint: **`docs/04_DOCX_ENGINE.md`** (đọc hết trước task đầu). Mọi file trong `crates/cf-docx/`.
Fixture: `fixtures/docx_render.json`, `fixtures/template_analysis.json`, `fixtures/docx/*.docx`,
mẫu thật trong repo `ContractForge/assets/default_templates/sample_*.docx`.
Khác biệt có chủ ý: KB-03, KB-04, KB-05, KB-06.

---

### T2.01 — Lỗi và đọc gói ZIP · S
- [ ] Xong
- **Làm:** `pub enum DocxError { Open(String), Xml(String), Io(io::Error) }`; `Package::read(bytes)`
  liệt kê entry theo thứ tự; `is_text_part(name)` theo bảng `04_DOCX_ENGINE.md` §2.
  Lỗi mở → `DocxError::Open` với `Display` = `"Không mở được file .docx: {chi tiết}"`.
- **File:** `src/lib.rs`, `src/error.rs`, `src/package.rs`
- **Test (unit):** đọc `fixtures/docx/synthetic_all_traps.docx` → có `word/document.xml`, `word/header1.xml`,
  `word/footer1.xml` là phần chữ; `word/styles.xml` không phải. Bytes rác → `Open`.
- **Xong khi:** `cargo test -p cf-docx package`.
- **Phụ thuộc:** T0.03

### T2.02 — Cây XML: parse và serialize · M
- [ ] Xong
- **Làm:** `xml::Node` (Element{name (giữ prefix), attrs (giữ thứ tự), children} | Text | Raw cho
  comment/PI/khai báo); `parse(&[u8]) -> Node`, `serialize(&Node) -> Vec<u8>` (giữ khai báo
  `<?xml ... standalone="yes"?>`, escape đúng). Text giữ nguyên khoảng trắng.
- **File:** `src/xml.rs`
- **Test:** với **mọi** phần chữ của mọi file `.docx` trong `fixtures/docx/` và `ContractForge/assets/default_templates/sample_*.docx`:
  `parse(serialize(parse(x))) == parse(x)` (so cây).
- **Xong khi:** `cargo test -p cf-docx xml`.
- **Phụ thuộc:** T2.01

### T2.03 — Trích văn bản theo đoạn (dùng cho test) · S
- [ ] Xong
- **Làm:** `pub fn paragraph_texts(part: &Node) -> Vec<String>` theo đúng `docx_render.json → text_extraction_rule`
  (mỗi `w:p` một chuỗi; `w:t`→chữ, `w:br`/`w:cr`→`\n`, `w:tab` là con của `w:r`→`\t`; bỏ chữ của đoạn lồng).
  `pub fn docx_texts(bytes) -> BTreeMap<String, Vec<String>>` cho document/header*/footer*.
  Đặt ở `src/inspect.rs` (public để cf-services dùng cho so sánh).
- **File:** `src/inspect.rs`
- **Test:** với mỗi file `fixtures/docx/expected_*.docx` (do Python sinh): `docx_texts(file)` == `expected_texts`
  của scenario cùng tên trong `docx_render.json`.
- **Xong khi:** `cargo test -p cf-docx inspect`.
- **Phụ thuộc:** T2.02

### T2.04 — Tiện ích dựng docx tối giản cho test · XS
- [ ] Xong
- **Làm:** trong `cf-testkit`: `pub fn make_docx(document_body_xml: &str) -> Vec<u8>` tạo ZIP hợp lệ tối thiểu
  (`[Content_Types].xml`, `_rels/.rels`, `word/document.xml` với namespace `w`, `word/_rels/document.xml.rels`).
- **File:** `crates/cf-testkit/src/docx.rs`
- **Test:** kết quả đọc lại được bằng `Package::read`.
- **Xong khi:** `cargo test -p cf-testkit`.
- **Phụ thuộc:** T2.01

### T2.05 — Đoạn chữ của một `w:p` · S
- [ ] Xong
- **Làm:** `para::text_nodes(p) -> Vec<NodePath>`: các `w:t` của chính đoạn (qua `w:r` trực tiếp hoặc trong
  `w:hyperlink`, `w:ins`, `w:smartTag`, `w:sdt/w:sdtContent`), bỏ những `w:t` có tổ tiên `w:p` khác gần hơn.
  `para::joined_text(p)` + bảng ánh xạ vị trí ký tự → (chỉ số nút, offset **theo char**).
- **File:** `src/para.rs`
- **Test (unit, dùng `make_docx`):** đoạn có hyperlink chứa run → được tính; textbox lồng → không tính vào đoạn ngoài.
- **Xong khi:** `cargo test -p cf-docx para`.
- **Phụ thuộc:** T2.02, T2.04

### T2.06 — Quét placeholder thành Slot · XS
- [ ] Xong
- **Làm:** regex `\{\{(\??)([A-Z][A-Z0-9_]*)\}\}`; `scan(p) -> Vec<Slot{key, optional, start, end}>`.
- **File:** `src/slot.rs`
- **Test (unit):** `{{A}}{{A}}` → 2 slot; `{{ A }}`, `{{a}}` → 0; `{{?A}}` → optional; placeholder cắt qua 3 `w:t` → start/end đúng nút.
- **Xong khi:** `cargo test -p cf-docx slot`.
- **Phụ thuộc:** T2.05

### T2.07 — Thay một slot (KB-04) · S
- [ ] Xong
- **Làm:** `replace_slot(p, slot, value)` theo `04_DOCX_ENGINE.md` §5 bước 4 mục 2–3, 5. Không xóa run, không đụng `w:rPr`.
  Đặt `xml:space="preserve"` khi cần.
- **File:** `src/replace.rs`
- **Test (unit):** run `{{` thường + `PARTY_A` đậm + `_NAME}}` thường → sau thay: run đậm vẫn còn (chữ rỗng),
  chữ đầy đủ đúng; chữ trước/sau placeholder trong cùng `w:t` giữ nguyên.
- **Xong khi:** `cargo test -p cf-docx replace`.
- **Phụ thuộc:** T2.06

### T2.08 — Xuống dòng và tab trong giá trị · XS
- [ ] Xong
- **Làm:** giá trị có `\r\n`/`\n` → `w:br`, `\t` → `w:tab`, chèn trong cùng run (tách `w:t`).
- **File:** `src/replace.rs`
- **Test:** đoạn `"Nhiều dòng: ..."` trong scenario `lists_full` cho ra `"Nhiều dòng: Dòng 1\nDòng 2\tsau tab"`.
- **Xong khi:** `cargo test -p cf-docx replace::newline`.
- **Phụ thuộc:** T2.07

### T2.09 — `Template::compile` + `render` (chỉ thay giá trị) · M
- [ ] Xong
- **Làm:** API `04_DOCX_ENGINE.md` §1 (`RenderData`); compile: parse các phần chữ; render: clone cây, thay mọi slot
  ở mọi `w:p` mọi độ sâu (KB-03), slot của khóa không có → giữ nguyên; ghi ZIP mới — entry không phải
  phần chữ **chép raw** (`raw_copy_file`), giữ thứ tự entry.
- **File:** `src/template.rs`, `src/lib.rs`, `tests/render_values.rs`
- **Test:** (a) render `synthetic_all_traps.docx` với `context` của scenario `lists_none`, `legacy_party_a_rows=false`:
  header = `"Đầu trang 01/TYTSONGXANH/2026"`, footer = `"Chân trang Trạm Y tế xã Sông Xanh"`, đoạn
  `"Bên A: Trạm Y tế xã Sông Xanh — ký ngày 05/01/2026"`, `"Hai khóa liền nhau: 1212 tháng"`,
  khóa lạ giữ nguyên, `"Lồng: 01/TYTSONGXANH/2026"` (KB-03). (So sánh **toàn bộ** scenario làm ở T2.13.)
  (b) mọi entry không phải phần chữ có bytes giống mẫu. (c) file ra `compile` lại được.
- **Xong khi:** `cargo test -p cf-docx --test render_values`.
- **Phụ thuộc:** T2.08

### T2.10 — Khóa của chính một dòng bảng · XS
- [ ] Xong
- **Làm:** `row_keys(tr) -> BTreeSet<(key, optional)>` chỉ tính chữ thuộc các ô của dòng này (bỏ bảng lồng trong ô).
- **File:** `src/table.rs`
- **Test (unit):** ô chứa bảng lồng có `ITEM_NAME` → dòng ngoài **không** có khóa đó.
- **Xong khi:** `cargo test -p cf-docx table::row_keys`.
- **Phụ thuộc:** T2.06

### T2.11 — Nhân dòng lặp · S
- [ ] Xong
- **Làm:** bước 1 của §5 (không gồm bỏ bảng). Áp cho mọi `w:tbl` mọi độ sâu. Chạy trước khi thay giá trị chung.
- **File:** `src/table.rs`, `src/template.rs`, `tests/render_lists.rs`
- **Test:** render với `lists` của scenario `lists_full`: bảng thứ 2 có 4 dòng, chữ các dòng
  `["1","Gói A","24","1.000"]`, `["2","Gói B","12","2.000"]`, dòng tổng `"11.639.998"`; bảng features có `["1","Quản lý"]`.
- **Xong khi:** `cargo test -p cf-docx --test render_lists full`.
- **Phụ thuộc:** T2.09, T2.10

### T2.12 — Bỏ bảng rỗng (KB-05) và `omit_empty_tables` · XS
- [ ] Xong
- **File:** `src/table.rs`, `tests/render_lists.rs`
- **Test:** với `lists` rỗng: bảng dòng lặp còn 2 dòng (tiêu đề + tổng), bảng features biến mất (KB-05);
  với `omit_empty_tables={"features"}`: bảng features biến mất. Đếm bảng cấp 1 bằng `inspect`.
- **Xong khi:** `cargo test -p cf-docx --test render_lists empty`.
- **Phụ thuộc:** T2.11

### T2.13 — Chế độ tương thích dòng Bên A · S
- [ ] Xong
- **Làm:** `04_DOCX_ENGINE.md` §6, chỉ bảng **cấp 1 của thân**, chạy trước bước thay giá trị.
- **Nguồn:** `ContractForge/app/services/docx_renderer.py::_insert_party_a_optional_rows, _set_cell_text`
- **File:** `src/legacy_party_a.rs`, `src/template.rs`, `tests/render_lists.rs`
- **Test:** **mọi** scenario của `docx_render.json` khớp toàn bộ (trừ KB-03/KB-05), gồm `no_party_a_optional`.
- **Xong khi:** `cargo test -p cf-docx --test render_lists`.
- **Phụ thuộc:** T2.12

### T2.14 — Dòng tùy chọn `{{?KHOA}}` (KB-06) · S
- [ ] Xong
- **Làm:** §5 bước 2 (dòng bảng và đoạn ngoài bảng). `{{?K}}` còn lại được thay như `{{K}}`.
- **File:** `src/optional.rs`, `src/template.rs`, `tests/render_optional.rs`
- **Test (dùng `make_docx`):** bảng 3 dòng `{{?A}}`, `{{?B}}`, `{{?C}}` với A="x", B="", C thiếu → còn 1 dòng chữ "x";
  dòng có `{{?A}}` và `{{?B}}`, A rỗng B có → giữ; đoạn ngoài bảng `{{?A}}` rỗng → đoạn bị xóa.
- **Xong khi:** `cargo test -p cf-docx --test render_optional`.
- **Phụ thuộc:** T2.11

### T2.15 — Phân tích mẫu `analysis()` · S
- [ ] Xong
- **Làm:** `04_DOCX_ENGINE.md` §7. `mapping` mặc định cần `CANONICAL_MAP` → `cf-docx` **không** phụ thuộc cf-core:
  trả `placeholders`, để cf-services dựng mapping. `uses_legacy_party_a`, `optional_keys` mới.
- **Nguồn:** `ContractForge/app/services/template_analyzer.py::analyze`
- **File:** `src/analysis.rs`, `tests/analysis.rs`
- **Test:** `template_analysis.json → results`: khớp `placeholders`, `repeat_groups`, `row_warnings`, `error`
  (không so `source_counts`, `split_run_warnings` — khác do KB-03).
- **Xong khi:** `cargo test -p cf-docx --test analysis`.
- **Phụ thuộc:** T2.09, T2.10

### T2.16 — Mẫu thật trong repo và báo giá tự nhập · XS
- [ ] Xong
- **File:** `tests/render_samples.rs`
- **Test:** `docx_render.json → samples` (render `sample_service_contract.docx`, `sample_form_08a.docx` với `context`/`lists`)
  và `manual_quotation` (`sample_quote_features.docx`, `omit_empty_tables={"features"}`).
- **Xong khi:** `cargo test -p cf-docx --test render_samples`.
- **Phụ thuộc:** T2.13

### T2.17 — `write_new_file` · XS
- [ ] Xong
- **Làm:** §8: `create_new`, hậu tố `_2`, `_3`…, `sync_all`, lỗi khi ghi → xóa file vừa tạo. Không dùng `rename`.
- **File:** `src/write.rs`
- **Test (unit, tempfile):** ghi 3 lần cùng tên → `a.docx`, `a_2.docx`, `a_3.docx`; nội dung file cũ không đổi.
- **Xong khi:** `cargo test -p cf-docx write`.
- **Phụ thuộc:** T2.01

### T2.18 — `TemplateCache` · XS
- [ ] Xong
- **Làm:** khóa `(PathBuf, len, mtime)`, giá trị `Arc<Template>`, tối đa 32 mục (bỏ mục dùng lâu nhất).
  `get_or_compile(path) -> Result<Arc<Template>>`.
- **File:** `src/cache.rs`
- **Test (unit):** gọi 2 lần → `Arc::ptr_eq`; sửa file (đổi nội dung) → biên dịch lại.
- **Xong khi:** `cargo test -p cf-docx cache`.
- **Phụ thuộc:** T2.09

### T2.19 — CLI `cf-render` và kiểm bằng Word · S
- [ ] Xong
- **Làm:** `src/bin/cf-render.rs`: `cf-render <mẫu.docx> <data.json> <thư_mục_ra> [tên]` — `data.json` dạng
  `{"values":{}, "lists":{}|null, "omit_empty_tables":[]}`; in đường dẫn file ra. Sinh 5 file từ 5 scenario
  vào `contract_rust/target/docx-check/` để chủ dự án mở bằng Word.
- **File:** `src/bin/cf-render.rs`
- **Xong khi:** chạy được; **ĐD2**: chủ dự án mở 5 file bằng Word không có cảnh báo, định dạng đúng.
- **Phụ thuộc:** T2.13, T2.14, T2.17

---

## Điểm kiểm Đợt 2
- [ ] `cargo test -p cf-docx` xanh
- [ ] `cargo tree -p cf-docx` không có cf-core, cf-db, rusqlite, slint
- [ ] ĐD2 đã qua
