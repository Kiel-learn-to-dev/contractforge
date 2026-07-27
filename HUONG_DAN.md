# ContractForge v43 — Hướng dẫn cài đặt và sử dụng

## Yêu cầu hệ thống
- Windows 10/11 (64-bit)
- Python 3.10+ (tải tại https://python.org — nhớ tick "Add Python to PATH")
- Không cần internet sau khi cài xong

---

## Cài đặt lần đầu (2 bước)

### Bước 1 — Giải nén
Giải nén file ZIP → được thư mục **ContractForge/**

### Bước 2 — Chạy lần đầu
Double-click **ContractForge.pyw**

Lần đầu chạy:
- Tự cài tất cả thư viện cần thiết (~1-2 phút)
- Tạo thư mục `data/` chứa database và file đầu ra
- Khởi động server và mở trình duyệt tự động

**Lần sau:** double-click `ContractForge.pyw` → mở ngay, không cài lại.

---

## (Tùy chọn) Build file .exe để click gọn hơn

Nếu muốn có file `ContractForge.exe` thay vì `.pyw`:

1. Double-click **build_exe.bat** → chờ ~2 phút
2. File `ContractForge.exe` sẽ xuất hiện trong thư mục này
3. Từ nay double-click `ContractForge.exe` để khởi động

---

## Sử dụng hàng ngày

| Thao tác | Cách làm |
|----------|----------|
| Khởi động | Double-click `ContractForge.pyw` (hoặc `.exe`) |
| Mở trình duyệt | Icon CF góc phải taskbar → **Mở trình duyệt** |
| Dừng server | Icon CF → **Dừng server** |
| Thoát hoàn toàn | Icon CF → **Thoát** |
| Xem log lỗi | Mở file `contractforge.log` |

**Địa chỉ truy cập:** http://localhost:8888

---

## Cấu trúc thư mục

```
ContractForge/              ← Thư mục gốc
  ContractForge.pyw         ← Launcher (double-click để chạy)
  ContractForge.exe         ← Launcher đã build (nếu đã build)
  build_exe.bat             ← Build exe (chạy 1 lần)
  HUONG_DAN.md              ← File này
  │
  ContractForge/            ← CODE ứng dụng
  │  main.py, app/, templates/, ...
  │
  data/                     ← DATA (KHÔNG bao giờ xóa)
     contract_manager.db    ← Toàn bộ dữ liệu KH, hợp đồng
     outputs/contracts/     ← File .docx hợp đồng đơn lẻ
     outputs/batch/         ← File ZIP sinh hàng loạt
     uploads/templates/     ← Mẫu hợp đồng đã upload
```

---

## Cập nhật phiên bản mới (v44, v45...)

1. Tải file `ContractForge_vXX.zip`
2. Giải nén → được thư mục `ContractForge/`
3. **Thay thế** thư mục `ContractForge/ContractForge/` cũ bằng cái mới
4. **GIỮ NGUYÊN** thư mục `data/` — toàn bộ 55 KH và hợp đồng giữ nguyên
5. Chạy lại `ContractForge.pyw`

> ⚠️ **Quan trọng:** Chỉ thay thư mục `ContractForge/` (code). KHÔNG xóa `data/`.

---

## Backup dữ liệu

Copy nguyên thư mục `data/` ra USB/Google Drive là đủ.  
Để phục hồi: copy `data/` trở lại đúng vị trí rồi chạy app.

---

## Xử lý sự cố

**App không mở được:**
→ Mở `contractforge.log` xem lỗi

**Python không tìm thấy:**
→ Cài Python tại https://python.org, nhớ tick "Add Python to PATH"

**Port 8888 bị chiếm:**
→ Mở `ContractForge/main.py`, sửa dòng `--port 8888` thành port khác

**Mất dữ liệu:**
→ Kiểm tra thư mục `data/contract_manager.db` còn không
