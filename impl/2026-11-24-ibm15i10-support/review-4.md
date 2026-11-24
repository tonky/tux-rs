# Stage 4 review

Reviewers: Opus (high) + Sonnet, scoped to `c762c02` with short reports (user asked to keep usage low).

| Reviewer | Sev | Finding | Action |
|----------|-----|---------|--------|
| Opus | **blocker** | Battery profile without EPP (custom/TCC-imported) never undid the AC `performance` override. | **Fixed** `8acb180`: quirked devices fall back to `balance_power` on battery when the profile sets no EPP; regression test. |
| Opus | nit | Quirk-less `apply()` could be used by future production code. | **Fixed** `8acb180`: `#[cfg(test)]`. |
| Opus | nit | No auto-switch-level AC→battery test. | Covered at applier level (`ibm15i10_epp_follows_unplug_and_replug`, `…without_epp_drops_ac_override`) + live unplug test. |
| Both | nit | Duplicated doc line above `apply_on`. | **Fixed** `1164370`. |
| Sonnet | nit | Quirk log on every AC apply even when EPP already `performance`. | **Fixed** `1164370` (logs only when it changes the EPP). |
| Sonnet | nit | No unplug/replug sequence test. | **Fixed** `1164370`. |
| Opus | — | Other EPP writers (`SetEpp`, `SetPowerSettings`) are explicit user actions; power-monitor-unavailable path applies no profile (battery-safe). | No change. |
