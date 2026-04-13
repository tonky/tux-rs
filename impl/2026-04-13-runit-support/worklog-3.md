# Stage 3 Worklog — CI Integration and Validation Documentation

## Scope Completed

- Added dedicated CI job `runit-smoke` in `.github/workflows/ci.yml`.
- Kept smoke execution isolated from core `check` job.
- Added README development command entry for `just runit-smoke-repeat`.
- Added README validation scope note clarifying container-vs-hardware guarantees.

## CI Job Design

- Build smoke image once per job.
- Run smoke container twice (`pass 1`, `pass 2`) for startup-race confidence.
- Keep existing fmt/clippy/test job unchanged for baseline signal stability.

## Verification Performed

- Existing Stage 2 local parity command already green:
	- `flox activate -- just runit-smoke-repeat`
- Workflow YAML checked by inspection for valid structure and independent job layout.

## Notes

- Full GitHub Actions execution was not run locally; PR CI run will provide final
  hosted verification.
