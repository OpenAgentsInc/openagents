# Khala Burndown Operator Runbook

`pylon khala burndown` is the owner-operated loop for issue #6355. It turns the
Khala roadmap into parallel `codex_agent_task` assignments across ready local
Codex accounts, runs each no-spend assignment, and verifies the owner-only proof
surface before reporting closeout refs.

It does not auto-merge arbitrary coding-agent diffs. The JSON plan and run
result carry `mergePolicy: "operator_review_required"` so an operator can review
the produced work, run the repo gate, commit, push, comment, and close the issue.

## Preflight

```sh
export PYLON_OPENAGENTS_BASE_URL="${PYLON_OPENAGENTS_BASE_URL:-https://openagents.com}"

pylon presence heartbeat --base-url "$PYLON_OPENAGENTS_BASE_URL" --json
pylon accounts list --json
```

The account list must show at least one Codex account with `homeState:
"present"` and readiness `state: "ready"`. `assignment run-no-spend` also
refreshes presence before claiming, so stale-presence blockers return a typed
recovery command instead of silently failing.

If `presence heartbeat` or `khala request --workflow codex_agent_task` reports
that the Pylon registration belongs to another agent immediately after rotating
or reissuing the local OpenAgents agent token, run `pylon auth openagents
--json` and retry. The Worker accepts active credentials linked to the same
OpenAuth owner and re-homes the Pylon registration on the next successful
register/heartbeat write; unrelated agent credentials remain forbidden.

## Dry Run

```sh
pylon khala burndown \
  --base-url "$PYLON_OPENAGENTS_BASE_URL" \
  --roadmap docs/khala/2026-06-26-khala-open-issues-master-roadmap.md \
  --repo OpenAgentsInc/openagents \
  --commit <40-char-origin-main-sha> \
  --verify "bun run --cwd apps/pylon test" \
  --max-parallel 3 \
  --iterations 1 \
  --json
```

Dry run emits a public-safe plan: issue slots, hashed account refs, request/run
commands, and proof commands. Use `--issues 6355,6356` to pin an explicit issue
set instead of parsing the roadmap.

## Execute One Round

```sh
pylon khala burndown \
  --base-url "$PYLON_OPENAGENTS_BASE_URL" \
  --issues 6356,6357,6358 \
  --repo OpenAgentsInc/openagents \
  --commit <40-char-origin-main-sha> \
  --verify "bun run --cwd apps/pylon test" \
  --max-parallel 3 \
  --iterations 1 \
  --execute \
  --json
```

Execution performs, per slot:

1. `pylon khala request --workflow codex_agent_task ...`
2. `pylon assignment run-no-spend --assignment-ref ...`
3. `pylon khala proof --assignment-ref ...`

The result is successful only when the assignment is accepted and proof reports
exact token usage with at least one token row. The output includes assignment
refs, durable request ids, proof event counts, trace counts, and verified token
totals. It also snapshots `/api/public/khala-tokens-served` before and after the
run and reports counter state, delta, and expected minimum delta from verified
proof totals; if the public counter is reachable but unchanged after a verified
closeout, the result carries `blocker.khala_burndown.counter_not_incremented`.

## Review And Closeout

For each successful slot:

1. Review the produced diff and proof refs.
2. Run the issue-specific verification plus the relevant repo gate.
3. Commit and push `main` from the owning repo.
4. Comment on the GitHub issue with commit, tests, assignment ref, and exact
   proof totals.
5. Close the issue only after the scoped change is live on `main`.

If an OpenAgents Worker surface changed, deploy through the repo-safe path only:

```sh
bun run --cwd apps/openagents.com/workers/api deploy:safe
```

Never use raw `wrangler deploy` for the openagents.com Worker.
