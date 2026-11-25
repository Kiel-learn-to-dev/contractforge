"""Development-only ASGI helpers for the source-mode launcher."""
from __future__ import annotations

import json
import hashlib
import os
import uuid
from pathlib import Path
from typing import Any


LIVE_SCRIPT = r'''(function () {
  "use strict";
  var revisionUrl = "/__live/revision";
  var script = document.currentScript;
  var previous = script ? script.dataset.revision : null;
  var dirty = false;
  var notice = document.createElement("div");
  notice.id = "cf-live-update";
  notice.setAttribute("role", "status");
  notice.style.cssText = "position:fixed;right:16px;bottom:16px;z-index:2147483000;background:#102a56;color:white;padding:10px 14px;border-radius:10px;box-shadow:0 4px 20px #0004;font:13px system-ui;display:flex;gap:12px;align-items:center";
  var label = document.createElement("span");
  label.textContent = "Chạy từ mã nguồn · cập nhật trực tiếp";
  notice.appendChild(label);
  var button = document.createElement("button");
  button.type = "button";
  button.textContent = "Tải bản mới";
  button.style.cssText = "border:0;border-radius:6px;padding:5px 9px;cursor:pointer";
  button.hidden = true;
  button.addEventListener("click", function () {
    if (!dirty || window.confirm("Trang có nội dung chưa lưu. Tải lại và bỏ nội dung đó?")) {
      window.location.reload();
    }
  });
  notice.appendChild(button);
  document.addEventListener("DOMContentLoaded", function () {
    document.body.appendChild(notice);
  });
  function markDirty(event) {
    var target = event.target;
    if (!target || !target.form || target.type === "search" || target.name === "q") return;
    dirty = true;
  }
  document.addEventListener("input", markDirty, true);
  document.addEventListener("change", markDirty, true);
  function checkRevision() {
    fetch(revisionUrl, { cache: "no-store", headers: { "Cache-Control": "no-cache" } })
      .then(function (response) { if (!response.ok) throw new Error("not ready"); return response.json(); })
      .then(function (data) {
        if (data.revision !== previous) {
          previous = data.revision;
          if (dirty) {
            label.textContent = "Đã có bản sửa · nội dung chưa lưu";
            button.hidden = false;
          } else {
            window.location.reload();
          }
        }
      })
      .catch(function () {});
  }
  checkRevision();
  window.setInterval(checkRevision, 1500);
})();'''


class LiveDevelopmentMiddleware:
    """Expose a worker revision and reload only HTML responses during dev runs."""

    def __init__(self, app: Any, revision: str | None = None,
                 db_path: str | os.PathLike[str] | None = None) -> None:
        self.app = app
        self.revision = revision or uuid.uuid4().hex
        self.db_path = Path(db_path) if db_path is not None else None

    def _data_revision(self) -> str:
        """Fingerprint SQLite DB/WAL metadata without exposing local paths."""
        if self.db_path is None:
            return ""
        fingerprint = []
        for path in (self.db_path, Path(f"{self.db_path}-wal")):
            try:
                stat = path.stat()
                fingerprint.append((stat.st_mtime_ns, stat.st_size))
            except FileNotFoundError:
                fingerprint.append(None)
        payload = json.dumps(fingerprint, separators=(",", ":")).encode("ascii")
        return hashlib.sha256(payload).hexdigest()

    def _revision(self) -> str:
        data_revision = self._data_revision()
        return f"{self.revision}:{data_revision}" if data_revision else self.revision

    async def __call__(self, scope, receive, send) -> None:
        if (scope["type"] == "http" and scope.get("method") == "GET"
                and scope.get("path") == "/__live/revision"):
            data_revision = self._data_revision()
            body = json.dumps({
                "revision": (f"{self.revision}:{data_revision}"
                             if data_revision else self.revision),
                "worker_revision": self.revision,
                "data_revision": data_revision,
            }).encode("utf-8")
            await send({
                "type": "http.response.start",
                "status": 200,
                "headers": [
                    (b"content-type", b"application/json; charset=utf-8"),
                    (b"content-length", str(len(body)).encode("ascii")),
                    (b"cache-control", b"no-store, no-cache, must-revalidate"),
                    (b"pragma", b"no-cache"),
                ],
            })
            await send({"type": "http.response.body", "body": body})
            return

        if (scope["type"] == "http" and scope.get("method") == "GET"
                and scope.get("path") == "/__live/script.js"):
            body = LIVE_SCRIPT.encode("utf-8")
            await send({
                "type": "http.response.start",
                "status": 200,
                "headers": [
                    (b"content-type", b"application/javascript; charset=utf-8"),
                    (b"content-length", str(len(body)).encode("ascii")),
                    (b"cache-control", b"no-store"),
                ],
            })
            await send({"type": "http.response.body", "body": body})
            return

        start_message = None
        body_parts: list[bytes] = []
        html_response = False
        # Snapshot before the view reads the DB. A commit during the request
        # then differs from this value on the next poll and cannot be missed.
        page_revision = self._revision()

        async def capture(message):
            nonlocal start_message, html_response, page_revision
            if message["type"] == "http.response.start":
                headers = list(message.get("headers", []))
                content_type = next(
                    (value for name, value in headers if name.lower() == b"content-type"),
                    b"",
                )
                headers_by_name = {name.lower(): value for name, value in headers}
                disposition = headers_by_name.get(b"content-disposition", b"")
                encoding = headers_by_name.get(b"content-encoding", b"")
                content_length = headers_by_name.get(b"content-length")
                html_response = (
                    scope.get("method") == "GET"
                    and b"text/html" in content_type.lower()
                    and not disposition
                    and not encoding
                    and content_length is not None
                    and int(content_length) <= 5_000_000
                )
                start_message = message
                if not html_response:
                    await send(message)
                return

            if message["type"] == "http.response.body" and html_response:
                body_parts.append(message.get("body", b""))
                if not message.get("more_body", False):
                    body = b"".join(body_parts)
                    marker = b"</body>"
                    injection = (
                        f'<script src="/__live/script.js" '
                        f'data-revision="{page_revision}"></script>'
                    ).encode("ascii")
                    lowered = body.lower()
                    position = lowered.rfind(marker)
                    body = body[:position] + injection + body[position:] if position >= 0 else body + injection
                    headers = [
                        (name, value) for name, value in start_message.get("headers", [])
                        if name.lower() not in {b"content-length", b"etag", b"content-md5", b"cache-control"}
                    ]
                    headers.append((b"content-length", str(len(body)).encode("ascii")))
                    headers.append((b"cache-control", b"no-store"))
                    start_message["headers"] = headers
                    await send(start_message)
                    await send({"type": "http.response.body", "body": body})
                return
            await send(message)

        await self.app(scope, receive, capture)


def create_app():
    """Build the wrapped app lazily inside Uvicorn's reload worker."""
    from main import app as application
    from app.paths import DB_PATH
    return LiveDevelopmentMiddleware(application, db_path=DB_PATH)
