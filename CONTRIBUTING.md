# Đóng góp cho ContractForge

Cảm ơn bạn đã quan tâm. Tài liệu này ghi những thứ khác với một dự án Python
thông thường — chủ yếu là vì đây là ứng dụng chạy trên máy người dùng, giữ dữ
liệu hợp đồng thật của họ.

## Ranh giới quan trọng nhất: dữ liệu không bao giờ vào repo

Repo này công khai. Cơ sở dữ liệu, mẫu Word của cơ quan, hồ sơ khách hàng,
hợp đồng đã sinh, log — tất cả đều nằm ngoài Git và phải giữ nguyên như vậy.

Trước khi commit:

```bash
python scripts/check_public_repo.py
```

Scanner chặn hai nhóm: **loại file cấm** (`.db`, `.log`, `.exe`, `.zip`, mọi thứ
dưới `data/` hay `uploads/`) và **từ khoá riêng** đọc từ
`data/private_denylist.txt` — file này cũng bị ignore, mỗi máy tự giữ danh sách
của mình. CI dùng secret `CONTRACTFORGE_PRIVATE_DENYLIST` cho cùng mục đích.

Nếu lỡ commit dữ liệu riêng: **dừng lại, đừng push**. Xoá khỏi cây làm việc rồi
viết lại lịch sử Git — xoá file ở commit sau không xoá nó khỏi lịch sử.

## Chạy dự án

```bash
pip install -r ContractForge/requirements.txt
cd ContractForge && python -m uvicorn main:app --host 127.0.0.1 --port 8000
```

Server **chỉ được** bind `127.0.0.1`. App không có đăng nhập; ranh giới bảo mật
của nó là ranh giới máy. Có test chặn việc này (`test_publication_readiness.py`).

## Test

```bash
python -m pytest -q
```

Test luôn chạy trên thư mục dữ liệu tạm — `tests/conftest.py` đặt
`CONTRACTFORGE_DATA_ROOT` trước khi ứng dụng được import. Chạy test **không bao
giờ** đụng vào cơ sở dữ liệu thật; nếu thấy nó đụng, đó là bug nghiêm trọng.

Vài quy ước:

- Test khẳng định **hành vi**, không chép lại cách cài đặt. Test chỉ lặp lại
  logic của code thì luôn xanh và chẳng bảo vệ được gì.
- Sửa bug thì viết test dựng lại đúng bug đó trước.
- Phần chạy trong trình duyệt hay trong cửa sổ WebView2 không tự động hoá được
  ở đây; ghi vào `tests/smoke/windows_release_checklist.md`.

## Những chỗ dễ sai

**Vòng đời hợp đồng.** Mọi luật nằm ở `app/services/lifecycle.py`: bảng chuyển
trạng thái, chứng từ bắt buộc, và các tập trạng thái. Đừng định nghĩa lại
"hợp đồng đang hiệu lực gồm những trạng thái nào" ở nơi khác — từng có sáu bản
sao lệch nhau, khiến dashboard, danh sách khách hàng và báo cáo Excel đếm ra ba
con số khác nhau trên cùng một dữ liệu.

**"Sắp hết hạn" không phải trạng thái.** Nó được tính từ `end_date` mỗi lần hiển
thị. Trước đây nó ghi đè trạng thái thật, khiến hợp đồng đã ký gần hết hạn không
bao giờ xuất hoá đơn được nữa.

**Đừng viết cứng thứ thuộc về một tổ chức.** Khuôn số hợp đồng, tên sản phẩm,
giá cả — tất cả là dữ liệu trong DB, không phải nhánh `if` trong Python. Xem
`app/services/numbering.py`.

**Dữ liệu người dùng trong HTML/JS.** Dùng `textContent` hoặc `| tojson`, không
bao giờ nối chuỗi vào `innerHTML` hay dùng `| safe`. Tên khách hàng là dữ liệu
do người dùng nhập.

## Kiến trúc

`OPEN_SOURCE_DESKTOP_PLAN.md` ghi các quyết định thiết kế (AD-1..AD-8) và lý do.
Đọc phần liên quan trước khi đổi cách phân giải đường dẫn, vòng đời hợp đồng,
hay mô hình đóng gói.
