# ContractForge

> Ứng dụng quản lý hợp đồng chạy offline, một người dùng, dành cho máy tính cá nhân.
> A neutral, single-user, offline-first desktop contract manager.

ContractForge quản lý toàn bộ vòng đời hợp đồng — từ bản nháp, sinh file Word từ mẫu
có sẵn, theo dõi ký kết, hoá đơn, thanh toán, cho đến khi hết hạn — mà **không cần
kết nối Internet và không gửi dữ liệu đi đâu cả**. Toàn bộ dữ liệu nằm trong một file
SQLite trên máy bạn.

---

## Tính năng chính

- **Danh mục khách hàng** kèm đơn vị trực thuộc, hồ sơ giấy tờ đính kèm, nhập/xuất Excel.
- **Mẫu hợp đồng Word**: tải lên file `.docx` của riêng bạn, đánh dấu chỗ điền bằng
  placeholder dạng `{{TEN_TRUONG}}`, app tự điền và sinh file hoàn chỉnh.
- **Sinh hàng loạt**: một job tạo hợp đồng cho nhiều khách hàng, đóng gói thành `.zip`.
- **Vòng đời hợp đồng** có kiểm soát: mỗi lần đổi trạng thái đều được kiểm tra hợp lệ,
  yêu cầu chứng từ kèm theo, và ghi lại lịch sử.
- **Dashboard & nhắc hạn**: cảnh báo hợp đồng sắp hết hạn, việc còn tồn đọng,
  thống kê theo sản phẩm và theo tháng.
- **Báo giá** và biểu mẫu phụ trợ sinh từ cùng bộ dữ liệu.

## Vòng đời hợp đồng

```
Draft ──► Generated ──► Sent ──► Signed ──► Invoiced ──► PaidActive ──► Expired
                                    └──────────┴─────────────┴────────► Terminated
```

Mỗi bước chuyển đều đi qua một bảng luật duy nhất
(`ContractForge/app/services/lifecycle.py`). `Invoiced` yêu cầu đã đính kèm hoá đơn,
`PaidActive` yêu cầu đã đính kèm chứng từ thanh toán.

**"Sắp hết hạn" không phải là một trạng thái** — nó được tính từ `end_date` mỗi lần
hiển thị. Nhờ vậy một hợp đồng gần hết hạn vẫn giữ nguyên trạng thái thật của nó và
vẫn xuất hoá đơn được bình thường.

---

## Cài đặt & chạy

Yêu cầu Python 3.11 trở lên.

```bash
pip install -r ContractForge/requirements.txt
```

Chạy từ mã nguồn:

```bash
cd ContractForge && python -m uvicorn main:app --host 127.0.0.1 --port 8000
```

Rồi mở http://127.0.0.1:8000

Trên Windows có thể chạy thẳng `ContractForge.pyw` — nó tự kiểm tra thư viện, khởi động
server và hiện biểu tượng ở khay hệ thống.

> Server **chỉ lắng nghe trên `127.0.0.1`**. Đây là ứng dụng một người dùng chạy cục bộ;
> nó không có cơ chế đăng nhập nên không được mở ra mạng LAN hay Internet.

---

## Dữ liệu của bạn nằm ở đâu

Mã nguồn và dữ liệu tách bạch hoàn toàn. Thư mục dữ liệu được xác định theo thứ tự
ưu tiên sau (xem `ContractForge/app/paths.py`):

1. Biến môi trường `CONTRACTFORGE_DATA_ROOT`, nếu được đặt.
2. `%LOCALAPPDATA%\ContractForge` — nếu ở đó đã có sẵn cơ sở dữ liệu.
3. `<thư mục dự án>/data` — bản cài đặt cũ vẫn chạy bình thường, không bị ép di chuyển.
4. Mặc định cho máy mới: `%LOCALAPPDATA%\ContractForge`
   (macOS: `~/Library/Application Support/ContractForge`,
   Linux: `~/.local/share/ContractForge`).

Bên trong thư mục dữ liệu:

| Thư mục | Nội dung |
|---|---|
| `contract_manager.db` | Toàn bộ dữ liệu nghiệp vụ (SQLite) |
| `uploads/templates/` | Mẫu Word bạn tải lên |
| `uploads/signed_scans/`, `invoice_docs/`, `payment_slips/` | Chứng từ đính kèm |
| `uploads/customer_docs/` | Hồ sơ giấy tờ khách hàng |
| `outputs/` | File `.docx` và `.zip` đã sinh |
| `backups/` | Bản sao lưu do công cụ di trú tạo ra |

**Sao lưu** = copy cả thư mục này. Không có gì khác cần giữ.

**Khi xoá hợp đồng**, các file gắn với nó (bản `.docx` đã sinh, bản scan đã ký, hoá đơn,
chứng từ thanh toán) cũng bị xoá khỏi đĩa cùng lúc. Hồ sơ giấy tờ của khách hàng thì
gắn với khách hàng chứ không gắn với hợp đồng, nên không bị ảnh hưởng.

---

## Phát triển

```bash
python -m pytest -q                        # bộ test
python scripts/check_public_repo.py        # kiểm tra không lộ dữ liệu riêng
```

Bộ test luôn chạy trên thư mục dữ liệu tạm — chạy test **không bao giờ** đụng vào
cơ sở dữ liệu thật của bạn (xem `tests/conftest.py`).

Trước khi commit, `scripts/check_public_repo.py` sẽ chặn nếu có file dữ liệu, file
sinh ra, log, file thực thi, hay từ khoá riêng của tổ chức lọt vào mã nguồn công khai.

Kiến trúc, các quyết định thiết kế và lộ trình: xem `OPEN_SOURCE_DESKTOP_PLAN.md`.

## Giấy phép

[MIT](LICENSE)
