# Kế hoạch sửa hướng tính năng báo giá

## Mục tiêu đã xác định

Người dùng cần mẫu **báo giá**, không phải thư mời, phiếu khảo sát hay yêu cầu báo giá.
Tổng cộng **6 lựa chọn mẫu báo giá**. Cần 5 mẫu mới có placeholder để tải về điền
hoặc nhập thông tin trong ứng dụng, đồng thời dùng lại báo giá cũ của hai sản phẩm.

Bản yêu cầu báo giá đã làm trước đây không đáp ứng đầu ra này. Không chỉ đổi tiêu đề
của các thư mời: phải thay nội dung, trường dữ liệu, bảng giá và luồng nhập liệu.

## Phạm vi đã được người dùng chốt

1. Đúng 5 mẫu mới + 1 lựa chọn báo giá cũ, dùng hai file riêng theo nhóm sản phẩm.
2. Năm mẫu mới **nhập riêng** thông tin đơn vị báo giá, đơn giá và thuế; không tự lấy
   thông tin đơn vị hay giá từ cấu hình bên mình. Có thể tải mẫu placeholder để điền Word.

## Sáu lựa chọn mẫu đề xuất

| Lựa chọn | Nội dung và bố cục |
|---|---|
| 1. Báo giá gọn | Thông tin bên báo giá, kính gửi, bảng giá, tổng cộng, hiệu lực, ký tên; ưu tiên một trang với ít hạng mục |
| 2. Báo giá trang trọng | Đầu văn bản đơn vị phát hành, số/ngày báo giá, nội dung chào giá, bảng giá, điều kiện và chữ ký |
| 3. Báo giá bảng ngang | Bảng rộng cho nhiều hạng mục, thể hiện số lượng, kỳ, đơn giá, thuế và thành tiền rõ ràng |
| 4. Báo giá kèm chức năng | Bảng giá và bảng chức năng/phạm vi dịch vụ; không có chức năng thì bỏ cả bảng riêng |
| 5. Báo giá kèm điều kiện cung cấp | Bảng giá, phạm vi cung cấp, triển khai, hỗ trợ/bảo hành, thanh toán, hiệu lực và chữ ký |
| 6. Báo giá cũ | Giữ hai file theo hai sản phẩm ở bên dưới một lựa chọn; chọn sản phẩm nào dùng đúng file của sản phẩm đó |

Năm mẫu mới dùng cho cả hai nhóm sản phẩm. "Hai sản phẩm" không tự được hiểu là
ghép hai gói chính khác nhóm vào cùng một báo giá; đó là thay đổi nghiệp vụ riêng.

## Trường dữ liệu và đầu ra

- Bên báo giá: tên đơn vị, địa chỉ, MST, người đại diện/chức vụ, điện thoại, email,
  thông tin ký tên; không ghi cố định thông tin đơn vị thật vào mẫu mới.
- Bên nhận báo giá: tên, địa chỉ, người nhận/liên hệ nếu có.
- Báo giá: số, ngày, tiêu đề, thời hạn hiệu lực, ghi chú.
- Dòng giá: STT, tên hàng hóa/dịch vụ, ĐVT, số lượng, kỳ/thời gian, đơn giá, thuế suất,
  tiền trước thuế, tiền thuế, tiền sau thuế; dòng có thể lặp.
- Tổng: trước thuế, thuế, sau thuế, bằng chữ.
- Điều kiện: triển khai, hỗ trợ/bảo hành, thanh toán theo nội dung đã nhập.
- Hai đầu ra: tải **mẫu DOCX có placeholder**; hoặc **sinh báo giá DOCX đã điền** từ form.
  Không tự thêm chức năng nhập Excel/CSV khi chưa có yêu cầu.
- Phép tính tiền tái dùng `app/services/pricing.py`, không đặt công thức riêng trong mẫu
  hay JavaScript. Đơn giá và thông tin đơn vị do người dùng nhập, thiếu giá phải báo lỗi.

## Thứ tự thực hiện

### 1. Chốt đặc tả và làm một mẫu hoàn chỉnh

- Ghi nhận hai câu trả lời vào kế hoạch, chốt danh sách đúng sáu lựa chọn.
- Xác định bộ placeholder chung và ánh xạ với renderer hiện có.
- Làm mẫu báo giá gọn cùng form nhập/sinh/tải; kiểm tra mẫu đầu ra trước khi nhân thành
  các bố cục còn lại.
- File chính: `app/routers/quotation.py`, dịch vụ báo giá, `templates/quotation/form.html`,
  `app/services/template_analyzer.py`, `scripts/make_sample_templates.py` và test tương ứng.

### 2. Hoàn thiện đủ năm mẫu mới và báo giá cũ

- Tạo bốn bố cục còn lại từ cùng bộ dữ liệu; các mẫu mới phân loại "Báo giá".
- Giữ nội dung và cấu trúc hai file cũ; lựa chọn cũ tự tìm mẫu đúng nhóm sản phẩm.
- Không ghi đè mẫu người dùng đã tải lên hoặc chỉnh sửa.
- Màn hình báo giá có đúng sáu lựa chọn bố cục theo phương án đã chốt; các biến thể
  file của mẫu cũ không làm danh sách thành bảy lựa chọn.

### 3. Sửa luồng đang làm sai

- Dùng một màn hình "Báo giá" rõ ràng cho đầu ra này; không tiếp tục quảng bá thư mời
  hay phiếu yêu cầu như mẫu báo giá.
- Thay menu/đường vào "Yêu cầu báo giá" đang gây nhầm bằng luồng đã thống nhất.
- Bỏ ba mẫu thư mời/phiếu yêu cầu khỏi bộ mẫu được đề xuất trong luồng báo giá mới.
  Giữ bản ghi/file cũ để không phá tài liệu hay mẫu người dùng đang lưu.
- Nhập sai phải giữ dữ liệu; chuyển sản phẩm phải dùng đúng biến thể mẫu cũ.

### 4. Nghiệm thu trên nguồn và Word thật

- Năm mẫu mới × hai nhóm sản phẩm, cộng hai biến thể mẫu cũ: ít nhất 12 đường sinh file.
- Tải mẫu thô còn đủ placeholder; file đã sinh không còn placeholder chưa điền ngoài
  những trường người dùng chủ động để lại trong chế độ mẫu.
- Kiểm tra tổng tiền khớp dữ liệu nhập, thông tin bên báo giá/bên nhận không bị đảo.
- Xác minh nhập liệu, sửa lỗi, tải file bằng trình duyệt; không thay đổi hợp đồng cũ.
- Mở bằng Word thật, xuất và xem từng trang: bảng nhiều dòng, chức năng rỗng, tên dài,
  tiêu đề lặp, ngắt trang và chữ ký. Mẫu mới không còn nội dung thư mời/yêu cầu báo giá.
- Chạy toàn bộ pytest, kiểm tra nội dung repo và lỗi khoảng trắng.

### 5. Đóng gói và bàn giao đúng EXE

- Build lại bằng `python -m PyInstaller --clean --noconfirm packaging/pyinstaller/contractforge.spec`.
- Kiểm tra đủ tài nguyên báo giá trong archive; chạy chính EXE với dữ liệu thử nghiệm
  riêng, không chỉ kiểm tra server từ mã nguồn.
- Xác nhận người dùng nhìn thấy sáu lựa chọn, điền và tải được báo giá từ bản đóng gói.
- Thay `ContractForge.exe` tại gốc dự án sau khi kiểm tra; xác minh hash file bàn giao
  trùng file đã chạy thử. Cập nhật hướng dẫn và báo rõ đường dẫn EXE cuối cùng.

## Điều kiện hoàn tất

Người dùng mở EXE được bàn giao, vào Báo giá, thấy đúng sáu lựa chọn theo phương án
đã chốt, tải được mẫu placeholder hoặc nhập thông tin để sinh báo giá; mẫu cũ của cả
hai sản phẩm vẫn dùng được. Không coi hoàn tất khi mới sửa mã nguồn hoặc chỉ đổi nhãn.

## Kết quả thực hiện ngày 24/11/2026 — phiên bản 0.10.1

- Đã tạo năm DOCX `sample_quote_compact/formal/landscape/features/terms.docx` cùng
  bộ placeholder riêng cho đơn vị báo giá. Mẫu mới đọc trực tiếp từ tài nguyên đóng
  gói theo allowlist; dùng ngay, không cần migration hoặc cài bản ghi mới vào DB.
- `/quotation` vào màn hình nhập riêng, có đúng sáu lựa chọn. Lựa chọn cũ vào
  `/quotation/legacy`, giữ endpoint sinh cũ và bộ lọc mẫu theo nhóm sản phẩm.
- Đã có tải mẫu placeholder, nhập từng hạng mục, xem trước tổng, giữ form khi lỗi,
  đơn giá thập phân được in đúng. Mẫu mới không lấy thông tin đơn vị/giá danh mục.
- Đã kiểm tra 655 test đạt; kiểm tra repo in OK; không có lỗi khoảng trắng.
  Cảnh báo đóng kết nối SQLite lúc kết thúc test đã có từ trước, vẫn còn.
- Edge thật: đủ sáu lựa chọn, không tự điền đơn vị/giá, xem trước 648.000 đúng,
  sửa lỗi và tải cả mẫu thô/bản đã điền, chuyển mẫu cũ, màn hình 390px, không lỗi JS.
- Word thật: năm mẫu bình thường đều một trang với dữ liệu thử; chức năng rỗng bỏ
  cả bảng riêng, dữ liệu dài lặp tiêu đề và giữ hàng/chữ ký. Hai mẫu cũ vẫn sinh được
  đầy đủ phụ lục 4/5 trang. Đã xem bản PDF xuất từ Word để kiểm tra bố cục.
- Bản sao DB thật: năm mẫu mới × hai sản phẩm + hai đường sinh mẫu cũ đạt; không
  sót placeholder. Hash DB gốc và hash hai mẫu báo giá cũ không đổi.
- Đã build và thay EXE gốc bằng bản 0.10.1, 45.390.960 byte, SHA256
  `748C5CD5C71905B6BF7D8C2A33E387F487872DAEB9C7C90C3DC994AF350600AD`.
  Archive có đủ năm mẫu và form/router/service mới, không gói DB.
- Đã chạy chính EXE bàn giao với DB thử nghiệm riêng: WebView2 có sáu lựa chọn,
  giá/đơn vị độc lập, tổng tiền đúng, hai mẫu cũ đổi theo sản phẩm; đóng cửa sổ
  thì server/tiến trình dừng. Hash DB thật giữ nguyên.

## Bổ sung lưu đơn vị báo giá — 0.10.2

- Theo yêu cầu tiếp theo của người dùng: có thể lưu bảy trường thông tin đơn vị để
  chọn lại lần sau. Dropdown "Đơn vị đã lưu" và nút "Lưu thông tin đơn vị" nằm ngay
  trong khối bên báo giá; không cần nhập xong khách hàng hoặc giá mới lưu được.
- Lưu vào bảng `quotation_issuers` riêng, đăng ký qua metadata nên DB cũ tự thêm
  bảng khi khởi động. Không dùng hồ sơ tổ chức, không lưu giá/khách hàng/điều kiện.
- Bản trùng hoàn toàn được dùng lại; thông tin đã sửa lưu thành bản riêng. Chọn
  hồ sơ chỉ điền bảy trường đơn vị; sinh báo giá dùng nội dung đang có trên form
  và không tự ghi đè hồ sơ đã lưu. Trang mới vẫn để trống cho đến khi chọn.
- Kiểm chứng: 663 test đạt; Edge thật lưu, tải lại, chọn lại, tải Word, kiểm tra
  bản cũ giữ nguyên sau chỉnh sửa, màn hình 390px không tràn, không có lỗi JS.
- Bản sao DB thật: thêm bảng, lưu đơn vị và khởi tạo lặp không đổi hàng nào trong
  các bảng cũ; hash DB gốc giữ nguyên.
- Bản EXE 0.10.2 đã chạy hai lần trên cùng dữ liệu thử: lưu bằng nút trong WebView2,
  đóng ứng dụng, mở lại, chọn và sinh Word thành công; thoát sạch, DB thật không đổi.
- Đã thay EXE tại gốc repo; 45.400.118 byte, SHA256
  `1170FCCD81A682F516E2A208A0021B3A418B307095CB7E536511CB1CA2B197B7`.
