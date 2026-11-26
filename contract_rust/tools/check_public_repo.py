#!/usr/bin/env python3
"""
check_public_repo.py — public-content scanner for ContractForge.

Purpose
-------
Guard the public repository boundary (OPEN_SOURCE_DESKTOP_PLAN.md Task 1 & 8).
The scanner fails (exit code 1) when it finds any forbidden class of file or
any known organization-specific term in trackable source/docs/filenames.

It is intentionally dependency-free (pure stdlib) so it runs in CI without
extra installs, and it does NOT require git to be initialized — it walks the
work-tree directly. It never prints sensitive file *content*; it only reports
the path and a short reason, as required by Task 8 ("Scanner output identifies
paths without printing sensitive file content").

Two failure families
--------------------
1. FORBIDDEN FILES — file classes that must never be tracked (databases,
   uploads, generated outputs, logs, executables, archives, build artifacts,
   caches, local tool settings).
2. FORBIDDEN TERMS  — organization-specific identifiers (real Party B names,
   authorization numbers, tax/bank values, legacy launcher branding, seeded
   real customers/products) found inside text-like files and filenames.

Exit codes
---------
0 = repository is clean (no forbidden files, no forbidden terms).
1 = violations found (prints a summary; never prints sensitive content).

Usage
-----
    python scripts/check_public_repo.py            # scan repo root
    python scripts/check_public_repo.py --root PATH # scan an explicit root
    python scripts/check_public_repo.py --json      # machine-readable output
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import os
import re
import subprocess
import sys
import zipfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

EXIT_CLEAN = 0
EXIT_VIOLATION = 1


# ─── 1. Forbidden file classes ───────────────────────────────────────────────
# Each rule: (description, match function over a relative POSIX path).
# A path matches if it equals or sits under one of the directory roots, OR if
# its name/extension matches a glob.

FORBIDDEN_DIRS = {
    "data",          # runtime database + uploads + outputs
    "uploads",
    "outputs",
    "logs",
    "backups",
    "build",
    "dist",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    "htmlcov",
    ".venv",
    "venv",
    "env",
    ".env",
    ".claude",
    ".codex",
    ".agents",
    ".vscode",
    ".idea",
}

FORBIDDEN_NAME_GLOBS = [
    "*.db",
    "*.db-journal",
    "*.db-shm",
    "*.db-wal",
    "*.sqlite",
    "*.sqlite3",
    "*.sqlite-journal",
    "*.log",
    "*.exe",
    "*.dll",
    "*.pyd",
    "*.so",
    "*.dylib",
    "*.zip",
    "*.7z",
    "*.rar",
    "*.gz",
    "*.tar",
    "*.tgz",
    "*.msi",
    "*.bak",
    "*.backup",
    "*.tmp",
    "*.temp",
    "*.pyc",
    "*.pyo",
    "*.swp",
    "*.swo",
    ".env",
    "*.env",
    ".env.local",
    "*.local",
    "Thumbs.db",
    ".DS_Store",
    "desktop.ini",
]

# Paths that are part of the scanner's own test fixtures. When the scanner runs
# from the repo root during normal CI these are never reached because they live
# under tests/. When running the scanner with an explicit --root that points at
# the fixture tree itself, the caller controls scope. We still allow these
# marker filenames to exist in tests by letting callers opt out per-file below.
SCANNER_FIXTURE_FILES = {
    "tests/unit/test_public_repo_scan.py",
}


# ─── 2. Private denylist ──────────────────────────────────────────────────────
# Exact private values never belong in public source. Locally they are loaded
# from ignored data/private_denylist.txt or CONTRACTFORGE_PRIVATE_DENYLIST.
PRIVATE_DENYLIST_PATH = Path("data/private_denylist.txt")

# Only scan text-like extensions for term content. Binary files (docx/xlsx/pdf/
# images) are NOT text-scanned here — the DOCX text-extraction check described
# in Task 7/8 is a separate integration concern; this scanner focuses on
# trackable source/text and on filenames for ALL files.
TEXT_EXTENSIONS = {
    ".py", ".html", ".js", ".ts", ".css", ".scss", ".json", ".yaml", ".yml",
    ".md", ".txt", ".rst", ".ini", ".toml", ".cfg", ".sh", ".bat", ".ps1",
    ".sql", ".env", ".gitignore", ".gitattributes", ".spec", ".xml", ".csv",
}


# ─── Ignored entries (directories never descended into) ──────────────────────
NEVER_DESCEND = {".git"}


def _rel_posix(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.as_posix()


def _path_in_dir(rel: str) -> bool:
    parts = rel.split("/")
    return any(part in FORBIDDEN_DIRS for part in parts)


def _name_matches_glob(name: str) -> bool:
    lower_name = name.lower()
    return any(fnmatch.fnmatch(lower_name, pat.lower()) for pat in FORBIDDEN_NAME_GLOBS)


def _normalize_for_search(text: str) -> str:
    """Lowercase + collapse internal whitespace so whitespace-flexible terms
    (e.g. multi-space inside READMEs) still match reliably."""
    return re.sub(r"\s+", " ", text.lower())


@dataclass
class Violation:
    kind: str            # "file" or "term"
    path: str            # relative posix path (no sensitive content)
    reason: str          # human-readable reason


@dataclass
class ScanResult:
    forbidden_files: list[Violation] = field(default_factory=list)
    forbidden_terms: list[Violation] = field(default_factory=list)

    @property
    def is_clean(self) -> bool:
        return not self.forbidden_files and not self.forbidden_terms

    def to_dict(self) -> dict:
        return {
            "clean": self.is_clean,
            "forbidden_files": [v.__dict__ for v in self.forbidden_files],
            "forbidden_terms": [v.__dict__ for v in self.forbidden_terms],
        }


def _walk_candidate_files(root: Path) -> Iterable[Path]:
    """Fallback tree walk used for isolated unit-test fixtures."""
    for dirpath, dirnames, filenames in os.walk(root):
        dp = Path(dirpath)
        dirnames[:] = [d for d in dirnames if d not in NEVER_DESCEND]
        for fn in filenames:
            yield dp / fn


def _git_candidate_files(root: Path) -> Iterable[Path]:
    """Yield tracked plus unignored untracked files, including force-tracked data."""
    proc = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError("git candidate discovery failed")
    for raw in proc.stdout.split(b"\0"):
        if raw:
            yield root / os.fsdecode(raw)


def iter_candidate_files(root: Path, use_git: bool | None = None) -> Iterable[Path]:
    """Use Git's publication boundary when available; otherwise walk fixtures."""
    if use_git is None:
        use_git = (root / ".git").exists()
    if use_git:
        yield from _git_candidate_files(root)
    else:
        yield from _walk_candidate_files(root)


def load_private_denylist(root: Path, explicit: Iterable[str] | None = None) -> list[str]:
    """Load exact private markers without embedding them in public source."""
    if explicit is not None:
        return [term.strip() for term in explicit if term and term.strip()]

    terms: list[str] = []
    denylist_path = root / PRIVATE_DENYLIST_PATH
    if denylist_path.is_file():
        for line in denylist_path.read_text(encoding="utf-8", errors="strict").splitlines():
            value = line.strip()
            if value and not value.startswith("#"):
                terms.append(value)

    env_value = os.environ.get("CONTRACTFORGE_PRIVATE_DENYLIST", "")
    terms.extend(value.strip() for value in env_value.splitlines() if value.strip())
    return terms


def classify_file(path: Path, rel: str) -> Violation | None:
    name = path.name
    if _path_in_dir(rel):
        return Violation("file", rel, "lives under a forbidden runtime/data directory")
    if _name_matches_glob(name):
        return Violation("file", rel, "matches a forbidden file pattern")
    return None


def scan_terms_in_file(path: Path, rel: str, deny_terms: list[str]) -> list[Violation]:
    if path.suffix.lower() not in TEXT_EXTENSIONS:
        return []
    try:
        raw = path.read_text(encoding="utf-8", errors="strict")
    except (OSError, UnicodeError):
        return [Violation("term", rel, "TEXT_READ_ERROR")]
    haystack = _normalize_for_search(raw)
    out: list[Violation] = []
    found: set[str] = set()
    for index, term in enumerate(deny_terms, start=1):
        key = term.lower()
        if key in found:
            continue
        if key in haystack:
            found.add(key)
            out.append(Violation("term", rel, f"PRIVATE_TERM_{index:03d}"))
    return out


def scan_terms_in_filename(rel: str, deny_terms: list[str]) -> list[Violation]:
    """Check the entire relative path without echoing matched private data."""
    name = rel.lower()
    out: list[Violation] = []
    for index, term in enumerate(deny_terms, start=1):
        t = term.lower()
        if len(t) < 4:
            continue
        if t in name:
            out.append(Violation("term", rel, f"PRIVATE_PATH_TERM_{index:03d}"))
            break
    return out


def scan_docx(path: Path, rel: str, deny_terms: list[str]) -> tuple[list[Violation], list[Violation]]:
    """Scan OOXML text, relationships, member names, and core metadata."""
    try:
        chunks: list[str] = []
        with zipfile.ZipFile(path) as archive:
            for info in archive.infolist():
                if info.file_size > 20 * 1024 * 1024:
                    raise ValueError("oversized DOCX member")
                chunks.append(info.filename)
                if Path(info.filename).suffix.lower() in {".xml", ".rels", ".txt"}:
                    chunks.append(archive.read(info).decode("utf-8", errors="ignore"))
    except (OSError, RuntimeError, ValueError, zipfile.BadZipFile):
        return [Violation("file", rel, "DOCX_READ_ERROR")], []

    haystack = _normalize_for_search("\n".join(chunks))
    hits: list[Violation] = []
    for index, term in enumerate(deny_terms, start=1):
        if term.lower() in haystack:
            hits.append(Violation("term", rel, f"DOCX_PRIVATE_TERM_{index:03d}"))
    return [], hits


def scan(
    root: Path,
    *,
    deny_terms: Iterable[str] | None = None,
    use_git: bool | None = None,
) -> ScanResult:
    result = ScanResult()
    root = root.resolve()
    private_terms = load_private_denylist(root, deny_terms)
    for path in iter_candidate_files(root, use_git=use_git):
        if not path.is_file():
            continue
        rel = _rel_posix(path, root)
        # 1) Forbidden file class
        fv = classify_file(path, rel)
        if fv:
            result.forbidden_files.append(fv)
            continue  # don't also term-scan a forbidden file's body
        if path.suffix.lower() == ".docx":
            file_hits, term_hits = scan_docx(path, rel, private_terms)
            result.forbidden_files.extend(file_hits)
            result.forbidden_terms.extend(term_hits)
            if file_hits:
                result.forbidden_terms.extend(scan_terms_in_filename(rel, private_terms))
                continue
        # 2) Forbidden terms in text bodies
        result.forbidden_terms.extend(scan_terms_in_file(path, rel, private_terms))
        # 3) Forbidden terms in filenames (covers binary assets too)
        result.forbidden_terms.extend(scan_terms_in_filename(rel, private_terms))
    return result


def _format_human(result: ScanResult) -> str:
    lines: list[str] = []
    if result.forbidden_files:
        lines.append("FORBIDDEN FILES (%d):" % len(result.forbidden_files))
        for v in result.forbidden_files:
            lines.append("  - %s  (%s)" % (v.path, v.reason))
    if result.forbidden_terms:
        lines.append("FORBIDDEN TERMS (%d):" % len(result.forbidden_terms))
        for v in result.forbidden_terms:
            lines.append("  - %s  (%s)" % (v.path, v.reason))
    if result.is_clean:
        lines.append("OK: public repository content is clean.")
    else:
        lines.append("")
        lines.append("FAILED: remove or ignore the violations above before committing.")
        lines.append("See OPEN_SOURCE_DESKTOP_PLAN.md §4.2 and §12.")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Scan repo for private/forbidden content.")
    parser.add_argument("--root", default=None, help="repository root (default: script parent dir)")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    args = parser.parse_args(argv)

    if args.root:
        root = Path(args.root)
    else:
        # scripts/check_public_repo.py → repo root is two levels up.
        root = Path(__file__).resolve().parent.parent

    if not root.exists():
        print("ERROR: root does not exist: %s" % root, file=sys.stderr)
        return 2

    result = scan(root)
    if args.json:
        print(json.dumps(result.to_dict(), ensure_ascii=False, indent=2))
    else:
        print(_format_human(result))
    return EXIT_CLEAN if result.is_clean else EXIT_VIOLATION


if __name__ == "__main__":
    raise SystemExit(main())
