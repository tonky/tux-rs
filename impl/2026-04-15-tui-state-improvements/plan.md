# Plan

## Stage Order

1. **BoundedIndex wrapper** — pure addition, no behavioral change, highly testable
2. **Explicit form field keys** — small mechanical change, eliminates normalization fragility
3. **Centralized text-edit state** — moderate refactor, benefits from cleaner codebase
4. **Form dirty-data warnings** — most complex behavioral change, benefits from prior stages

Stages have no compile-time dependencies but are ordered by risk (lowest first).

## Key Decisions

- BoundedIndex passes `len` per-call (not stored) because backing collections change independently
- Form.selected_index stays as `usize` — Form has special skip-disabled navigation
- Stage 4 is TUI-only (no daemon changes) — skip load when dirty, warn user
- Profile editor fields are out of scope for Stage 2 (use struct serialization, not TOML)
