"""Single source of truth for the application version.

``pyproject.toml`` carries the packaging version and this module carries the
runtime one. They drifted once (0.7.4 in the app, 0.8.0 in packaging), which
made the `/health` endpoint report a version that no release actually matched.
``tests/unit/test_version_consistency.py`` now keeps the two in lockstep.

Bump both together when cutting a release.
"""

from __future__ import annotations

__version__ = "0.8.0"

# Human-facing application name. Kept here so the window title, the FastAPI
# title, and the packaging metadata cannot drift apart either.
APP_NAME = "ContractForge"
