"""The runtime version and the packaging version must never drift apart.

They did once: ``main.py`` served 0.7.4 from ``/health`` while
``pyproject.toml`` declared 0.8.0, so the version a user could read from the
running app matched no release at all.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
PYPROJECT = REPO_ROOT / "pyproject.toml"


def _packaging_version() -> str:
    """Read ``[project] version`` without needing a TOML parser dependency."""
    text = PYPROJECT.read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    assert match, "pyproject.toml has no [project] version"
    return match.group(1)


def test_runtime_version_matches_pyproject():
    from app.version import __version__

    assert __version__ == _packaging_version(), (
        "app/version.py and pyproject.toml disagree — bump both together."
    )


def test_health_endpoint_reports_the_same_version():
    """/health is what users and the launcher read; it must not hardcode.

    Deliberately built without the ``with`` block: that would run the startup
    event (seeds, migrations, status sweep) and open a second engine we would
    then have to tear down. ``/health`` touches no database.
    """
    from fastapi.testclient import TestClient

    import main
    from app.version import __version__

    payload = TestClient(main.app).get("/health").json()

    assert payload["status"] == "ok"
    assert payload["version"] == __version__


def test_readme_exists_for_packaging():
    """pyproject declares readme = "README.md"; a build fails without it."""
    readme = REPO_ROOT / "README.md"
    assert readme.is_file(), "pyproject.toml references README.md"
    assert readme.read_text(encoding="utf-8").strip(), "README.md is empty"


def test_license_file_exists():
    assert (REPO_ROOT / "LICENSE").is_file(), "pyproject declares the MIT license"
