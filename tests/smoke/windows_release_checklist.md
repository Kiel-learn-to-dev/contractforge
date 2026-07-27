# Checklist kiểm thử bản Windows trước khi phát hành

Bộ test tự động phủ được vòng đời server, chọn cổng, và chính sách điều hướng
(`tests/unit/test_desktop_lifecycle.py`). Những mục dưới đây **phải làm tay** vì
chúng cần một cửa sổ WebView2 thật và một máy Windows sạch.

Chạy trên **máy ảo Windows chưa cài Python**.

> Những mục đánh dấu ✔(dev) đã được xác nhận trên máy phát triển với bản .exe
> 42 MB build ngày 27/07/2026 — nhưng máy đó **có** Python và WebView2 Runtime
> sẵn, nên vẫn phải làm lại trên máy sạch trước khi phát hành.

## Chuẩn bị

- [ ] Tải `ContractForge.exe` từ artifact của workflow `Windows release`.
- [ ] Xác nhận máy chưa cài Python: `python --version` phải báo lỗi.
- [ ] Ghi lại kích thước file .exe (bản tham chiếu: ~36 MB).

## Khởi động lần đầu

- [x] ✔(dev) Double-click `ContractForge.exe`. Cửa sổ mở, tiêu đề
      "ContractForge — Quản lý Hợp đồng", **không có cửa sổ đen console**.
- [x] ✔(dev) Không có tab trình duyệt nào bật lên.
- [ ] Trang đầu tiên là màn hình thiết lập tổ chức (`/settings?setup=1`), vì hồ sơ
      còn trống. *(xác nhận từ mã nguồn, chưa xác nhận trên .exe máy sạch)*
- [ ] `%LOCALAPPDATA%\ContractForge` được tạo, bên trong có `contract_manager.db`,
      `uploads/`, `outputs/`, `logs/`, `backups/`.
- [ ] Thư mục cài đặt **không** bị ghi thêm gì.
- [x] ✔(dev) Log có dòng `Server ready`, cổng là số ngẫu nhiên trên `127.0.0.1`.
- [x] ✔(dev) Nếu có `data/` nằm cạnh `.exe` thì dùng đúng nó, **không** tạo
      cơ sở dữ liệu trống trong LocalAppData.

## Nghiệp vụ đầy đủ

- [ ] Điền hồ sơ tổ chức, lưu lại.
- [ ] Tạo một khách hàng, kèm ít nhất 2 đơn vị con.
- [ ] Tải lên một mẫu Word `.docx` có placeholder, kiểm tra bảng ánh xạ trường.
- [ ] Tạo một hợp đồng: số hợp đồng tự gợi ý, giá tự điền từ sản phẩm.
- [ ] Sinh file `.docx` và **tải về** — file mở được bằng Word, đã điền đúng dữ liệu.
- [ ] Sinh hàng loạt cho ≥2 khách hàng, tải về file `.zip`.
- [ ] Xuất báo cáo khách hàng `.xlsx` và danh sách hợp đồng `.xlsx`.
- [ ] Tải lên một PDF hợp đồng đã ký, mở xem được ngay trong ứng dụng.
- [ ] Xuất biểu 08a cho một hợp đồng.

## Vòng đời cửa sổ

- [ ] Thu nhỏ, phóng to, đổi kích thước cửa sổ — bố cục không vỡ.
- [ ] Bấm một link ra ngoài (nếu có) → mở bằng trình duyệt hệ thống, **không**
      điều hướng bên trong cửa sổ ứng dụng.
- [x] ✔(dev) Icon khay hệ thống hiện ra.
- [ ] Menu khay "Thoát" đóng được ứng dụng.
- [x] ✔(dev) Đóng cửa sổ → **mọi** tiến trình `ContractForge.exe` biến mất
      (onefile chạy một bootloader cha + một tiến trình con), server tắt theo,
      và không còn tiến trình `msedgewebview2.exe` nào của ứng dụng.
- [ ] Mở lại ngay lập tức → chạy được, không báo "cổng đang bận".
- [ ] Mở hai bản cùng lúc → mỗi bản tự lấy một cổng riêng, cả hai đều chạy.

## WebView2

- [ ] Trên máy chưa có WebView2 Runtime: ứng dụng báo rõ ràng và chỉ chỗ tải,
      hoặc tự lùi về mở bằng trình duyệt — **không** im lặng chết.
      *(chưa kiểm được: máy phát triển đã có sẵn Runtime 150.0.4078.99)*
- [x] ✔(dev) Có WebView2 Runtime thì cửa sổ riêng hiện lên và điều hướng trong
      ứng dụng hoạt động.

## Nâng cấp và gỡ cài đặt

- [ ] Thay `.exe` bằng bản mới hơn, mở lại → dữ liệu cũ còn nguyên, không mất
      khách hàng/hợp đồng nào.
- [ ] Xoá `.exe` → `%LOCALAPPDATA%\ContractForge` **vẫn còn** (dữ liệu người dùng
      không đi theo lúc gỡ cài đặt).

## Không rò rỉ dữ liệu

- [ ] Trong bản đóng gói không có `contract_manager.db`.
- [ ] Trong bản đóng gói không có mẫu Word riêng của cơ quan nào — chỉ có
      `sample_form_08a.docx` hư cấu.
- [ ] Màn hình thiết lập tổ chức trống trơn, không điền sẵn tên đơn vị nào.
- [ ] Danh mục khách hàng trống.
