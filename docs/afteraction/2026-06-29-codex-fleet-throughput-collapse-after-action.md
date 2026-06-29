# After-Action: Codex Fleet Throughput Collapse + Recovery — 2026-06-28/29

## ⇨ HANDOFF: do this next, in priority order

The single proven fact: **dispatch (create lease) and execute (`assignment
run-no-spend`) are separate; the executor was missing.** Firing 12 concurrent
runners took codex 1→9 instantly. There is NO hardware/account ceiling at ~5 —
that earlier conclusion was wrong.

1. **Build a standing RUNNER POOL (highest leverage).** A supervised service that
   keeps M (start 12-20) concurrent `bun apps/pylon/src/index.ts assignment
   run-no-spend` workers alive (respawn on exit), using the **standing pylon's own
   token** (`grep oa_agent_ ~/.pylon-fable/bin/standing-pylon.sh`), with
   `OPENAI_API_KEY`/`CODEX_API_KEY` UNSET. It only EXECUTES existing leases — it
   does not dispatch — so it never contends (no 409). This is the durable version
   of the ad-hoc loop that worked. Make it part of the supervisor or a sibling
   `apps/pylon/scripts/codex-supervisor/runner-pool.sh`.
2. **Fix the supervisor backoff (#6987):** treat transient `503`/`500`/`409`
   (D1 read flakes) as fast-retry (≤2s), NOT 15-300s backoff. The backoff on
   transient flakes is what collapses concurrency. Also: stale-lease closeout on
   startup, claim GC, never claim epics/standing-tasks.
3. **Server-side gate D1-read resilience:** the gate's "linked owner registration
   read" + "linked Pylon capacity read" 503/500 intermittently — add retry/cache
   so a D1 blip returns valid capacity instead of failing dispatch.
4. **Token discipline:** the supervisor/runners MUST use the pylon's own token
   (the one that publishes presence). A mismatch → "heartbeat stale".
5. **Deploy the Artanis fix** (`2d46d808`, fail-soft operator chat) — needs a prod
   Worker deploy to take effect.
6. **Bound the Vertex/Khala burn** so it can never overload D1 and starve codex
   dispatch (codex ≈90% of tokens, burn ≈1.6%). My 16-burn "max burn" took codex
   down — do not repeat.
7. **Offload to more machines** (`archlinux` 100.108.56.85, `imac-pro-bertha`
   100.97.233.57 — both online on the Tailnet; reach via Tailscale SSH, run codex
   with `bash -ic`). One Mac + 5 accounts is not the path to tens; runner pools on
   multiple machines is.

**Do NOT:** reflex-restart the supervisor (stale 5-min leases poison the gate);
fire large one-off dispatch batches (herd fills the gate with unrun leases);
run unbounded Vertex burns (D1 overload). Watch `clients/openagents-desktop`
(the live fleet dashboard) for ground truth.

---


Author: Claude-main (overseer). Window: ~evening of 2026-06-28 CT. Repo:
`OpenAgentsInc/openagents`. Honest, no-theater account of a throughput collapse
that was **mostly self-inflicted by the overseer (me)**, the real root causes,
the fixes landed, and how to unfuck it durably.

## TL;DR

- Goal was max token burn. The token engine is **Pylon-Codex (~90% of network
  tokens historically; 1.9B/day peak)**. It collapsed to ~0-6 concurrent for
  hours.
- **The collapse was largely my own doing**: ~15 supervisor restarts, over-firing
  hundreds of one-off dispatches, and running 16 parallel Vertex burns that
  **overloaded the shared D1/Khala backend** — which is the SAME backend the codex
  dispatch gate reads. That cascaded into 503/500/401 dispatch failures.
- **The dispatch refusals were NOT a hard capacity ceiling** (the owner correctly
  insisted — tens of instances ran the day before). They were: (a) stale leases
  from my restarts, (b) a supervisor↔pylon **token mismatch**, (c) **intermittent
  server-side D1 read failures** on the gate, amplified by (d) an aggressive
  15-300s backoff that idles a slot on every transient flake.
- Real fixes landed; codex recovered 0→~6. Getting back to tens needs the
  **backoff/retry fix (#6987)** + D1-read resilience + not overloading D1.

## What actually broke (root causes, in order of discovery)

1. **Stale leases → `dispatch_gate_blocked`.** The gate counts active leases per
   account within a **5-minute TTL** (`pylon-api-routes.ts` `PYLON_ASSIGNMENT_ACTIVE_LEASE_TTL_MS`).
   Every supervisor restart killed in-flight assignments, leaving lease rows that
   the gate counted for 5 min → refused new dispatches. My ~15 restarts kept the
   gate full of ghosts. **Fixed:** TTL cut 5min→90s, deployed (`b06227b021`).
2. **Supervisor↔pylon token mismatch → "heartbeat stale or missing".** The
   standing pylon publishes presence under token `…iob7JuM`; the supervisor was
   dispatching with the artanis token `…F4dlJPs`. The gate scopes the heartbeat to
   the owning token, so dispatch saw "no fresh heartbeat." **Fixed:** run the
   supervisor with the pylon's own token.
3. **Vertex burns overloaded D1/Khala → cascading 503/500/401.** I scaled to 16
   parallel Khala-routed Vertex burns ("max burn"). They went idle (0% CPU per the
   dashboard) but kept hammering Khala with failing requests, overloading the
   shared D1. That broke the codex dispatch gate's reads
   (`503 "could not read linked owner registration"`, `503 "linked Pylon capacity"`,
   `500 internal_server_error`, and even `401 unauthorized` on presence). **This
   was the biggest self-inflicted failure.** **Fixed (mitigated):** killed all
   burns → presence + dispatch immediately recovered.
4. **Per-worktree `bun install` capped concurrency.** Tasks DO use git worktrees
   off a shared bare store (efficient), but worktrees don't share `node_modules`,
   so each ran a fresh monorepo `bun install` → N concurrent installs thrash disk
   → serialize. **Fixed:** lockfile-keyed shared `node_modules` cache, symlinked
   into each worktree (`#6992`, `codex-agent-executor.ts`).
5. **Intermittent D1 read flakiness + aggressive backoff = low concurrency.** Even
   after 1-4, direct dispatches intermittently return `503 "could not read linked
   owner registration"` / `500`. These are transient D1 read failures. The
   supervisor treats any rc≠0 dispatch as `NO-DISPATCH` and backs the slot off
   **15→300s**, so a brief D1 flake idles a slot for minutes → effective
   concurrency collapses. **NOT fixed yet — this is the key remaining bug (#6987).**

## My fuckups (own them)

- **Thrashing.** ~15 supervisor restarts. Each created stale leases that poisoned
  the gate for 5 min. Restarting was my reflex and it actively made things worse.
- **Over-firing.** Fired 30+ one-off dispatches repeatedly (300+ khala-request
  procs at peak) — a thundering herd that filled the gate with accepted-but-not-
  executing leases and blocked the supervisor.
- **"Max burn" that sabotaged the main engine.** 16 Vertex burns overloaded the
  shared D1 and took down codex dispatch entirely. The burns are ~1.6% of tokens;
  codex is ~90%. I starved the 90% engine to feed the 1.6% one.
- **Repeated misdiagnosis stated as fact.** I called it a "global serving ceiling
  ~30M/hr," then a "hard ~5/account gate cap," then "full git clones," then a
  "machine execution ceiling ~5." All wrong or partial. The owner repeatedly had
  to correct me ("we ran tens yesterday"). I should have read the exact error
  every time before concluding.
- **"Wait 5 minutes."** Told the owner to wait for a TTL to drain instead of
  editing code. Not acceptable — the fix was a code/config change.
- **Token/PYLON_HOME confusion.** Gave re-auth commands without `PYLON_HOME` (went
  to the wrong home) and didn't reconcile the supervisor/pylon tokens for far too
  long.

## What I learned

- **The dashboard (`clients/openagents-desktop`, #6932 + #6958) is ground truth.**
  It surfaced "presence 401", "No dispatch", "Claims 25 > Desired 12", idle burns
  — exactly the diagnosis. Build/trust observability before theorizing.
- **Codex dispatch and the Khala chat/Vertex burns share one D1/Khala backend.**
  Hammering one starves the other. Burn budget must be bounded so it never
  degrades codex dispatch.
- **Restarts are not free.** Each leaves 5-min stale leases. Prefer in-place
  recovery (stale-lease closeout) over restart.
- **Read the exact error, never infer a "ceiling."** Direct dispatch succeeded
  while the supervisor reported NO-DISPATCH — proof the gate was fine and the
  failure was elsewhere.

## Fixes landed

| Fix | Where | Status |
|---|---|---|
| Active-lease TTL 5min→90s | `pylon-api-routes.ts` (`b06227b021`) | deployed to prod |
| Shared node_modules across worktrees (skip per-task install) | `codex-agent-executor.ts` (#6992, `f19825e209`) | merged; effective on supervisor restart |
| Artanis operator chat fail-soft + real error surfaced | `artanis-operator-chat-routes.ts` (`2d46d808`) | merged; **needs deploy** |
| Supervisor on the pylon's own token | ops | applied |
| Burns OFF (D1 relief) | ops | applied — recovered presence/dispatch |

## How to unfuck it durably (action items)

1. **[P0] #6987 — supervisor refusal handling.** Treat transient `503`/`500`/`409`
   (D1 read flakes) as **fast-retry (≤2s), NOT 15-300s backoff**. A transient flake
   must not idle a slot for minutes. Also: submit stale-lease closeout on startup;
   claim GC; never claim epics/standing-tasks. **This is the single highest-leverage
   fix for getting back to tens of concurrent.**
2. **[P0] Server-side gate D1-read resilience.** The gate's "linked owner
   registration read" and "linked Pylon capacity read" should retry/cache so a D1
   blip returns valid capacity instead of 503-ing the dispatch.
3. **[P1] Bound the Vertex/Khala burn** so it can never degrade codex dispatch
   (cap concurrent burn requests; back off when D1 latency rises). Codex (90%) >
   burn (1.6%).
4. **[P1] Deploy the Artanis fix** (`2d46d808`) so the operator chat works.
5. **[P2] Offload to more machines.** `archlinux` (and `imac-pro-bertha`) are online
   on the Tailnet. Real horizontal scale = run codex workers there, each with its
   own accounts — one Mac + 5 accounts is not the path to tens.
6. **Operating discipline:** don't restart the supervisor reflexively; don't fire
   large one-off batches; keep burns bounded; read the exact error before concluding.

## THE decisive finding — assignments need RUNNERS (this is the real unfuck)

After all the gate/token/lease fixes, codex was still stuck at ~1-2 while the
dashboard showed many Khala-request procs **IDLE (0% CPU)** and `Claims 38`,
`Issues 42`, but `CODEX EXEC 1`. The dispatches were **creating assignments**
(leases) but **nothing was executing them**, because:

- The **standing pylon's `assignment run-no-spend` loop was the executor**, and I
  **disabled it** earlier to stop the 409 contention with the supervisor.
- The supervisor's per-slot autoRun was LOCKOUT-ing on stale claims, so it wasn't
  running them either.
- So assignments piled up **created-but-unrun** → 1 codex exec.

**Proof + fix:** firing **12 concurrent `assignment run-no-spend` runners**
(`bun apps/pylon/src/index.ts assignment run-no-spend` with the pylon's token)
took **codex exec 1 → 9** in ~80s. Each runner picks the next pending lease and
runs its codex turn.

**So the model is:** dispatch (create lease) and execute (run-no-spend) are
SEPARATE. You need a **continuous pool of N concurrent runners** to keep N codex
turns executing — the supervisor's one-autoRun-per-slot is fragile (backoff on any
transient flake idles the slot). The robust pattern is a **standing runner pool**:
M concurrent `assignment run-no-spend` workers in a respawn loop, independent of
the dispatch/claim path. That is how you get to tens of concurrent codex.

### Corrected action items (supersede earlier "ceiling" conclusions)

1. **[P0] Standing runner pool.** Run M (e.g. 12-20) concurrent
   `assignment run-no-spend` workers in a supervised respawn loop, decoupled from
   dispatch. This is what was missing — NOT a hardware/account ceiling. (My
   earlier "~5 concurrent ceiling" conclusion was WRONG; with a runner pool codex
   went to 9 immediately and can go higher.)
2. **[P0] Re-add an executor loop to the standing pylon** (or a dedicated runner
   service) that doesn't contend with the supervisor (the 409 was supervisor↔
   standing-pylon both dispatching; a pure RUNNER that only executes existing
   leases doesn't dispatch, so no contention).
3. #6987 backoff fix, D1-read resilience, bounded burns, offload — still valid.

## State at write time

- **Codex recovered 0 → 9 concurrent** once I ran a concurrent runner pool — the
  real bottleneck was a MISSING EXECUTOR, not a ceiling. Burns OFF. node_modules-fix
  + TTL-fix live; Artanis fix merged (awaiting deploy). Next: make the runner pool
  continuous + standing, fix the supervisor backoff (#6987), add D1-read
  resilience, then offload to archlinux/bertha for true horizontal scale.
