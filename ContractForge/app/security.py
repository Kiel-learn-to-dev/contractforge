"""security.py — ranh giới bảo mật của ứng dụng cục bộ.

ContractForge là ứng dụng một người dùng, chạy trên máy cá nhân, **không có
đăng nhập**. Ranh giới bảo mật của nó vì thế chính là ranh giới máy: chỉ tiến
trình trên máy này mới được nói chuyện với server.

Hai lớp bảo vệ (OPEN_SOURCE_DESKTOP_PLAN.md Task 11):

1. **Chỉ loopback.** Mọi launcher bind `127.0.0.1`. Máy khác trong mạng LAN
   không kết nối được. Lớp này do launcher lo, không phải code ở đây.

2. **Chống cross-origin.** Ngay cả khi chỉ nghe loopback, một trang web bất kỳ
   mà người dùng đang mở trên trình duyệt vẫn có thể gửi form POST tới
   ``http://localhost:8000/contracts/1/delete``. Trình duyệt cho phép form
   cross-origin gửi đi tự do, và vì app không có phiên đăng nhập nên chẳng có
   cookie nào để mà kiểm. Middleware dưới đây từ chối mọi request đổi trạng thái
   có `Origin`/`Referer` không thuộc loopback — đây chính là lỗ hổng CSRF cổ điển
   của các ứng dụng desktop chạy nền HTTP.
"""

from __future__ import annotations

from urllib.parse import urlsplit

from starlette.middleware.base import BaseHTTPMiddleware
from starlette.responses import PlainTextResponse

#: Các method thay đổi dữ liệu. GET/HEAD/OPTIONS chỉ đọc nên bỏ qua.
UNSAFE_METHODS = frozenset({"POST", "PUT", "PATCH", "DELETE"})

#: Host được coi là "chính máy này".
LOOPBACK_HOSTS = frozenset({"localhost", "127.0.0.1", "[::1]", "::1", "0.0.0.0"})


def host_of(url: str) -> str:
    """Lấy hostname (không kèm cổng) từ một URL. Chuỗi rỗng nếu không phân tích được."""
    if not url:
        return ""
    try:
        parsed = urlsplit(url)
    except ValueError:
        return ""
    return (parsed.hostname or "").lower()


def is_local_origin(url: str) -> bool:
    """True nếu URL trỏ về chính máy này."""
    return host_of(url) in LOOPBACK_HOSTS


class LocalOriginMiddleware(BaseHTTPMiddleware):
    """Chặn request đổi dữ liệu đến từ nguồn không phải máy này.

    Quy tắc:
      * Method chỉ đọc → cho qua.
      * Có `Origin` → phải là loopback.
      * Không có `Origin` nhưng có `Referer` → Referer phải là loopback.
        (Trình duyệt bỏ `Origin` ở một số form same-origin cũ, nhưng vẫn gửi
        `Referer`, nên vẫn kiểm được.)
      * Không có cả hai → cho qua. Đây là hình dạng của request từ curl hay
        từ chính bộ test; trình duyệt luôn gắn ít nhất một trong hai khi gửi
        cross-origin, nên khe hở này không mở đường cho tấn công qua web.
    """

    async def dispatch(self, request, call_next):
        if request.method in UNSAFE_METHODS:
            origin = request.headers.get("origin")
            referer = request.headers.get("referer")

            if origin:
                allowed = is_local_origin(origin)
            elif referer:
                allowed = is_local_origin(referer)
            else:
                allowed = True

            if not allowed:
                return PlainTextResponse(
                    "Yêu cầu bị từ chối: nguồn gửi không phải máy này.\n"
                    "ContractForge chỉ phục vụ ứng dụng chạy trên chính máy bạn.",
                    status_code=403,
                )

        return await call_next(request)
