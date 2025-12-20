# Dogfooding: Use Autopilot to Improve Autopilot

**Goal:** Create issues for autopilot improvements, then run autopilot to work on them.

---

## Strategy

1. Seed the issue database with well-scoped improvement tasks
2. Run autopilot with `--with-issues` to pick up and complete issues
3. Review results, merge PRs, iterate

---

## Initial Issue Set (Starter Pack)

Prioritized by: self-contained scope, clear acceptance criteria, and value for dogfooding.

| # | Title | Priority | Type | Why First |
|---|-------|----------|------|-----------|
| 1 | Add `--model` flag to autopilot CLI | high | feature | Simple, improves dogfooding flexibility |
| 2 | Add `AUTOPILOT_MODEL` env var support | medium | feature | Pairs with #1 |
| 3 | Clean up .mcp.json on signal/panic | high | bug | Critical for reliability |
| 4 | Add `issue_refresh_claim` MCP tool | medium | feature | Needed for long tasks |
| 5 | Add `type` filter to `issue_list` tool | low | feature | Quick enhancement |

---

## Workflow

### Phase 1: Seeding Run (Autopilot creates its own issues)
```bash
cargo autopilot run --with-issues --max-turns 10 --max-budget 1.0 \
  "You are improving the autopilot system. Use issue_create to create 3-5 well-scoped
   improvement issues for autopilot. Focus on:
   1. Add --model flag to CLI (high priority, feature)
   2. Clean up .mcp.json on signal/panic (high priority, bug)
   3. Add AUTOPILOT_MODEL env var (medium priority, feature)
   Then list the issues you created."
```

### Phase 2: Work Runs (Autopilot works on issues)
```bash
cargo autopilot run --with-issues --max-turns 20 --max-budget 2.0 \
  "Use issue_ready to get the next task. Claim it with issue_claim using your session_id.
   Implement the change, run 'cargo build' and 'cargo test' to verify.
   If successful, commit the changes. Then complete the issue with issue_complete.
   If blocked, use issue_block with the reason."
```

### Phase 3: Review
- Check trajectory logs in `docs/logs/YYYYMMDD/`
- Review git commits made by autopilot
- Merge or provide feedback
- Run Phase 2 again for next issue

---

## Files to Modify

| File | Change |
|------|--------|
| `crates/autopilot/src/main.rs` | Add --model flag, signal handlers |
| `crates/issues-mcp/src/main.rs` | Add refresh_claim tool, type filter |
| `crates/issues/src/issue.rs` | Add refresh_claim function if missing |

---

## Success Criteria

- [ ] At least 2 issues completed by autopilot autonomously
- [ ] Trajectory logs show proper tool usage (claim → work → complete)
- [ ] Code changes are correct and pass tests
- [ ] PRs can be merged with minimal human intervention

---

## Implementation Steps

1. **Merge PR #1524** - Ensure issues-mcp is on main branch
2. **Seeding run** - Run autopilot to create improvement issues (Phase 1)
3. **First work run** - Run autopilot to pick up and complete an issue (Phase 2)
4. **Review trajectory** - Check logs, verify code quality
5. **Iterate** - Push commits, run Phase 2 again for next issue
6. **Scale** - Add more issues as we learn what works

---

## Risk Mitigation

- Start with simple, isolated changes (CLI flags, not core logic)
- Set low max_turns (10-15) for first runs to limit blast radius
- Review all commits before pushing
- Keep max_budget low ($1-2) during testing
