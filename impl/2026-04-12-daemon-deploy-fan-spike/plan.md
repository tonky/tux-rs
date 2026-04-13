# High-Level Plan: Daemon Deploy Fan Spike

## Stage 1: Trace deploy/startup fan-control path

- inspect `deploy-daemon`, daemon startup/shutdown sequencing, and Uniwill fan backend behavior
- identify whether the spike comes from daemon safety fallback, EC auto mode, or an extra profile-side effect
- remove the conflicting startup/profile behavior with the smallest possible code change
- validate with focused daemon tests and static checks