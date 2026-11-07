# ContractForge — Hướng dẫn cài đặt và sử dụng

Bản tiếng Anh, đầy đủ hơn về kiến trúc và phát triển: [README.md](README.md).

## Yêu cầu

- **Python 3.11 trở lên** — tải tại <https://www.python.org/downloads/>.
  Trên Windows, **bắt buộc tick "Add Python to PATH"** khi cài.
- Windows 10/11, macOS hoặc Linux.
- Cần internet **lần đầu** để tải thư viện. Sau đó chạy hoàn toàn offline.

---

## Cài đặt và chạy (cách đơn giản nhất)

1. Tải mã nguồn: `git clone https://github.com/Kiel-learn-to-dev/contractforge.git`
   (hoặc nút **Code → Download ZIP** trên GitHub rồi giải nén).
2. Mở thư mục **`ContractForge`** nằm bên trong thư mục dự án — thư mục có `main.py` và
   `requirements.txt`.
3. Chạy file khởi động:
   - **Windows:** double-click `chay_windows.bat`
   - **macOS / Linux:** mở Terminal và chạy `bash ContractForge/chay_mac_linux.sh`
     (chạy từ thư mục nào cũng được)
4. Lần đầu mất khoảng 2–5 phút để tạo môi trường riêng (`venv/`) và tải thư viện.
   Các lần sau chỉ vài giây.
5. Mở trình duyệt vào **<http://localhost:8888>** (Windows tự mở giúp bạn).

Tắt server: nhấn **Ctrl+C** trong cửa sổ đen / Terminal.

Mỗi lần khởi động, script tự kiểm tra lại thư viện — nên sau khi `git pull` bản mới có
thêm thư viện, bạn chỉ cần chạy lại script, không phải làm gì thêm.

---

## Cách khác: cửa sổ ứng dụng riêng (desktop)

Dành cho người muốn app mở trong cửa sổ riêng, có icon khay hệ thống, không cần trình duyệt.

```bash
pip install -r requirements-desktop.txt
```

Sau đó double-click `ContractForge.pyw` (hoặc chạy `python ContractForge.pyw`).

> Lệnh `pip install` phải cài vào **đúng Python sẽ mở file `.pyw`** — tức Python hệ thống khi bạn
> double-click. Nếu bạn cài vào một venv thì phải chạy `.pyw` bằng chính Python của venv đó
> (`.venv\Scripts\pythonw.exe ContractForge.pyw`), nếu không app sẽ báo thiếu thư viện.

- **Phải chạy `pip install` ở trên trước.** `ContractForge.pyw` không tự cài thư viện; nếu
  thiếu, nó tắt ngay và ghi lý do vào `contractforge.log` (trong thư mục `logs/` của
  thư mục dữ liệu, xem bên dưới).
- Cổng do hệ điều hành tự chọn nên **không phải 8888**. Đóng cửa sổ là server dừng theo.
- Không cài `pywebview` thì app vẫn chạy, chỉ mở bằng trình duyệt hệ thống.
- Windows cần **WebView2 Runtime** (Windows 11 và Windows 10 mới đã có sẵn).

## (Tùy chọn) Build file `.exe`

Double-click `build_exe.bat` (Windows) → chờ khoảng 2 phút → có `ContractForge.exe`
ngay trong thư mục dự án. File `.exe` không chứa dữ liệu của ai và không cần cài Python.

---

## Dữ liệu được lưu ở đâu?

Mã nguồn và dữ liệu **tách riêng**. Thư mục dữ liệu được chọn theo thứ tự:

1. Biến môi trường `CONTRACTFORGE_DATA_ROOT`, nếu có đặt.
2. Thư mục `data/` ngay trong thư mục dự án — **nếu đã có sẵn** `contract_manager.db`
   ở đó (để bản cài cũ không bị mất dữ liệu).
3. Máy mới tinh:
   - Windows: `%LOCALAPPDATA%\ContractForge`
   - macOS: `~/Library/Application Support/ContractForge`
   - Linux: `~/.local/share/ContractForge`

Bên trong thư mục dữ liệu:

| Thư mục / file | Nội dung |
|---|---|
| `contract_manager.db` | Toàn bộ dữ liệu khách hàng, hợp đồng (SQLite) |
| `uploads/templates/` | Mẫu hợp đồng `.docx` đã upload |
| `uploads/` (các thư mục còn lại) | Chứng từ đính kèm, hồ sơ khách hàng |
| `outputs/contracts/`, `outputs/batch/` | Hợp đồng `.docx` sinh ra; file ZIP sinh hàng loạt |
| `logs/` | `contractforge.log` của bản desktop |
| `backups/` | Bản sao lưu do công cụ di chuyển dữ liệu tạo ra |

**Sao lưu:** copy nguyên thư mục dữ liệu ra USB/Google Drive là đủ. Muốn khôi phục thì
copy lại đúng chỗ rồi chạy app.

> ⚠️ **Không bao giờ xóa thư mục dữ liệu** khi cập nhật. Cập nhật chỉ là thay mã nguồn
> (`git pull` hoặc giải nén bản mới đè lên), dữ liệu nằm ngoài mã nguồn nên được giữ nguyên.

---

## Cập nhật phiên bản mới

- Đã dùng `git clone`: chạy `git pull` rồi chạy lại script khởi động.
- Tải ZIP: giải nén bản mới, **giữ nguyên thư mục dữ liệu** (và thư mục `data/` nếu bạn đang
  dùng nó), rồi chạy script khởi động.

---

## Xử lý sự cố

**Cửa sổ đen mở lên rồi tắt ngay (Windows)?**
→ Gần như chắc chắn bạn double-click `chay_windows.bat` sai chỗ. Phải mở đúng thư mục
`ContractForge` (có `main.py`, `requirements.txt`) rồi mới double-click. Nếu vẫn lỗi, chạy
`kiem_tra_moi_truong.bat` — script này kiểm tra từng bước và **không tắt ngay**.

**Xem lỗi ở đâu?**
→ Mở `startup.log` và `kiem_tra.log` (tạo ngay trong thư mục `ContractForge`) bằng Notepad.
Bản desktop ghi `contractforge.log` trong thư mục `logs/` của thư mục dữ liệu.

**"Python was not found" / "Khong tim thay Python" / "Can Python 3.11 tro len"?**
→ Cài Python 3.11 trở lên, nhớ tick "Add Python to PATH", rồi mở lại cửa sổ.

**Cài thư viện thất bại?**
→ Kiểm tra internet (lần đầu cần mạng), proxy/VPN công ty, rồi chạy lại script.
Xem `startup.log` để biết gói nào lỗi.

**Linux: "externally-managed-environment" hoặc "ensurepip is not available"?**
→ Dùng script khởi động (nó tự tạo venv) và cài thêm gói venv của hệ điều hành:
`sudo apt install python3-venv`.

**"Port 8888 already in use"?**
→ Server đang chạy rồi (hoặc app khác chiếm cổng). Thử mở <http://localhost:8888>. Muốn đổi cổng:
sửa `--port 8888` thành cổng khác (ví dụ `9000`) trong `chay_windows.bat` /
`chay_mac_linux.sh`. Bản desktop (`ContractForge.pyw`) không bị lỗi này vì tự chọn cổng trống.

**"Connection refused" khi mở trang?**
→ Server chưa khởi động xong. Đợi 5–10 giây rồi nhấn F5.

**Mất dữ liệu?**
→ Kiểm tra `contract_manager.db` trong thư mục dữ liệu (xem bên trên). Nếu bạn chạy từ máy
khác hoặc từ thư mục khác thì app có thể đang dùng một thư mục dữ liệu khác.

> Server **chỉ lắng nghe trên `127.0.0.1`**. App không có đăng nhập, vì vậy tuyệt đối không
> mở nó ra mạng LAN hay internet.
