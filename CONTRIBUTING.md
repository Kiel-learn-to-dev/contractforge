# Đóng góp cho ContractForge

Ứng dụng chạy trên máy người dùng và giữ dữ liệu hợp đồng thật của họ. Chỉ phát triển trên
nhánh `rust-rewrite` (thư mục `contract_rust/`); bản Python ở `pricing-model`/`main` chỉ để tham khảo.

## Ranh giới quan trọng nhất: dữ liệu không bao giờ vào repo

Cơ sở dữ liệu, mẫu Word của cơ quan, hồ sơ khách hàng, hợp đồng đã sinh, log đều nằm ngoài Git.
Trước khi commit chạy:

```powershell
python contract_rust/tools/check_public_repo.py
```

Scanner chặn **loại file cấm** (`.db`, `.log`, `.exe`, `.zip`, mọi thứ dưới `data/`, `uploads/`)
và **từ khoá riêng** đọc từ `data/private_denylist.txt` (file này bị ignore, mỗi máy tự giữ).
Mẫu Word riêng đặt ở `contract_rust/assets/default_templates/` bị ignore; chỉ `sample_*.docx` được commit.
Nếu lỡ commit dữ liệu riêng: dừng, đừng push, viết lại lịch sử.

## Kiểm tra

```powershell
cd contract_rust
powershell -File tools/check.ps1
```

Test chạy trên thư mục dữ liệu tạm, không bao giờ đụng DB thật. Test khẳng định hành vi, không chép lại cách cài đặt.

## Những chỗ dễ sai

- Luật vòng đời hợp đồng chỉ ở một nơi (`cf-core`); đừng định nghĩa lại tập trạng thái ở chỗ khác.
- Khuôn số hợp đồng, tên/giá sản phẩm là dữ liệu trong DB, không viết cứng.
- Thiết kế và quyết định: `contract_rust/docs/`.
