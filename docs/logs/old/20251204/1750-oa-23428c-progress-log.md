# 1750 Work Log - oa-23428c

- Added HealingAttempt tracking to healer types and exported helper utilities.
- Introduced healing key builder in policy using task/subtask/error hash.
- Refactored healer service to load/persist healer-state, dedup resolved attempts, and support injectable spell runner for tests.
