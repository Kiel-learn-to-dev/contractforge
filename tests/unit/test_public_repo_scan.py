"""
Unit tests for the public-content scanner (OPEN_SOURCE_DESKTOP_PLAN.md Task 1 & 8).

These tests NEVER touch the real repository tree. Each test builds an isolated
temporary directory, seeds it with controlled content, and asserts the scanner's
pass/fail behaviour. The real database and uploads are never read.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest
from docx import Document

# Import the scanner directly from its file path so tests work regardless of
# whether the repo root is on sys.path as a package.
_SCANNER_PATH = (
    Path(__file__).resolve().parent.parent.parent / "scripts" / "check_public_repo.py"
)


def _load_scanner():
    spec = importlib.util.spec_from_file_location("check_public_repo", _SCANNER_PATH)
    module = importlib.util.module_from_spec(spec)
    sys.modules["check_public_repo"] = module
    spec.loader.exec_module(module)
    return module


scanner = _load_scanner()


# ─── Clean-tree baseline ──────────────────────────────────────────────────────


def _make_clean_tree(tmp_path: Path) -> Path:
    """A minimal neutral tree that the scanner must accept."""
    (tmp_path / "ContractForge" / "app").mkdir(parents=True)
    (tmp_path / "ContractForge" / "app" / "main.py").write_text(
        "# ContractForge — neutral sample application\n", encoding="utf-8"
    )
    (tmp_path / "README.md").write_text(
        "# ContractForge\nA neutral desktop contract manager.\n", encoding="utf-8"
    )
    return tmp_path


def test_clean_tree_passes(tmp_path):
    result = scanner.scan(_make_clean_tree(tmp_path))
    assert result.is_clean, result.to_dict()


def test_clean_tree_exit_code_zero(tmp_path):
    rc = scanner.main(["--root", str(_make_clean_tree(tmp_path))])
    assert rc == scanner.EXIT_CLEAN


# ─── Forbidden file classes ───────────────────────────────────────────────────


@pytest.mark.parametrize(
    "name",
    [
        "contract_manager.db",
        "data.sqlite3",
        "build.log",
        "ContractForge.exe",
        "release.zip",
        "archive.7z",
        "module.pyc",
        "Thumbs.db",
        ".DS_Store",
        "secrets.env",
    ],
)
def test_forbidden_file_pattern_detected(tmp_path, name):
    _make_clean_tree(tmp_path)
    (tmp_path / name).write_text("x", encoding="utf-8")
    result = scanner.scan(tmp_path)
    assert not result.is_clean
    rels = [v.path for v in result.forbidden_files]
    assert name in rels


@pytest.mark.parametrize(
    "subdir",
    ["data", "uploads", "outputs", "logs", "backups", "build", "dist", "__pycache__"],
)
def test_forbidden_directory_detected(tmp_path, subdir):
    _make_clean_tree(tmp_path)
    d = tmp_path / subdir
    d.mkdir()
    (d / "anything.txt").write_text("hello", encoding="utf-8")
    result = scanner.scan(tmp_path)
    assert not result.is_clean
    assert any(
        v.path.startswith(subdir + "/") for v in result.forbidden_files
    ), result.to_dict()


def test_database_in_subdirectory_still_detected(tmp_path):
    """A tracked db-like file anywhere is forbidden, even outside data/."""
    _make_clean_tree(tmp_path)
    (tmp_path / "ContractForge" / "app" / "stolen.db").write_text("x", encoding="utf-8")
    result = scanner.scan(tmp_path)
    assert not result.is_clean


def test_never_descend_into_dotgit(tmp_path):
    """The scanner must not descend into .git internals."""
    _make_clean_tree(tmp_path)
    gitdir = tmp_path / ".git" / "objects"
    gitdir.mkdir(parents=True)
    (gitdir / "AB" ).mkdir()
    (gitdir / "AB" / "cd1234").write_text("blob", encoding="utf-8")
    result = scanner.scan(tmp_path, use_git=False)
    assert result.is_clean, ".git internals must not be flagged"


# ─── Forbidden terms ──────────────────────────────────────────────────────────


def test_forbidden_term_in_source_detected(tmp_path):
    _make_clean_tree(tmp_path)
    (tmp_path / "ContractForge" / "app" / "settings.py").write_text(
        'ORG = "PRIVATE_ORG_SENTINEL_9Z"\n', encoding="utf-8"
    )
    result = scanner.scan(tmp_path, deny_terms=["PRIVATE_ORG_SENTINEL_9Z"])
    assert not result.is_clean
    assert any(v.kind == "term" for v in result.forbidden_terms)
    assert all("PRIVATE_ORG_SENTINEL_9Z" not in v.reason for v in result.forbidden_terms)


def test_forbidden_term_case_insensitive(tmp_path):
    _make_clean_tree(tmp_path)
    (tmp_path / "notes.md").write_text("private_org_sentinel_9z\n", encoding="utf-8")
    result = scanner.scan(tmp_path, deny_terms=["PRIVATE_ORG_SENTINEL_9Z"])
    assert not result.is_clean


def test_forbidden_term_in_branded_filename_detected(tmp_path):
    """A private marker in a filename is detected without echoing it."""
    _make_clean_tree(tmp_path)
    asset = tmp_path / "ContractForge" / "assets" / "default_templates"
    asset.mkdir(parents=True)
    (asset / "PRIVATE_ORG_SENTINEL_9Z.docx").write_bytes(b"PK\x03\x04fake")
    result = scanner.scan(tmp_path, deny_terms=["PRIVATE_ORG_SENTINEL_9Z"])
    flagged = [v.path for v in result.forbidden_terms]
    assert any("PRIVATE_ORG_SENTINEL_9Z.docx" in p for p in flagged), flagged


def test_neutral_content_has_no_term_hits(tmp_path):
    _make_clean_tree(tmp_path)
    (tmp_path / "ContractForge" / "app" / "welcome.py").write_text(
        "GREETING = 'Welcome to ContractForge'\n", encoding="utf-8"
    )
    result = scanner.scan(tmp_path)
    assert result.forbidden_terms == []


def test_output_json_shape(tmp_path):
    _make_clean_tree(tmp_path)
    (tmp_path / "leak.db").write_text("x", encoding="utf-8")
    result = scanner.scan(tmp_path)
    payload = result.to_dict()
    assert set(payload.keys()) == {"clean", "forbidden_files", "forbidden_terms"}
    assert payload["clean"] is False
    assert payload["forbidden_files"][0]["path"] == "leak.db"
    # Sensitive content must never be echoed in the reason.
    for v in payload["forbidden_files"] + payload["forbidden_terms"]:
        assert "content" not in v["reason"].lower() or "pattern" in v["reason"]


def test_git_ignored_untracked_runtime_data_is_not_a_candidate(tmp_path):
    import subprocess

    _make_clean_tree(tmp_path)
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / ".gitignore").write_text("data/\n", encoding="utf-8")
    data = tmp_path / "data"
    data.mkdir()
    (data / "private.db").write_bytes(b"private")

    result = scanner.scan(tmp_path, use_git=True)

    assert result.is_clean, result.to_dict()


def test_git_force_tracked_runtime_data_is_still_rejected(tmp_path):
    import subprocess

    _make_clean_tree(tmp_path)
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / ".gitignore").write_text("data/\n", encoding="utf-8")
    data = tmp_path / "data"
    data.mkdir()
    private_db = data / "private.db"
    private_db.write_bytes(b"private")
    subprocess.run(["git", "add", "-f", "data/private.db"], cwd=tmp_path, check=True)

    result = scanner.scan(tmp_path, use_git=True)

    assert any(v.path == "data/private.db" for v in result.forbidden_files)


def test_docx_xml_and_metadata_are_scanned(tmp_path):
    _make_clean_tree(tmp_path)
    asset_dir = tmp_path / "ContractForge" / "assets" / "sample_templates"
    asset_dir.mkdir(parents=True)
    docx_path = asset_dir / "neutral.docx"
    document = Document()
    document.add_paragraph("PRIVATE_ORG_SENTINEL_9Z")
    document.core_properties.author = "PRIVATE_ORG_SENTINEL_9Z"
    document.save(docx_path)

    result = scanner.scan(tmp_path, deny_terms=["PRIVATE_ORG_SENTINEL_9Z"])

    assert any(v.path.endswith("neutral.docx") for v in result.forbidden_terms)


def test_corrupt_docx_fails_closed(tmp_path):
    _make_clean_tree(tmp_path)
    asset_dir = tmp_path / "ContractForge" / "assets" / "sample_templates"
    asset_dir.mkdir(parents=True)
    (asset_dir / "broken.docx").write_bytes(b"not-a-docx")

    result = scanner.scan(tmp_path)

    assert any(v.path.endswith("broken.docx") for v in result.forbidden_files)
