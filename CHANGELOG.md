# Changelog

Định dạng theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
đánh phiên bản theo [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0] — chưa phát hành

Danh mục gói cước, hợp đồng theo dòng, phụ lục và gia hạn, văn bản kèm theo.

### Thêm mới

- **Danh mục sản phẩm** thêm, sửa, ngừng bán và bật lại trên giao diện; nhập và
  xuất bằng Excel (xem trước trước khi ghi, không ghi một phần khi có dòng lỗi).
  Sản phẩm có nhóm, kiểu gói chính / bổ sung / lưu trữ, đơn vị tính, kỳ giá.
- **Hợp đồng theo dòng:** một hợp đồng có gói chính và các gói bổ sung cùng nhóm;
  tiền tính theo từng dòng trên máy chủ. Hợp đồng cũ giữ nguyên số đã lưu.
- **Báo giá nhiều dòng** và **sinh hàng loạt theo dòng**, dùng cùng một cách tính.
- **Biểu mẫu Word** lặp dòng cho bảng giá trị và bảng chức năng.
- **Phụ lục bổ sung gói** giữa kỳ: thời hạn là phần còn lại của hợp đồng gốc.
- **Gia hạn** bằng phụ lục gia hạn hoặc bằng hợp đồng mới, người dùng chọn.
  Hợp đồng đã gia hạn không còn nằm trong danh sách sắp hết hạn.
- **Văn bản kèm theo** (nghiệm thu, bàn giao, thanh lý, khác) sinh từ mẫu bất kỳ
  của loại đó, không đổi trạng thái hợp đồng.
- **Xuất Excel** có thêm trước thuế, thuế, loại văn bản, hợp đồng gốc và sheet
  chi tiết từng dòng.
- **Cài đặt:** khuôn số phụ lục, và đầu mối liên hệ của Bên B.

### Thay đổi

- Giá trên form hợp đồng và báo giá do máy chủ tính; trình duyệt không còn tính tiền.
- Khi xóa hợp đồng còn phụ lục, hệ thống từ chối và nói rõ cần xóa phụ lục trước.

## [0.8.0] — chưa phát hành

Bản đầu tiên đủ điều kiện công khai mã nguồn.

### Sửa lỗi

- **Hợp đồng đã ký gần hết hạn không còn bị khoá.** Bản quét tự động đổi
  `Signed → ExpiringSoon` khi còn ≤30 ngày, mà `ExpiringSoon` không có lối đi
  tới `Invoiced` — càng gần hạn thì càng không xuất hoá đơn được. "Sắp hết hạn"
  nay được tính từ `end_date` mỗi lần hiển thị, không còn ghi đè trạng thái
  nghiệp vụ thật.
- **Đổi trạng thái hàng loạt không còn bỏ qua kiểm tra.** Trước đây thao tác này
  gán thẳng trạng thái: bỏ qua bảng chuyển, bỏ qua yêu cầu hoá đơn/uỷ nhiệm chi,
  và không ghi lịch sử. Nay dùng chung đúng bộ luật với đổi từng hợp đồng, theo
  chính sách tất-cả-hoặc-không.
- **Dashboard, danh sách khách hàng và báo cáo Excel đếm khớp nhau.** Sáu nơi tự
  định nghĩa "trạng thái nào là đang hiệu lực", mỗi nơi thiếu một trạng thái
  khác nhau.
- Trang "sắp hết hạn" không còn hiện `?` cho hợp đồng đã xuất hoá đơn / đã thanh
  toán, và bộ đếm mức khẩn không còn bỏ sót dòng khi xem dải 60 hay 365 ngày.
- Xoá hợp đồng nay xoá luôn file `.docx` và các chứng từ PDF của nó, thay vì bỏ
  lại trên đĩa vĩnh viễn.
- Bản đóng gói `.exe` khởi động được. Bản dựng dạng cửa sổ có `sys.stdout = None`,
  làm bộ định dạng log mặc định của uvicorn ném lỗi và server không chạy nổi.

### Bảo mật

- Mọi launcher chỉ bind `127.0.0.1`. Hai script khởi động cũ mở `0.0.0.0:8888`,
  tức mọi thiết bị trong mạng LAN đều vào được một cơ sở dữ liệu không có xác thực.
- Chặn yêu cầu thay đổi dữ liệu đến từ nguồn khác máy này (`app/security.py`).
- Vá XSS lưu trữ ở ô tìm kiếm toàn cục và trang sinh hàng loạt — tên khách hàng
  bị nối thẳng vào `innerHTML`. Dữ liệu nhúng vào `<script>` chuyển sang `| tojson`.
- Sửa đơn vị con nay kiểm cả chủ sở hữu: đổi id trong form không còn ghi đè được
  đơn vị của khách hàng khác.

### Thay đổi

- **Khuôn số hợp đồng thành cấu hình** trên từng sản phẩm (`{seq}/{slug}/{year}`)
  thay vì viết cứng trong Python. Bản cài đặt đang chạy được migration suy ngược
  khuôn từ chính các hợp đồng đã có, nên cách đánh số giữ nguyên tuyệt đối.
- **Ứng dụng desktop có cửa sổ riêng.** Server chạy trong tiến trình chính trên
  một cổng trống do hệ điều hành cấp, hiện trong cửa sổ WebView2. Đóng cửa sổ là
  server dừng — không còn tiến trình mồ côi giữ khoá cơ sở dữ liệu. Bản đóng gói
  Windows nặng khoảng 42 MB và không cần cài Python.
- Nội dung công việc trong biểu 08a lấy từ tên sản phẩm trong danh mục (bỏ phần
  viết tắt trong ngoặc) thay vì hai nhãn viết cứng. **Chữ in ra sẽ khác một chút
  so với bản cũ** — đổi tên sản phẩm trong danh mục nếu muốn chỉnh.
- Báo cáo khách hàng dùng mã sản phẩm trong danh mục và so với danh mục thực tế,
  nên tự đúng khi thêm sản phẩm mới.
- Giá tự điền trên form đọc từ danh mục sản phẩm, không còn bảng giá viết cứng
  trong template.
- Uvicorn không nạp lớp WebSocket nữa — ứng dụng không có endpoint nào dùng tới.
- `main.py` chuyển từ `@app.on_event` (đã bị FastAPI khai tử) sang `lifespan`.

### Thêm mới

- README, LICENSE (MIT), CONTRIBUTING, SECURITY, CHANGELOG.
- `app/services/lifecycle.py` — nơi duy nhất giữ luật vòng đời hợp đồng.
- `app/services/numbering.py` — khuôn số hợp đồng dạng dữ liệu.
- Mẫu `sample_form_08a.docx` hư cấu, để bản clone sạch dùng được biểu 08a.
- Workflow đóng gói Windows, có kiểm tra không lộ dữ liệu trước khi build.
- Bộ test từ 70 lên 216, gồm cả checklist công bố chạy tự động.

### Gỡ bỏ

- Phụ thuộc `aiosqlite` không dùng tới.
- Sáu quy tắc CSS `.ttype-*` không nơi nào tham chiếu.
