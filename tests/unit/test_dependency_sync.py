"""Runtime dependencies are declared twice; the two lists must not drift.

``ContractForge/requirements.txt`` is what users install and what CI/PyInstaller
use. ``pyproject.toml`` repeats the list for tooling that reads PEP 621
metadata. Nothing derived one from the other, so they were free to diverge.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent


def _requirements(path: Path) -> set[str]:
    lines = []
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line and not line.startswith("-"):
            lines.append(line.replace(" ", ""))
    return set(lines)


def _pyproject_list(name: str) -> set[str]:
    """Items of a TOML string array (``dependencies`` or an extras key)."""
    text = (REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    match = re.search(rf"^{name}\s*=\s*\[(.*?)^\]", text, re.MULTILINE | re.DOTALL)
    assert match, f"pyproject.toml has no `{name}` array"
    body = re.sub(r"#.*", "", match.group(1))
    return {item.replace(" ", "") for item in re.findall(r'"([^"]+)"', body)}


def test_runtime_dependencies_match_requirements_txt():
    assert _pyproject_list("dependencies") == _requirements(
        REPO_ROOT / "ContractForge" / "requirements.txt"
    )


def test_dev_extra_matches_requirements_dev():
    expected = _requirements(REPO_ROOT / "requirements-dev.txt")
    assert _pyproject_list("dev") == expected


def test_desktop_extra_matches_requirements_desktop():
    expected = _requirements(REPO_ROOT / "requirements-desktop.txt")
    assert _pyproject_list("desktop") == expected
