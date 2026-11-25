# 07 — Kiểm thử và nghiệm thu

## 1. Fixture vàng

Sinh bằng: `.venv\Scripts\python.exe contract_rust\tools\gen_fixtures.py` (từ thư mục gốc repo).
Script trỏ `CONTRACTFORGE_DATA_ROOT` vào thư mục tạm → **không đụng DB thật**. Chỉ dùng dữ liệu hư cấu.

| File | Kiểm cái gì | Crate |
|---|---|---|
| `schema.json`, `schema_baseline.sql` | Cột bắt buộc, DDL cài mới, sản phẩm seed | cf-db |
| `amount_to_words.json` | Đọc số thành chữ (+ KB-01) | cf-core::words |
| `names.json` | Slug, làm sạch tên file, tên file đầu ra | cf-core::names |
| `dates.json` | Cộng tháng, ngày kết thúc, số tháng, đọc ngày | cf-core::dates |
| `formatting.json` | Định dạng tiền/số/VAT, đọc số từ form, số kiểu báo giá tự nhập | cf-core::fmt, parse |
| `pricing.json` | Làm tròn, chuẩn hóa SL, số kỳ, dòng, tổng, mô tả, số tháng gói bổ sung | cf-core::pricing |
| `lifecycle.json` | Tập trạng thái, ma trận chuyển (kèm chứng từ), mức khẩn hết hạn | cf-core::lifecycle |
| `numbering.json` | Kiểm khuôn, ghép số, số thứ tự, khuôn phụ lục (+ KB-02) | cf-core::numbering |
| `template_rules.json` | Vai trò mẫu, kiểm mẫu theo nhóm, suy loại mẫu, tick sẵn | cf-core::template_rules |
| `render_context.json` | Đủ mọi khóa ngữ cảnh cho 4 tình huống, ngữ cảnh 08a, mapping, dòng lặp | cf-core::render_context |
| `docx_render.json` + `docx/*.docx` | Engine docx 5 tình huống + 2 mẫu thật trong repo + báo giá tự nhập (+ KB-03, KB-05) | cf-docx |
| `template_analysis.json` | Phân tích placeholder | cf-docx |
| `intentional_differences.json` | Kết quả mong muốn ở các chỗ khác Python | mọi crate |

### Cách viết test fixture (mẫu cho mọi task)

```rust
// crates/cf-core/tests/fixture_words.rs
use serde_json::Value;
fn fixture(name: &str) -> Value {
    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/");
    serde_json::from_str(&std::fs::read_to_string(format!("{p}{name}.json")).unwrap()).unwrap()
}
#[test]
fn amount_to_words_matches_python() {
    let f = fixture("amount_to_words");
    for case in f["cases"].as_array().unwrap() {
        // case["words"] là {"ok": "..."} hoặc {"err": "...", "msg": "..."}
        ...
    }
}
```

- Lỗi Python (`{"err", "msg"}`) ↔ `Err(e)` với `e.to_string() == msg` (câu tiếng Việt).
- `Decimal` trong fixture là **chuỗi**; so sánh bằng `Decimal::from_str` (không so chuỗi,
  vì `"10.00"` và `"10"` bằng nhau về giá trị) — **trừ** các hàm định dạng trả chuỗi.
- Một test cho mỗi khóa cấp 1 của file fixture, đặt tên `<khóa>_matches_python`.

## 2. Các lớp test

| Lớp | Ở đâu | Chạy bằng |
|---|---|---|
| Fixture vàng | `crates/*/tests/fixture_*.rs` | `cargo test` |
| Đơn vị (luật mới, KB) | `#[cfg(test)]` trong từng module | `cargo test` |
| Store/DB | `crates/cf-db/tests/` — DB tạm từ `schema_baseline.sql` | `cargo test -p cf-db` |
| Ca sử dụng | `crates/cf-services/tests/` — DB tạm + thư mục tạm (`tempfile`) | `cargo test -p cf-services` |
| Tương thích hai chiều với Python | `tools/compat_check.py` (Đợt 3) | thủ công, xem §3 |
| UI | Kiểm bằng tay theo danh sách §4 | thủ công |

Quy tắc: `cargo test --workspace` và `cargo clippy --workspace -- -D warnings` phải xanh trước
mỗi commit.

## 3. Tương thích DB hai chiều (Đợt 3)

Trên **bản sao** DB (không bao giờ DB thật):
1. Sao DB bằng sqlite backup API vào thư mục tạm; **viết lại mọi cột đường dẫn** sang thư mục tạm
   (bản sao giữ đường dẫn tuyệt đối vào dữ liệu thật — sinh file trên bản sao có thể xóa file thật).
2. Rust: tạo khách hàng, hợp đồng items, phụ lục, upload chứng từ, đổi trạng thái → đóng.
3. Python (`CONTRACTFORGE_DATA_ROOT` = thư mục tạm): mở các trang danh sách/chi tiết, sinh lại file
   → không lỗi; các con số tiền giống hệt.
4. Ngược lại: Python tạo dữ liệu → Rust nạp `Store` không lỗi, hiển thị đúng.
5. So hash DB thật trước/sau: phải giống hệt.

## 4. Nghiệm thu cuối (trước khi xóa bản Python)

- [ ] Toàn bộ test xanh; clippy sạch.
- [ ] RAM ≤ 80 MB khi mở danh sách hợp đồng; khởi động < 1 s; không có tiến trình WebView2; không mở cổng.
- [ ] Gõ tiếng Việt bằng Unikey/EVKey ở mọi ô nhập.
- [ ] Trên bản sao DB thật: sinh lại 10 hợp đồng thật bằng cả hai bản → so văn bản từng đoạn
      (quy tắc trích của `04_DOCX_ENGINE.md` §10) → chỉ khác ở các mục KB-03/KB-05.
- [ ] Mở các file sinh ra bằng Word: không có cảnh báo hỏng file; định dạng đúng.
- [ ] Dashboard: mọi con số bằng bản Python trên cùng bản sao DB (trừ KB-08 nếu có hợp đồng vừa qua hạn).
- [ ] Báo cáo Excel khách hàng: đối chiếu tay 5 khách hàng với phân hạng mới (KB-09).
- [ ] `python scripts/check_public_repo.py` sạch với toàn bộ `contract_rust/`.
