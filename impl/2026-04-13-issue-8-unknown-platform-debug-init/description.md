# Feature Description

## Title
Handle Issue #8 unknown platform on InfinityBook Pro Gen9 AMD and improve startup diagnostics.

## Problem
Issue #8 reports daemon startup failure on a supported TUXEDO laptop:
- board_vendor: NB02
- product_sku: IBP14A09MK1 / IBP15A09MK1
- error: unknown platform

Current detection can fail when SKU is new/unmapped and platform probing does not classify the device.

## Goals
- Make detection resilient for this hardware class so daemon can initialize.
- Improve diagnostics on initialization failure so users can copy-paste actionable details.
- Keep behavior safe by preferring conservative fallback capabilities when exact SKU is unknown.

## Non-goals
- Full feature tuning for every new SKU variant in this issue.
- Large refactor of detection architecture.
