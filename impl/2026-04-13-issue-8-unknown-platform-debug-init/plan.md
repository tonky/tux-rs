# High-Level Plan

## Stage 1 - Detection and Diagnostics Hardening
- Extend platform heuristics to recognize NB02 board vendor as Uniwill fallback class.
- Add SKU normalization/splitting support for combined SKU strings where useful.
- Add startup failure diagnostics (copy-paste block) for detection/init failures.
- Add or update tests for new heuristics and diagnostics output invariants.
- Validate with targeted tests and formatting/lint checks relevant to touched code.

## Stage 2 - Follow-up Documentation
- Update README troubleshooting section with startup diagnostics collection commands.
- Ensure issue-repro instructions include exact command users should run.

## Exit Criteria
- Issue #8 scenario no longer fails with unknown platform when board vendor is NB02.
- Startup failure logs include structured copy-paste diagnostics with DMI/probe details.
- Test coverage exists for the detection path change.
