# 08 — Lộ trình

Mỗi đợt kết thúc bằng một **điểm dừng**: chủ dự án kiểm rồi mới sang đợt sau.

| Đợt | Nội dung | Xong khi | Điểm dừng |
|---|---|---|---|
| 0 | Khung workspace + **bản thử UI Slint** (gõ tiếng Việt, bảng 1.000 dòng, đo RAM 2 renderer) | `docs/DOT0_KET_QUA.md` có kết quả; chọn renderer | ĐD0: chủ dự án tự gõ thử tiếng Việt |
| 1 | `cf-core`: words, dates, names, fmt/parse, lifecycle, numbering, pricing, template_rules, render_context | Mọi fixture tương ứng xanh (kể cả KB-01, KB-02) | — |
| 2 | `cf-docx`: compile, render, dòng lặp, dòng tùy chọn, chế độ tương thích Bên A, analyze, ghi file độc quyền, CLI `cf-render` | `docx_render.json`, `template_analysis.json` xanh; file ra mở bằng Word không lỗi | ĐD2: chủ dự án mở 5 file ra bằng Word |
| 3 | `cf-db`: codec, kiểm schema, tạo DB mới, Store (nạp/ghi/chỉ mục), khóa ghi; tương thích hai chiều với Python | Test cf-db xanh; §3 của `07_KIEM_THU.md` qua | ĐD3: chạy trên bản sao DB thật |
| 4 | `cf-services`: khách hàng, sản phẩm, mẫu, hợp đồng (tạo, sinh file, chứng từ, trạng thái, xóa), phụ lục/gia hạn, kèm theo, 08a, hàng loạt, báo giá ×3, hồ sơ đấu nối, dashboard, cài đặt, Excel nhập/xuất | Test ca sử dụng xanh | — |
| 5 | `cf-app`: khung + 20 màn hình (`05_GIAO_DIEN.md`) | Làm được mọi việc của bản web | ĐD5: chủ dự án dùng thử 1 tuần trên bản sao dữ liệu |
| 6 | Đóng gói 1 file exe (icon, assets nhúng), hướng dẫn, nghiệm thu `07_KIEM_THU.md` §4 | Mọi ô nghiệm thu được tick | ĐD6: chuyển dữ liệu thật sang bản Rust, **xóa bản Python** |
| 7 | (Sau khi xóa Python) migration Rust số 1–4 (`02_DU_LIEU.md` §5.3) | | |

Thứ tự trong Đợt 4 và 5 đi theo mức dùng nhiều: hợp đồng → khách hàng → sinh file → báo giá →
hàng loạt → dashboard → phần còn lại.

Đợt 1 và Đợt 2 độc lập nhau, làm song song được. Đợt 3 cần Đợt 1 (struct dùng chung kiểu Decimal/ngày).
