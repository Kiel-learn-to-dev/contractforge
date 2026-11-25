# ContractForge Rust — Blueprint tổng quan

> **Đọc file này trước.** Đây là bản thiết kế để viết lại ContractForge (Python/FastAPI/Jinja +
> WebView2) thành **ứng dụng desktop native bằng Rust + Slint**, giữ nguyên nghiệp vụ.
>
> - Bản Python làm chuẩn: commit `04d33f8` trên nhánh `pricing-model`, **working tree còn thay
>   đổi chưa commit** (báo giá tự nhập, yêu cầu báo giá, người phát hành báo giá). Blueprint mô
>   tả đúng trạng thái working tree lúc khảo sát (25/11/2026). Nếu code Python đổi sau ngày đó,
>   chạy lại `tools/gen_fixtures.py` và đối chiếu lại.
> - Người đọc: chủ dự án và **các AI lập trình viên** sẽ làm từng task trong `TASKS.md`.

## 1. Mục tiêu

| # | Mục tiêu | Đo bằng |
|---|---|---|
| G1 | Nhẹ: RAM khi mở danh sách hợp đồng ≤ 80 MB (bản Python + WebView2 hiện khoảng 250–400 MB) | Task Manager, cột Memory |
| G2 | Không phụ thuộc Chromium/WebView2, không mở cổng mạng nào | Không có `msedgewebview2.exe` con; `netstat` không thấy cổng |
| G3 | Một file `.exe`, khởi động < 1 giây | Đồng hồ bấm giờ |
| G4 | **Giữ nghiệp vụ, nâng cấp giải thuật**: kết quả nghiệp vụ y hệt bản Python ở mọi chỗ Python đúng; chỗ Python sai/yếu thì làm tốt hơn theo `06_KHAC_BIET_CO_CHU_Y.md` | Fixture vàng qua test; mỗi khác biệt có fixture riêng |
| G5 | Dùng chung database SQLite đang có **trong giai đoạn chuyển đổi**; khi bản Rust ổn định thì **xóa hẳn bản Python** | Test tương thích hai chiều (xem `07_KIEM_THU.md`) |
| G6 | Thao tác tức thì: lọc/tìm kiếm phản hồi < 50 ms khi gõ, dashboard mở < 100 ms | Đo bằng log `tracing` |

### Nguyên tắc số 1: giữ nghiệp vụ, không giữ giải thuật

Bản Python là **nguồn sự thật về nghiệp vụ** (luật, con số, câu chữ cho người dùng), **không phải**
khuôn mẫu về cách làm. Bản Rust được và nên dùng giải thuật tốt hơn, miễn kết quả nghiệp vụ đúng.
Mọi chỗ kết quả khác bản Python phải nằm trong `06_KHAC_BIET_CO_CHU_Y.md` và có fixture
`fixtures/intentional_differences.json`. Không có trong danh sách đó mà khác = lỗi.

## 2. Ngoài phạm vi (không làm)

- Không làm lại giao diện web, không có HTTP server, không API.
- Không viết lại các migration lịch sử của bản Python (xem `02_DU_LIEU.md` §5).
- Không đổi schema trong giai đoạn chuyển đổi.
- Không có đăng nhập / nhiều người dùng (giống bản Python).
- Không chế độ "live reload" (`run_live.py`, `live_app.py`) — đó là công cụ phát triển của bản Python.

## 3. Các quyết định đã chốt

| Mã | Quyết định | Lý do | Có đảo ngược được? |
|---|---|---|---|
| QĐ-1 | Rust + **Slint** (UI native, renderer `femtovg`/`skia` tùy bản thử Đợt 0) | Không Chromium, RAM thấp | Khó — chốt sau Đợt 0 |
| QĐ-2 | **Dùng chung `contract_manager.db`** với bản Python, đúng schema hiện tại, cho tới khi bản Rust ổn; sau đó xóa hẳn Python và mới đổi schema | Chạy song song để đối chiếu, quay lui được | Được |
| QĐ-3 | Chỉ một tiến trình mở DB tại một thời điểm: file khóa `.cf_writer.lock`. Không chạy bản Python và Rust cùng lúc | Bản sao trong RAM (QĐ-8) sẽ lệch nếu có tiến trình khác ghi | Được |
| QĐ-4 | Engine docx mới: đọc mẫu → **biên dịch** (tìm sẵn vị trí placeholder) → chọn tên file mới độc quyền → điền → ghi. Không ghi đè file có sẵn | Giữ nguyên mọi byte ngoài chỗ điền; sinh hàng loạt chỉ biên dịch mẫu một lần | Được |
| QĐ-8 | **Bản sao dữ liệu trong RAM** (`Store`): đọc toàn bộ DB khi mở (< 1 MB), mọi truy vấn chạy trên bộ nhớ, mọi thay đổi ghi xuống SQLite trong transaction rồi mới cập nhật RAM | Lọc/tìm kiếm tức thì, code đơn giản hơn SQL rải rác | Được |
| QĐ-9 | **Dòng tùy chọn khai báo trong mẫu** `{{?KHOA}}`: dòng bảng chứa dấu này bị xóa khi giá trị rỗng. Cơ chế dò nhãn "Chức vụ" cũ chỉ còn là chế độ tương thích cho mẫu chưa sửa | Bỏ heuristic dựa vào câu chữ | Được |
| QĐ-10 | Mọi thay đổi trạng thái hợp đồng (kể cả tự động khi upload, quét hết hạn) đi qua **một hàm duy nhất** | Bản Python có đường vòng ở router | — |
| QĐ-5 | Nghiệp vụ thuần (tính tiền, đánh số, vòng đời…) nằm trong crate `cf-core` **không I/O**, kiểm bằng fixture vàng | Để AI yếu cũng làm được: chỉ cần "làm cho test xanh" | — |
| QĐ-6 | Thông điệp lỗi cho người dùng **giữ nguyên tiếng Việt** như bản Python (lấy từ fixture) | Người dùng quen câu chữ; test so chuỗi | Được |
| QĐ-7 | Mọi "tải về" (download) trên web → hộp thoại Lưu (rfd) hoặc mở bằng ứng dụng mặc định của Windows | Không có trình duyệt | Được |

## 4. Bản đồ tài liệu

| File | Nội dung |
|---|---|
| `00_TONG_QUAN.md` | File này |
| `01_KIEN_TRUC.md` | Workspace, crate, thư viện (đã pin phiên bản), quy ước lỗi/số/ngày, thư mục dữ liệu, khóa ghi |
| `02_DU_LIEU.md` | Schema SQLite (DDL nguyên văn), cách lưu từng kiểu, cột JSON, chính sách migration |
| `03_NGHIEP_VU.md` | Toàn bộ luật nghiệp vụ, theo module, kèm file Python làm chuẩn |
| `04_DOCX_ENGINE.md` | Đặc tả engine sinh docx mới (luồng copy → đặt tên → điền) |
| `05_GIAO_DIEN.md` | Danh sách màn hình, hành vi form (chép từ JS), thay thế cho tính năng web |
| `06_KHAC_BIET_CO_CHU_Y.md` | Danh sách chỗ bản Rust **cố ý** khác bản Python (sửa lỗi, nâng cấp giải thuật), mỗi chỗ có fixture |
| `07_KIEM_THU.md` | Fixture vàng, cách test, test tương thích DB, nghiệm thu |
| `08_LO_TRINH.md` | Các đợt (Đợt 0–6), điểm dừng, tiêu chí qua đợt |
| `../TASKS.md` | Danh sách task nhỏ nhất (sinh bằng skill planning-and-task-breakdown) |
| `../fixtures/` | Fixture vàng JSON + docx, sinh bởi `../tools/gen_fixtures.py` |

## 5. Ba ranh giới bất di bất dịch (thừa kế từ bản Python)

1. **Dữ liệu riêng không bao giờ vào Git.** Không chép tên tổ chức, khuôn số hợp đồng thật, tên
   sản phẩm thật, đường dẫn dữ liệu thật vào code, test hay tài liệu. Fixture chỉ dùng dữ liệu hư
   cấu ("Trạm Y tế xã Sông Xanh", "Tỉnh Mẫu"…). Chạy `python scripts/check_public_repo.py` trước
   mỗi commit.
2. **Không mở cổng mạng.** Bản Python bind `127.0.0.1`; bản Rust còn chặt hơn: **không có socket
   nào**. Không thêm thư viện HTTP server.
3. **Không viết cứng thứ thuộc về một tổ chức.** Khuôn số hợp đồng, tên/giá sản phẩm, thông tin
   Bên B đều là dữ liệu trong DB. Luật vòng đời chỉ ở `cf-core::lifecycle`, khuôn số chỉ ở
   `cf-core::numbering`.

## 6. Cách dùng blueprint này cho AI lập trình viên

1. Mỗi task trong `TASKS.md` có: file phải tạo/sửa, file Python làm chuẩn, fixture phải qua, lệnh
   kiểm tra "xong khi".
2. **Không tự ý "sửa" hành vi của bản Python** (làm tròn, cắt số, câu chữ…). Chỉ những khác biệt
   đã liệt kê trong `06_KHAC_BIET_CO_CHU_Y.md` là được phép. Thấy chỗ nghi sai mà chưa có trong
   danh sách → làm giống Python, ghi chú `// TODO(KB): ...` và báo lại.
3. Thứ tự ưu tiên khi mâu thuẫn: `fixtures/intentional_differences.json` > fixture vàng khác >
   tài liệu. Ghi lại mâu thuẫn tìm thấy vào cuối `06_KHAC_BIET_CO_CHU_Y.md`.
