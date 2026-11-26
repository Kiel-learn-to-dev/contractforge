# Nghiệm thu bản Rust (ĐD5)

## Phạm vi dữ liệu

- Ngày kiểm tra: 2026-11-26.
- DB dùng cho thử nghiệm là bản sao của `data/contract_manager.db`, gồm 89 hợp đồng, 66 khách hàng và 21 mẫu. Đường dẫn tệp lưu trong DB được chuyển sang thư mục thử trước khi chạy ứng dụng.
- Có hai DB cần phân biệt. DB trong `data` có dữ liệu nghiệp vụ. DB mặc định trong `%LOCALAPPDATA%\ContractForge` hiện không có hợp đồng hoặc khách hàng. Khi chuyển sang bản Rust với DB trong `data`, chỉ định thư mục qua `--data-root` hoặc `CONTRACTFORGE_DATA_ROOT`.

## Bản phát hành

`contract_rust/dist/ContractForge.exe` có kích thước 28.393.984 byte. Đã kiểm tra tài nguyên icon và chế độ Windows GUI trong tệp PE. Một bản chép chỉ gồm `.exe` mở được ngoài repo với bản sao DB nghiệp vụ và giữ đủ 21 mẫu đã có. Lượt này cửa sổ và log khởi động xuất hiện sau khoảng 660 ms. Với thư mục dữ liệu trống, bản chép tự tạo DB và nạp một mẫu hợp đồng dịch vụ nhúng.

Ngày 2026-11-26, `ContractForge.ini` cạnh `.exe` được thêm để nhấp đúp mở `data/contract_manager.db` ở repo. Tệp dùng `data_root=..\\..\\data` tương đối với vị trí `.exe`. Đã thử bản chép `.exe` + INI từ một thư mục làm việc khác trên bản sao 89 hợp đồng; startup log xác nhận đúng thư mục DB, và SHA-256 DB nguồn giữ nguyên. Tham số CLI và biến môi trường tiếp tục được ưu tiên trên INI.

Phép thử dịch vụ xác nhận báo giá tự nhập và biểu 08a sinh được từ mẫu nhúng. Mẫu riêng đặt cạnh tệp thực thi được ưu tiên cho cả báo giá và biểu 08a. Việc mở kết quả bằng Word và thử gõ qua bộ gõ tiếng Việt nằm trong danh sách nghiệm thu thủ công.

## Hiệu năng

Windows 11 Home Single Language, AMD Ryzen AI 7 350, RAM 32 GB. Đo ba lượt với bản đo biên dịch theo profile release, renderer FemtoVG và bản sao DB nghiệp vụ. Bản đo dùng callback Slint của bản ứng dụng để mở danh sách và gửi từng lần cập nhật bộ lọc. Bản sao đã có thêm một mẫu đi kèm sau lần chạy đầu của bản đo.

| Hạng mục | Mục tiêu | Kết quả | Đánh giá |
|---|---:|---:|---|
| RAM riêng đang thường trú khi mở danh sách | ≤ 80 MB | 141,0–152,3 MiB | Chưa đạt G1 |
| Từ đầu `main` đến khung hình đầu | < 1.000 ms | 1.535 ms lượt đầu, 416–421 ms hai lượt sau | Lượt đầu chưa đạt G3 |
| Tải dữ liệu dashboard | < 100 ms | < 1 ms cả ba lượt | Đạt G6 theo log tải |
| Từ callback lọc đến cập nhật dữ liệu hiển thị | < 50 ms | 0–1 ms mỗi lần, bốn lần mỗi lượt | Đạt G6 theo log cập nhật |

Working set toàn tiến trình 183,4–200,5 MiB. Thời gian trước khi vào vòng lặp giao diện 132–349 ms, còn mốc khung hình đầu được đo bằng `AfterRendering`. Trong lượt đo đầu, tiến trình không có WebView con, cổng TCP hoặc cổng UDP tại thời điểm lấy mẫu. Xem hai việc khắc phục trong [kế hoạch P5](../tasks/P5_dong_goi.md#t503--đo-hiệu-năng--xs).

## Đối chiếu tài liệu

Sinh lại 10 hợp đồng trên hai bản sao độc lập của DB rồi so từng đoạn trong DOCX. Kết quả không có khác biệt văn bản ngoài KB-03/KB-05. Python thêm một đầu trang mặc định với đúng một đoạn rỗng trong mỗi hợp đồng. Rust giữ nguyên cấu trúc mẫu gốc vốn không có phần đầu trang đó. Phép kiểm xác minh mã băm DB nguồn giữ nguyên.

Đã sửa chế độ mẫu cũ để chèn thông tin tùy chọn vào mọi bảng Bên A cấp một phù hợp. Test tạo hai bảng Bên A xác nhận các trường được điền vào cả hai bảng.

Để chạy lại từ thư mục `contract_rust`:

```powershell
cargo build --locked -p cf-services --bin cf-compare-generate
python .\tools\compare_outputs.py ..\data\contract_manager.db --rust-binary .\target\debug\cf-compare-generate.exe
```

## Dashboard và báo cáo khách hàng

Đã đối chiếu dữ liệu dashboard trên hai bản sao DB đồng nhất, sau khi mỗi bản ứng dụng cập nhật trạng thái đến hạn. Các số liệu khớp ở 18 chỉ số tổng hợp, sáu nhóm trạng thái có dữ liệu, hai danh sách sắp hết hạn, nhóm sản phẩm, thống kê tháng, dự báo 12 tháng và danh sách việc cần làm. Số hợp đồng ở cả năm vị trí của bảng xếp hạng khớp. Ở vị trí thứ năm, hai bản chọn khách hàng khác nhau trong nhóm đồng hạng. DB nguồn giữ nguyên.

Đã sửa báo cáo Excel để đếm hợp đồng theo nhóm trạng thái hiệu lực dùng chung và lấy nhãn trạng thái từ ứng dụng. Đọc trực tiếp tệp XLSX và đối chiếu năm khách hàng trên bản sao DB. Số hợp đồng hiệu lực, giá trị hiệu lực và phân hạng khớp ở cả năm trường hợp.

## Sao lưu trước chuyển đổi

Ngày 2026-11-26 đã tạo hai bản sao SQLite bằng backup API trong `backups/contractforge-rust-p5-20261126-064735/`. `repo-data-contract_manager.db` là bản sao DB nghiệp vụ và `localappdata-contract_manager.db` là bản sao DB mặc định. Cả hai đều qua `PRAGMA quick_check`, số bản ghi đối chiếu khớp và SHA-256 của từng DB nguồn không đổi.

Bản sao SQLite chỉ chứa DB. Thực hiện thêm sao lưu thư mục dữ liệu và tài liệu ngoài thư mục theo [hướng dẫn](../HUONG_DAN.md) khi chuyển vị trí lưu dữ liệu.

Khi đã ký ĐD5, đóng bản Python. Từ gốc repo, dùng lệnh sau nếu `data` là thư mục vừa được xác nhận:

```powershell
.\contract_rust\dist\ContractForge.exe --data-root (Resolve-Path .\data).Path
```

Nếu cần quay về bản Python, đóng bản Rust trước rồi mở bản Python với cùng thư mục dữ liệu. Chỉ khôi phục bản sao DB trước chuyển đổi khi cần quay lại thời điểm sao lưu. Giữ một bản sao của DB hiện hành trước khi khôi phục để có thể lấy lại những thay đổi mới hơn.

## Xác nhận của chủ dự án

- [ ] Đã đọc [hướng dẫn sử dụng](../HUONG_DAN.md).
- [ ] Đã hoàn tất các kiểm tra trong [danh sách nghiệm thu](07_KIEM_THU.md#4-nghiệm-thu-cuối-trước-khi-xóa-bản-python).
- [ ] Đã xác nhận thư mục dữ liệu sẽ dùng và ký nghiệm thu ĐD5.
- [ ] Đã dùng bản Rust ổn định hai tuần kể từ ngày chuyển đổi và quyết định việc giữ hay gỡ bản Python.
