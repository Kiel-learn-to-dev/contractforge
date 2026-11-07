# ContractForge

> A neutral, single-user, offline-first desktop contract manager.

ContractForge manages the full contract lifecycle — from draft, through generating Word
documents from your own templates, tracking signatures, invoices and payments, all the
way to expiry — **without an internet connection and without sending your data
anywhere**. Everything lives in a single SQLite file on your machine.

---

## Features

- **Customer directory** with subordinate units, attached paperwork, and Excel import/export.
- **Word templates**: upload your own `.docx`, mark the fill-in points with `{{FIELD_NAME}}`
  placeholders, and the app fills them in and produces a finished document.
- **Batch generation**: one job produces contracts for many customers, packaged as a `.zip`.
- **A governed contract lifecycle**: every status change is validated, may require
  supporting documents, and is recorded in a history log.
- **Dashboard and reminders**: warnings for contracts nearing expiry, outstanding work,
  and breakdowns by product and by month.
- **Quotations** and supporting forms generated from the same data.
- **Document dossiers**: gather every file belonging to one customer into a single folder,
  numbered to match the upload fields of an external portal. Files are hardlinked, so the
  folder costs no extra disk space and deleting it cannot touch the originals.

## Contract lifecycle

```
Draft ──► Generated ──► Sent ──► Signed ──► Invoiced ──► PaidActive ──► Expired
                                    └──────────┴─────────────┴────────► Terminated
```

Every transition goes through a single rules table
(`ContractForge/app/services/lifecycle.py`). `Invoiced` requires an attached invoice;
`PaidActive` requires attached proof of payment.

**"Expiring soon" is not a status** — it is derived from `end_date` each time it is
displayed. That way a contract close to expiry keeps its real status and can still be
invoiced normally.

---

## Install and run

Requires **Python 3.11 or newer** (tested on 3.11 – 3.14). The first install needs an
internet connection; after that the app runs fully offline.

### Quickest start

From a clone of this repository, run the launcher for your system. It creates a private
virtual environment, installs the dependencies, and starts the server:

| System | Command |
|---|---|
| Windows | double-click `ContractForge\chay_windows.bat` |
| macOS / Linux | `bash ContractForge/chay_mac_linux.sh` |

Then open http://localhost:8888. Stop the server with `Ctrl+C`. If something goes wrong
on Windows, `ContractForge\kiem_tra_moi_truong.bat` checks the environment step by step.
Each launcher re-checks the dependencies on every start, so it also picks up a changed
`requirements.txt` after a `git pull`.

### Manual install

```bash
python -m venv .venv
source .venv/bin/activate            # Windows: .venv\Scripts\activate
pip install -r ContractForge/requirements.txt
cd ContractForge && python -m uvicorn main:app --host 127.0.0.1 --port 8888
```

Then open http://127.0.0.1:8888. Using a virtual environment keeps the install away from
other Python projects (and from "externally managed environment" errors on recent
Debian, Ubuntu and macOS/Homebrew Pythons).

### Running as a desktop application

```bash
pip install -r requirements-desktop.txt
```

Then run `ContractForge.pyw` (on Windows, double-click it). The server starts inside that
same process on a free port assigned by the operating system (so it does not use 8888),
and appears in its own WebView2 window — no browser required. Closing the window stops
the server. Without `pywebview` the application still runs; it just opens in your system
browser, and you quit from the tray icon or with `Ctrl+C`.

`ContractForge.pyw` does **not** install anything itself: install the dependencies first
(`requirements-desktop.txt` includes the base ones), otherwise it exits and the reason is
written to `contractforge.log` in the data directory's `logs/` folder.

On Windows the window uses the **WebView2 Runtime**. Windows 11 and recent Windows 10
builds ship with it; otherwise install it from
<https://developer.microsoft.com/microsoft-edge/webview2/>. On Linux, `pywebview` needs
GTK or Qt from your system (see the
[pywebview installation guide](https://pywebview.flowrl.com/guide/installation.html));
without them the app falls back to the browser.

### Building a standalone .exe

```bash
python -m PyInstaller --clean --noconfirm packaging/pyinstaller/contractforge.spec
```

Or run `build_exe.bat` on Windows. The packaged build needs no Python installation and
**contains no user data** — on first run it creates a clean data directory of its own.
Pre-release checklist:
[tests/smoke/windows_release_checklist.md](tests/smoke/windows_release_checklist.md).

> The server **listens on `127.0.0.1` only**. This is a single-user local application with
> no login mechanism, so it must never be exposed to a LAN or to the internet.

---

## Where your data lives

Source code and data are kept strictly separate. The data directory is resolved in this
order of precedence (see `ContractForge/app/paths.py`):

1. The `CONTRACTFORGE_DATA_ROOT` environment variable, if set.
2. `%LOCALAPPDATA%\ContractForge` — if a database already exists there.
3. `<project directory>/data` — so existing installations keep working and are never
   forced to move.
4. Default for a fresh machine: `%LOCALAPPDATA%\ContractForge`
   (macOS: `~/Library/Application Support/ContractForge`,
   Linux: `~/.local/share/ContractForge`).

Inside the data directory:

| Directory | Contents |
|---|---|
| `contract_manager.db` | All business data (SQLite) |
| `uploads/templates/` | Word templates you uploaded |
| `uploads/signed_scans/`, `invoice_docs/`, `payment_slips/` | Attached supporting documents |
| `uploads/customer_docs/` | Customer paperwork |
| `outputs/` | Generated `.docx` and `.zip` files |
| `dau-noi/` | Assembled document dossiers (hardlinks, regenerable) |
| `backups/` | Backups created by the migration tooling |

**To back up**, copy this whole directory. There is nothing else to keep.

**When a contract is deleted**, the files attached to it (the generated `.docx`, the signed
scan, the invoice, the payment record) are deleted from disk at the same time. Customer
paperwork belongs to the customer rather than to any contract, so it is left untouched.

---

## Development

```bash
pip install -r requirements-dev.txt        # app dependencies + pytest
python -m pytest -q                        # test suite
python scripts/check_public_repo.py        # check that no private data leaked
```

The test suite always runs against a temporary data directory — running the tests will
**never** touch your real database (see `tests/conftest.py`).

Before committing, `scripts/check_public_repo.py` fails the run if data files, generated
output, logs, executables, or organisation-specific keywords have found their way into the
public source tree.

For architecture, design decisions and roadmap, see `OPEN_SOURCE_DESKTOP_PLAN.md`.

## License

[MIT](LICENSE)
