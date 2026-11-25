# ĐD0 — Kết quả bản thử UI

Trạng thái: **đã nghiệm thu (ĐD0 Đạt)**. Chủ dự án đã tự kiểm tra gõ tiếng Việt và cuộn bảng 1.000 dòng trên cả hai renderer (`winit-femtovg` và `winit-skia`): đều mượt, không lỗi dấu. Chọn **`winit-femtovg`** làm renderer mặc định theo đề xuất (working set nhẹ hơn ~15 MB, khung hình đầu ~0,3 s).

Chạy bản thử: `cargo run -p ui-spike --release` (mặc định dùng skia) hoặc đặt `SLINT_BACKEND=winit-femtovg`
/ `SLINT_BACKEND=winit-skia`.

## 1. Kiểm tra theo renderer (blueprint `docs/05_GIAO_DIEN.md` §6)

| Renderer | Kiểm tra | Kết quả | Ghi chú |
|---|---|---|---|
| `winit-femtovg` | 1. Unikey Telex/VNI | **Đạt** | Chủ dự án xác nhận gõ tiếng Việt rất tốt, không mất dấu/nhân đôi |
| `winit-femtovg` | 1. EVKey Telex/VNI | **Đạt** | Chủ dự án xác nhận |
| `winit-femtovg` | 2. Dấu cỡ 10–14 pt | Đạt (xem ảnh chụp) | AI chụp cửa sổ; cả 3 cỡ đúng dấu, không vỡ chữ |
| `winit-femtovg` | 3. Cuộn bảng 1.000 dòng | **Đạt** | Chủ dự án xác nhận cuộn rất mượt |
| `winit-skia` | 1. Unikey Telex/VNI | **Đạt** | Chủ dự án xác nhận gõ tiếng Việt rất tốt |
| `winit-skia` | 1. EVKey Telex/VNI | **Đạt** | Chủ dự án xác nhận |
| `winit-skia` | 2. Dấu cỡ 10–14 pt | Đạt (xem ảnh chụp) | AI chụp cửa sổ; cả 3 cỡ đúng dấu |
| `winit-skia` | 3. Cuộn bảng 1.000 dòng | **Đạt** | Chủ dự án xác nhận cuộn rất mượt |
| cả hai | Bố cục + biểu đồ (T0.07) | Đạt (xem ảnh chụp) | 12 cột T1/26…T12/26 và biểu đồ tròn 3 phần đều (đã sửa lỗi cung tròn méo) |
| cả hai | Sắp xếp khi bấm tiêu đề cột (T0.06) | **Chưa kiểm** | Cần thao tác chuột; mã có callback `sort-ascending/descending` |

Ghi chú về đọc chữ: lần chạy đầu, chữ trong bảng và ô nhập gần như trắng trên nền sáng vì Slint theo màu hệ thống
(đang tối). Đã ép `Palette.color-scheme = ColorScheme.light` trong `init`; sau sửa, chữ đọc được ở femtovg.
Skia cùng mã giao diện nên áp dụng giống nhau, nhưng ảnh chụp sau sửa chỉ có ở femtovg.

## 2. Đo đạc (Windows 11, máy thử, Slint 1.19.1)

| Renderer | RAM: Private (commit) | RAM: Working set (≈ cột "Memory" Task Manager) | Khung hình đầu tiên (đo trong app) | Thời gian tới khi cửa sổ hiện (đo từ ngoài) |
|---|---:|---:|---:|---:|
| `winit-femtovg` | 139–140 MB | 145–147 MB | 278–296 ms (đo bằng rendering notifier, 3 lần) | 760–782 ms (3 lần) |
| `winit-skia` | 134–136 MB | 161–162 MB | **không đo được**: notifier không được gọi trong bản thử này; mốc dự phòng "vòng lặp sự kiện bắt đầu" ~311–319 ms | 632–883 ms (3 lần, dao động lớn) |

Cách đo:
- RAM: `Start-Process` bản `--release`, chờ 5 giây, đọc `PrivateMemorySize64` và `WorkingSet64`, rồi dừng tiến trình.
- Thời gian từ ngoài: từ lúc khởi chạy tới lúc cửa sổ có tiêu đề "kiểm tra Slint" xuất hiện. Có cả chi phí của PowerShell, nên chỉ so sánh tương đối.
- Thời gian trong app: `Instant` từ đầu `main()`, in khi khung hình đầu tiên được vẽ (`RenderingState::AfterRendering`).
- Xác nhận renderer đang dùng: `SLINT_DEBUG_PERFORMANCE=refresh_lazy,console` in
  "FemtoVG renderer with OpenGL backend" (femtovg) và "Skia renderer (skia backend wgpu)" (skia).

Sự khác biệt đáng chú ý: skia chiếm thêm khoảng 15 MB working set; femtovg khởi động nhanh hơn ở phép đo trong app.
Phần khởi động của skia chưa so sánh được trực tiếp vì thiếu hook.

## 3. Đề xuất & Quyết định (ĐD0)

Renderer được chọn: **`winit-femtovg`**. Lý do: cả hai renderer đều gõ tiếng Việt tốt và cuộn bảng rất mượt; `winit-femtovg` có working set nhỏ hơn khoảng 15 MB và khung hình đầu tiên đo được nhanh (~0,3 s).

Quyết định chủ dự án: **Đã nghiệm thu (ĐD0 Pass)**. Đủ điều kiện mở khóa P4.A.
