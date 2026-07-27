# Chính sách bảo mật

## Mô hình bảo mật

ContractForge là ứng dụng **một người dùng, chạy cục bộ**. Nó không có đăng nhập,
không có phân quyền, không có nhiều người dùng. Ranh giới bảo mật của nó là
**ranh giới máy**: ai truy cập được vào tài khoản Windows đang chạy ứng dụng thì
truy cập được toàn bộ dữ liệu.

Điều đó có chủ đích, và kéo theo vài hệ quả:

- Server **chỉ lắng nghe trên `127.0.0.1`**. Đừng đổi sang `0.0.0.0` để "cho máy
  khác trong phòng vào xem" — làm vậy là công khai toàn bộ cơ sở dữ liệu hợp
  đồng cho mọi thiết bị trong mạng, không cần mật khẩu.
- Yêu cầu thay đổi dữ liệu bị từ chối nếu đến từ nguồn không phải máy này
  (`app/security.py`). Nếu không, một trang web bất kỳ đang mở trên trình duyệt
  vẫn gửi được form POST tới `localhost`.
- Cơ sở dữ liệu **không được mã hoá**. Cần bảo vệ ở mức nghỉ thì dùng BitLocker
  hoặc mã hoá toàn ổ đĩa.
- Không có sao lưu tự động. Sao lưu = copy thư mục dữ liệu (xem README).

## Phạm vi

Được coi là lỗ hổng:

- Bất cứ đường nào khiến ứng dụng lắng nghe ngoài loopback.
- XSS lưu trữ qua dữ liệu người dùng nhập (tên khách hàng, số hợp đồng, ghi chú).
- Đường vòng cho phép sửa/đọc tài nguyên của khách hàng khác.
- Path traversal ở các endpoint tải file hoặc xem chứng từ.
- Rò rỉ dữ liệu riêng vào bản đóng gói hoặc vào kho mã công khai.
- Bất kỳ cách nào bỏ qua kiểm tra chuyển trạng thái hoặc chứng từ bắt buộc.

Không được coi là lỗ hổng:

- Người dùng cục bộ đọc được file cơ sở dữ liệu của chính họ.
- Không có xác thực — đó là thiết kế, xem trên.
- Tấn công cần quyền quản trị sẵn có trên máy.

## Báo cáo

Mở một issue trên GitHub. Nếu vấn đề nhạy cảm và bạn không muốn công khai ngay,
dùng chức năng **Private vulnerability reporting** của GitHub trong tab Security
của kho mã.

Xin **đừng đính kèm dữ liệu thật** — không cơ sở dữ liệu, không hợp đồng, không
tên khách hàng. Hãy mô tả các bước tái hiện bằng dữ liệu hư cấu.

## Trước khi đóng góp

`scripts/check_public_repo.py` chặn dữ liệu riêng lọt vào kho mã công khai. Chạy
nó trước mỗi commit; CI cũng chạy lại. Xem CONTRIBUTING.md.
