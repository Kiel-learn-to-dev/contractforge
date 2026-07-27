# Kế hoạch Nâng cấp Dashboard — ContractForge v0.8

> Ngày lập: 2026-05-29  
> Phiên bản hiện tại: v0.7.4  
> Mục tiêu: Nâng dashboard từ "hiển thị số liệu" → "chủ động gợi ý hành động + phân tích xu hướng"

---

## Tổng quan 3 pha

| Pha | Tên | Tính năng | Ưu tiên |
|---|---|---|---|
| 1 — Quick Wins | Sửa nhanh, không rủi ro | F1, F2, F3, F4 | Làm ngay, <1h/item |
| 2 — Core Upgrades | Nâng giá trị nghiệp vụ | F5, F6, F7, F8 | 2–4h/item |
| 3 — Advanced | Tính năng lớn | F9, F10 | Cần thiết kế kỹ |

---

## Pha 1 — Quick Wins

### F1. Fix lỗi duplicate script `#today-date`

**Vấn đề:**  
`templates/base.html:668` đã fill `#today-date` trong topbar. `templates/dashboard/index.html:328` có thêm 1 IIFE làm đúng việc đó → lỗi silently (element đã được fill rồi).

**Sửa:**  
Xóa block `<script>` cuối `templates/dashboard/index.html` (dòng 327–336).

**Files thay đổi:**
- `ContractForge/templates/dashboard/index.html` — xóa ~10 dòng

**Effort:** 5 phút

---

### F2. Widget "Đề xuất hành động" (Action Items)

**Mô tả:**  
Card mới trên dashboard, tự động sinh danh sách việc cần làm từ data thực. Không AI — thuần if/else logic. Hiển thị ngay sau Row 1 (stat cards). Nếu không có gì → hiện "Tất cả ổn ✓".

**Các điều kiện kiểm tra:**

| Điều kiện | Level | Icon | Message mẫu | Link |
|---|---|---|---|---|
| HĐ `ExpiringSoon` còn ≤7 ngày | 🔴 high | `bi-alarm-fill` | "3 hợp đồng hết hạn trong 7 ngày — cần gia hạn ngay" | `/dashboard/expiring?days=7` |
| HĐ `Draft` chưa có `output_file_path` | 🟡 medium | `bi-file-earmark-x` | "5 HĐ nháp chưa được sinh file" | `/contracts?status=Draft` |
| HĐ `Generated` > 14 ngày chưa chuyển `Sent` | 🟡 medium | `bi-clock` | "2 HĐ đã sinh file nhưng chưa gửi khách hàng" | `/contracts?status=Generated` |
| HĐ `Signed` > 7 ngày chưa kích hoạt | 🟡 medium | `bi-pen` | "4 HĐ đã ký, chưa chuyển trạng thái hiệu lực" | `/contracts?status=Signed` |
| Không có template nào `is_active = True` | 🔵 low | `bi-file-earmark-word` | "Chưa có mẫu hợp đồng nào đang hoạt động" | `/templates/new` |

**Data structure từ service:**
```python
# Trả về list[dict], chỉ các item có count > 0
[
  {
    "level":   "high",            # high | medium | low
    "icon":    "bi-alarm-fill",
    "message": "3 HĐ hết hạn trong 7 ngày",
    "link":    "/dashboard/expiring?days=7",
    "count":   3,
  },
  ...
]
```

**Files thay đổi:**
- `ContractForge/app/services/dashboard_service.py` — thêm hàm `get_action_items(db) -> list[dict]`
- `ContractForge/app/routers/dashboard.py` — gọi `get_action_items`, truyền `action_items` vào context
- `ContractForge/templates/dashboard/index.html` — thêm card sau Row 1 stat cards

**UI sketch:**
```
┌─────────────────────────────────────────────────────────┐
│ ⚡ Việc cần xử lý                                        │
├─────────────────────────────────────────────────────────┤
│ 🔴  3 hợp đồng hết hạn trong 7 ngày         [Xem →]    │
│ 🟡  5 HĐ nháp chưa sinh file                [Xem →]    │
│ 🟡  2 HĐ đã sinh file, chưa gửi khách hàng  [Xem →]    │
└─────────────────────────────────────────────────────────┘
```
Nếu list rỗng → card hiện badge xanh "Không có việc tồn đọng".

**Effort:** ~2–3h

---

### F3. Export CSV danh sách sắp hết hạn

**Mô tả:**  
Nút "Tải Excel" trên `/dashboard/expiring` → download file `.csv` UTF-8 BOM (mở được bằng Excel tiếng Việt không bị lỗi font).

**Route mới:** `GET /dashboard/expiring/export?days=60`

```python
@router.get("/dashboard/expiring/export")
def export_expiring(days: int = 60):
    contracts = get_expiring_contracts(db, days=days)
    # Tạo CSV với cột: Số HĐ, Khách hàng, Ngày hết hạn, Còn lại (ngày), Trạng thái, Giá trị (VNĐ)
    # Dùng stdlib csv + StreamingResponse, content-type text/csv
    # BOM: ﻿ ở đầu file để Excel nhận UTF-8
```

**Files thay đổi:**
- `ContractForge/app/routers/dashboard.py` — thêm route `/dashboard/expiring/export`
- `ContractForge/templates/dashboard/expiring.html` — thêm nút "Tải Excel" cạnh filter days

**Lưu ý:** Không cần thư viện mới — chỉ dùng `csv` stdlib + `io.StringIO`.

**Effort:** ~1h

---

### F4. AJAX auto-refresh stat cards

**Mô tả:**  
Dùng `/api/stats` đã có sẵn. Mỗi 5 phút, JS gọi endpoint và update số liệu trong 6 stat cards mà không reload trang. Hiển thị "Cập nhật lúc HH:mm" góc card.

**Mở rộng `/api/stats` response** (hiện thiếu `expiring_soon`, `invoiced`):
```json
{
  "summary": {
    "active": 12, "signed": 3, "invoiced": 5,
    "expiring_soon": 2, "expired": 1, "generated": 18,
    "total_customers": 45, "total_value_active": "..."
  },
  "updated_at": "2026-05-29"
}
```

**JS trong dashboard:**
```javascript
// Gọi mỗi 300_000ms (5 phút)
setInterval(() => fetch('/api/stats').then(...).then(updateCards), 300_000);
```

**Files thay đổi:**
- `ContractForge/app/routers/dashboard.py` — mở rộng JSON response của `/api/stats`
- `ContractForge/templates/dashboard/index.html` — thêm `data-stat-key` attribute vào các card + JS block

**Effort:** ~1h

---

## Pha 2 — Core Upgrades

### F5. Bundle Chart.js cho offline

**Mô tả:**  
`base.html` hiện load Bootstrap/TomSelect từ CDN — app chạy offline vẫn OK nếu đã cache. Chart.js cần được bundle thủ công vì không có trong CDN cache mặc định.

**Cách làm:**
1. Download `chart.umd.min.js` từ `https://cdn.jsdelivr.net/npm/chart.js@4.x/dist/chart.umd.min.js`
2. Lưu vào `ContractForge/static/chart.umd.min.js`
3. Thêm vào `base.html`: `<script src="/static/chart.umd.min.js"></script>`

**Lưu ý:** F6 phụ thuộc F5.

**Files thay đổi:**
- `ContractForge/static/chart.umd.min.js` — file mới (~200KB)
- `ContractForge/templates/base.html` — thêm 1 dòng script

**Effort:** 30 phút (phần lớn là download + test)

---

### F6. Biểu đồ "HĐ hết hạn theo tháng" (Bar Chart)

**Mô tả:**  
Bar chart ngang cho 6 tháng tới — mỗi cột = số HĐ hết hạn trong tháng đó. Đặt trong Row mới hoặc thay thế panel "31–60 ngày". Giúp lập kế hoạch gia hạn chủ động.

**Query SQLite:**
```sql
SELECT strftime('%Y-%m', end_date) AS ym, COUNT(*) AS cnt
FROM contracts
WHERE status IN ('Active','PaidActive','Signed','Invoiced','ExpiringSoon')
  AND end_date >= date('now')
  AND end_date <= date('now', '+6 months')
GROUP BY ym
ORDER BY ym
```

**Service mới:** `get_monthly_expiry_forecast(db, months=6) -> list[dict]`  
Trả về: `[{"month": "2026-06", "label": "Tháng 6", "count": 3}, ...]`

**UI:** `<canvas id="expiryChart" height="120">` với Chart.js bar chart màu gradient theo urgency.

**Files thay đổi:**
- `ContractForge/app/services/dashboard_service.py` — thêm `get_monthly_expiry_forecast`
- `ContractForge/app/routers/dashboard.py` — truyền `monthly_forecast` vào context
- `ContractForge/templates/dashboard/index.html` — thêm card mới với canvas

**Phụ thuộc:** F5 (Chart.js)

**Effort:** ~3h

---

### F7. Biểu đồ Doughnut phân bổ sản phẩm

**Mô tả:**  
Thay thế bullet list "Theo sản phẩm" (hiện chỉ là text + số) bằng doughnut chart Chart.js. Hover tooltip hiển thị giá trị VNĐ. Data `by_product` đã có sẵn trong context — không cần thay đổi backend.

**UI:** Doughnut 200x200 + legend bên phải với % và số HĐ.

**Files thay đổi:**
- `ContractForge/templates/dashboard/index.html` — replace card `by_product` bullet list

**Phụ thuộc:** F5 (Chart.js)

**Effort:** ~1.5h

---

### F8. Thống kê so sánh tháng này

**Mô tả:**  
Strip nhỏ sau stat cards, hiển thị tóm tắt hoạt động tháng hiện tại với so sánh tháng trước.

```
Tháng 5/2026:  +7 HĐ mới  |  +3 HĐ đã ký  |  5 HĐ hết hạn  |  Giá trị mới: 450.000.000 VNĐ
```

Nếu có data tháng trước: hiện ▲3 (màu xanh) hoặc ▼1 (màu đỏ) bên cạnh.

**Service mới:** `get_monthly_stats(db, year, month) -> dict`

```python
{
  "new_contracts":  7,   # created_at >= đầu tháng
  "signed":         3,   # sign_date >= đầu tháng  
  "expired":        5,   # end_date trong tháng
  "new_value":      450_000_000,  # tổng total_amount của HĐ tạo tháng này
  "vs_prev_new":    +3,  # so sánh với tháng trước (None nếu không có)
}
```

**Files thay đổi:**
- `ContractForge/app/services/dashboard_service.py` — thêm `get_monthly_stats`
- `ContractForge/app/routers/dashboard.py` — truyền `monthly_stats`
- `ContractForge/templates/dashboard/index.html` — thêm strip sau Row 1

**Effort:** ~2.5h

---

### F9. Top 5 khách hàng nhiều HĐ nhất

**Mô tả:**  
Panel trong cột phải (cạnh Quick actions), liệt kê 5 khách hàng có nhiều HĐ hiệu lực nhất kèm link sang trang chi tiết.

**Service mới:** `get_top_customers(db, limit=5) -> list[dict]`  
Query: GROUP BY `customer_id`, chỉ tính HĐ status trong `active_set`.

**Files thay đổi:**
- `ContractForge/app/services/dashboard_service.py` — thêm `get_top_customers`
- `ContractForge/app/routers/dashboard.py` — truyền `top_customers`
- `ContractForge/templates/dashboard/index.html` — thêm card

**Effort:** ~1.5h

---

## Pha 3 — Advanced

### F10. Global Search

**Mô tả:**  
Input tìm kiếm trên topbar (`base.html`) — gõ số HĐ hoặc tên KH → dropdown kết quả realtime (debounce 300ms) → click navigate.

**Route mới:** `GET /api/search?q=<string>&limit=10`

```python
# Search LIKE trên: contract_number, customer.legal_name, customer.short_name
# Trả về JSON: [{"type": "contract"|"customer", "id": ..., "label": ..., "sub": ..., "url": ...}]
```

**UI:** Input trong `.topbar` với dropdown overlay tuyệt đối, dùng CSS từ `.cf-menu` đã có trong base.

**Files thay đổi:**
- `ContractForge/app/routers/dashboard.py` — thêm `/api/search`
- `ContractForge/templates/base.html` — thêm search input + JS vào topbar

**Lưu ý offline:** Fetch gọi `localhost:8888` — không vấn đề.

**Effort:** ~4h

---

### F11. Trang Báo cáo tổng hợp `/reports`

**Mô tả:**  
Trang mới phục vụ báo cáo định kỳ cho quản lý. Bảng pivot: hàng = sản phẩm, cột = tháng, giá trị = số HĐ + tổng tiền. Nút export `.xlsx`.

**Routes:**
- `GET /reports` → HTML trang báo cáo
- `GET /reports/export?year=2026` → download `.xlsx` (dùng `openpyxl` đã có trong requirements)

**Files mới:**
- `ContractForge/app/routers/reports.py`
- `ContractForge/app/services/report_service.py`
- `ContractForge/templates/reports/index.html`

**Files thay đổi:**
- `ContractForge/templates/base.html` — thêm link "Báo cáo" vào sidebar
- `ContractForge/main.py` — include router mới

**Effort:** ~1 ngày

---

## Sơ đồ phụ thuộc

```
F1  ───── độc lập
F2  ───── độc lập
F3  ───── độc lập
F4  ───── độc lập

F5  ───── độc lập (prerequisite cho F6, F7)
F6  ──── phụ thuộc F5
F7  ──── phụ thuộc F5
F8  ───── độc lập
F9  ───── độc lập

F10 ───── độc lập
F11 ───── sau F8, F9 (tái dùng queries)
```

---

## Thứ tự triển khai đề xuất

```
Sprint 1 (nhanh):   F1 → F2 → F3 → F4
Sprint 2 (charts):  F5 → F6 → F7 → F8 → F9
Sprint 3 (reports): F10 → F11
```

---

## Tham chiếu file quan trọng

| File | Vai trò |
|---|---|
| `ContractForge/app/services/dashboard_service.py` | Tất cả business logic dashboard — thêm hàm mới vào đây |
| `ContractForge/app/routers/dashboard.py` | Routes + truyền data vào template |
| `ContractForge/templates/dashboard/index.html` | Template chính — 4 rows hiện tại |
| `ContractForge/templates/dashboard/expiring.html` | Trang danh sách sắp hết hạn |
| `ContractForge/templates/base.html` | Layout + design tokens `--c-*` — dùng biến này cho UI mới |
| `ContractForge/static/` | Thư mục chứa asset offline (thêm `chart.umd.min.js` vào đây) |
| `ContractForge/app/models/contract.py` | Model đầy đủ — có `created_at`, `sign_date`, `end_date`, `total_amount` |

---

## Ghi chú kỹ thuật

- **Offline-first:** App chạy hoàn toàn offline. Mọi asset JS/CSS mới phải được bundle vào `ContractForge/static/`, không dùng CDN mới.
- **Design system:** Dùng CSS variables `--c-primary`, `--c-green`, `--c-amber`, `--c-red` từ `base.html`. Không hardcode màu hex.
- **Tách CODE/DATA:** Không bao giờ ghi vào `data/`. Code chỉ đọc/ghi qua SQLAlchemy Session.
- **Bootstrap 5 + Chart.js 4:** Tương thích tốt. Chart.js không phụ thuộc jQuery.
- **SQLite date functions:** Dùng `strftime('%Y-%m', column)` cho GROUP BY tháng.
- **CSV export:** Thêm BOM `﻿` đầu file để Excel Windows nhận đúng UTF-8.
