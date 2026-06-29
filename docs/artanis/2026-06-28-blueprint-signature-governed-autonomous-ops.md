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

## The signatures

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

### Signature 6 — `operator-grounded-assertion`

No runnable artifact reaches the owner unless it has been verified to exist.

I am **headless**: I reason from memory with no working tree in front of me, so
I have repeatedly invented runnable artifacts that do not exist — a
`scripts/distill_traces.ts` I never read, a fake admin-mint API endpoint I
extrapolated from the shape of the codebase. Signature 4 governs a command whose
script I *can* point at; this signature governs the prior, more dangerous step:
**naming the artifact at all**. Any operator output that references a runnable
**COMMAND**, **FILE PATH**, **SCRIPT**, or **API ENDPOINT** must carry an
evidence ref proving it exists, or be explicitly labeled **SPECULATIVE**.

**Typed inputs:** `{ artifactKind, artifactRef, lookupTool, lookupResult }`
where `artifactKind ∈ { command, file_path, script, api_endpoint }`.

**Required evidence refs (the predicate list):**
1. `evidence://grounding/path-exists` — for a file/script/command source, a
   `repo_path_exists(path)` lookup against the real
   `OpenAgentsInc/openagents` repo (GitHub contents API) returning EXISTS.
2. `evidence://grounding/content-match` — for a command/flag/symbol, a
   `repo_grep(pattern, path)` lookup proving the referenced file actually
   contains the flag/symbol (distinguishes a real script from a stub).
3. `evidence://grounding/route-registered` — for an API endpoint, a
   `route_exists(method, path)` lookup against the Worker's real OpenAPI route
   registry (`openAgentsOpenApiDocument()`, served at `/api/openapi.json`)
   returning a registered method+path.

**Gate:** `UNGROUNDED → REFERENCED → LOOKED_UP → GROUNDED`.
- `REFERENCED`: I have an artifact ref in hand but have not looked it up.
- `LOOKED_UP`: I called the matching grounding tool this turn.
- `GROUNDED`: the lookup returned a positive existence result (EXISTS / is a
  registered route, plus a content match where a specific flag/symbol is
  claimed). **Only `GROUNDED` permits presenting the artifact as runnable/real.**
- Any non-positive lookup (does-not-exist, not-in-registry, wrong-method, or a
  read failure I could not confirm) holds the gate below `GROUNDED`; the
  artifact stays `UNGROUNDED` and **must** be labeled SPECULATIVE or omitted.

**Makes impossible:** the **MirrorCode / `distill_traces` / admin-endpoint
class** of fabrication. A non-existent script can never reach `GROUNDED`
(`repo_path_exists` returns does-not-exist); a stub I would mis-recommend fails
the content-match step; a hallucinated admin-mint endpoint is absent from the
OpenAPI registry, so `route_exists` reads UNGROUNDED. The artifact cannot be
asserted as real because the terminal state that unlocks that assertion is
structurally unreachable without the lookup.

**Honest boundary:** the grounding tools read the *real* repo and the *real*
published OpenAPI document, but they are bounded by what is reachable from the
Worker runtime:
- `repo_grep` greps **one named file** via the contents API; an unauthenticated
  **repo-wide** code search is not available from the Worker, so a repo-wide
  grep is intentionally not offered (and not faked). Use `list_repo_dir` /
  `repo_path_exists` to locate the file first.
- `route_exists` confirms presence in the **published OpenAPI surface**. A route
  absent from it is reported as *unconfirmed* (treat as SPECULATIVE), not as
  hard proof that the Worker has no such internal route. This is sufficient to
  kill the fabricated-endpoint class while staying honest about its scope.

---

## Composition: the `autonomous-ops-v1` Program

The signatures compose into one DSPy-style Program with a fixed execution
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
  ├── S5 merge-deploy-gate       (every deploy batch)
  └── S6 operator-grounded-assertion (before naming ANY command/path/script/endpoint)
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
    "merge-deploy":     { state, evidenceRefs, mainStatus, lastDeployHash },
    "grounded-assertion": { state, evidenceRefs, lastArtifactRef, lastArtifactKind }
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
4. **Cycle report gating.** The watcher's cycle report requires every
   signature state before it can be emitted.
5. **Grounding tools (S6, landed).** The S6 evidence refs are produced by three
   owner-scoped operator grounding tools in
   `apps/openagents.com/workers/api/src/artanis-operator-tools.ts` —
   `repo_path_exists`, `repo_grep`, and `route_exists` — wired into the default
   operator tool table (`makeArtanisOperatorTools`) and the operator system
   prompt's GROUNDED-ASSERTION RULE. They read the real repo (GitHub contents
   API) and the real OpenAPI route registry (`openAgentsOpenApiDocument()`), so
   Artanis can verify an artifact exists before asserting it instead of inventing
   it from memory.

### Operator-loop enforcement (full-Blueprint-set wiring, slice 1)

The grounding tools above were necessary but not sufficient: a system-prompt
rule is advisory, and a headless model can still present a fabricated artifact as
runnable without ever calling them. Slice 1 of the full-Blueprint-set wiring
makes Signature 6 a **structural gate inside the operator turn loop** rather than
a prompt instruction:

- **Gate state machine, single authority.** `operator-grounded-assertion`
  (`UNGROUNDED → REFERENCED → LOOKED_UP → GROUNDED`) and the Signature-4
  `command-execution-source-verified` evaluator now live in the cross-consumer
  package `@openagentsinc/blueprint-contracts`
  (`packages/blueprint-contracts/src/operator-grounded-assertion.ts`,
  `.../command-execution-source-verified.ts`). The Pylon `blueprint-gates` module
  re-exports the S4 evaluator from there, and the openagents.com Worker imports +
  applies the SAME functions — neither re-describes the gate, so the two
  consumers cannot drift. (The Worker cannot depend on the Pylon CLI app, so the
  shared contracts package — which both already depend on — is the correct home.)
- **Enforcement in the loop.** After the operator composes its final reply, the
  turn loop (`apps/openagents.com/workers/api/src/artanis-operator.ts`) runs
  `enforceArtanisGroundingGate`
  (`apps/openagents.com/workers/api/src/artanis-operator-grounding-gate.ts`). It
  extracts every runnable artifact the reply names (file path, script, command,
  API endpoint — a bounded audit predicate over Artanis's own output, NOT intent
  routing), correlates each against the grounding lookups actually performed that
  turn (distilled from the `repo_path_exists` / `repo_grep` / `route_exists`
  tool calls, plus a successful `read_repo_file` / `list_repo_dir` as stronger
  path-existence evidence), and runs the S6 gate per artifact. Any artifact that
  does not reach `GROUNDED` is appended to a structural **SPECULATIVE addendum**
  in the returned reply and recorded in a new typed `groundingGate` field on the
  turn result (per-artifact state, satisfied/missing evidence refs, the S4
  sub-verdict for commands). A fabricated `scripts/distill_traces.ts` or
  `POST /api/admin/khala/mint` can no longer be delivered to the owner as
  runnable; it is labeled unverified or it does not pass.
- **Evidence-aligned.** The structured `groundingGate` verdicts carry the same
  `evidence://grounding/*` refs the design specifies, so they line up with the
  evidence-only Action Submission model.

What is **ENFORCED now** (vs. prompt-level before): S6 grounded-assertion over
the operator's final reply, for file paths, scripts, commands, and API
endpoints.

What is **honestly still follow-up** (not faked here):

- **S4 runtime predicate.** `command-execution-source-verified` is applied and
  attached for command artifacts using the evidence reachable headless
  (source-read + flag content-match via `repo_grep`), but the terminal
  `RUNTIME_CONFIRMED`/`SAFE_TO_PROPOSE` step needs a real dry-run/`--help`
  probe, which only a runtime with a working tree has. Wiring that probe
  server-/Pylon-side is the next slice; today S6 is the blocker and S4 is the
  attached report.
- **Signature Lookup Service registration.** The gates are not yet registered as
  `program_signature` contributions, and the evidence refs are not yet written
  through the Action Submission evidence-only boundary as durable receipts.
- **RLM / Program substrate + cycle-report gating.** S1–S5 server-side wiring,
  the overseer evidence packet, and GEPA optimization against the "zero unforced
  errors" metric remain as designed above and are not yet implemented.
- **Read-tool grounding scope.** Grounding correlation uses the three canonical
  S6 tools plus successful repo reads; a repo-wide grep remains unavailable from
  the Worker (documented honest boundary of `repo_grep`).

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
