# Dùng thử bản Rust

## Kết quả ngày 2026-11-26

- Chủ dự án bỏ qua giai đoạn dùng thử một tuần.
- Đã khởi động bản Rust trên bản sao dữ liệu và xác nhận việc sinh hợp đồng tạm ổn.
- Chủ dự án sẽ tự chỉnh mẫu Word sau.
- Chủ dự án xác nhận đã đối chiếu ba hợp đồng sinh bằng Rust và Python, kết quả đạt. Mục ĐD4-E đã hoàn thành.

## Bản sao dữ liệu dùng để kiểm tra

Đường dẫn bản sao: `%TEMP%\ContractForge-rust-trial-68c09d50`. Bản này được tạo từ `data\contract_manager.db`. Các đường dẫn file lưu trong DB đã được chuyển vào bản sao và mã băm DB nguồn không đổi. Ứng dụng đã chạy trên bản sao này, vì vậy có thể đã thêm mẫu mặc định hoặc log.

Khởi chạy từ PowerShell:

```powershell
$trial = Join-Path $env:TEMP 'ContractForge-rust-trial-68c09d50'
& 'D:\Download\ContractForge\contract_rust\target\debug\ContractForge.exe' --data-root $trial
```

## Theo dõi việc còn lại

- [ ] Chủ dự án chỉnh mẫu Word.
