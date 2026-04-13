# Stage 1 Worklog

## 2026-04-13
- Stage started.
- Next: implement NB02 fallback heuristic and initialization diagnostics output.
- Added curated TCC-derived SKU platform hints for recent combined IBP AMD identifiers:
	- `IBP14A09MK1 / IBP15A09MK1`
	- `IBP14A10MK1 / IBP15A10MK1`
	- `IIBP14A10MK1 / IBP15A10MK1` (typo variant present in TCC)
- Kept hints as platform-level fallback (`Uniwill`) rather than adding potentially incorrect full device descriptors.
