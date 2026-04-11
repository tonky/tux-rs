# Stage 4: OSS Readiness & Follow-up Polish

## Objective
Finalize the codebase for public launch by standardizing the repository structure, separating deep-dive documentation from the landing page, introducing contributor templates, and resolving the final TUI CLI feature request.

## Breakdown

### 1. Repository Documentation Refactor
- Move `OVERVIEW.md` contents into dedicated documents within `docs/`:
  - `docs/architecture.md`: In-depth D-Bus interfaces, trait hierarchies, and kernel shim architecture diagram.
  - `docs/hardware_support.md`: Hardware capability table and supported models.
- Clean up `README.md` to be a punchy landing page:
  - Add standard OSS badges (License, Rust version).
  - Keep the Splash GIF and quick-start `just` commands.
  - Link directly to `docs/` and `CONTRIBUTING.md`.

### 2. Contributor Onboarding
- `CONTRIBUTING.md`: Add a guide pointing to the `Justfile` development loops, tests, and formatting checks.
- `LICENSE`: Add standard `GPL-3.0` license text.
- `.github` templates:
  - `ISSUE_TEMPLATE/bug_report.md`
  - `ISSUE_TEMPLATE/feature_request.md`
  - `PULL_REQUEST_TEMPLATE.md`

### 3. TUI Follow-up Feature
- Add `--tab=<name>` flag to `tux-tui` so the users can quickly drop into a specific view (e.g. `tux-tui --tab=keyboard`). 
- Tab names will be `dashboard`, `profiles`, `fancurve`, `settings`, `keyboard`, `charging`, `power`, `display`, `webcam`, `info`.
