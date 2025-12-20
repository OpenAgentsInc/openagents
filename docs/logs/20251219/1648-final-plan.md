# Final Push: Transition to Autopilot as Primary Interface

**Goal:** Complete autopilot improvements and switch from Claude Code CLI to autopilot terminal.

---

## Completed ✅

| Issue | Title | Status |
|-------|-------|--------|
| #1 | Add --model flag to CLI | ✅ Done |
| #2 | Clean up .mcp.json on signal/panic | ✅ Done |

---

## Remaining Issues

| Issue | Title | Priority | Critical for Transition? |
|-------|-------|----------|-------------------------|
| #3 | Add AUTOPILOT_MODEL env var | medium | No |
| #4 | Add trajectory replay/debug mode | medium | No |
| #5 | Add max iterations safety limit | medium | No |

---

## Transition Checklist

What autopilot needs to be the primary interface:

- [x] Issue tracking (MCP tools work)
- [x] Model selection (--model flag added)
- [x] Crash resilience (.mcp.json cleanup)
- [x] Trajectory logging (rlog + json)
- [ ] Push commits to main
- [ ] Create wrapper script for easy invocation

---

## Implementation Steps

### 1. Push Current Work
```bash
git push origin main
```

### 2. Create Convenience Script
Create `autopilot.sh` in repo root for easy terminal access:
```bash
#!/bin/bash
cargo autopilot run --with-issues --max-turns 30 --max-budget 3.0 "$@"
```

### 3. Seed More Issues
Run one more seeding pass to add issues for:
- Better error messages when tools fail
- Summary of work done at end of session
- `--continue` flag to resume sessions

### 4. Document the Workflow
Update `docs/autopilot/README.md` with:
- How to run autopilot
- How to check issue queue
- How to review trajectories
- Git workflow (branch → PR → squash → main)

---

## Git Workflow Going Forward

```
1. autopilot creates branch: git checkout -b autopilot/issue-N
2. autopilot makes changes and commits
3. autopilot pushes and opens PR
4. Human reviews, squash-merges
5. autopilot pulls main, continues
```

---

## Files to Create/Modify

| File | Purpose |
|------|---------|
| `autopilot.sh` | Convenience wrapper script |
| `docs/autopilot/README.md` | Usage documentation |
| `.gitignore` | Add autopilot.db if not present |

---

## Handoff Message

After this plan executes, interaction moves to:
```bash
./autopilot.sh "Your task here"
```

Logs will be in `docs/logs/YYYYMMDD/`. Issue queue in `autopilot.db`.

**See you in openagents autopilot.** 🚀
