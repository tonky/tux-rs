# Review 1

## Stage

Stage 1: Contract Mapping and Hardware Fixture Capture

## Status

Ready For Hardware Capture Sign-Off

## Checklist

- [x] Stage goals implemented
- [x] Fixture schema and metadata validated
- [x] Clippy and fmt clean
- [x] Contract drift risks documented

## Findings

- Implemented and validated:
	- Contract matrix added.
	- Fixture schema docs and sample fixture added.
	- Capture helper script added.
	- Schema validation integration tests added and passing.
	- Workspace fmt/clippy/test checks passing.
- Additional hardening after independent review:
	- TOML escaping strengthened for multiline payloads.
	- Missing tool/platform capture warnings added.
	- Required key non-empty checks and payload parsing tightened.
	- Raw/normalized duty consistency checks added.
- Open item before marking stage complete:
	- Run capture helper on real Uniwill hardware and review resulting fixture provenance and normalized values.
	- Current session host does not expose /sys/devices/platform/tuxedo_uw_fan and returned empty D-Bus payloads for capture methods, so this run cannot serve as canonical hardware capture.

## Follow-up

- If hardware capture differs from sample assumptions, update fixture constraints and matrix notes before Stage 1 closeout.
