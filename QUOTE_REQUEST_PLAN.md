# Kế hoạch: Yêu cầu báo giá cho đối tác tự điền

> Đã được thay thế theo yêu cầu người dùng bởi [kế hoạch sáu mẫu báo giá](QUOTATION_TEMPLATES_PLAN.md).
> Không dùng thư mời/phiếu yêu cầu làm đầu ra của tính năng báo giá mới.

## 1. Bài toán

Khách hàng mua sắm trực tiếp cần xin báo giá từ các đơn vị khác nhưng đối tác ngại soạn.
App sinh sẵn **một file Word** để khách hàng gửi đối tác. Đối tác tự điền thông tin đơn vị
và **đơn giá của chính họ**, đóng mộc rồi trả lại.

Ranh giới thiết kế: app chuẩn bị phần *yêu cầu*, không bao giờ chuẩn bị phần *giá*.
Mọi con số đơn giá, thành tiền trong file do đối tác nhập. App không có trường, nút
hay công thức nào suy giá của đối tác từ giá của mình.

## 2. File Word gồm gì

| Phần | Nội dung | Nguồn |
|---|---|---|
| Đầu văn bản | Tên, địa chỉ, MST, người đại diện của **bên mời báo giá** (khách hàng) | `Customer` |
| Tiêu đề, kính gửi, ngày, hạn nộp | Ngày lập và hạn nộp nhập trên form; "Kính gửi" để trống cho khách hàng ghi | form |
| Bảng hạng mục | STT, tên hàng hóa/dịch vụ, ĐVT, số lượng. **Cột đơn giá, VAT, thành tiền để trống** | form + `Product` |
| Bảng yêu cầu chức năng | Tên và mô tả chức năng | `Product.feature_list` (nhóm `FEATURE_`) |
| Khối đối tác | Tên đơn vị, MST, địa chỉ, người đại diện, chức vụ, điện thoại, email, thời hạn hiệu lực báo giá, ngày, chữ ký đóng dấu. **Toàn bộ để trống** | mẫu cố định |
| Chân trang | "Đơn giá do đơn vị báo giá tự xác định" | mẫu cố định |

Tên hạng mục lấy từ tên gói nhưng **sửa được trên form**, để khách hàng ghi mô tả trung
lập nếu muốn đối tác báo cho sản phẩm tương đương.

## 3. Thiết kế kỹ thuật

Tái dùng: `render_docx`, cơ chế dòng lặp `ITEM_` / `FEATURE_`, `template_service`
(loại mẫu và vai trò), `check_template_for` (khớp nhóm sản phẩm), `_content_disposition`.

Thêm mới:

- `template_service.py`: hằng `QUOTE_REQUEST_TYPE = "Yêu cầu báo giá"`,
  `ROLE_QUOTE_REQUEST`, đưa vào `CONTRACT_TYPES` và `_ROLE_TYPES`. Vì `_NON_CONTRACT`
  suy ra từ `_ROLE_TYPES`, mẫu loại này tự không lọt vào danh sách mẫu hợp đồng.
- `template_analyzer.py`: canonical `REQUEST_DATE*`, `REQUEST_DEADLINE`;
  `_template_kind` nhận ra mẫu có `REQUEST_DATE*` là loại mới (đặt trước nhánh `QUOTE_DATE`).
- `app/services/quote_request.py` (mới): hai hàm thuần, không đụng DB ghi.
  - `build_request_context(customer, request_date, deadline, ...)`: chỉ trả khóa
    `PARTY_A_*`, `REQUEST_*`, `UNIT_COUNT`, `MONTH_COUNT`. **Không có khóa nào về tiền.**
  - `build_request_lists(items, feature_lists)`: dòng `ITEM_NO/NAME/UNIT/QTY`
    và `FEATURE_*`. Không có `ITEM_PRICE` hay `ITEM_AMOUNT_*`.
  - Vì khóa giá không tồn tại trong dữ liệu, mẫu muốn hiện giá cũng không có gì để điền.
    Hàng rào nằm ở cấu trúc, không dựa vào việc ai đó nhớ kiểm tra.
- `app/routers/quote_request.py` (mới): `GET /quotation/request` (form),
  `POST /quotation/request/generate` (trả .docx). Tách file riêng, không sửa
  `routers/quotation.py`.
- `templates/quotation/request_form.html` và một liên kết trong trang báo giá/menu.
- `scripts/make_sample_templates.py`: thêm mẫu trung lập `sample_quote_request.docx`
  (bỏ ignore giống 08a để bản clone sạch vẫn chạy được).
- Tên file tải về: `YeuCauBaoGia_{slug}_{yyyymmdd}.docx`.

Không làm trong đợt này: bảng DB mới, theo dõi trạng thái đối tác, lưu lịch sử.
File sinh ra là văn bản rời, không gắn vào hợp đồng. Nếu sau này cần theo dõi thì thêm.

## 4. Các đợt

**Đợt 1 — Nền (không giao diện)**
- Loại/vai trò mẫu mới, canonical, `_template_kind`.
- `quote_request.py` với hai hàm thuần.
- Test đơn vị: xem mục 5.
- Xong khi: `pytest` xanh, `check_public_repo.py` in OK.

**Đợt 2 — Mẫu, route, form**
- Mẫu mẫu trung lập và script sinh mẫu.
- Router, form (chọn khách hàng, gói và số tháng, sửa tên hạng mục, ngày, hạn nộp, mẫu).
- Test tích hợp: sinh file, đọc lại bằng python-docx.
- **Điểm dừng**: mở file trong Word thật xem bảng, ngắt trang, in ra giấy.

**Đợt 3 — Hoàn thiện**
- `HUONG_DAN.md`, `CHANGELOG.md`, nâng phiên bản nếu muốn.
- Chạy trên bản sao DB thật với một khách hàng thật, đối chiếu hash DB gốc không đổi.

## 5. Test bắt buộc

1. Ngữ cảnh và dòng lặp **không chứa khóa về tiền** (so sánh tập khóa).
2. File sinh ra **không chứa** đơn giá hay thành tiền của gói đã chọn, kể cả khi gói
   có giá cấu hình trong DB (quét toàn bộ văn bản, bảng, đầu/chân trang).
3. Mẫu loại "Báo giá" bị từ chối ở trang yêu cầu, mẫu loại "Yêu cầu báo giá" bị từ chối
   ở trang báo giá và danh sách mẫu hợp đồng.
4. Mẫu thuộc nhóm sản phẩm khác bị từ chối (`check_template_for`).
5. Khách hàng thiếu MST, người đại diện: ra ô trống, không ra chữ `None`.
6. Danh sách chức năng rỗng thì bảng chức năng biến mất, không để dòng `{{FEATURE_...}}`.
7. Tên file tiếng Việt qua `Content-Disposition` không lỗi.
8. Không còn `{{...}}` sót lại trong văn bản sinh ra.

## 6. Việc đang dang dở cần xử lý trước

Cây làm việc có sửa đổi chưa commit ở `routers/quotation.py`, `template_analyzer.py`,
`contract_service.py`, `attachment_service.py`. Đợt 1 sửa `template_analyzer.py`, nên nên
commit hoặc cất phần đó trước để lịch sử commit không bị lẫn.

## 7. Cố ý không làm

- Tự đề xuất, tự tính hay tự điền giá cho đối tác, dưới bất kỳ tên gọi nào
  ("giá tham chiếu", "chênh lệch %", "gợi ý").
- Đặt tên, logo hay thông tin của bên mình lên văn bản của bên mời báo giá.

## 8. Có thể làm sau, chưa cam kết

- Biến vùng điền của đối tác thành ô nhập (content control) trong Word cho dễ điền.
- Bản Excel để đối tác điền rồi app đọc lại.
- Bảng so sánh các báo giá thật đã nhận.

## 9. Trạng thái (24/11/2026)

Đợt 1, 2, 3 đã hoàn tất về mã, tài liệu và kiểm chứng; chưa commit.

Khác với kế hoạch ban đầu:
- Có 3 mẫu có sẵn (thư mời, phiếu gọn, kèm yêu cầu chức năng) cài bằng nút "Cài mẫu có sẵn",
  không tự nhập vào danh mục.
- Thêm khóa `REQUEST_ISSUER_NAME` cho đầu thư, vì renderer tự chèn dòng "Mã số thuế" vào mọi bảng
  có `{{PARTY_A_NAME}}` ở ô đầu.
- Đã chạy trên bản sao DB thật: 24 file sinh ra đạt (không sót placeholder, không có giá);
  số file đầu ra, số mẫu và hash DB thật không đổi.

### Rà soát tiếp ngày 24/11/2026

- Đã chặn số lượng không hữu hạn (`NaN`, `Infinity`), giữ số 0 từ JSON gói bổ sung
  để báo lỗi thay vì tự lấy số lượng mặc định; chặn kỳ tính số lượng bằng 0 hoặc âm.
- Đã giữ dữ liệu form khi nhập lỗi (HTML 422), kể cả trường thiếu/sai định dạng, tên
  hạng mục, số lượng, ngày, ghi chú và gói bổ sung. Test kiểm tra escaping và sửa/gửi lại.
- Đã ẩn cả bảng chức năng riêng khi danh sách rỗng, qua tùy chọn renderer chỉ bật cho
  yêu cầu báo giá. Bảng hỗn hợp và hành vi mặc định của hợp đồng được giữ nguyên.
- Đã chỉnh ba mẫu A4: cột mô tả rộng, tiêu đề bảng lặp lại, dòng không bị cắt qua trang,
  khối thông tin đối tác và chữ ký nằm cùng trang. Tiêu đề chức năng nằm trong bảng.
- Đã thêm nút bổ sung mẫu mới nhất, cài bản riêng nếu mẫu đã có khác bản đi kèm;
  giữ nguyên file/bản ghi cũ, bấm lại không tạo trùng.
- Kiểm chứng trên Edge thật (Playwright): lỗi hạn nộp giữ nguyên dữ liệu, sửa lại tải
  DOCX thành công, giao diện 390px không tràn ngang, không có lỗi JavaScript.
- Kiểm chứng bằng Word thật (COM mở chỉ đọc, xuất PDF và xem từng trang): ba mẫu,
  mẫu chi tiết không có chức năng, nội dung dài 15 chức năng kèm gói bổ sung và file
  tải từ trình duyệt. Phiếu gọn/thư mời 1 trang, chi tiết 2 trang, nội dung dài 3 trang.
- Chạy lại trên bản sao DB thật: 12 file đạt, các cột giá trống, không sót placeholder;
  hash DB gốc và danh sách/kích thước/thời gian sửa file đầu ra không đổi.
- Kiểm tra cuối: `python -m pytest -q` đạt 619 test; `check_public_repo.py` in OK;
  `git diff --check` không có lỗi khoảng trắng. Cảnh báo kết nối SQLite chưa đóng khi
  kết thúc tiến trình test đã có từ lượt chạy ban đầu, vẫn còn và nằm ngoài tính năng này.

### Bản EXE ngày 24/11/2026

- Đã build bằng PyInstaller từ mã nguồn hiện tại và thay `ContractForge.exe` ở gốc repo
  sau khi kiểm tra bản đóng gói; bản cũ được giữ trong `backups/releases/`.
- Bản mới có kích thước 45.179.881 byte, SHA256
  `FB78923FF3D4DF30D4C03EF0CA0054AF82453B69D562AF8B0CAD6BB90D3E13DC`.
- Đã kiểm tra archive có router/service, form và đủ ba mẫu yêu cầu báo giá; không chứa DB.
- Đã chạy EXE trên thư mục dữ liệu thử nghiệm riêng: server sẵn sàng, WebView2 hiện menu
  và form mới, cài đủ ba mẫu, sinh DOCX có cột giá trống, lỗi nhập liệu giữ nguyên form.
  Đóng cửa sổ thì tiến trình/server dừng bình thường; hash DB thật không thay đổi.
