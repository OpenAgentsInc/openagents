# 0404 Work Log (oa-97f9ab)

- Added parallelExecution install timeout/args defaults (15m, --frozen-lockfile) to schema.
- Updated parallel runner and overnight-parallel to honor configurable install timeout/args and avoid premature 5m aborts.
- Added project config test coverage for new defaults.
