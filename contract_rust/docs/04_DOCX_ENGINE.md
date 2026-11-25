# 04 — Engine sinh docx (`cf-docx`)

Đây là phần **được phép thay đổi** so với bản Python (chủ dự án chốt). Mục tiêu:

1. Giữ nguyên **mọi byte** của mẫu ngoài chỗ điền (style, hình, numbering, theme…).
2. Thay placeholder ở **mọi chỗ có chữ**, kể cả khi Word cắt placeholder thành nhiều run, và
   **không làm mất định dạng** của phần chữ còn lại.
3. Sinh hàng loạt nhanh: **biên dịch mẫu một lần**, render nhiều lần.
4. Không bao giờ ghi đè file có sẵn; không để lại file dở dang.

## 1. API công khai

```rust
pub struct Template { /* đã biên dịch, bất biến, Send + Sync */ }

impl Template {
    /// Đọc .docx (bytes), giải nén, phân tích các phần chứa chữ. Lỗi → DocxError::Open.
    pub fn compile(bytes: &[u8]) -> Result<Template, DocxError>;
    /// Danh sách placeholder, nhóm dòng lặp, dòng tùy chọn, cảnh báo (dùng cho trang Mẫu).
    pub fn analysis(&self) -> Analysis;
    /// Điền dữ liệu, trả bytes .docx mới. Không đụng đĩa.
    pub fn render(&self, data: &RenderData) -> Result<Vec<u8>, DocxError>;
}

pub struct RenderData {
    pub values: BTreeMap<String, String>,          // KHÓA → giá trị (đã là chuỗi hiển thị)
    pub lists: Option<BTreeMap<String, Vec<BTreeMap<String, String>>>>, // "items"/"features" → dòng
    pub omit_empty_tables: BTreeSet<String>,        // ví dụ {"features"}
    pub legacy_party_a_rows: bool,                  // chế độ tương thích (mục 6)
}

/// Ghi bytes ra thư mục, tên không trùng. Xem mục 8.
pub fn write_new_file(dir: &Path, file_name: &str, bytes: &[u8]) -> io::Result<(PathBuf, String)>;
```

`cf-docx` **không biết** hợp đồng, khách hàng hay DB là gì.

## 2. Phần nào trong gói .docx được xử lý

Gói .docx là ZIP. Các phần **chứa chữ** cần xử lý (khớp theo tên trong ZIP):

| Phần | Khớp |
|---|---|
| Thân | `word/document.xml` |
| Đầu/chân trang (mọi loại: mặc định, trang đầu, trang chẵn) | `word/header*.xml`, `word/footer*.xml` |
| Chú thích | `word/footnotes.xml`, `word/endnotes.xml` |

Mọi phần khác (ảnh, `styles.xml`, `[Content_Types].xml`, …) được **chép nguyên bytes đã nén**
(`ZipWriter::raw_copy_file`), không giải nén lại. Thứ tự entry trong ZIP giữ nguyên như mẫu.

Trong các phần chứa chữ, xử lý **mọi** `<w:p>` ở mọi độ sâu: trong bảng, bảng lồng, textbox
(`w:txbxContent`), `mc:AlternateContent` (cả `mc:Choice` lẫn `mc:Fallback`). **[KB-03]**

## 3. Placeholder

- Cú pháp chuẩn: `{{KHOA}}` với `KHOA` khớp `[A-Z][A-Z0-9_]*`. Khoảng trắng bên trong
  (`{{ KHOA }}`) hoặc chữ thường → **không phải placeholder**, giữ nguyên.
- Cú pháp dòng tùy chọn: `{{?KHOA}}` — xem mục 5. Regex chung: `\{\{(\??)([A-Z][A-Z0-9_]*)\}\}`.
- Khóa không có trong `values` → **giữ nguyên** `{{KHOA}}` trong file ra (để người dùng thấy thiếu gì).
- Giá trị `None` từ phía gọi đã được đổi thành `""` trước khi vào engine.

## 4. Biên dịch (`compile`) — giải thuật

Với mỗi phần chứa chữ:

1. Parse XML thành cây (dùng `quick-xml` đọc sự kiện, dựng cây node tối giản: element có tên, thuộc
   tính, con; text; giữ nguyên mọi thứ khác như raw). Khi ghi ra phải tái tạo XML **tương đương**
   (khai báo namespace, thuộc tính, `xml:space` giữ nguyên).
2. Với mỗi `<w:p>`: lập danh sách **đoạn chữ** của chính đoạn đó theo thứ tự tài liệu:
   các `<w:t>` thuộc `<w:r>` mà tổ tiên `<w:p>` gần nhất là đoạn này (bỏ qua chữ của đoạn lồng
   bên trong textbox). Bao gồm run nằm trong `w:hyperlink`, `w:ins`, `w:smartTag`, `w:sdtContent`.
3. Nối text các `<w:t>` → chuỗi `S`, giữ bảng ánh xạ vị trí ký tự → (chỉ số `<w:t>`, offset).
4. Tìm mọi match của regex placeholder trong `S`. Mỗi match lưu thành **Slot**:
   `{ key, optional: bool, start: (t_idx, off), end: (t_idx, off) }`.
5. Ghi nhận cấu trúc bảng: với mỗi `<w:tr>`, tập khóa của **chính dòng đó** (chữ thuộc các ô của
   dòng, không tính bảng lồng bên trong ô) → dùng cho dòng lặp và dòng tùy chọn.

Kết quả biên dịch được giữ trong `Template`; `render` chỉ làm việc trên bản sao của cây.

## 5. Render — thứ tự bắt buộc

Cho mỗi phần chứa chữ, trên **bản sao** cây đã biên dịch:

**Bước 1 — Dòng lặp** (chỉ khi `lists` là `Some`; `None` = không đụng bảng nào):
- Một `<w:tr>` là **dòng mẫu** của nhóm `items` nếu tập khóa của chính dòng có khóa bắt đầu bằng
  `ITEM_`; nhóm `features` nếu có khóa bắt đầu bằng `FEATURE_` (xét `ITEM_` trước).
- Bỏ cả bảng: nếu nhóm nằm trong `omit_empty_tables`, danh sách của nhóm rỗng, và **mọi** khóa
  của bảng (chỉ tính các dòng của chính bảng) đều thuộc nhóm đó → xóa `<w:tbl>`.
- Ngược lại: với mỗi phần tử của danh sách, nhân bản dòng mẫu (deep clone), chèn liên tiếp ngay
  sau, thay các khóa của phần tử đó (theo mục 5 bước 4) trong dòng mới; khóa khác giữ nguyên để
  bước 4 xử lý. Cuối cùng xóa dòng mẫu.
- **[KB-05]** Bảng còn **0 dòng** sau bước này → xóa luôn `<w:tbl>` (bảng 0 dòng là OOXML hỏng).

**Bước 2 — Dòng tùy chọn** **[KB-06]**:
- Dòng `<w:tr>` mà tập khóa của chính dòng có ít nhất một `{{?KHOA}}`: nếu **mọi** khóa tùy chọn
  trong dòng có giá trị rỗng (sau `trim`) hoặc không có trong `values` → xóa dòng. Ngược lại giữ
  dòng; `{{?KHOA}}` được thay như `{{KHOA}}`.
- `{{?KHOA}}` nằm ngoài bảng → áp cho **đoạn** `<w:p>` chứa nó (xóa đoạn nếu rỗng).

**Bước 3 — Chế độ tương thích dòng Bên A** (chỉ khi `legacy_party_a_rows = true`, xem mục 6).

**Bước 4 — Thay placeholder**, cho mỗi Slot còn trong cây (theo thứ tự từ cuối đoạn lên đầu để
offset không lệch):
1. Giá trị thay = `values[key]`, không có → bỏ qua Slot này (giữ nguyên chữ).
2. Placeholder nằm gọn trong một `<w:t>` → thay chuỗi con.
3. Placeholder trải qua nhiều `<w:t>` → **chỉ** các `<w:t>` bị phủ bị sửa: `<w:t>` đầu giữ phần
   trước placeholder + giá trị thay; các `<w:t>` ở giữa → rỗng; `<w:t>` cuối giữ phần sau
   placeholder. **Không xóa run**, không đụng `<w:rPr>` → mọi định dạng còn nguyên. **[KB-04]**
   Định dạng của giá trị thay = định dạng của run chứa `{{`.
4. Giá trị có `\n` hoặc `\r\n` → tách: các đoạn chữ ngăn bởi `<w:br/>`; `\t` → `<w:tab/>`
   (khớp python-docx, xem fixture `docx_render.json`, đoạn "Nhiều dòng"). Chèn các phần tử này
   vào **cùng run**, ngay sau `<w:t>` tương ứng, tách `<w:t>` khi cần.
5. Mọi `<w:t>` đã sửa có chữ bắt đầu/kết thúc bằng khoảng trắng → đặt `xml:space="preserve"`.
6. Ký tự đặc biệt (`& < > "`) được escape khi ghi XML (quick-xml làm tự động).

**Bước 5 — Ghi gói**: ZIP mới = các entry không đổi chép raw + các phần chứa chữ đã serialize
(nén deflate). Trả bytes.

## 6. Chế độ tương thích dòng Bên A (mẫu cũ)

Bật `legacy_party_a_rows` khi mẫu **không có** `{{?...}}` nào (tự động, do `cf-services` quyết).
Hành vi y như Python `_insert_party_a_optional_rows`:

- Danh sách (nhãn, khóa) theo thứ tự: Mã số thuế/`PARTY_A_TAX_CODE`, Số tài khoản/`PARTY_A_BANK_ACCOUNT`,
  Ngân hàng/`PARTY_A_BANK_NAME`, Tên thụ hưởng/`PARTY_A_BENEFICIARY`, Mã QHNS/`PARTY_A_BUDGET_CODE`;
  chỉ lấy khóa có giá trị khác rỗng sau `trim`. Không có → dừng.
- Bảng đích: bảng **cấp 1 của thân** có ô (0,0) chứa chữ `PARTY_A_NAME` (chạy **trước** bước 4).
- Dòng neo: dòng đầu tiên có ô 0 = "Chức vụ" hoặc "Chức danh" (trim); không có → dòng cuối.
  Dòng khuôn = dòng ngay trên dòng neo.
- Mỗi trường: clone dòng khuôn; 3+ ô → (nhãn, ":", giá trị); 2 ô → (nhãn, giá trị); mỗi ô xóa hết
  run rồi tạo một run mới, chép `<w:rPr>` của **run đầu tiên** trong dòng khuôn; chèn nối tiếp
  sau dòng neo.

Hướng dẫn cho người dùng (đưa vào trang Mẫu): thay cơ chế này bằng 5 dòng có sẵn trong mẫu, ví dụ
`Mã số thuế | : | {{?PARTY_A_TAX_CODE}}`.

## 7. Phân tích mẫu (`analysis`)

Trả về (fixture `template_analysis.json`, cộng phần mới):
- `placeholders`: sắp xếp, không trùng (khóa của cả `{{K}}` và `{{?K}}`).
- `split_run_warnings`: chữ (120 ký tự đầu, trim) của các đoạn có placeholder bị cắt qua nhiều `<w:t>`.
  Engine mới xử lý được nên chỉ là **thông tin**, không còn là cảnh báo đỏ.
- `source_counts`: đếm theo `para` (đoạn thân ngoài bảng), `table`, `header`, `footer`.
- `repeat_groups`: các nhóm dòng lặp có trong bảng.
- `row_warnings`: `` "`{{ITEM_X}}` phải nằm trong một dòng bảng" `` khi placeholder dòng lặp nằm ngoài bảng.
- **Mới:** `optional_keys` — các khóa dùng dạng `{{?K}}`; `uses_legacy_party_a` — mẫu có ô chứa
  `PARTY_A_NAME` ở bảng cấp 1 và không có `{{?...}}`.
- Lỗi mở file → `error = "Không mở được file .docx: ..."`.

Chênh lệch so với fixture Python do phạm vi quét rộng hơn (bảng lồng, header trang đầu…) được
liệt kê ở `fixtures/intentional_differences.json`.

## 8. Luồng ghi file: "copy mẫu → đặt tên → điền" (QĐ-4)

Ý tưởng của chủ dự án được hiện thực an toàn như sau:

```
1. Đọc mẫu (bytes)                         ← "copy mẫu" (trong bộ nhớ, không chạm file gốc)
2. Template::compile (hoặc lấy từ cache)
3. Chọn tên đích theo quy tắc nghiệp vụ    ← "đổi tên"
4. Template::render(data) → bytes          ← "điền placeholder"
5. write_new_file(dir, tên, bytes):
     for n in 1.. {
        tên_thử = n == 1 ? tên : "{gốc}_{n}{đuôi}"
        mở với OpenOptions::new().write(true).create_new(true)   // độc quyền, không ghi đè
        AlreadyExists → thử n+1; lỗi khác → trả lỗi
        ghi bytes, flush, sync_all; lỗi khi ghi → xóa file vừa tạo, trả lỗi
        trả (đường dẫn, tên_thử)
     }
```

Vì sao không "copy file mẫu ra đĩa rồi sửa tại chỗ":
- .docx là ZIP, không sửa tại chỗ được; kiểu gì cũng phải ghi ra một ZIP mới.
- Nếu copy trước rồi điền sau, lỗi giữa chừng sẽ để lại một file mang tên hợp đồng nhưng nội dung
  là mẫu trống. Cách trên đảm bảo: **file mang tên chính thức chỉ xuất hiện khi đã đầy đủ**.
- **Không dùng `std::fs::rename`** để đặt tên cuối: trên Windows nó ghi đè file đích nếu đã tồn tại.

## 9. Cache biên dịch

`cf-services` giữ `HashMap<(PathBuf, u64 size, SystemTime mtime), Arc<Template>>`, tối đa 32
mục (bỏ mục cũ nhất). Sinh hàng loạt N khách = 1 lần compile + N lần render.

## 10. Kiểm thử engine

- `fixtures/docx/synthetic_all_traps.docx` + `fixtures/docx_render.json`: 5 tình huống (không có
  dòng lặp, đủ dòng, dòng rỗng, dòng rỗng + bỏ bảng features, không có trường Bên A tùy chọn).
- So sánh theo **quy tắc trích văn bản** ghi trong `docx_render.json → text_extraction_rule`:
  mỗi phần XML → danh sách chuỗi, mỗi `<w:p>` một chuỗi (`w:t` → chữ, `w:br`/`w:cr` → `\n`,
  `w:tab` trong run → `\t`). Viết hàm này một lần trong `cf-docx/tests/common.rs`.
- Kỳ vọng = fixture, **trừ** các mục trong `intentional_differences.json`.
- Thêm test riêng của Rust: (a) mở lại file ra bằng chính `Template::compile` không lỗi;
  (b) mọi entry không phải phần chữ có bytes giống hệt mẫu; (c) định dạng: placeholder bị cắt có
  run in đậm ở giữa → sau render run đó vẫn tồn tại với `<w:b/>`; (d) `{{?K}}` xóa đúng dòng.
- Kiểm bằng mắt (thủ công, Đợt 2): mở 5 file ra bằng Word, không có hộp thoại "nội dung không đọc được".
