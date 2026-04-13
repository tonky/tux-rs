# Worklog Stage 1: Daemon Deploy Fan Spike

## Summary

- Investigated the deploy path and found the fan spike is most consistent with firmware regaining control during restart, combined with a newly added unconditional Uniwill AC performance-profile write.
- Removed the extra performance-profile write so startup/profile application no longer fights direct fan control on Uniwill hardware.
- Investigated the remaining stuck-at-100% state and found `tuxedo-uw-fan` was returning `EIO` on redundant `fan_mode=1` writes, preventing PWM updates while manual mode was already active.
- Changed the Uniwill backend to avoid redundant manual-mode writes and confirmed PWM values drop from max after redeploy.

## Decisions

- Keep the existing startup/shutdown `set_auto()` safety behavior.
- Remove the extra Uniwill AC override instead of weakening fan-safety resets.
- Treat real-hardware verification as mandatory follow-up because EC behavior during restart cannot be modeled by unit tests.