# Stage 4: Workflow Hardening and Release Validation

## Objective

Make the reliability suite repeatable and enforceable through local and CI workflows,
with clear manual hardware refresh procedures.

## Scope

- Add or refine just targets for contract tests and fixture validation.
- Integrate deterministic suites into CI-safe paths.
- Document manual capture/compare workflow and drift review criteria.

## Target Files

- Justfile
- README.md
- impl/2026-04-13-driver-daemon-reliable-test-suite/worklog-4.md
- impl/2026-04-13-driver-daemon-reliable-test-suite/review-4.md

## Tasks

1. Add workflow commands for contract tests and fixture checks.
2. Integrate deterministic suites into standard check and CI flows where appropriate.
3. Document fixture refresh rules and approval process for expected behavior drift.
4. Run full validation and capture results in worklog and review documents.

## Risks

- Long-running suites can slow feedback if split strategy is not clear.
- Poorly documented capture refresh can cause unreviewed behavior drift.

## Verification

- flox activate -- just fmt
- flox activate -- just clippy
- flox activate -- just test
- flox activate -- just ci
- manual fixture capture and compare run

## Exit Criteria

- Reliability suite is documented, repeatable, and quality-gated.
- Manual hardware refresh process is clear and auditable.
