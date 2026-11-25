# 05 — Giao diện (Slint)

## 1. Nguyên tắc

- **Giữ bố cục quen thuộc** của bản web: thanh bên trái rộng 240 px + vùng nội dung; cùng tên
  menu, cùng thứ tự cột, cùng nhãn tiếng Việt. Người dùng không phải học lại.
- **Không tính toán nghiệp vụ trong Slint.** File `.slint` chỉ hiển thị và phát callback. Mọi
  con số (tiền, ngày kết thúc, gợi ý số) do `cf-services` tính rồi đẩy vào model.
  (Bản web có JS tự tính ngày kết thúc theo cách khác server — xem [KB-10].)
- Style: `fluent` của Slint (`std-widgets`), font hệ thống **Segoe UI** (hỗ trợ đủ tiếng Việt).
- Mỗi màn hình là một component `.slint` trong `crates/cf-app/ui/screens/`, có
  một struct Rust "view model" tương ứng trong `crates/cf-app/src/screens/`.
- Danh sách dài dùng `ListView`/`StandardTableView` (ảo hóa, chỉ vẽ dòng nhìn thấy).
- Tiền hiển thị bằng `fmt_currency`; ngày bằng `dd/mm/yyyy`; nhãn trạng thái + màu từ
  `STATUS_LABELS` (map màu Bootstrap → bảng màu cố định trong `ui/theme.slint`:
  secondary xám, info xanh nhạt, primary xanh dương, success xanh lá, warning cam, danger đỏ, dark đen).

## 2. Khung ứng dụng

```
┌───────────────┬──────────────────────────────────────────────┐
│ ContractForge │  [Tiêu đề màn hình]          [Ô tìm kiếm toàn cục]│
│               ├──────────────────────────────────────────────┤
│ Dashboard     │                                              │
│ Khách hàng    │              nội dung màn hình               │
│ Sản phẩm      │                                              │
│ Mẫu hợp đồng  │                                              │
│ DS hợp đồng   │                                              │
│ Sinh hợp đồng │                                              │
│ Sinh hàng loạt│                                              │
│ Báo giá       │                                              │
│ Cài đặt       │   ┌ toast: thông báo thành công/cảnh báo/lỗi ┐│
│ v{phiên bản}  │   └─────────────────────────────────────────┘│
└───────────────┴──────────────────────────────────────────────┘
```

- Điều hướng: `enum Screen` trong Rust; có nút **Quay lại** (ngăn xếp lịch sử tối đa 20).
- Toast thay cho flash message: tự ẩn sau 5 giây (lỗi: không tự ẩn).
- Ô tìm kiếm toàn cục: gõ → gợi ý khách hàng + hợp đồng (không dấu) **[KB-11]**, Enter mở mục đầu.
- Hộp thoại xác nhận (xóa, đổi trạng thái hàng loạt) là `PopupWindow` của Slint.

## 3. Danh sách màn hình

Cột "Nguồn" là file Jinja/route của bản web để đối chiếu bố cục và câu chữ.

| # | Màn hình | Nguồn | Nội dung chính |
|---|---|---|---|
| S01 | Dashboard | `dashboard/index.html` | Thẻ số liệu (§14 nghiệp vụ), biểu đồ cột dự báo hết hạn 12 tháng, biểu đồ tròn theo sản phẩm, danh sách ≤30 / 31–60 ngày, việc cần làm, hoạt động gần đây, thống kê tháng, top khách hàng |
| S02 | Sắp hết hạn | `dashboard/expiring.html` | Chọn 7/30/60/90/365 ngày, bảng có màu mức khẩn, Xuất Excel |
| S03 | Danh sách khách hàng | `customers/list.html` | Lọc (từ khóa, xã/phường, loại, đang hoạt động), sắp xếp, phân trang, chọn nhiều → xóa; Nhập Excel; Xuất báo cáo; Thùng rác |
| S04 | Khách hàng — form | `customers/form.html` | Các trường hồ sơ, chọn tỉnh/xã từ `dia_chinh.json`, bảng đơn vị con thêm/xóa dòng |
| S05 | Khách hàng — chi tiết | `customers/detail.html` | Hồ sơ, cơ sở tính phí (tick + Lưu), giấy tờ (upload/xem/xóa), danh sách hợp đồng, xuất thông tin, tải gói hồ sơ, nút tạo hợp đồng |
| S06 | Hồ sơ đấu nối | `customers/dossier.html` | Bảng các ô (đủ/thiếu, nguồn, tên đích), Tạo hồ sơ, Mở thư mục, cảnh báo hồ sơ cũ |
| S07 | Nhập khách hàng | `customers/import.html` | Chọn file, kiểu phẳng/nhóm, kết quả từng dòng |
| S08 | Danh sách sản phẩm | `products/list.html` | Nhóm theo `group_code`, gói chính trước, số HĐ đang phục vụ, ngừng bán/bán lại |
| S09 | Sản phẩm — form | `products/form.html` | Mọi trường của `clean_form`, ô chức năng nhiều dòng `Tên \| Mô tả`, hiện **tất cả** lỗi cùng lúc |
| S10 | Nhập danh mục | `products/import.html` | Đọc file → xem trước từng dòng (tạo/cập nhật/lỗi) → Áp dụng |
| S11 | Danh sách mẫu | `contract_templates/list.html` | Lọc theo loại/nhóm, bật/tắt, tải lên nhiều file, nút "Dọn file mẫu thừa" [KB-12] |
| S12 | Mẫu — form + chi tiết | `contract_templates/form.html`, `detail.html` | Thông tin mẫu; bảng placeholder → canonical (dropdown `ALL_CANONICAL_FIELDS`); kết quả phân tích; hướng dẫn `{{?KHOA}}` nếu `uses_legacy_party_a` |
| S13 | Danh sách hợp đồng | `contracts/list.html` | Bộ lọc §7.1, viên chọn năm, huy hiệu cảnh báo, tổng tiền kết quả lọc, chọn nhiều → đổi trạng thái/xóa, Xuất Excel |
| S14 | Sinh hợp đồng (form) | `contracts/form.html` + `static/js/contract_items.js` | Xem §4 |
| S15 | Hợp đồng — chi tiết | `contracts/detail.html` | Thông tin, bảng dòng, phụ lục/hợp đồng gốc, timeline sự kiện, đổi trạng thái (chỉ các bước hợp lệ), 3 khối chứng từ, sinh lại file (tick văn bản kèm theo), mở file/ZIP, biểu 08a, văn bản kèm theo lẻ, tạo phụ lục, gia hạn |
| S16 | Phụ lục bổ sung | `contracts/addendum_form.html` | Ngày bắt đầu (kẹp trong kỳ gốc), số gợi ý, chọn gói bổ sung, chọn mẫu |
| S17 | Gia hạn | `contracts/renewal_form.html` | Chọn phụ lục gia hạn / hợp đồng mới, dòng gợi ý, so sánh giá theo tháng |
| S18 | Sinh hàng loạt | `batch/form.html`, `preview.html`, `result.html`, `import.html` | Chọn mẫu + khách hàng + tham số → Xem trước → Chạy (thanh tiến độ, Hủy) [KB-13] → Kết quả + Lưu ZIP |
| S19 | Báo giá | `quotation/form.html` | 3 tab: Theo danh mục / Tự nhập (`manual_form.html`) / Yêu cầu báo giá (`request_form.html`) |
| S20 | Cài đặt | `settings/index.html` | Hồ sơ Bên B, khuôn số phụ lục, thư mục dữ liệu |

## 4. Màn hình Sinh hợp đồng (S14) — hành vi chi tiết

Chép từ JS của bản web, **chuyển phần tính toán sang Rust**:

| Sự kiện | Hành vi |
|---|---|
| Gõ ô lọc khách hàng | Lọc dropdown theo `search_key` (không dấu); còn đúng 1 kết quả và chưa chọn → tự chọn |
| Chọn khách hàng | (1) gợi ý số HĐ nếu ô số chưa sửa tay; (2) điền địa điểm ký = tên pháp lý với **chữ đầu viết thường**, nếu chưa sửa tay; (3) nạp danh sách cơ sở; (4) nạp lại tùy chọn gói |
| Danh sách cơ sở | Trạm chính (nếu `bill_main_station`, tick sẵn) + đơn vị con đang hoạt động (tick theo `include_in_billing`); số cơ sở = số mục đã tick (ô số cơ sở chỉ đọc); nút Chọn hết / Bỏ hết |
| Chọn gói chính | Nạp `product_options`: mẫu hợp đồng của nhóm (chọn sẵn mẫu mặc định), văn bản kèm theo (tick sẵn theo `preselect`), bảng dòng (gói chính + gói bổ sung cùng nhóm có ô tick); xóa ô "bằng chữ" và "mô tả" tự sinh; gợi ý lại số HĐ |
| Chưa chọn gói | Ô mẫu bị khóa với chữ "— Chọn sản phẩm trước —" |
| Sửa số lượng/đơn giá/thuế/tick gói bổ sung/số tháng | Gọi `price_preview` (debounce 300 ms); hiện bảng dòng + tổng + bằng chữ + mô tả; lỗi → hiện đỏ dưới bảng và khóa nút Lưu |
| Ô "bằng chữ"/"mô tả" | Sửa tay → không bị ghi đè bởi xem trước nữa (cờ `manual`) |
| Đổi ngày ký | Nếu ngày hiệu lực chưa sửa tay → ngày hiệu lực = ngày ký |
| Đổi ngày hiệu lực hoặc số tháng | Ngày kết thúc = `compute_end_date` (Rust) [KB-10]; ngày thanh lý = ngày kết thúc; ngày nghiệm thu = ngày hiệu lực + 2 ngày — mỗi ô chỉ tự điền khi chưa sửa tay |
| Hai ô số tháng (Thời hạn, Giá trị) | Luôn đồng bộ với nhau |
| Cảnh báo ngày (không chặn lưu) | nghiệm thu < hiệu lực; thanh lý < kết thúc; thanh lý < nghiệm thu; ký > hiệu lực |
| Hồ sơ Bên B chưa đầy đủ | Băng cảnh báo trên đầu form, có liên kết tới Cài đặt |
| Lưu | `create_draft` → nếu có mẫu thì `generate_docx` (kèm văn bản đã tick). Lỗi → giữ nguyên mọi ô đã nhập, hiện lỗi đầu form |

## 5. Biểu đồ trên Dashboard

- Cột: 12 cột, nhãn `T{m}/{yy}`, chiều cao tỷ lệ với số lớn nhất (tối thiểu 1). Vẽ bằng `Rectangle`.
- Tròn: theo sản phẩm, vẽ bằng `Path` (cung tròn), chú thích bên cạnh với số lượng và giá trị.
- Không cần thư viện biểu đồ.

## 6. Đợt 0 — bản thử UI (bắt buộc trước khi làm màn hình thật)

Một cửa sổ có: ô nhập text, `StandardTableView` 1.000 dòng tiếng Việt, một biểu đồ cột.
Kiểm và ghi kết quả vào `docs/DOT0_KET_QUA.md`:
1. Gõ tiếng Việt bằng **Unikey** và **EVKey**, kiểu Telex và VNI: "Trạm y tế xã Đồng Tâm",
   "Nguyễn Thị Hương", "Quyết định số 12/QĐ-UBND". Không mất dấu, không nhân đôi ký tự.
2. Hiển thị đúng dấu ở cỡ 10–14 pt, không vỡ chữ.
3. Cuộn bảng 1.000 dòng mượt.
4. RAM sau khi mở (Task Manager) với hai renderer `femtovg` và `skia` → chọn cái nhẹ hơn mà
   qua mục 1–3. Ghi phiên bản Slint dùng.
Nếu mục 1 không qua với cả hai renderer: **dừng**, báo chủ dự án trước khi làm tiếp.
