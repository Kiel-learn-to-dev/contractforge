# Hướng dẫn cài đặt và sử dụng
## Hệ thống Quản lý Hợp đồng — ContractForge

---

## Yêu cầu duy nhất: Máy tính cài Python

Tải Python tại: **https://www.python.org/downloads/**

> ⚠️ Khi cài Python trên Windows, **bắt buộc tick vào "Add Python to PATH"**

---

## Cách chạy chương trình

### Windows
1. Giải nén file ZIP vào bất kỳ thư mục nào (VD: `C:\HopDong\`)
2. Mở thư mục `contract_manager`
3. **Double-click vào file `chay_windows.bat`**
4. Lần đầu chạy sẽ mất 2–5 phút để cài thư viện tự động
5. Trình duyệt tự động mở tại `http://localhost:8888`

### Mac / Linux
1. Giải nén file ZIP
2. Mở Terminal, `cd` vào thư mục `contract_manager`
3. Chạy: `bash chay_mac_linux.sh`

---

## Sử dụng hàng ngày (sau lần cài đầu)

- Chạy lại `chay_windows.bat` → server khởi động trong vài giây
- Mở trình duyệt → vào `http://localhost:8888`
- Khi dùng xong: Nhấn **Ctrl+C** trong cửa sổ đen để tắt server

---

## Dữ liệu được lưu ở đâu?

Tất cả dữ liệu lưu trong file `contract_manager.db` ngay trong thư mục chương trình.

> 💡 **Backup**: Copy file `contract_manager.db` định kỳ để tránh mất dữ liệu.

File hợp đồng `.docx` sinh ra lưu trong:
- `outputs/contracts/` — hợp đồng đơn lẻ
- `outputs/batch/` — sinh hàng loạt (file ZIP)

---

## Các tính năng chính

| Tính năng | Truy cập |
|-----------|---------|
| Dashboard & nhắc hạn | Trang chủ |
| Quản lý khách hàng | Menu → Khách hàng |
| Upload mẫu hợp đồng | Menu → Mẫu hợp đồng |
| Tạo hợp đồng + sinh file .docx | Menu → Hợp đồng → Tạo mới |
| Sinh hàng loạt → file ZIP | Menu → Sinh hàng loạt |
| Import khách hàng từ Excel | Khách hàng → Import Excel |

---

## Upload mẫu hợp đồng của bạn

1. Vào **Mẫu hợp đồng → Upload mẫu mới**
2. Trong file Word `.docx`, các chỗ cần tự động điền phải viết dạng:
   `{{TÊN_TRƯỜNG}}` — chỉ chữ IN HOA và dấu gạch dưới

   Ví dụ: `{{CONTRACT_NUMBER}}`, `{{PARTY_A_NAME}}`, `{{TOTAL_AMOUNT}}`

3. Sau khi upload, hệ thống tự phát hiện tất cả các trường
4. Vào tab **Field Mapping** để kết nối trường trong Word với dữ liệu hệ thống

---

## Câu hỏi thường gặp

**Mở trang web lên thấy lỗi "Connection refused"?**
→ Server chưa khởi động xong. Đợi thêm 5–10 giây rồi F5.

**Cửa sổ đen hiện lỗi "Port 8888 already in use"?**
→ Server đang chạy rồi. Mở trình duyệt vào `http://localhost:8888` trực tiếp.

**Muốn đổi port (ví dụ 9000)?**
→ Mở `chay_windows.bat`, tìm dòng `--port 8888` và đổi thành `--port 9000`.

**Dữ liệu cũ đi đâu sau khi cập nhật chương trình?**
→ Giữ nguyên file `contract_manager.db`, copy vào thư mục chương trình mới.

---

*Phiên bản: v0.7.0 — Pha 1 hoàn thành*

---

## Khắc phục lỗi cửa sổ mở lên rồi tắt ngay

**Nguyên nhân phổ biến nhất:** Bạn double-click file `.bat` từ sai vị trí.

**Cách đúng:**
1. Giải nén ZIP
2. Mở thư mục `contract_manager` (vào bên trong thư mục đó)
3. Thấy các file: `main.py`, `requirements.txt`, `chay_windows.bat`, v.v.
4. Lúc này mới double-click `chay_windows.bat`

**Nếu vẫn lỗi — dùng script chẩn đoán:**

Mở thư mục `contract_manager`, double-click `kiem_tra_moi_truong.bat`.
Script này kiểm tra từng bước và **không tắt ngay** — giúp bạn biết chính xác lỗi ở đâu.

**Xem file log:**

Sau mỗi lần chạy, file `startup.log` và `kiem_tra.log` được tạo ngay trong thư mục.
Mở bằng Notepad để xem chi tiết lỗi.
