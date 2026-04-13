# Stage 3 — CI Integration and Validation Documentation

## Objective

Enforce runit smoke coverage in CI and provide clear operator guidance on what is and is not validated.

## Scope

- Add CI job for runit container smoke tests.
- Add/adjust `just` targets for local parity with CI steps.
- Document expected runtime limits and required real-hardware follow-up.

## Target Files

- .github/workflows/ci.yml
- Justfile
- README.md
- impl/2026-04-13-runit-support/worklog-3.md
- impl/2026-04-13-runit-support/review-3.md

## Tasks

1. Add dedicated CI job for runit smoke tests with clear failure output.
2. Keep existing CI runtime reasonable by isolating smoke setup.
3. Add README note: container smoke validates supervision/integration only.
4. Record validation matrix and residual risk explicitly.

## Risks

- CI runtime increase and transient network/package flakiness.

## Verification

- Full CI workflow passes on pull request.
- Local command parity confirmed against CI steps.

## Exit Criteria

- Runit smoke checks are automatic in CI.
- Documentation clearly separates container confidence from hardware confidence.
