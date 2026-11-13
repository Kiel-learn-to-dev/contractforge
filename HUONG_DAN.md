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

## Danh mục sản phẩm, phụ lục và gia hạn (0.9.0)

- **Danh mục:** vào Sản phẩm để thêm, sửa, ngừng bán. Nhập nhiều gói bằng
  "Nhập / xuất Excel": tải file mẫu, điền, xem trước, rồi mới ghi.
- **Gói bổ sung:** chỉ được ghép vào hợp đồng khi cùng nhóm với gói chính.
  Nếu không bán kèm, không tạo gói bổ sung trong danh mục.
- **Phụ lục bổ sung:** trên trang hợp đồng đang phục vụ, bấm "Tạo phụ lục bổ sung".
  Thời hạn tự tính đến ngày hết hạn của hợp đồng gốc.
- **Gia hạn:** bấm "Gia hạn", chọn phụ lục gia hạn hoặc hợp đồng mới. Gói cũ đã
  ngừng được gợi ý thay bằng gói thay thế; so sánh giá theo tháng hiện ngay trên form.
- **Khuôn số phụ lục:** đặt trong Cài đặt, phải có {seq} và {parent}.
- **Văn bản kèm theo khi sinh hợp đồng:** trong form tạo hợp đồng hoặc ở chi tiết,
  mở "Kèm văn bản" rồi tick các biên bản cần thiết. Có tick thì tải về một file ZIP
  gồm hợp đồng chính và các văn bản đã chọn. Mẫu Phụ lục 01 BM02 / BM03 (tích hợp,
  cho thuê bổ sung) là mẫu hợp đồng chính: chọn trong ô "Mẫu hợp đồng".

## Bộ văn bản theo sản phẩm (0.10.0)

Mỗi nhóm sản phẩm có bộ hợp đồng, phụ lục, biên bản và báo giá riêng. Mẫu không
thuộc nhóm nào thì gọi là "dùng chung" và hiện ở mọi nhóm.

- **Gán nhóm cho mẫu:** vào Mẫu hợp đồng, sửa mẫu, chọn "Thuộc nhóm sản phẩm".
  Chọn "Dùng chung cho mọi sản phẩm" nếu mẫu dùng cho tất cả nhóm.
- **Tải nhiều mẫu một lượt:** ở đầu trang Mẫu hợp đồng, chọn nhóm, chọn nhiều file .docx
  rồi bấm "Tải nhiều mẫu". Loại mẫu (hợp đồng, phụ lục, biên bản...) được nhận ra từ
  nội dung file. File trùng nội dung với mẫu đã có sẽ bị bỏ qua và được báo lại.
- **Bật / tắt mẫu:** trong danh sách mẫu, bỏ tick "Bật trong danh sách chọn" để ẩn mẫu
  khỏi các ô chọn. Mẫu tắt vẫn còn trong thư viện, không bị xóa.
- **Biên bản tick sẵn:** khi lập hợp đồng, biên bản bàn giao và nghiệm thu của đúng nhóm
  được tick sẵn. Biên bản thông tin (khác) và biên bản dùng chung không tick sẵn.
- **Phụ lục bổ sung:** chỉ ghép được gói bổ sung cùng nhóm với gói chính.
- **Ngày hết hạn là ngày cuối kỳ:** hợp đồng từ 01/01/2027 đủ 12 tháng hết hạn 31/12/2027.
  Gia hạn bắt đầu từ 01/01/2028. Hợp đồng cũ giữ nguyên ngày đã lưu.
- **Sinh hàng loạt:** ô chọn mẫu hiện mọi mẫu đang bật. Dòng nào chọn mẫu khác nhóm
  với sản phẩm của dòng đó sẽ báo lỗi và không tạo hợp đồng.
