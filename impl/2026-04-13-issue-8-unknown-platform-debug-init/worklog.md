# Worklog

## 2026-04-13
- Created feature tracking folder for Issue #8.
- Captured issue context: unknown platform on NB02 with SKU string "IBP14A09MK1 / IBP15A09MK1".
- Drafted staged implementation plan focused on detection fallback + copy-paste diagnostics.
- Implemented Stage 1 detection hardening:
	- added NB02 board-vendor fallback to Uniwill as a last-resort platform hint,
	- added composite SKU token lookup for slash-delimited SKU values,
	- added curated TCC-derived SKU hints for recent IBP AMD combined identifiers.
- Implemented startup diagnostics improvements:
	- added explicit startup diagnostics CLI (`--dump-startup-diagnostics` / `--dump-init-diagnostics`),
	- added structured copy-paste diagnostics block with DMI fields and probe booleans,
	- enriched diagnostics with daemon version, build mode, and argv context.
- Added regression tests for:
	- Issue #8 Gen9 combined SKU path,
	- Gen10 combined SKU hint path,
	- TCC typo variant combined SKU hint path.
- Validation complete:
	- `cargo test -p tux-core dmi -- --nocapture` passing,
	- `cargo test -p tux-daemon --no-run` passing,
	- `cargo fmt --all -- --check` passing.
