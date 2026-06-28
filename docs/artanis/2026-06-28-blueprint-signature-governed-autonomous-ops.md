# Blueprint-Signature-Governed Autonomous Ops (`autonomous-ops-v1`)

> Status: canonical design spec, 2026-06-28. Authored by Artanis (operator
> agent) at the owner's direction, after the overnight after-action
> (`workspace:docs/afteraction/2026-06-28-overnight-fleet-after-action.md`).
> This is a design + implementation spec for the autonomous fleet-ops control
> loop; it is not public-claim copy and does not widen any promise registry
> entry.

## Premise

The overnight failures were **not** failures of operator judgment. They were
failures of the **tools and systems around the intelligence** — there was no
typed contract forcing each step to present evidence before it acted. So a
process could be "alive" without dispatching, a diagnosis could be stated
without reading the ledger, an epic could be auto-closed by one sub-PR, and a
non-runnable command could be recommended.

The fix is to govern **every consequential step of autonomous ops with a
modular Blueprint Signature**: a typed I/O contract with an ordered evidence
predicate list (a gate) and a state model whose **terminal state is the only
state that unlocks the consequential action**. If a required evidence ref is
missing, the action is structurally impossible — not discouraged, impossible.

This applies the existing Blueprint model directly:

- **Signature** = typed I/O contract carrying tool scopes + evidence refs +
  receipt refs + release-gate refs + risk ceilings, resolved via the typed
  Signature Lookup Service (`packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`)
  — a semantic/typed selector, never string matching.
- **Evidence-only write boundary** (`apps/openagents.com/workers/api/src/blueprint/repositories/action-submissions.ts`):
  proposals reference evidence refs and carry evidence-only authority. Historical
  invariant preserved: *"Program Runs are decision evidence; they do not
  authorize writes."*
- **Gate = ordered predicate list + state model** (pattern from
  `autopilot-omega:docs/2026-06-08-signature-marketplace-revenue-gate.md`): the
  consequential action is impossible until an ordered list of evidence refs all
  exist; only the terminal state unlocks it.

---

## The five signatures

Each signature below is stated as: typed inputs → required evidence refs
(the predicate list) → gate/state model → what the terminal state unlocks →
which overnight mistake category it makes **impossible**.

### Signature 1 — `fleet-liveness-dispatch-proof`

Proves the fleet is producing *work*, not merely that a process exists.

**Typed inputs:** `{ supervisorPid, lastHeartbeatAt, lastDispatchAttemptAt,
activeSlotCount, quotaLedgerSnapshot }`

**Required evidence refs (all must exist):**
1. `evidence://supervisor/last-dispatch-attempt` — timestamp written by the
   supervisor itself at the moment it calls `spawnSlot()`.
2. `evidence://supervisor/quota-ledger-snapshot` — a read of every account's
   `~/.pylon/account-quota/<hash>.json`, proving availability was actually
   checked.
3. `evidence://supervisor/heartbeat-payload` — full heartbeat JSON including
   `last_dispatch_time`.

**Gate:** `BLOCKED → DISPATCH_ATTEMPT_SEEN → QUOTA_CHECKED → PROVEN_ALIVE`
- `PROVEN_ALIVE` requires all three refs **and** `lastDispatchAttemptAt` within
  the last 10 minutes. Only `PROVEN_ALIVE` unlocks the watcher reporting
  "fleet healthy."
- If state regresses from `PROVEN_ALIVE`, the watcher **must** emit an alert —
  the "only on change" throttle is overridden by a state regression.

**Makes impossible:** **#1 WEDGE** (heartbeat-alive but dispatch-dead can never
reach `PROVEN_ALIVE`; detected ≤10 min) and **#2 BLIND OVERSIGHT** ("fleet
healthy" cannot be said without the three refs).

### Signature 2 — `diagnosis-grounding`

No root-cause claim reaches the owner without the data behind it.

**Typed inputs:** `{ claimedRootCause, quotaLedgerSnapshot,
supervisorDispatchLog, accountRateLimitHeaders|null }`

**Required evidence refs:**
1. `evidence://diagnosis/quota-ledger-read` — actual contents of
   `~/.pylon/account-quota/` at diagnosis time.
2. `evidence://diagnosis/supervisor-dispatch-log` — last 20 dispatch attempts +
   outcomes.
3. `evidence://diagnosis/provider-rate-limit-headers` — if the claim is
   "rate-limited," the actual 429 `Retry-After` / `X-RateLimit-Reset` headers.

**Gate:** `UNGROUNDED → LEDGER_READ → DISPATCH_LOG_EXAMINED → PROVIDER_VERIFIED
→ GROUNDED` — only `GROUNDED` (all refs exist AND the claim matches the
evidence) unlocks proposing a remediation.

**Makes impossible:** **#3 UNGROUNDED DIAGNOSIS** — "rate-limited" cannot be
claimed without the quota-ledger read and the provider 429 headers.

### Signature 3 — `issue-close-safe`

Closing an issue requires proof it is safe to close — epics protected.

**Typed inputs:** `{ issueNumber, issueLabels, parentEpicNumber|null, prNumber,
prBody }`

**Required evidence refs:**
1. `evidence://issue/labels` — full label set.
2. `evidence://issue/parent-epic-check` — if the issue has a parent EPIC,
   confirmation this is the **last open sub-issue**; otherwise the close is
   blocked.
3. `evidence://pr/body-contains-closes` — PR body contains `Closes #XXXX`
   matching the issue.

**Gate:** `UNCHECKED → LABELS_READ → EPIC_SAFE → CLOSE_VERIFIED →
SAFE_TO_CLOSE`. If the issue is an EPIC sub-issue and not the last open one, the
gate locks at `EPIC_SAFE`. An EPIC itself can only be closed by a separate
manual signature with a higher risk ceiling.

**Makes impossible:** **#4 WRONG STATE TRANSITION** — a single sub-PR can never
auto-close an epic (the #6376 failure, which recurred during the very next
merge pass because this signature was not yet implemented).

### Signature 4 — `command-execution-source-verified`

No command is recommended without reading its source.

**Typed inputs:** `{ commandString, scriptPath, expectedFlags, sourceReadHash }`

**Required evidence refs:**
1. `evidence://command/source-read` — full file contents, read + hashed before
   proposing.
2. `evidence://command/flag-verification` — every flag in the proposed command
   exists in the script's actual argument parser.
3. `evidence://command/runtime-check` — a dry-run / `--help` proving the script
   is executable and accepts the expected flags.

**Gate:** `UNVERIFIED → SOURCE_READ → FLAGS_VERIFIED → RUNTIME_CONFIRMED →
SAFE_TO_PROPOSE` — only `RUNTIME_CONFIRMED` unlocks proposing the command.

**Makes impossible:** **#5 FABRICATED EXECUTABLE** — a stub with no CLI flags
fails at `FLAGS_VERIFIED`; the MirrorCode-style recommendation cannot occur.

### Signature 5 — `merge-deploy-gate`

Merged ≠ deployed; main never left red.

**Typed inputs:** `{ prNumbers, mergeCommitHashes, checkDeployExitCode,
checkDeployStdout, deployExitCode|null, smokeTestResults }`

**Required evidence refs:**
1. `evidence://merge/check-deploy-pass` — full stdout + exit code of
   `bun run check:deploy` against main-after-merge.
2. `evidence://deploy/exit-code` — exit code of
   `env -u CLOUDFLARE_API_TOKEN -u CLOUDFLARE_ACCOUNT_ID bun run deploy:safe`.
3. `evidence://deploy/smoke-tests` — post-deploy smoke results.

**Gate:** `MERGED → CHECK_DEPLOY_GREEN → DEPLOYED → SMOKED → LIVE`. Only `LIVE`
(all refs + all smokes pass) unlocks "deployment live and verified." Any gate
failure → report "main RED — rollback required" and **block all further merges**
until a rollback evidence ref is presented.

**Makes impossible:** **#6 MERGE/DEPLOY DISCIPLINE** — two separate tracked
transitions enforce merged≠deployed; a red check:deploy or failed smoke locks
the gate and blocks subsequent merges. (Note: this gate must verify the *real*
`check:deploy` exit code, not a wrapper's trailing-echo exit — a subtlety that
bit the merge lane and was caught by auditing the actual `EXIT=` lines.)

---

## Composition: the `autonomous-ops-v1` Program

The five signatures compose into one DSPy-style Program with a fixed execution
order. The Program metric is **"zero unforced errors per 24-hour cycle"** — any
gate violation is a failed cycle, and the GEPA optimizer is constrained so the
metric is zero whenever any required evidence ref is missing for the cycle.

```
Cycle Start
  ├── S1 fleet-liveness-dispatch-proof
  │     └── if NOT PROVEN_ALIVE → EMERGENCY: restart supervisor, notify owner
  ├── S2 diagnosis-grounding
  │     └── any anomaly → must reach GROUNDED before proposing a fix
  ├── S3 issue-close-safe        (every merge-orchestrator pass)
  ├── S4 command-execution-source-verified (before every command proposal)
  └── S5 merge-deploy-gate       (every deploy batch)
```

### Overseer evidence packet (required each cycle)

The overseer (Claude-main) must present a structured packet each cycle; the
Program refuses to emit a cycle report until every signature's required refs
exist:

```typescript
{
  cycle, timestamp,
  signatures: {
    "fleet-liveness":   { state, evidenceRefs, lastDispatchDelta },
    "diagnosis-grounding": { state, evidenceRefs, activeDiagnoses },
    "issue-close-safe": { state, evidenceRefs, closedIssues, blockedEpics },
    "command-execution":{ state, evidenceRefs, lastProposedCommand },
    "merge-deploy":     { state, evidenceRefs, mainStatus, lastDeployHash }
  }
}
```

**Skipping a step is structurally impossible:** if the supervisor is wedged and
not writing `last-dispatch-attempt`, S1 stays `BLOCKED`, the cycle stalls, and
intervention is forced — exactly the signal that was missing overnight.

---

## Implementation path

Every signature maps to a real surface; the only new code is wiring evidence at
the right points:

1. **Write evidence refs at the source.** The supervisor writes
   `evidence://supervisor/last-dispatch-attempt` on every `spawnSlot()`; the
   heartbeat payload gains `last_dispatch_time`.
2. **Gate state machines.** A small TS module `apps/pylon/src/blueprint-gates/`
   implements each signature's ordered-predicate state machine.
3. **Register signatures.** Each becomes a `program_signature` contribution in
   the Signature Lookup Service; evidence refs are written through the existing
   Action Submission evidence-only boundary.
4. **Cycle report gating.** The watcher's cycle report requires all five
   signature states before it can be emitted.

GEPA optimization (bounded scheduled runner,
`docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`) tunes the Program
against the "zero unforced errors" metric over time.

### Tracking

Implement under the umbrella of the rate-limit/observability epic
(`#6637`) where the evidence sources overlap (quota ledger, `last_dispatch_time`
heartbeat), plus the after-action action items: [P0] timeout guards on
supervisor spawns, [P1] `last_dispatch_time` in the heartbeat. Each signature
should land as its own scoped PR with its gate state-machine test.

---

*Design authored by Artanis (operator agent), 2026-06-28, at the owner's
direction. Transcribed verbatim-faithful by Claude-main (overseer).*
