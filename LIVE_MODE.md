# Chạy ContractForge trực tiếp từ mã nguồn

Mở shortcut `ContractForge - Live.lnk` ở thư mục dự án để khởi động. Ứng dụng mở
trong cửa sổ WebView riêng, còn cửa sổ dòng lệnh được thu nhỏ. Đóng cửa sổ app để
dừng cả server. Shortcut chứa đường dẫn tuyệt đối của máy đã tạo nó nên không nằm
trong repo; trên máy khác hãy dùng `Chay_ContractForge.bat`.

Nếu chuyển thư mục dự án, mở `Chay_ContractForge.bat` thay cho shortcut. File này
cũng dùng để xem log, địa chỉ cục bộ và vị trí database; nhấn `Ctrl+C` để dừng.

Khi sửa Python, HTML, CSS hoặc JavaScript, server tự khởi động lại; trang sạch tự
tải lại sau khi server sẵn sàng. Nếu đang nhập biểu mẫu chưa lưu, trang giữ nguyên
và hiện nút tải bản mới.

Chế độ này dùng cùng quy tắc chọn dữ liệu của ứng dụng. Hiện máy này có database
ở `data/contract_manager.db`, nên chạy từ mã nguồn sẽ tiếp tục dùng database đó.
Không mở cùng lúc bản EXE hoặc một server ContractForge khác để tránh hai tiến
trình cùng ghi SQLite.

Các thay đổi bên trong thư mục `ContractForge/` được theo dõi tự động. Khi sửa
chính launcher `run_live.py` hoặc cài thêm thư viện, đóng rồi mở lại ứng dụng.

Có thể chạy trực tiếp bằng:

```powershell
.\.venv\Scripts\python.exe .\run_live.py
```

Tuỳ chọn kiểm thử server không mở cửa sổ:

```powershell
.\.venv\Scripts\python.exe .\run_live.py --no-window --port 8123
```
