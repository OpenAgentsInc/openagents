# Build Health Status

> **Last Updated:** 2025-12-05T14:00:04.176Z
> **Auto-generated** by `bun run health:report`

## Summary

| Metric | Status | Details |
|--------|--------|---------|
| **Tests** | ✅ PASS | 1346 pass, 0 fail |
| **TypeCheck** | ⚠️ WARN | See issues below |
| **E2E Tests** | ❌ FAIL | 1 command(s) run |
| **Coverage** | 📊 TRACKED | Run tests with --coverage to see details |

## Test Results

```
1346 pass
0 fail
3552 expect() calls
Ran 1346 tests across 137 files. [DEBUG]
```

### Test Status: ✅ **ALL TESTS PASSING**

## TypeCheck Status

⚠️ **Issues Found**

```
$ tsc --noEmit -p tsconfig.typecheck.json

```

## E2E Test Results

- `bun run e2e:test`: ❌ FAIL

## Coverage Highlights

To see detailed coverage, run:

```bash
bun test --coverage
```

### Critical Modules

Coverage tracking for:
- `src/agent/orchestrator/` - Agent orchestration logic
- `src/tools/` - Tool implementations (bash, edit, read, write, etc.)
- `src/hud/` - HUD server and protocol
- `src/tasks/` - Task management system

## Trends

*Historical data will be tracked once CI automation is in place.*

## How to Update

Run the health report generator:

```bash
bun run health:report
```

This will:
1. Run tests, typechecks, and e2e tests (as configured in .openagents/project.json)
2. Update this file with latest results
3. Exit with non-zero code if any checks fail

## CI Integration

Add to your CI workflow:

```yaml
- name: Update Build Health
  run: bun run health:report

- name: Commit Health Report
  if: always()
  run: |
    git config user.name "CI Bot"
    git config user.email "ci@openagents.com"
    git add docs/BUILD-HEALTH.md
    git commit -m "chore: update build health report [skip ci]" || true
    git push
```
