# Checklist kiểm thử bản Windows trước khi phát hành

Bộ test tự động phủ được vòng đời server, chọn cổng, và chính sách điều hướng
(`tests/unit/test_desktop_lifecycle.py`). Những mục dưới đây **phải làm tay** vì
chúng cần một cửa sổ WebView2 thật và một máy Windows sạch.

Chạy trên **máy ảo Windows chưa cài Python**.

## Chuẩn bị

- [ ] Tải `ContractForge.exe` từ artifact của workflow `Windows release`.
- [ ] Xác nhận máy chưa cài Python: `python --version` phải báo lỗi.
- [ ] Ghi lại kích thước file .exe (bản tham chiếu: ~36 MB).

## Khởi động lần đầu

- [ ] Double-click `ContractForge.exe`. Cửa sổ mở, **không có cửa sổ đen console**.
- [ ] Không có tab trình duyệt nào bật lên.
- [ ] Trang đầu tiên là màn hình thiết lập tổ chức (`/settings?setup=1`), vì hồ sơ
      còn trống.
- [ ] `%LOCALAPPDATA%\ContractForge` được tạo, bên trong có `contract_manager.db`,
      `uploads/`, `outputs/`, `logs/`, `backups/`.
- [ ] Thư mục cài đặt **không** bị ghi thêm gì.
- [ ] Mở `%LOCALAPPDATA%\ContractForge\logs\contractforge.log` — có dòng
      `Server ready`, và cổng là một số ngẫu nhiên trên `127.0.0.1`.

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
- [ ] Icon khay hệ thống hiện ra; menu "Thoát" đóng được ứng dụng.
- [ ] Đóng cửa sổ → tiến trình `ContractForge.exe` biến mất khỏi Task Manager
      (kiểm tra **cả hai** tiến trình: onefile chạy một bootloader cha và một
      tiến trình con).
- [ ] Mở lại ngay lập tức → chạy được, không báo "cổng đang bận".
- [ ] Mở hai bản cùng lúc → mỗi bản tự lấy một cổng riêng, cả hai đều chạy.

## WebView2

- [ ] Trên máy chưa có WebView2 Runtime: ứng dụng báo rõ ràng và chỉ chỗ tải,
      hoặc tự lùi về mở bằng trình duyệt — **không** im lặng chết.
- [ ] Sau khi cài WebView2 Runtime, mở lại thì cửa sổ riêng hiện lên.

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
