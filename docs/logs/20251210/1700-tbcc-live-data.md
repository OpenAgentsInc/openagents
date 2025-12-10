# TBCC Live Data Integration

**Date:** 2024-12-10
**Status:** Complete

## Summary

Implemented live data infrastructure for TBCC (Terminal-Bench Command Center) components, replacing hardcoded sample data with persistent storage and real task loading.

## Changes

### New Services Module (`crates/gym/src/services/`)

Created a complete data layer:

1. **`mod.rs`** - Central `TBCCDataService` orchestrating all stores
2. **`task_loader.rs`** - Loads tasks from JSON suite files
   - Searches `tasks/` directory for `*.json` suite files
   - Parses Terminal-Bench task format
   - Converts to internal `TBTask` type
3. **`run_store.rs`** - Run history persistence
   - Stores runs to `tb_runs.json`
   - Calculates dashboard statistics (success rate, avg steps, etc.)
   - Supports start/update/complete lifecycle
4. **`settings_store.rs`** - Settings persistence
   - Saves/loads `ExecutionSettings` and `ContainerSettings`
   - Uses `directories` crate for platform-appropriate paths

### Updated Views

**Task Browser:**
- Now uses `TaskLoader` to load tasks from JSON files
- Added `refresh()` and `load_suite()` methods
- Automatically discovers available suite files

**Dashboard:**
- Uses `RunStore` for live statistics
- Added `set_run_store()` and `refresh()` methods
- All KPIs calculated from actual run history

**Run Browser:**
- Uses `RunStore` for run history
- Added `set_run_store()` and `refresh()` methods

**Settings:**
- Uses `SettingsStore` for persistence
- Added `save()`, `reset()`, and getter methods

### Dependencies Added

```toml
directories = "5.0"  # Platform-appropriate data directories
uuid = { version = "1.0", features = ["v4"] }  # Run IDs
```

## Data Paths

| Data | Location |
|------|----------|
| Tasks | `./tasks/*.json` (local suite files) |
| Runs | `~/.local/share/commander/tb_runs.json` |
| Settings | `~/.local/share/commander/tbcc_settings.json` |

## Tests

All 3 new tests pass:
- `task_loader::tests::test_parse_difficulty`
- `run_store::tests::test_run_store_basics`
- `settings_store::tests::test_settings_persistence`

## TBCCScreen Integration

Created `crates/gym/src/tbcc/screen.rs` - main TBCC container with:
- 4 sub-tabs (Dashboard, Tasks, Runs, Settings)
- Initializes `TBCCDataService` and wires stores to views
- Replaces standalone `DashboardView` in `GymScreen`

Updated `GymScreen`:
- Now uses `TBCCScreen` instead of `DashboardView` for TBCC tab
- Full 4-tab TBCC interface now accessible
