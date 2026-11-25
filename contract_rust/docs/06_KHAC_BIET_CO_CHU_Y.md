# 06 — Khác biệt có chủ ý so với bản Python

Mỗi mục là chỗ bản Rust **cố ý** cho kết quả khác bản Python: sửa lỗi hoặc nâng cấp giải thuật.
Chủ dự án đã đồng ý **nguyên tắc** "giữ nghiệp vụ, nâng cấp giải thuật" và ba quyết định cụ thể
(25/11/2026): Store trong RAM, dòng tùy chọn `{{?KHOA}}` (KB-06), sửa phân hạng báo cáo (KB-09).
**Các mục còn lại đang chờ chủ dự án duyệt từng dòng** — thêm chữ "✔ đã duyệt" vào cột "Loại" khi xong; mục bị bác
thì chuyển về hành vi Python và xóa ca tương ứng khỏi fixture.

Mục có ⚙ có dữ liệu kiểm trong [`fixtures/intentional_differences.json`](../fixtures/intentional_differences.json)
(file này **do `tools/gen_fixtures.py` sinh**: `python_result` từ code Python thật, `rust_expected` từ bản
cài tham chiếu trong script — không sửa tay).

KB-02 đã được đo trên bản sao chỉ-đọc của DB thật (25/11/2026): với mọi khuôn số sản phẩm đang dùng,
gợi ý số mới **trùng 100%** với cũ; khác biệt chỉ ở khuôn mặc định (gói chưa cấu hình khuôn), nơi luật
cũ đếm lẫn số của khuôn khác. Mọi số hợp đồng thật đều khớp regex mới.

Ngoài danh sách này, bản Rust phải ra **y hệt** bản Python.

| Mã | Khu vực | Bản Python | Bản Rust | Loại |
|---|---|---|---|---|
| KB-01 ⚙ | Số tiền bằng chữ | Số ≥ 1.000 tỷ ném `IndexError` (dù chú thích ghi hỗ trợ tới 999 nghìn tỷ) | Phần tỷ ≥ 1000 đọc đệ quy bằng chính hàm này rồi thêm " tỷ". Giới hạn trên `u64` | Sửa lỗi |
| KB-02 ⚙ | Gợi ý số hợp đồng | Đổi khuôn thành `LIKE` (`{seq}`,`{slug}` → `%`) rồi lấy số ở phần trước `/` đầu tiên. Khuôn `HD{seq}` luôn ra 01 (rồi trùng); khuôn khác nhau có thể đếm chung | Khuôn → regex khớp nguyên chuỗi: `{seq}`→`(?P<seq>\d+)`, `{slug}`→`[A-Za-z0-9]*`, `{year}`→năm, phần còn lại escape. Vẫn lấp khoảng trống, vẫn đếm theo (khách, khuôn, năm) | Sửa lỗi + nâng cấp |
| KB-03 ⚙ | Phạm vi điền docx | Chỉ đoạn thân, ô bảng cấp 1, header/footer **mặc định** | Mọi `<w:p>` ở mọi độ sâu, trong thân, mọi header/footer, footnote, endnote, textbox | Nâng cấp |
| KB-04 | Placeholder bị cắt run | Gộp **cả đoạn** vào run đầu → mất in đậm/nghiêng của phần chữ khác trong đoạn | Chỉ sửa các `<w:t>` bị placeholder phủ; không xóa run nào | Nâng cấp |
| KB-05 ⚙ | Danh sách dòng lặp rỗng | Xóa dòng mẫu, để lại `<w:tbl>` 0 dòng (OOXML không hợp lệ) | Bảng còn 0 dòng → xóa bảng | Sửa lỗi |
| KB-06 | Dòng tùy chọn | Dò bảng có `PARTY_A_NAME`, tìm dòng "Chức vụ", chèn 5 dòng với nhãn viết cứng | Cú pháp khai báo `{{?KHOA}}`: dòng tự xóa khi mọi khóa tùy chọn của nó rỗng. Cơ chế cũ giữ làm **chế độ tương thích**, chỉ bật khi mẫu không có `{{?...}}` nào | Nâng cấp ✔ đã duyệt |
| KB-07 | Tự chuyển trạng thái khi upload | Router gán thẳng `status`, mô tả sự kiện riêng "Tự động chuyển X → Y sau khi upload ..." | Đi qua hàm chuyển trạng thái duy nhất; sự kiện `status_changed`, actor `system`, mô tả `"Chuyển trạng thái X → Y: tự động sau khi upload {scan PDF \| hóa đơn \| ủy nhiệm chi}"`. Bước chuyển giống hệt | Nhất quán |
| KB-08 | Quét hết hạn | Chạy khi mở dashboard / trang sắp hết hạn (và lúc khởi động) | Chạy lúc khởi động và mỗi lần qua 00:00 giờ địa phương; mọi màn hình luôn thấy trạng thái đúng | Sửa lỗi |
| KB-09 | Báo cáo Excel khách hàng | Tự định nghĩa "đang hiệu lực" = `{Active, ExpiringSoon}` (đã không còn dữ liệu) → phân hạng sai; bảng nhãn thiếu Invoiced/PaidActive | Dùng tập của `lifecycle` (`ACTIVE_LIFECYCLE`…) và `STATUS_LABELS`. **Con số báo cáo sẽ khác** | Sửa lỗi ✔ đã duyệt |
| KB-10 | Ngày kết thúc trên form | JS `setMonth` tràn tháng (31/01 + 1 tháng − 1 ngày = 02/03); server kẹp cuối tháng | Form gọi `compute_end_date` của core → một kết quả duy nhất (27/02 hoặc 28/02) | Sửa lỗi |
| KB-11 | Tìm kiếm | Server `ILIKE` phân biệt dấu; chỉ ô lọc khách hàng (JS) bỏ dấu | Mọi tìm kiếm (danh sách HĐ, khách hàng, toàn cục) so trên `search_key` không dấu | Nâng cấp |
| KB-12 | Nhập mẫu cục bộ khi khởi động | Băm SHA-256 lại mọi file mẫu mỗi lần khởi động; tự xóa file mẫu mồ côi | Cache băm trong phiên; dọn file mồ côi chỉ khi bấm nút | Nâng cấp |
| KB-13 | Sinh hàng loạt | Đồng bộ trong một request, không tiến độ, đọc lại mẫu cho từng khách | Chạy nền, có tiến độ + Hủy; biên dịch mẫu một lần. Hủy giữa chừng: các hợp đồng đã xong giữ nguyên, ZIP gồm các file đã xong | Nâng cấp |
| KB-14 | Xuất thông tin khách hàng (.md) | Bảng nhãn trạng thái riêng, thiếu Invoiced/PaidActive | Dùng `STATUS_LABELS` | Sửa lỗi |
| KB-15 | Cờ "sắp hết hạn" trên dòng danh sách HĐ | `0 ≤ ngày ≤ 60`, không xét gia hạn — khác bộ lọc `expiring_60` (`> 0`, chưa gia hạn, không phải phụ lục bổ sung) | Cờ dùng đúng điều kiện của bộ lọc `expiring_60` → cờ và bộ lọc luôn khớp | Sửa lỗi |
| KB-16 | Đọc đường dẫn file trong DB | Chỉ tuyệt đối | Chấp nhận cả tuyệt đối và tương đối (so với data root). Ghi vẫn tuyệt đối khi còn dùng chung DB | Chuẩn bị |
| KB-17 | Tên file sinh hàng loạt | `open(..., "wb")` — trùng tên trong cùng giây là ghi đè file của hợp đồng khác | Ghi độc quyền, trùng thì thêm `_2`, `_3` (như sinh file lẻ) | Sửa lỗi |
| KB-18 | Mở nhiều cửa sổ | Mở được nhiều tiến trình cùng ghi SQLite | Khóa `.cf_writer.lock`: chỉ một cửa sổ | Nâng cấp an toàn |
| KB-19 ⚙ | Tên file báo giá tự nhập | Ngày người dùng gõ chèn thẳng vào tên (`05/01/2026` → có dấu `/`) | Thay ký tự không hợp lệ trong tên file Windows (`\ / : * ? " < > \|`) bằng `-` | Sửa lỗi |

## Ghi chú cho người làm task

- Đọc mục tương ứng trước khi port hàm có liên quan. Ví dụ port `amount_to_words` thì phải làm
  theo KB-01, không chép nguyên vòng `_ONES[tram]`.
- Test fixture: dùng giá trị `rust_expected` trong `intentional_differences.json` thay cho
  kết quả ghi trong fixture gốc ở đúng các ca được liệt kê.
- **Tìm thấy chỗ nghi sai khác** mà không có trong bảng → làm giống Python, đánh dấu
  `// TODO(KB): <mô tả>` và thêm một dòng vào mục "Chờ duyệt" dưới đây.

## Chờ duyệt

_(trống)_
