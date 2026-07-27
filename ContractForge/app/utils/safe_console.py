"""Tien ich ghi log/console an toan tren Windows console ma hoa hep."""

from __future__ import annotations

import sys
from typing import Any


def safe_print(*args: Any, sep: str = " ", end: str = "\n") -> None:
    """In ra stdout nhung khong lam app crash vi loi ma hoa console.

    Neu console/file redirect khong ho tro ky tu Unicode, ham se tu dong
    fallback sang backslashreplace de giu lai thong tin va tranh crash.
    """
    text = sep.join(str(a) for a in args) + end
    stream = sys.stdout
    try:
        stream.write(text)
    except UnicodeEncodeError:
        encoding = getattr(stream, "encoding", None) or "utf-8"
        data = text.encode(encoding, errors="backslashreplace")
        if hasattr(stream, "buffer") and stream.buffer is not None:
            stream.buffer.write(data)
        else:
            stream.write(data.decode(encoding, errors="ignore"))
    try:
        stream.flush()
    except Exception:
        pass
