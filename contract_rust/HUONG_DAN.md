# Hướng dẫn sử dụng ContractForge Rust

## Cài và mở ứng dụng

1. Chép `ContractForge.exe` vào thư mục bạn có quyền ghi, chẳng hạn `C:\Apps\ContractForge`.
2. Đóng ContractForge Python trước khi mở bản Rust. Hai bản có thể dùng cùng cơ sở dữ liệu khi mở lần lượt.
3. Chạy `.exe`. Mở **Cài đặt** để kiểm tra thư mục dữ liệu và thông tin Bên B trước khi sinh hợp đồng.

Khi mở bằng cách nhấp đúp, ứng dụng đọc `ContractForge.ini` cạnh `.exe`. Bản phát hành trong repo được cấu hình sẵn dùng `data` ở thư mục gốc repo. Nếu chuyển `.exe` ra nơi khác, giữ file INI cạnh nó và sửa dòng `data_root=..\..\data` thành đường dẫn dữ liệu mới. Đường dẫn tương đối được tính từ thư mục chứa `.exe`.

Tham số `--data-root` được ưu tiên cao nhất, tiếp theo là biến môi trường `CONTRACTFORGE_DATA_ROOT`, rồi đến `ContractForge.ini`. Nếu không có cấu hình, ứng dụng tìm DB trong `%LOCALAPPDATA%\ContractForge`, sau đó thư mục `data` cạnh `.exe`. Khi cài mới, ứng dụng tạo dữ liệu tại `%LOCALAPPDATA%\ContractForge`. Dùng tham số để mở bản sao thử:

```powershell
.\ContractForge.exe --data-root 'C:\ContractForge-thu-nghiem'
```

Thư mục dữ liệu chứa `contract_manager.db`, mẫu và giấy tờ trong `uploads`, tệp sinh ra trong `outputs`, hồ sơ trong `dau-noi` và nhật ký trong `logs`. DB có thể lưu đường dẫn tuyệt đối tới giấy tờ và tệp Word. Nếu chuyển thư mục, kiểm tra các đường dẫn này trước khi sinh lại tài liệu.

Mẫu mặc định đi kèm ứng dụng. Có thể đặt tệp mẫu riêng cùng tên vào `assets\default_templates` cạnh `.exe`. Với mẫu hợp đồng đã có trong danh mục, cập nhật qua **Mẫu hợp đồng**.

## Sao lưu và khôi phục

1. Xem đường dẫn trong **Cài đặt** rồi đóng cả hai bản ContractForge.
2. Chép toàn bộ thư mục dữ liệu sang vị trí lưu riêng theo ngày. Giữ cả DB, mẫu, giấy tờ và tệp đã sinh. Nếu tài liệu nằm ở vị trí khác, sao lưu các vị trí đó cùng đợt.
3. Kiểm tra bằng bản sao đã được tách đường dẫn theo [quy trình kiểm thử](docs/07_KIEM_THU.md). Khi khôi phục, đóng cả hai bản, giữ lại thư mục hiện tại rồi phục hồi bản sao về đường dẫn ban đầu để các liên kết tuyệt đối vẫn hoạt động.

## Chỉnh mẫu Word

1. Giữ một bản gốc của tệp `.docx`. Mở bản cần chỉnh bằng Word.
2. Giữ `{{KHOA}}` cho trường bắt buộc. Nếu một dòng bảng cần ẩn khi trường trống, dùng `{{?KHOA}}`, chẳng hạn `{{?PARTY_A_EMAIL}}`. Dòng bị bỏ khi mọi khóa tùy chọn trên dòng đều trống.
3. Nhập lại mẫu hoặc phân tích lại trong **Mẫu hợp đồng**, kiểm tra ánh xạ, rồi sinh thử hai hồ sơ, một hồ sơ có dữ liệu tùy chọn và một hồ sơ thiếu dữ liệu đó. Mở cả hai tệp trong Word để kiểm tra bố cục.

## Thay đổi khi chuyển từ bản Python

- Tìm kiếm chấp nhận tiếng Việt không dấu. Gợi ý số hợp đồng theo khuôn của từng gói sản phẩm.
- Trường Word có thể được điền trong bảng lồng, đầu và chân trang bổ sung, chú thích và hộp văn bản. Chữ in đậm hoặc nghiêng quanh trường được giữ lại. Bảng lặp rỗng được bỏ.
- Sinh hàng loạt chạy nền và có tiến độ. Khi hủy, các hợp đồng đã sinh xong vẫn được giữ lại.
- Trạng thái hết hạn cập nhật lúc mở ứng dụng và khi qua nửa đêm. Báo cáo Excel khách hàng áp dụng nhóm trạng thái hiệu lực mới nên số liệu có thể khác bản Python.
- Tệp sinh trùng tên được thêm hậu tố. Tên tệp báo giá tự nhập thay ký tự Windows không cho phép bằng dấu gạch ngang.

Xem [danh sách đầy đủ các khác biệt đã duyệt](docs/06_KHAC_BIET_CO_CHU_Y.md).
