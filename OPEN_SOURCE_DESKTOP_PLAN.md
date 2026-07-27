# ContractForge Open-Source Desktop Plan

## Document status

- Status: Proposed
- Date: 2026-07-09
- Scope: Convert the current local FastAPI application into a neutral, single-user, open-source desktop application for Windows.
- Primary distribution: Standalone Windows desktop application using WebView2 through `pywebview`.
- Public repository rule: No real customer data, organization-specific information, private templates, generated contracts, or operational documents may enter Git history.

---

## 1. Executive summary

ContractForge already has a suitable core for a single-user desktop application:

- FastAPI and Jinja2 provide the local application.
- SQLite provides embedded persistence.
- `python-docx` and `openpyxl` provide document generation and import/export.
- The existing launcher already manages a background server and system tray.

The public version must not be produced by simply deleting a few visible brand names. Organization-specific information is currently spread across:

- hard-coded Party B defaults;
- seed customers and products;
- contract-number conventions;
- quotation rules and fixed prices;
- UI labels and documentation;
- five bundled Word templates;
- the real SQLite database, uploaded identity documents, signed scans, generated contracts, logs, ZIP files, and executable artifacts.

The target design separates the application into two strict layers:

1. **Public ContractForge core**
   - neutral source code;
   - generic contract and quotation workflows;
   - fake sample data and sample templates only;
   - test suite, build configuration, documentation, and release automation.

2. **Private local profile**
   - organization details;
   - customer database;
   - private products and pricing;
   - private Word templates;
   - uploads and generated outputs;
   - stored under the operating system user-data directory, outside the repository and installation directory.

This work must be delivered incrementally. Data isolation and tests come first, business correctness and security come second, WebView packaging comes only after the core is stable.

---

## 2. Goals

### 2.1 Product goals

- Run as a desktop application in its own native window instead of opening a normal browser.
- Remain a single-user, offline-first application.
- Require no account, password, cloud service, or external database.
- Preserve customer, template, contract, batch generation, reporting, and document workflows.
- Support source-based installation for contributors.
- Support a standalone Windows release for non-technical users.
- Allow each user to configure their own organization, products, prices, and Word templates.

### 2.2 Open-source goals

- Publish a repository that is safe to clone, inspect, build, and redistribute.
- Keep all public code and assets organization-neutral.
- Prevent private data and generated artifacts from being committed.
- Provide reproducible tests and a repeatable release process.
- Document important architectural decisions and migration behavior.

### 2.3 Quality goals

- Protect the real production database during development and testing.
- Centralize contract lifecycle rules.
- Keep expiry warnings separate from billing/workflow state.
- Remove stored-XSS and unsafe local-server behavior.
- Preserve atomicity between database updates and file operations.
- Establish measurable performance baselines before optimization.

---

## 3. Non-goals

The first public desktop release will not:

- become a multi-user server application;
- support remote/LAN access;
- implement accounts, roles, or cloud synchronization;
- host customer files on an external service;
- provide automatic background updates;
- include private organization templates, customer data, pricing, contract numbers, authorization details, or corporate banking information;
- guarantee cross-platform desktop releases in the first milestone;
- rewrite the UI in Electron, Tauri, React, or another frontend framework.

Windows is the release target for v1. The source may remain portable, but macOS and Linux packaging are later work.

---

## 4. Current-state inventory

### 4.1 Runtime and architecture

- Application entry point: `ContractForge/main.py`
- Desktop launcher: `ContractForge.pyw`
- Database: `data/contract_manager.db`
- Generated files: `data/outputs/`
- Uploaded files: `data/uploads/`
- Templates and static assets: `ContractForge/templates/`, `ContractForge/static/`
- Bundled Word assets: `ContractForge/assets/default_templates/`
- Current server: Uvicorn on port 8888
- Current primary launcher binding: `127.0.0.1`
- Legacy Windows and Linux launchers bind to `0.0.0.0`

### 4.2 Private or organization-specific material

The public repository must exclude or neutralize:

- ten Party B organization fields currently represented by hard-coded defaults;
- three seeded customer records;
- two domain-specific seeded products;
- domain-specific quotation behavior and fixed prices;
- organization-specific contract-number prefixes and abbreviations;
- organization and locality labels in code, HTML, documentation, and shell scripts;
- all five bundled DOCX files, each of which contains organization-specific terms;
- the real database and every uploaded/generated document;
- launcher/server logs;
- existing `.exe`, `.zip`, `.pyc`, and cache files;
- local Codex/Claude configuration.

### 4.3 Known correctness findings

- Bulk status updates bypass lifecycle validation and required documents.
- Expiry automation can overwrite a `Signed` workflow state with `ExpiringSoon`, preventing invoicing.
- Customer reporting omits `PaidActive` and `Invoiced` from important totals.
- Sub-unit updates do not consistently verify customer ownership.
- Hard-deleting a contract can leave generated and uploaded files behind.

### 4.4 Known security findings

- Legacy launchers expose the unauthenticated application on all network interfaces.
- Customer-controlled values are inserted into `innerHTML`.
- Some JSON/JavaScript values are rendered through Jinja `safe` rather than a JSON encoder.
- Unsafe POST requests lack a local-origin/CSRF boundary.
- The application contains sensitive local files next to publishable source code.

### 4.5 Current performance baseline

Measured against the current database:

- Customer list: approximately 33 ms and 2 SQL statements.
- Contract list: approximately 10 ms and 2 SQL statements.
- Dashboard composition: approximately 28 ms and 27 SQL statements.

The current dataset is small enough that response time is acceptable. Query consolidation is a later guarded improvement, not a prerequisite for correctness.

---

## 5. Architecture decisions

### AD-1: Keep FastAPI and Jinja2

**Decision:** Preserve the existing server-rendered application.

**Rationale:**

- The working UI and business logic can be reused.
- Rewriting the frontend would add risk without improving the single-user product.
- WebView can host the current localhost application directly.

**Rejected alternatives:**

- Electron: larger runtime and substantial packaging/UI changes.
- Tauri: attractive footprint, but would require a new desktop shell and more integration work.
- Native Qt rewrite: discards the existing UI and increases maintenance.

### AD-2: Use `pywebview` with WebView2 on Windows

**Decision:** Replace `webbrowser.open()` with a native WebView window.

**Rationale:**

- Minimal change to FastAPI/Jinja2.
- Native application window and lifecycle events.
- WebView2 is available on most supported Windows systems and can be installed as a prerequisite.
- Avoids bundling a full Chromium copy in the application.

### AD-3: Keep a localhost HTTP server

**Decision:** Continue serving the UI through FastAPI on loopback.

**Constraints:**

- Bind only to `127.0.0.1`.
- Reject untrusted Host and Origin values.
- Protect unsafe methods from cross-origin submission.
- Do not expose the application on LAN in the desktop release.

### AD-4: Store user data outside source and installation folders

**Decision:** Use `%LOCALAPPDATA%\ContractForge` on Windows.

**Target layout:**

```text
%LOCALAPPDATA%\ContractForge\
├── data\
│   └── contract_manager.db
├── uploads\
│   ├── templates\
│   ├── customer_docs\
│   ├── signed_scans\
│   ├── invoice_docs\
│   └── payment_slips\
├── outputs\
│   ├── contracts\
│   └── batch\
├── logs\
└── backups\
```

**Rationale:**

- Application upgrades cannot overwrite user data.
- Public source and private runtime state are physically separated.
- Installed applications can run without write access to their installation directory.

### AD-5: Make organization data configurable

**Decision:** Replace hard-coded Party B defaults with a first-run organization profile.

**Behavior:**

- A new installation starts without real organization information.
- The user completes a setup screen before generating documents.
- Existing databases retain their current settings.
- Public sample values use clearly fictional data.

### AD-6: Make templates and pricing data-driven

**Decision:** Keep quotation and contract-generation capabilities, but remove domain-specific branches and fixed prices.

**Behavior:**

- Products own their default pricing, tax, duration, and default templates.
- Quotations select a product/template from the database.
- Public assets contain only a neutral sample template.
- Private product packs and templates remain local user data.

### AD-7: Treat expiry as derived information

**Decision:** Expiry warning is calculated from `end_date`; it must not overwrite the billing/workflow state.

**Rationale:**

- A signed contract can be both “signed” and “expiring soon.”
- A single enum cannot safely represent two independent dimensions.
- Dashboard warnings should not block invoicing or payment transitions.

### AD-8: Build releases in CI

**Decision:** Source stays in Git; Windows binaries are produced by GitHub Actions and attached to GitHub Releases.

**Rationale:**

- Avoid committing large opaque binaries.
- Make builds repeatable and reviewable.
- Ensure release artifacts come from a known commit.

---

## 6. Target repository structure

```text
ContractForge/
├── .github/
│   └── workflows/
├── docs/
│   ├── decisions/
│   └── user-guide/
├── packaging/
│   ├── pyinstaller/
│   └── windows/
├── scripts/
│   ├── check_public_repo.py
│   └── migrate_legacy_data.py
├── tests/
│   ├── fixtures/
│   ├── integration/
│   └── unit/
├── contractforge/
│   ├── app/
│   ├── assets/
│   ├── static/
│   ├── templates/
│   ├── desktop.py
│   └── main.py
├── .gitignore
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE
├── README.md
├── SECURITY.md
├── pyproject.toml
└── requirements.lock
```

The physical package rename/restructure is optional for the first milestone. Do not combine it with business fixes unless tests are already protecting behavior.

---

## 7. Dependency graph

```text
T1 Repository safety boundary
 └─ T2 Isolated test foundation
     ├─ T3 Runtime data directory
     │   └─ T4 Legacy data migration
     ├─ T5 Neutral organization profile
     │   └─ T6 Neutral seeds and products
     │       └─ T7 Neutral templates and quotation
     │           └─ T8 Public-content scanner
     ├─ T9 Lifecycle and bulk status
     │   └─ T10 Derived expiry and reporting
     └─ T11 Local security and file integrity
         └─ T12 Desktop WebView shell
             └─ T13 Standalone Windows packaging
                 └─ T14 GitHub publication and beta release
```

Tasks that share models, migration logic, or lifecycle policy must remain sequential. Documentation and sanitized sample assets may be prepared in parallel after their contracts are defined.

---

## 8. Implementation tasks

## Phase 1: Repository and test foundation

### Task 1: Establish the public/private repository boundary

**Description:** Create the files and checks that make it difficult to commit runtime data, sensitive documents, local configuration, caches, logs, and build artifacts.

**Acceptance criteria:**

- [ ] `.gitignore` excludes `data/`, runtime databases, uploads, outputs, logs, executables, ZIP files, Python caches, virtual environments, build folders, and local tool settings.
- [ ] A public-repository scanner fails when a forbidden file class or known organization term is introduced.
- [ ] Git is initialized only after the ignore rules and scanner are verified.

**Verification:**

- [ ] Run `git status --short --ignored` and confirm private/runtime paths are ignored.
- [ ] Add representative fake forbidden files in a temporary test directory and confirm the scanner rejects them.
- [ ] Confirm no real data file is staged.

**Dependencies:** None.

**Files likely touched:**

- `.gitignore`
- `scripts/check_public_repo.py`
- `tests/unit/test_public_repo_scan.py`

**Estimated scope:** Medium, 3 files.

---

### Task 2: Add an isolated pytest foundation

**Description:** Establish tests that never mutate the real SQLite database and can protect subsequent refactors.

**Acceptance criteria:**

- [ ] Every test receives an isolated temporary database.
- [ ] Test fixtures can create customers, products, templates, contracts, documents, and events.
- [ ] Running tests does not change the size, timestamp, or hash of the real database.

**Verification:**

- [ ] Run `python -m pytest`.
- [ ] Compare the production database hash before and after the suite.
- [ ] Run the suite twice and confirm deterministic results.

**Dependencies:** Task 1.

**Files likely touched:**

- `pyproject.toml`
- `tests/conftest.py`
- `tests/fixtures/factories.py`
- `tests/integration/test_database_isolation.py`

**Estimated scope:** Medium, 4 files.

---

### Checkpoint A: Safe foundation

- [ ] No private file can be staged accidentally.
- [ ] Tests use only temporary data.
- [ ] The existing application still starts against the existing data directory.
- [ ] The real database has not changed.

---

## Phase 2: Runtime-data isolation

### Task 3: Move runtime paths to the operating-system user-data directory

**Description:** Replace the source-relative `data/` location with a platform-aware application data directory while preserving a test override.

**Acceptance criteria:**

- [ ] Windows runtime data defaults to `%LOCALAPPDATA%\ContractForge`.
- [ ] Tests can override the data root through an explicit configuration value.
- [ ] Source and installation directories remain read-only during normal use.

**Verification:**

- [ ] Start with an empty LocalAppData directory and verify all runtime folders are created.
- [ ] Create a test record and verify it appears only in the configured user-data directory.
- [ ] Run from a read-only installation directory.

**Dependencies:** Task 2.

**Files likely touched:**

- `ContractForge/app/paths.py`
- `ContractForge/app/database.py`
- `ContractForge.pyw`
- `tests/integration/test_runtime_paths.py`

**Estimated scope:** Medium, 4 files.

---

### Task 4: Provide an explicit legacy-data migration

**Description:** Detect the old sibling `data/` layout and migrate it safely to LocalAppData without deleting the source copy.

**Acceptance criteria:**

- [ ] Migration creates a timestamped backup before copying.
- [ ] Migration is idempotent and cannot overwrite a non-empty destination silently.
- [ ] The old data directory remains untouched until the user explicitly confirms cleanup.

**Verification:**

- [ ] Test empty destination, existing destination, interrupted copy, and repeated migration.
- [ ] Compare database integrity and key record counts before and after migration.
- [ ] Verify uploaded and generated files remain reachable.

**Dependencies:** Task 3.

**Files likely touched:**

- `scripts/migrate_legacy_data.py`
- `ContractForge/app/services/migration_service.py`
- `ContractForge.pyw`
- `tests/integration/test_legacy_migration.py`

**Estimated scope:** Medium, 4 files.

---

### Checkpoint B: Data migration

- [ ] A new installation starts with isolated LocalAppData.
- [ ] A legacy installation can migrate without data loss.
- [ ] Rollback consists of restoring the backup and selecting the old data root.
- [ ] No migration code deletes the source automatically.

---

## Phase 3: Neutral public core

### Task 5: Replace hard-coded Party B data with an organization profile

**Description:** Introduce a required organization profile and remove real organization defaults from public code.

**Acceptance criteria:**

- [ ] Public source contains no real Party B name, address, bank, tax, representative, or authorization value.
- [ ] A clean installation displays first-run organization setup.
- [ ] Existing databases retain their previously saved settings without being overwritten.

**Verification:**

- [ ] Start with an empty database and complete organization setup.
- [ ] Generate a sample contract and confirm organization fields come from settings.
- [ ] Open an existing database and confirm its settings are unchanged.

**Dependencies:** Tasks 2 and 3.

**Files likely touched:**

- `ContractForge/app/services/settings_service.py`
- `ContractForge/app/routers/settings.py`
- `ContractForge/templates/settings/index.html`
- `tests/integration/test_organization_profile.py`

**Estimated scope:** Medium, 4 files.

---

### Task 6: Remove real customers and domain-specific seed assumptions

**Description:** Make initial data neutral and safe for a public installation.

**Acceptance criteria:**

- [ ] No customer is created automatically in a clean installation.
- [ ] Public sample products use fictional, generic names and values, or the catalog starts empty.
- [ ] Seed execution remains idempotent.

**Verification:**

- [ ] Initialize an empty database twice and compare record counts.
- [ ] Run the public-content scanner.
- [ ] Confirm existing user-created customers and products are not modified.

**Dependencies:** Tasks 2 and 5.

**Files likely touched:**

- `ContractForge/app/models/seed.py`
- `tests/integration/test_neutral_seed.py`

**Estimated scope:** Small, 2 files.

---

### Task 7: Replace branded Word assets and genericize quotation generation

**Description:** Remove all organization-specific DOCX assets and replace fixed quotation branches with product/template-driven behavior.

**Acceptance criteria:**

- [ ] The public repository contains only fictional, neutral sample DOCX files.
- [ ] Quotation price, VAT, duration, and template come from the selected product/template.
- [ ] No product code triggers organization-specific behavior in Python.

**Verification:**

- [ ] Extract text from every bundled DOCX and run the public-content scanner.
- [ ] Generate quotations for two generic products with different prices and templates.
- [ ] Verify an existing private uploaded template still renders correctly.

**Dependencies:** Tasks 5 and 6.

**Files likely touched:**

- `ContractForge/app/routers/quotation.py`
- `ContractForge/templates/quotation/form.html`
- `ContractForge/app/services/template_service.py`
- `ContractForge/assets/default_templates/`
- `tests/integration/test_generic_quotation.py`

**Estimated scope:** Medium, 4 code/test files plus replacement assets.

---

### Task 8: Enforce neutral public content in CI

**Description:** Run the public-content scanner against source, documentation, filenames, archives, and text extracted from DOCX assets.

**Acceptance criteria:**

- [ ] CI fails on forbidden organization terms and internal number patterns.
- [ ] CI fails when database, PDF, customer upload, generated contract, log, EXE, or ZIP artifacts are tracked.
- [ ] Scanner output identifies paths without printing sensitive file content.

**Verification:**

- [ ] Add test fixtures for each forbidden class.
- [ ] Run the scanner locally and in GitHub Actions.
- [ ] Confirm the clean repository passes.

**Dependencies:** Tasks 1 and 7.

**Files likely touched:**

- `scripts/check_public_repo.py`
- `tests/unit/test_public_repo_scan.py`
- `.github/workflows/quality.yml`

**Estimated scope:** Small, 3 files.

---

### Checkpoint C: Neutralization

- [ ] Clean installation contains no real organization or customer information.
- [ ] All public templates use fictional identities.
- [ ] Existing local private data remains usable.
- [ ] CI rejects reintroduction of branded/private material.

---

## Phase 4: Business correctness

### Task 9: Centralize lifecycle transitions and bulk updates

**Description:** Make every status change use one validated transition path with required-document checks and event logging.

**Acceptance criteria:**

- [ ] Bulk updates enforce the same `VALID_TRANSITIONS` rules as single updates.
- [ ] `Invoiced` requires its configured invoice evidence and `PaidActive` requires payment evidence.
- [ ] Every successful status change creates a `ContractEvent`; failed batches roll back consistently.

**Verification:**

- [ ] Test every allowed and rejected transition.
- [ ] Reproduce the former `Generated -> PaidActive` bulk bypass and confirm rejection.
- [ ] Test a mixed-validity bulk operation and its transaction policy.

**Dependencies:** Task 2.

**Files likely touched:**

- `ContractForge/app/services/contract_service.py`
- `ContractForge/app/routers/contracts.py`
- `tests/unit/test_contract_lifecycle.py`
- `tests/integration/test_bulk_status.py`

**Estimated scope:** Medium, 4 files.

---

### Task 10: Derive expiry warnings and unify reporting policy

**Description:** Stop using `ExpiringSoon` as a destructive workflow transition and centralize status sets used by dashboard, lists, and reports.

**Acceptance criteria:**

- [ ] A `Signed` contract nearing its end date remains `Signed` and can become `Invoiced`.
- [ ] Dashboard warnings are derived from `end_date` and configurable thresholds.
- [ ] `PaidActive` and `Invoiced` contribute correctly to active/signed/report totals.

**Verification:**

- [ ] Reproduce the former `Signed -> ExpiringSoon` failure and confirm invoicing remains possible.
- [ ] Compare dashboard and Excel report totals for the same fixture.
- [ ] Test expiry boundaries at 0, 7, 30, 31, and 60 days.

**Dependencies:** Task 9.

**Files likely touched:**

- `ContractForge/app/services/dashboard_service.py`
- `ContractForge/app/services/contract_service.py`
- `ContractForge/app/routers/customers.py`
- `ContractForge/app/models/contract.py`
- `tests/integration/test_expiry_reporting.py`

**Estimated scope:** Medium, 5 files.

---

### Checkpoint D: Business behavior

- [ ] End-to-end lifecycle works from draft through payment.
- [ ] Expiry warnings do not alter billing state.
- [ ] Dashboard, contract list, and reports agree.
- [ ] All lifecycle and reporting regression tests pass.

---

## Phase 5: Local security and file integrity

### Task 11: Secure the local application boundary

**Description:** Keep the single-user model while preventing LAN exposure, cross-origin writes, stored XSS, cross-customer updates, and orphaned sensitive files.

**Acceptance criteria:**

- [ ] Every launcher binds to `127.0.0.1`.
- [ ] Unsafe requests from untrusted Origin/Host values are rejected.
- [ ] Customer-controlled values are rendered through `textContent`, DOM construction, or Jinja `tojson`.
- [ ] Sub-unit updates verify both unit ID and customer ID.
- [ ] Contract deletion applies a documented database/file cleanup policy.

**Verification:**

- [ ] Confirm another LAN device cannot connect.
- [ ] Submit cross-origin POST requests and expect rejection.
- [ ] Test HTML/script payloads in customer names, addresses, search, and batch lists.
- [ ] Attempt cross-customer sub-unit modification and expect rejection.
- [ ] Delete a contract fixture and verify the selected cleanup policy.

**Dependencies:** Tasks 2 and 10.

**Files likely touched:**

- `ContractForge/main.py`
- `ContractForge/app/services/customer_service.py`
- `ContractForge/app/services/contract_service.py`
- `ContractForge/templates/base.html`
- `tests/integration/test_local_security.py`

**Estimated scope:** Medium, 5 files. If file cleanup requires substantial work, split it into a separate task before implementation.

---

### Checkpoint E: Security

- [ ] Application is loopback-only.
- [ ] Known stored-XSS payloads do not execute.
- [ ] Cross-origin state-changing requests fail.
- [ ] Ownership checks protect customer sub-resources.
- [ ] Sensitive files follow the documented deletion policy.

---

## Phase 6: Desktop WebView

### Task 12: Introduce the WebView desktop shell

**Description:** Replace normal-browser startup with a `pywebview` window while preserving system tray and reliable server lifecycle management.

**Acceptance criteria:**

- [ ] Launcher starts FastAPI in a controlled background process/thread.
- [ ] WebView opens only after `/health` succeeds.
- [ ] Closing the final window follows a defined policy: exit completely or minimize to tray.
- [ ] Server is stopped cleanly when the application exits.
- [ ] External URLs are opened in the system browser rather than inside the application.

**Verification:**

- [ ] Test clean launch, second launch, server-start failure, occupied port, window close, tray restore, restart, and full exit.
- [ ] Confirm no normal browser window opens.
- [ ] Confirm only localhost navigation remains inside WebView.

**Dependencies:** Task 11.

**Files likely touched:**

- `ContractForge.pyw`
- `ContractForge/requirements.txt`
- `ContractForge/app/desktop.py`
- `tests/unit/test_desktop_lifecycle.py`

**Estimated scope:** Medium, 4 files.

---

### Task 13: Package a standalone Windows application

**Description:** Build a distributable application containing Python, FastAPI, WebView integration, static files, templates, and neutral assets.

**Acceptance criteria:**

- [ ] End user does not need to install Python.
- [ ] Package includes all required application assets but no user database or private profile.
- [ ] Installer checks for WebView2 Runtime and offers the approved installation path when missing.
- [ ] Application writes only to LocalAppData.

**Verification:**

- [ ] Install and run on a clean Windows virtual machine.
- [ ] Exercise customer creation, template upload, contract generation, DOCX/Excel/ZIP download, PDF/image viewing, and backup.
- [ ] Uninstall and verify user data is preserved unless explicitly selected for removal.

**Dependencies:** Task 12.

**Files likely touched:**

- `packaging/pyinstaller/contractforge.spec`
- `packaging/windows/installer.iss`
- `build_exe.bat`
- `.github/workflows/windows-release.yml`
- `tests/smoke/windows_release_checklist.md`

**Estimated scope:** Medium, 5 files.

---

### Checkpoint F: Desktop release candidate

- [ ] Standalone application runs without Python installed.
- [ ] WebView supports every critical download/upload workflow.
- [ ] Installation, upgrade, rollback, and uninstall behavior are documented.
- [ ] No private data appears in the release package.

---

## Phase 7: Open-source publication

### Task 14: Publish documentation, license, CI, and beta release

**Description:** Complete the repository metadata and produce the first public beta from a clean commit.

**Acceptance criteria:**

- [ ] README explains purpose, screenshots, quick start, architecture, data location, backup, source development, and Windows installation.
- [ ] License is selected only after confirming ownership and redistribution rights for code and bundled assets.
- [ ] SECURITY, CONTRIBUTING, and CHANGELOG documents are present.
- [ ] CI runs tests, syntax checks, public-content scanning, and Windows build validation.
- [ ] GitHub Release contains checksums, release notes, known limitations, and rollback instructions.

**Verification:**

- [ ] Clone the repository into a clean directory and follow README instructions exactly.
- [ ] Build from source on a clean Windows runner.
- [ ] Inspect the Git tree and release archive for forbidden/private content.
- [ ] Complete the post-release smoke checklist.

**Dependencies:** Tasks 8, 10, 11, and 13.

**Files likely touched:**

- `README.md`
- `LICENSE`
- `SECURITY.md`
- `CONTRIBUTING.md`
- `CHANGELOG.md`

**Estimated scope:** Medium, 5 files plus existing CI workflow updates.

---

## 9. Test strategy

### 9.1 Unit tests

- Date parsing and end-date calculation.
- Amount-to-words conversion.
- Contract number generation with generic patterns.
- Lifecycle transition policy.
- Expiry classification.
- Filename sanitization.
- Public-content scanning.
- Local path selection.

### 9.2 Integration tests

- Database initialization and idempotent migrations.
- Existing database compatibility.
- Organization first-run setup.
- Product/template-driven quotation.
- Customer and sub-unit ownership.
- Contract generation and render snapshot.
- Batch generation with partial failures.
- Upload validation and file cleanup.
- Dashboard/report consistency.
- Local Host/Origin/CSRF boundary.

### 9.3 Desktop smoke tests

- First launch.
- Existing-data migration.
- Second-instance behavior.
- WebView close/minimize/restore.
- Download DOCX, XLSX, CSV, and ZIP.
- View PDF and images.
- Upload template and customer documents.
- Backup and restore.
- Offline operation.
- Missing WebView2 Runtime.
- Read-only installation directory.

### 9.4 Release tests

- Clean Windows VM.
- No Python installed.
- No development environment variables.
- No private data in installation package.
- Upgrade from previous beta.
- Rollback to previous version while preserving user data.

---

## 10. Definition of Done

Every implementation task is complete only when:

- [ ] Acceptance criteria are satisfied.
- [ ] New behavior has automated tests.
- [ ] Existing tests pass.
- [ ] Real production data was not used or mutated by tests.
- [ ] No sensitive data appears in diffs, logs, screenshots, fixtures, or artifacts.
- [ ] Application starts and critical flow works manually.
- [ ] Error handling covers the changed path.
- [ ] Documentation is updated when behavior or architecture changes.
- [ ] Performance is measured when the task changes critical queries or rendering.
- [ ] The public-content scanner passes.
- [ ] The change is small enough to review and revert independently.

---

## 11. Migration and rollback strategy

### 11.1 Data migration principles

- Never migrate the only copy.
- Always create a timestamped backup.
- Copy first; validate; switch active data root last.
- Do not automatically delete legacy data.
- Record migration version in application settings.
- Make every migration safe to re-run.

### 11.2 Release rollback

If a release fails:

1. Close ContractForge.
2. Restore or install the previous application version.
3. Keep `%LOCALAPPDATA%\ContractForge` unchanged.
4. If the release performed a data migration, restore its timestamped backup.
5. Run `/health` and the critical smoke flow.
6. Record the failure and block the faulty release.

### 11.3 Rollback triggers

- Database integrity failure.
- Missing or unreadable uploads.
- Contract values or statuses change unexpectedly.
- Generated documents contain incorrect organization/customer data.
- WebView cannot perform critical download/upload workflows.
- Private information appears in public source or release artifacts.
- Error rate or startup failures materially exceed the previous release.

---

## 12. Security and privacy checklist for publication

- [ ] `data/` has never entered Git history.
- [ ] No database file is tracked.
- [ ] No customer identity document, scan, invoice, payment slip, or contract output is tracked.
- [ ] Logs are excluded and scrubbed from examples.
- [ ] Public screenshots use fictional data.
- [ ] Public DOCX assets use fictional data.
- [ ] Organization settings have no real defaults.
- [ ] Seed data has no real customer or organization.
- [ ] Build artifacts contain no private profile.
- [ ] Application binds only to loopback.
- [ ] Unsafe requests enforce the local-origin policy.
- [ ] User-controlled content is encoded before HTML/JavaScript insertion.
- [ ] Dependency versions are locked for releases.
- [ ] License compatibility of bundled libraries/assets is documented.

If private content is committed accidentally:

1. Stop publication immediately.
2. Treat exposed customer/organization information as compromised.
3. Remove it from the working tree.
4. Rewrite Git history before public release.
5. Rotate any exposed secret or authorization credential where applicable.
6. Re-run the complete repository scan.

---

## 13. Performance plan

Performance work is measurement-driven:

1. Preserve the current baseline.
2. Add query-count and duration tests for customer list, contract list, and dashboard.
3. Optimize only after correctness and WebView behavior are stable.
4. Consolidate dashboard queries only if measurements demonstrate value.

Initial budgets for the current expected single-user scale:

- Customer list: no more than 4 SQL statements.
- Contract list: no more than 4 SQL statements.
- Dashboard: target no more than 15 SQL statements after consolidation.
- Typical page server time: under 200 ms on supported local hardware.
- App ready-to-window time: target under 5 seconds on a clean warm start.

Budgets may be adjusted from measured release-candidate hardware.

---

## 14. Release sequence

### Alpha

- Internal source run.
- Existing database compatibility.
- Neutral organization profile.
- Business/security regression suite.

### Desktop alpha

- WebView shell.
- Manual document workflow testing.
- LocalAppData migration.

### Public beta

- Clean repository.
- Standalone Windows installer.
- Fictional screenshots and sample assets.
- Known limitations documented.

### Stable v1

- Beta migration issues resolved.
- Upgrade and rollback verified.
- Release build reproducible.
- Public-content and privacy review complete.

---

## 15. Suggested commit sequence

Keep commits independently reviewable:

1. `chore: add repository safety boundary`
2. `test: add isolated sqlite test harness`
3. `feat: move runtime data to user data directory`
4. `feat: add safe legacy data migration`
5. `feat: add neutral organization profile`
6. `chore: remove organization-specific seed data`
7. `feat: make quotations product and template driven`
8. `test: enforce neutral public content`
9. `fix: validate bulk contract transitions`
10. `fix: derive expiry without overwriting lifecycle`
11. `fix: harden local application boundary`
12. `feat: add webview desktop shell`
13. `build: package standalone windows application`
14. `docs: prepare public beta release`

Do not combine data migration, lifecycle changes, WebView integration, and repository publication into one commit.

---

## 16. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Real customer data enters Git history | Critical | Ignore rules before Git init, scanner in CI, clean review before first push |
| Existing user data is lost during relocation | Critical | Copy-only migration, timestamped backups, integrity checks, no automatic cleanup |
| Removing defaults breaks existing private installation | High | Preserve stored settings, migrate only missing values, compatibility tests |
| Expiry refactor changes report totals unexpectedly | High | Shared policy module, fixture-based reconciliation, compare old/new outputs |
| WebView breaks file download or PDF behavior | High | Dedicated desktop smoke matrix before packaging |
| PyInstaller misses templates/static/native dependencies | High | Explicit spec file, clean-VM test, CI artifact inspection |
| WebView2 Runtime missing | Medium | Installer prerequisite check and supported installation path |
| Public template licensing is unclear | High | Replace with newly authored fictional template and document asset licenses |
| Genericizing quotation scope grows too large | Medium | Keep one product/template-driven flow; postpone plugin architecture |
| Query optimization changes behavior | Medium | Measure first, snapshot output, separate optimization commits |
| Existing manual migrations diverge | High | Version migrations, idempotent tests, backup and rollback documentation |

---

## 17. Open questions requiring owner decisions

These decisions should be recorded before their dependent task begins:

1. **License**
   - Recommended default: MIT for simplicity.
   - Confirm that the owner has the right to publish the code and newly selected sample assets.

2. **Public sample products**
   - Option A: empty product catalog on first run.
   - Option B: one or two clearly fictional sample products.
   - Recommendation: fictional samples that the onboarding flow offers to remove.

3. **Quotation feature**
   - Recommendation: keep it, but make it entirely product/template-driven.
   - Do not preserve named YTCS/HSSK branches in the public core.

4. **Window-close behavior**
   - Option A: close window and exit application.
   - Option B: close window to tray; explicit tray action exits.
   - Recommendation: minimize to tray only when the user enables that preference.

5. **Legacy `ExpiringSoon` records**
   - Decide how to reconstruct their underlying lifecycle state.
   - Recommendation: use event history where available; otherwise present a one-time migration review rather than guessing silently.

6. **Contract deletion policy**
   - Option A: delete DB record and all associated files.
   - Option B: move record/files to a recoverable trash.
   - Recommendation: recoverable trash for a desktop business application.

7. **Backup policy**
   - Recommendation: manual “Create backup” action plus automatic pre-migration backups.
   - Scheduled/cloud backup remains outside v1 scope.

---

## 18. Implementation start gate

Implementation may begin when:

- [ ] This plan is reviewed and accepted.
- [ ] License direction is selected.
- [ ] Public sample-product strategy is selected.
- [ ] Quotation genericization direction is accepted.
- [ ] Window-close behavior is selected.
- [ ] Contract deletion policy is selected.
- [ ] A verified external backup of the current `data/` directory exists.
- [ ] Task 1 is executed before any Git initialization or public push.

Recommended first implementation slice:

```text
Task 1: Repository safety boundary
Task 2: Isolated pytest foundation
Checkpoint A: Confirm data safety
```

No neutralization, migration, lifecycle refactor, WebView work, packaging, or GitHub publication should begin before that checkpoint passes.
