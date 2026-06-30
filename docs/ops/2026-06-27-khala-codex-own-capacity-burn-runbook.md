# Khala → Pylon → Codex Own-Capacity Token Burn — Operations Runbook

**Date:** 2026-06-27
**Scope:** Keeping the own-capacity Codex coding-delegation engine lit at scale,
and diagnosing it when it stalls.
**Audience:** Operators driving the daily Khala token target.

This is the *operational* companion to the canonical
**"Khala -> Pylon -> Codex Coding Delegation Runbook"** in
[`CLAUDE.md`](../../CLAUDE.md) (the protocol/proof source of truth) and the
[`2026-06-26` delegation after-action](../afteraction/2026-06-26-khala-pylon-codex-delegation-afteraction.md).
Read the CLAUDE.md section for the request/proof contract; read this file for how
to run the engine 24/7, scale it, and debug a stall. The deeper E2E smoke doc is
`docs/khala/2026-06-25-bare-agent-pylon-mcp-khala-e2e-smoke.md`; the invariant
ledger is `apps/openagents.com/INVARIANTS.md` ("Khala Coding Delegation Through
Pylons").

> Hard-won knowledge from a long debugging session is captured in **§5 Known
> gaps & gotchas** and the **§7 diagnosis decision-tree**. Skim those first when
> something is broken.

---

## 1. Purpose & the daily token goal

The owner mandate is a large daily Khala token target (≈ **4× the prior day**).
The dominant lever for hitting it is **own-capacity Codex coding work** routed
Khala → Pylon → Codex:

- provider `pylon-codex-own-capacity`
- model `openagents/pylon-codex`
- `demand_kind = own_capacity`, `demand_source = khala_coding_delegation`
- counted in `token_usage_events`, projected to the public counter.

This lane already dominates the public mix. A live read of
`/api/public/khala-tokens-served/model-mix` on 2026-06-27 showed the
`pylon_codex` family at ~61% of the 30-day token total — far ahead of every
model-routed family. **No paid API, no spend, own-capacity only.** This runbook
is how to keep that engine lit and diagnose it when it stalls.

---

## 2. Architecture (three moving parts)

### 2a. Standing pylon — `~/.pylon-fable/bin/standing-pylon.sh`

A launchd job (`KeepAlive`) that keeps the owner Pylon **online and executing
leased assignments**. It loops every 60s.

- **launchd label: `com.openagents.pylon.fable`** (verified:
  `launchctl list | grep openagents` → `com.openagents.pylon.fable`; its plist
  `ProgramArguments` is `/usr/bin/caffeinate -s …/.pylon-fable/bin/standing-pylon.sh`).
  - *Correction to prior notes:* the separate job `com.openagents.pylon-node`
    is **not** the standing pylon — it runs
    `apps/pylon/scripts/run-discovery-node.sh` (the discovery node), a different
    role. There is no `com.openagents.pylon.pylon-node` label. Always confirm
    the live label with `launchctl list | grep openagents`.
- **Critical gotcha — go-online, not bare heartbeat.** Each cycle it runs
  `bun src/index.ts provider go-online` (from `apps/pylon`), **not** bare
  `presence heartbeat`. On older Pylon code a bare heartbeat publishes
  `codex available=0`, so every coding request 409s. (With the §4 gate-fix
  deployed, a current-code heartbeat now also refreshes capability — but the
  standing loop still uses `go-online` so it works regardless.)
- **Advertises N codex slots** via env exported in the script:
  `OPENAGENTS_PYLON_CODEX_CONCURRENCY=8`, `OPENAGENTS_PYLON_CODEX_BUSY=0`,
  `OPENAGENTS_PYLON_CODEX_QUEUED=0`.
- **Executes** leased assignments each cycle with
  `bun src/index.ts assignment run-no-spend`. (The standing pylon only
  *executes* — it does not *fire* `khala request`. See §3 on why.)
- **Auth:** an agent token (the "fable" token, prefix `oa_agent_Ut5…`) exported
  in the script as `OPENAGENTS_AGENT_TOKEN`. This token can heartbeat +
  `run-no-spend`, but **403s when firing** `khala request` ("requested Pylon is
  not linked to this OpenAuth account"). See §3.

Loop body (verified from the live script):

```sh
while true; do
  bun src/index.ts provider go-online        >> standing.log 2>&1
  bun src/index.ts assignment run-no-spend   >> standing.log 2>&1
  sleep 60
done
```

Log: `~/.pylon-fable/standing.log`. Pylon home: `~/.pylon-fable`.

### 2b. Codex supervisor — `apps/pylon/scripts/codex-supervisor/{launch.sh,codex-supervisor.sh}`

The thing that actually **fires** requests and auto-scales a fire-and-run pool.

The supervisor scripts are tracked in this checkout. Confirm with
`git ls-files apps/pylon/scripts/codex-supervisor/` before launching from a new
checkout, then run from clean current `origin/main`.

Behavior (verified from the scripts):

- **Auto-scaling:** target `desired = min(SUP_MAX_SLOTS, ready_codex_accounts ×
  SUP_PER_ACCOUNT)`, recomputed from `codex accounts list --json` (ready
  accounts only).
- **Heartbeater** republishes `presence heartbeat --json` with
  `OPENAGENTS_PYLON_CODEX_CONCURRENCY=<desired>` (busy/queued=0) every
  `SUP_HEARTBEAT_SECS` (default **45s**) so presence never goes stale (#6354) and
  the gate sees current availability. One `provider go-online` runs at startup.
- **`SUP_MAX_SLOTS` worker loops** each continuously fire
  `khala request --workflow codex_agent_task …` (which auto-runs the matching
  local `assignment run-no-spend` to closeout), round-robin across ready accounts
  via `--account-ref`, pinned to a rotated public backlog issue + current
  `origin/main` HEAD. On 409/`target_pylon_unavailable` → `refused` backoff; on
  429/rate-limit → `rate_limited` backoff. Exponential backoff
  `SUP_BACKOFF_MIN`→`SUP_BACKOFF_MAX` (**15s → 300s**). On a finished session the
  worker immediately fires the next → continuous refill.
- **Owner-session tripwire:** if a run output contains "access token could not be
  refreshed / please sign in again / reauthenticate" (i.e. `~/.codex` broke), it
  **GLOBAL-PAUSEs** (touches `~/.codex-supervisor/paused`) and appends a
  `NEEDS-OWNER` note to `NEEDS_OWNER.md`. It **never** runs `codex login`.
- **Self-throttling:** because refusals back off, the pool settles at the login's
  true headroom instead of rate-limit-fighting. Do **not** run it as a second
  driver against a login another runner is already saturating.

Verified defaults (overridable via env), from `codex-supervisor.sh`:

| Var | Default | Meaning |
|---|---|---|
| `PYLON_HOME` | `$HOME/.pylon` | Pylon home (registered owner home). |
| `SUP_PYLON_REF` | `pylon.33afd48282a649047e3a` | **STALE default — override it.** |
| `SUP_PER_ACCOUNT` | `2` | Same-account parallel sessions per ready login. |
| `SUP_MAX_SLOTS` | `8` | Hard ceiling on total concurrent sessions. |
| `SUP_HEARTBEAT_SECS` | `45` | Heartbeat cadence. |
| `SUP_BACKOFF_MIN` / `_MAX` | `15` / `300` | Backoff bounds (s). |
| `SUP_REPO` | `OpenAgentsInc/openagents` | Repo for real work. |
| `SUP_ISSUES` | `6310 6311 6320 6354 6355 6358` | Rotated public backlog. |
| `SUP_VERIFY` | a named worker test | Verification per throwaway workspace. |

> **Critical: `SUP_PYLON_REF` defaults to the stale `pylon.33afd48282a649047e3a`
> — you MUST override it to the current pylon ref.** A previously-used live ref
> was `pylon.a1469b9cdf6965a57530`, but pylon refs drift; **always fetch the live
> one at launch** with `provider go-online --json` (read `codingCapacity` /
> `pylonRef`) rather than trusting any hardcoded value.

**Launch (run from a clean worktree on current `origin/main`** so its
`presence heartbeat` carries the §4 gate-fix capability behavior):

```sh
# 1. Get the live pylon ref first (do NOT trust the stale default):
PYLON_HOME=$HOME/.pylon-fable \
OPENAGENTS_AGENT_TOKEN=<owner-linked token> \
  bun apps/pylon/src/index.ts provider go-online --json
# -> read the live pylonRef / codingCapacity from the JSON.

# 2. Launch the supervisor with that ref:
PYLON_HOME=$HOME/.pylon-fable \
SUP_PYLON_REF=<live-pylon-ref> \
SUP_MAX_SLOTS=8 \
SUP_PER_ACCOUNT=2 \
OPENAGENTS_AGENT_TOKEN=<owner-linked token> \
  bash apps/pylon/scripts/codex-supervisor/launch.sh start

bash apps/pylon/scripts/codex-supervisor/launch.sh status   # pid + last 30 log lines
bash apps/pylon/scripts/codex-supervisor/launch.sh stop      # TERM the supervisor
```

- `OPENAGENTS_AGENT_TOKEN` here must be an **owner-linked** token (see §3) — the
  Artanis token, **not** the fable token (the supervisor *fires* requests).
- State dir: `~/.codex-supervisor/` (pidfile `supervisor.pid`, `paused`,
  `desired-slots`). Log: **`~/.codex-supervisor/supervisor.log`**.
- `launch.sh` runs the supervisor under `nohup caffeinate -i` and is idempotent
  (won't start a second one).

### 2c. The Worker dispatch gate (server-side admission)

In `apps/openagents.com/workers/api/src/`:

- `inference/coding-workflow-delegation.ts` — `hasAvailableCodexCapacity()`
  (line ~299) requires the registration to advertise the Codex capability **and**
  have a coding-capacity projection with `service === 'codex' && available > 0`.
  Candidates are also filtered to `status === 'active'` and a **fresh online
  heartbeat** (≤ 5 min old, status ∈ `available|healthy|idle|online|ready`).
- `pylon-api-routes.ts` — the controlled assignment dispatch gate.

It admits at most the heartbeat-advertised available Codex slots for the
**caller-owned** pylon. Typed refusals it emits (verified):

| HTTP | error / evidence ref | meaning |
|---|---|---|
| 403 | `evidence.khala_coding.target_pylon_ref.not_linked` | token doesn't own/link the pylon (the fable-token-firing 403). |
| 409 | `target_pylon_unavailable` / `…target_pylon_ref.unavailable` | "not active, heartbeat-fresh, Codex-capable, and available." |
| 409 | `…target_pylon_ref.dispatch_gate_blocked` (+ controlled-gate blocker refs) | pylon available but the controlled gate refused the lease. |

The granular admission blocker refs (`registration_not_active`,
`capability_not_advertised`, `no_heartbeat`, `heartbeat_not_healthy`,
`heartbeat_stale`, `no_serving_lane_advert`, `wallet_not_ready`,
`no_spark_payout_target`) live in
`inference/khala-pylon-admission.ts` (the serving-lane admission gate).

> *Correction:* the conceptual sub-condition names from the gate-fix lane
> (`not_active` / `stale_or_missing_heartbeat` / `not_codex_capable` /
> `no_available_codex_capacity`) are **not** the literal strings in the coding
> delegation gate. The actual surfaced reason for a refused codex lease is the
> 409 `target_pylon_unavailable` text above; the granular typed refs are in the
> admission module listed above. Use those exact strings when grepping logs.

---

## 3. Identity & tokens (the #1 footgun)

- **Firing `khala request` requires an OWNER-LINKED agent token** — one whose
  credential resolves to the owner's OpenAuth account / admin email. We use the
  **Artanis** token (prefix `oa_agent_yCqh…`,
  `~/work/.secrets/openagents-artanis-agent.env`), which was owner-promoted.
- **The standing-pylon "fable" token (prefix `oa_agent_Ut5…`)** can heartbeat +
  `run-no-spend`, but **403s when firing**:
  > "The requested Pylon is not linked to this OpenAuth account and cannot be
  > used for caller-owned Khala coding capacity."

  This is the verified 403 `…target_pylon_ref.not_linked` path. So: the standing
  pylon executes with the fable token; the supervisor fires with the Artanis
  token.

- **Never print agent tokens** into tracked files, commits, issue comments, or
  normal terminal output. Read them from `.secrets/` env files
  (`source ~/work/.secrets/openagents-artanis-agent.env`).

### Codex accounts

```sh
# Add an isolated, per-account Codex login (device login). NEVER ~/.codex.
PYLON_HOME=$HOME/.pylon-fable pylon auth codex --account codex-N

# Verify ready accounts:
PYLON_HOME=$HOME/.pylon-fable pylon codex accounts list --json
# -> expect state: ready and capability.pylon.local_codex per account.

# Inspect a plan / usage (consumes a minimal provider call):
PYLON_HOME=$HOME/.pylon-fable pylon accounts usage --account codex-N --refresh --json
```

- Each account gets an **isolated home** under
  `~/.pylon-fable/accounts/codex/<ref>`. **NEVER run `codex login` /
  `pylon auth codex` against the default `~/.codex` home** — `codex login` clears
  `~/.codex/auth.json` at flow-start and wipes the owner's live session (§9).
- **Distinct ChatGPT accounts = distinct rate budgets** = real added throughput.
  Reusing the same underlying account (and possibly `chris+alias@` aliases) may
  share **one** rate budget — distinct accounts are what actually scale (§8).

---

## 4. The dispatch-gate bug we fixed (codex linked-after-register now works)

Two bugs made a genuinely codex-available pylon 409 ("not Codex-capable /
available"):

1. **Heartbeat didn't refresh capabilities.** The heartbeat schema had no
   `capabilityRefs` field, so the server never refreshed registration
   capabilities from the heartbeat — and `provider go-online` is purely local,
   never re-registers. A Codex account **linked after the initial register** was
   invisible server-side.
2. **Pylon client stripped the probed capability.** `loadOrCreateRuntimeState`
   in `apps/pylon/src/state.ts` (function at line ~204) overwrote, instead of
   unioned, the probed `local_codex` capability, so every heartbeat republished
   `codex available=0`.

**Fix + deploy (verified commit hashes / issue):**

- `982c33f521` — `fix(khala): heartbeat refreshes Pylon capability refs so
  just-linked Codex dispatches (#6354)` (server).
- `1cc0e9ba03` — `fix(pylon): runtime state load must not strip
  dynamically-probed codex/claude capability (#6354)` (Pylon).

Both reference issue **#6354**. The 409 is now a typed refusal naming the failed
sub-condition (see §2c table).

> **Could not verify:** the specific deploy Worker version `68da222b` cited in
> the gate-fix lane. The recorded after-action for the adjacent #6358
> counter-health deploy lists Worker version
> `95d3fcee-f740-477d-b3c4-368f198e8255`. Treat the exact Worker version as
> unverified here; confirm via the live deploy log / `deploy:safe` output.

---

## 5. Known gaps & gotchas

- **Bare `presence heartbeat` doesn't advertise codex on older code** → use
  `provider go-online` (standing pylon) or current-code heartbeat (post-#6354).
- **`presence heartbeat` can hang (not exit).** A runtime handle can keep the
  process alive after a successful heartbeat. The standing loop uses
  `go-online`; any heartbeat-based loop must background + `timeout` it. **A
  wedged heartbeat process stalls the whole loop** — we found one wedged ~30h.
- **The dispatch gate is per account when per-account refs are advertised.**
  Current Worker delegation picks an advertised account slot for unpinned
  requests and persists `codingAssignment.codex.accountRefHash`, so one hot
  Codex account does not consume another account's slots on the same Pylon.
  Heartbeats without per-account refs still use the legacy pooled accounting.
- **Over-spawning requesters thrashes.** Firing more concurrent requesters than
  advertised slots just 409-thrashes. Right-size requesters to advertised
  concurrency. The supervisor self-throttles via backoff; **manual loops do
  not** — don't hand-roll a second firing loop against a saturated login.
- **Cloudflare edge blocks default urllib UA.** `Python-urllib/*` (the default
  UA) is hard-blocked at the edge (HTTP 403 / Cloudflare error 1010) for
  `/api/v1/*`. httpx / curl / node / browser are fine. Any urllib-default-UA
  client *looks* like "fleet down" when the fleet is healthy. The full WAF
  carve-out for `/api/v1/*` is **owner-gated** (needs a Zone·WAF·Edit token);
  script: `~/work/scripts/cloudflare-unblock-api-v1.sh`, steps in
  `~/work/NEEDS_OWNER.md`. **Interim fix:** set any non-urllib User-Agent on the
  client.

---

## 6. Procedures

### Add a Codex account
```sh
PYLON_HOME=$HOME/.pylon-fable pylon auth codex --account codex-N   # device login; isolated home
PYLON_HOME=$HOME/.pylon-fable pylon codex accounts list --json      # confirm state: ready
```

### Verify accounts
```sh
PYLON_HOME=$HOME/.pylon-fable pylon codex accounts list --json
# each codex account: readiness.state == "ready", capability.pylon.local_codex present
```

### Launch / stop / status the supervisor
```sh
# (fetch live SUP_PYLON_REF first — see §2b)
PYLON_HOME=$HOME/.pylon-fable SUP_PYLON_REF=<live-ref> SUP_MAX_SLOTS=8 SUP_PER_ACCOUNT=2 \
OPENAGENTS_AGENT_TOKEN=<artanis token> \
  bash apps/pylon/scripts/codex-supervisor/launch.sh start
bash apps/pylon/scripts/codex-supervisor/launch.sh status
bash apps/pylon/scripts/codex-supervisor/launch.sh stop
```

### Offload Codex profiles to Tailnet machines (#6432)

Use this when the owner's desktop is CPU-oversubscribed but the Codex profiles
are already authenticated under `~/.pylon-fable/accounts/codex/<ref>`. The
offload path **copies serialized isolated Pylon account homes only**; it never
runs `codex login`, never runs `pylon auth codex`, and never touches the default
`~/.codex`.

Dry-run first from a clean checkout:

```sh
PYLON_HOME=$HOME/.pylon-fable \
SUP_PYLON_REF=<live-ref> \
  bash apps/pylon/scripts/codex-supervisor/fleet-offload.sh \
    --hosts imac-pro-bertha,macbook-pro-m2 \
    --accounts codex-4,codex-5,codex-6,codex-7
```

Expected dry-run shape:

- `imac-pro-bertha` receives `codex-4,codex-6`.
- `macbook-pro-m2` receives `codex-5,codex-7`.
- Each profile is archived from
  `$PYLON_HOME/accounts/codex/<ref>`, copied with `scp`, expanded under the
  remote `~/.pylon-fable/accounts/codex/<ref>`, and the remote supervisor launch
  includes `SUP_ACCOUNT_REFS=<only-that-hosts-refs>`.

Execute only after the dry-run command is sane and both Tailnet hosts have a
clean current checkout, Bun dependencies, and a remote env file exporting
`OPENAGENTS_AGENT_TOKEN` at `~/.pylon-fable/openagents-agent.env` (override with
`REMOTE_AGENT_ENV`). Do not put the token into the dry-run output, issue
comments, or shell history.

```sh
PYLON_HOME=$HOME/.pylon-fable \
SUP_PYLON_REF=<live-ref> \
  bash apps/pylon/scripts/codex-supervisor/fleet-offload.sh \
    --hosts imac-pro-bertha,macbook-pro-m2 \
    --accounts codex-4,codex-5,codex-6,codex-7 \
    --execute
```

The launched remote supervisors use:

- `REMOTE_PYLON_HOME=~/.pylon-fable` by default.
- `REMOTE_REPO=~/work/openagents` by default.
- `REMOTE_AGENT_ENV=~/.pylon-fable/openagents-agent.env` by default.
- `SUP_MAX_SLOTS_PER_HOST=4` and `SUP_PER_ACCOUNT=2` by default.
- `SUP_ACCOUNT_REFS` so bertha and m2 do not both schedule the same copied
  profiles.

After offload, reduce the desktop supervisor/standing-pylon advertised slots by
the moved accounts before declaring a 12-turn split across three machines. The
gate admits by pylon-advertised capacity, so the sum of desktop + bertha + m2
advertisements must match real CPU/account headroom.

### Restart the standing pylon
```sh
launchctl kickstart -k gui/$(id -u)/com.openagents.pylon.fable   # graceful kick
# or: pkill -f standing-pylon.sh   # KeepAlive respawns it
tail -n 40 ~/.pylon-fable/standing.log
```

### Change advertised concurrency
- **Standing pylon:** edit `OPENAGENTS_PYLON_CODEX_CONCURRENCY=N` in
  `~/.pylon-fable/bin/standing-pylon.sh`, then `launchctl kickstart -k …`.
- **Supervisor:** relaunch with a different `SUP_MAX_SLOTS` / `SUP_PER_ACCOUNT`
  (the heartbeater recomputes `desired` and republishes each cycle).

---

## 7. Monitoring & diagnosing a stall

### Burn rate
```sh
# tokens post at TURN CLOSEOUT, so the counter steps in bursts.
# Short samples can read 0 between turns — sample over >= 60s.
curl -fsS https://openagents.com/api/public/khala-tokens-served      # tokensServed (instant)
curl -fsS https://openagents.com/api/public/khala-tokens-served/history      # per-day, America/Chicago
curl -fsS https://openagents.com/api/public/khala-tokens-served/model-mix    # by family (watch pylon_codex)
```

All operational "today" totals use the America/Chicago calendar day. Do not
compare the public `/stats` page against ad hoc queries with a different day
boundary; the page, public history endpoint default, Artanis pace block,
operator fleet state, and `khala apm` all share the America/Chicago boundary.

### Live APM, including active sessions

Use the Pylon CLI when the dashboard number looks flat but Codex sessions are
still running. This endpoint combines completed `token_usage_events` with active
assignment progress and falls back to raw Codex event chunk byte length when the
run has not posted exact usage yet:

```sh
PYLON_OPENAGENTS_BASE_URL=https://openagents.com \
  bun apps/pylon/src/index.ts khala apm --base-url https://openagents.com --json \
  | jq '{observedAt, counted, active}'
```

Expected fields:

- `counted.todayTokens` = completed rows already in `token_usage_events`,
  bucketed by the America/Chicago day.
- `counted.completedTokensPerMinute` = completed-token rolling window.
- `active.inFlightTokens` = live active-session estimate before closeout.
- `active.adjustedTokensPerMinute` = completed window plus active-session rate.
- `active.serverAssignments[]` = live server assignments with `tokens`,
  `tokensPerMinute`, `source`, and optional `tokenCountKind`.

APM triage rule:

- `completedTokensPerMinute > 0` means exact closeout rows are landing.
- `completedTokensPerMinute == 0` with `active.serverAssignmentCount > 0` and
  increasing `active.inFlightTokens` means the fleet is still working; wait for
  turn closeout before calling the public counter stalled.
- `active.serverAssignmentCount > 0` with `active.inFlightTokens == 0` is an
  observability gap unless the run just started. Enable progress token estimates
  on trusted operators, then re-check whether `active.serverAssignments[].source`
  moves from `unavailable` to `fleet.activeAssignments.tokensSoFar`.
- `tokenCountKind: "estimated"` is operator telemetry only. Final proof still
  comes from exact `token_usage_events` rows and the owner-only trace/closeout
  checks in "Token proof" below.

Operator flag for faster in-run visibility:

```sh
OPENAGENTS_PYLON_CODEX_PROGRESS_TOKEN_ESTIMATES=1 \
  bun apps/pylon/src/index.ts assignment run-no-spend --base-url https://openagents.com
```

When this flag is on, the Codex runner posts `tokensSoFar` progress during the
run instead of waiting for final turn closeout. After the Worker schema accepting
`tokenCountKind` is deployed, also set
`OPENAGENTS_PYLON_CODEX_PROGRESS_TOKEN_KIND=1` on trusted operators to preserve
whether the progress count is `exact` or `estimated`. If that schema is not yet
deployed, omit the kind flag; `tokensSoFar` still posts.

### Supervisor log signals (`~/.codex-supervisor/supervisor.log`)
```
heartbeat ready_codex=N desired_slots=M           # auto-scale is computing slots
slot=… acc=… issue=#… OK (rc=0)                    # a turn closed out
slot=… acc=… issue=#… NO-DISPATCH (refused rc=…); backoff …s    # 409 gate refusal
slot=… acc=… issue=#… NO-DISPATCH (rate_limited rc=…); backoff …s   # 429 account limit
counter tokensServed=… desired_slots=…            # periodic progress line
```

### Decision tree — "burn slowed / stopped"

1. **Is anything *firing* requests?** The standing pylon only *executes* leased
   assignments; it does **not** create them. You need the supervisor (or some
   requester) firing `khala request`. No firer ⇒ no new work ⇒ counter flat.
   → `launch.sh status`; check for `slot=… OK` / `NO-DISPATCH` lines.
2. **Is the pylon advertising `codex available > 0`?**
   → `PYLON_HOME=$HOME/.pylon-fable bun apps/pylon/src/index.ts provider go-online --json`
   and read `codingCapacity` (expect `capacity.coding.codex.available=N`). If 0,
   the heartbeat/capability path is wrong (older code, or §4 regressed).
3. **Error class:**
   - **403** (`…not_linked`) = token is **not owner-linked** → use the Artanis
     token, not fable (§3).
   - **409** (`target_pylon_unavailable` / `dispatch_gate_blocked`) = gate
     refused: pylon not active / heartbeat stale / not codex-capable / **no
     advertised capacity** / all slots taken (pylon-level, §5).
   - **429** (`rate_limited`) = account rate limit → add a **distinct** account
     (§3, §8); same-account won't help.
4. **Wedged heartbeat process?** A `presence heartbeat` stuck for hours stalls a
   loop. → `ps aux | grep -i 'presence heartbeat'`; kill the wedged one; the
   standing loop's `go-online` avoids this.
5. **Right pylon ref?** `SUP_PYLON_REF` defaults to the **stale**
   `pylon.33afd48282a649047e3a`. → confirm you launched with the live ref from
   step 2; relaunch the supervisor if not.

### Token proof (never trust the counter alone)

Counter movement is never proof (other agents run too). Reconcile to the exact
rows — the `token_usage_events` query from the canonical CLAUDE.md runbook
(provider `pylon-codex-own-capacity`, model `openagents/pylon-codex`,
`usage_truth='exact'`, `demand_kind='own_capacity'`,
`demand_source='khala_coding_delegation'`, filtered by `task_ref =
'<assignmentRef>'`). Then verify the owner-only `agent_traces` row. See
CLAUDE.md §6 for the full SQL.

Assignment-scoped readiness evidence should be checked in this order:

1. `khala apm --json` may show active in-flight estimates while the public
   counter is flat; treat it as live operations telemetry, not completion proof.
2. `GET /api/pylon/codex/trace-status?assignment_ref=<assignmentRef>` should
   show owner-only raw chunk presence during the run and progress toward final
   exact rows without exposing raw prompts, shell output, local paths, or
   secrets.
3. `khala proof <assignmentRef> --json` is the closeout proof: expect exact
   `token_usage_events`, owner-only ATIF trace refs, owner-only raw archive refs,
   and no-spend closeout policy
   (`settlementState: not_applicable`, `payoutClaimAllowed: false`).

---

## 8. Capacity math for the daily target

Operator measurements from the burn session (attributed as measured, not
independently re-verified here):

- A fully-lit **distinct** Codex account sustains roughly **a few hundred million
  tokens/day**. We measured **~1.9M tokens/min** with ~8 concurrent turns on
  essentially **one** account.
- **~3 fully-lit distinct accounts ≈ ~1.3B tokens/day.**

The three levers:

1. **Distinct accounts** (distinct rate budgets) — the real multiplier.
2. **Advertised concurrency** (`SUP_MAX_SLOTS`, `SUP_PER_ACCOUNT`, the standing
   pylon's `OPENAGENTS_PYLON_CODEX_CONCURRENCY`).
3. **Keeping it firing 24/7** (launchd KeepAlive + supervisor refill + caffeinate).

**Reusing one account does NOT multiply** — it shares one rate budget and just
429-backs-off. Hitting a 4× day means more distinct logins, not more runners.

---

## 9. Safety invariants

- **NEVER** `codex login` / `pylon auth codex` against the default `~/.codex`
  home — it wipes the owner's live Codex session. Always isolated per-account
  homes (`~/.pylon-fable/accounts/codex/<ref>`) or a throwaway `CODEX_HOME`.
- **NEVER** print agent tokens or secrets into tracked files, commits, issue
  comments, Forum posts, or normal terminal output. Read them from `.secrets/`.
- **Deploy only via `deploy:safe`** (see `docs/DEPLOYMENT.md`); publish/deploy
  only from clean `origin/main`.
- **Own-capacity only:** no paid API, no spend, no payout claim
  (`settlementState: not_applicable`, `payoutClaimAllowed: false`).
- **Owner action items go to `~/work/NEEDS_OWNER.md`**, never the terminal (long
  URLs/tokens get mangled and scroll away). The supervisor's tripwire already
  writes there on a broken `~/.codex`.
- Keep delegation prompts **public-safe**: public issue numbers, public file
  paths, public verification commands only — no raw prompts, secrets, local
  paths, provider payloads, wallet material, or private repo content.

---

## 10. "Fleet never silently stalls" watchdog (#6408)

Owner mandate: **nothing may stop the fleet**; if it ever stalls it must
auto-detect and auto-recover within minutes. This is built in three Cloudflare-
native layers (no third-party services).

### 10a. Server detector + auto-recovery (Worker cron, 1-min)

`apps/openagents.com/workers/api/src/inference/fleet-burn-stall-detector.ts`
runs every minute from the existing scheduled handler. Each tick it:

1. measures the live own-capacity burn over a rolling window from
   `token_usage_events` (`demand_kind='own_capacity'`,
   `demand_source='khala_coding_delegation'`), and reads active coding leases;
2. classifies: **healthy** (burn ≥ threshold), **idle_no_work** (burn below
   threshold but no active leases — NOT an alarm), or **stalled** (burn below
   threshold WHILE leases are held — the gate-poison failure mode);
3. on **stalled** only: writes a loud `fleet_alerts` D1 row + a
   `fleet_burn_stall_watchdog` warning log, and (when configured) force-flushes
   abandoned leases for the owner pylon(s) by marking them `cancelled` +
   expiring the lease so they stop tripping the gate's
   `duplicate_active_assignment`. It does **not** touch the dedup gate logic.

Config (Worker `vars`, all overridable):

| Var | Default | Meaning |
|---|---|---|
| `FLEET_WATCHDOG_ENABLED` | `true` | Master switch for detection. |
| `FLEET_WATCHDOG_RECOVERY_ENABLED` | `true` | Allow auto-flush on a stall. |
| `FLEET_WATCHDOG_WINDOW_MINUTES` | `5` | Rolling burn window. |
| `FLEET_WATCHDOG_STALL_THRESHOLD_TOKENS` | `1000000` | Min tokens/window = healthy. |
| `FLEET_WATCHDOG_STALE_LEASE_MIN_AGE_MINUTES` | `10` | Lease must be idle this long before it's flushable. |
| `FLEET_WATCHDOG_OWNER_PYLON_REFS` | `""` | **Set this** (comma/space list) to enable auto-recovery; empty = alert only. |

> Auto-recovery is intentionally a **no-op until owner pylon refs are
> configured** — a safety scoping so the watchdog never flushes a pylon it was
> not told to own. Detection (alerting) works regardless. Coordinates with the
> lease-TTL sweep (#6410): if that lands, prefer its sweep; this minimal flush
> clears the same poisoned rows in the meantime.

Audit a stall:

```sql
SELECT detected_at, classification, reason_ref, burn_tokens_window,
       active_assignments, queued_assignments, recovered_lease_count,
       recovery_actions_json
  FROM fleet_alerts
 ORDER BY detected_at DESC LIMIT 20;
```

### 10b. launchd KeepAlive for the supervisors (closes the crash hole)

The codex + claude supervisors are now launchd-managed so they auto-restart if
they die (previously hand-launched → a crash went unnoticed). Mirrors the
standing-pylon job `com.openagents.pylon.fable`. Plists + wrappers live in
`apps/pylon/scripts/supervisor-launchd/`. The wrapper sources the owner-linked
Artanis token from `~/work/.secrets/openagents-artanis-agent.env` (never in the
plist), resolves the live pylon ref, and `exec`s the supervisor in the
foreground (so KeepAlive restarts it), all under `caffeinate -i`.

Install:

```sh
# from the repo root
bash apps/pylon/scripts/supervisor-launchd/install.sh install both
# under the hood: sed-substitutes repo root/home into the plist, copies it to
# ~/Library/LaunchAgents, then:
#   launchctl bootout   gui/$(id -u)/com.openagents.<codex|claude>-supervisor  (idempotent)
#   launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.openagents.<...>.plist
#   launchctl enable    gui/$(id -u)/com.openagents.<...>-supervisor

bash apps/pylon/scripts/supervisor-launchd/install.sh status      # list loaded jobs
bash apps/pylon/scripts/supervisor-launchd/install.sh uninstall both
tail -f ~/.codex-supervisor/supervisor.log ~/.claude-supervisor/supervisor.log
```

### 10c. In-loop self-heal in the supervisors

Each supervisor now runs a `selfheal_watchdog_loop`: if it sees
`SUP_STALL_REFUSALS` (default **20**) consecutive `NO-DISPATCH` log lines with
**zero `OK`** in between, it logs a loud `FLEET-STALL` line and self-recovers —
(a) re-asserts advertisement with `provider go-online`, (b) sweeps its own
interrupted/stale local leases via `assignment run-no-spend` (which submits the
public-safe stale closeouts) — then cools down `SUP_SELFHEAL_COOLDOWN_SECS`
(default 300s) before re-checking. So the local loop tries to self-recover
instead of backing off into silence.

## 11. GCE VM migration for own-capacity accounts (#6433)

Issue #6433's durable target is: one lightweight GCE VM per distinct Codex
account, each running a headless Pylon and one Codex supervisor slot under
systemd. The desktop stops being the durable executor; it is only the place where
the isolated account homes are prepared and copied from.

The repo pieces are:

- `apps/pylon/deploy/gcloud/setup-pylon.sh` — creates/starts an IAP-only Ubuntu
  VM, copies a local env file over SSH, and installs `openagents-pylon.service`.
- `apps/pylon/scripts/install-cloud-node.sh` — the Linux Pylon node installer.
- `apps/pylon/scripts/install-codex-supervisor-systemd.sh` — installs
  `openagents-codex-supervisor.service`, sourcing `/etc/openagents-pylon.env`
  plus `/etc/openagents-codex-supervisor.env`, and running the existing
  supervisor in the foreground so systemd restarts it.

Minimal per-account migration:

```sh
# 1. Base VM + Pylon node. The env file stays off Git and is copied over IAP/SSH,
# never instance metadata or startup scripts.
apps/pylon/deploy/gcloud/setup-pylon.sh \
  --instance oa-codex-codex-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --subnet oa-lightning-us-central1 \
  --machine-type e2-standard-4 \
  --env-file ~/work/.secrets/openagents-artanis-agent.env

# 2. Copy one isolated authenticated account home. Do NOT copy ~/.codex.
tar -C "$HOME/.pylon-fable/accounts/codex" -czf /tmp/codex-1.tgz codex-1
gcloud compute scp /tmp/codex-1.tgz oa-codex-codex-1:/tmp/codex-1.tgz \
  --project openagentsgemini --zone us-central1-a --tunnel-through-iap
gcloud compute ssh oa-codex-codex-1 \
  --project openagentsgemini --zone us-central1-a --tunnel-through-iap \
  --command 'sudo mkdir -p /var/lib/openagents-pylon/accounts/codex && sudo tar -C /var/lib/openagents-pylon/accounts/codex -xzf /tmp/codex-1.tgz && sudo chown -R pylon:pylon /var/lib/openagents-pylon/accounts'
rm -f /tmp/codex-1.tgz

# 3. Install durable Linux supervisor. One VM/account starts with one slot; raise
# only after proving that account's real headroom.
gcloud compute ssh oa-codex-codex-1 \
  --project openagentsgemini --zone us-central1-a --tunnel-through-iap \
  --command 'sudo SUP_MAX_SLOTS=1 SUP_PER_ACCOUNT=1 PYLON_HOME=/var/lib/openagents-pylon /opt/openagents-pylon/apps/pylon/scripts/install-codex-supervisor-systemd.sh'
```

Verification:

```sh
gcloud compute ssh oa-codex-codex-1 \
  --project openagentsgemini --zone us-central1-a --tunnel-through-iap \
  --command 'sudo systemctl --no-pager --full status openagents-pylon openagents-codex-supervisor && sudo -u pylon env PYLON_HOME=/var/lib/openagents-pylon bun /opt/openagents-pylon/apps/pylon/src/index.ts codex accounts list --json && sudo journalctl -u openagents-codex-supervisor -n 80 --no-pager'
```

Expected:

- `openagents-pylon.service` and `openagents-codex-supervisor.service` are
  active and enabled.
- The copied account reports `readiness.state: "ready"` and
  `capability.pylon.local_codex`.
- `/var/lib/openagents-pylon/.codex-supervisor/supervisor.log` shows
  `heartbeat ready_codex=1 desired_slots=1` and either `OK` closeouts or typed
  `NO-DISPATCH` backoff lines.

Scale the fleet by repeating the VM shape for roughly seven distinct Codex
accounts (`oa-codex-codex-1` ... `oa-codex-codex-7`). Distinct underlying
accounts are the throughput multiplier; aliases or copied sessions for the same
account may share one rate budget.

---

## Appendix — verification status of this runbook

Verified against the repo / live system on 2026-06-27:

- `~/.pylon-fable/bin/standing-pylon.sh` contents (go-online loop, concurrency=8,
  fable token).
- launchd label `com.openagents.pylon.fable` runs it; `com.openagents.pylon-node`
  is a **separate** discovery-node job (`run-discovery-node.sh`).
- Supervisor scripts + all §2b defaults (read from
  `apps/pylon/scripts/codex-supervisor/{launch.sh,codex-supervisor.sh}`).
- Linux/GCE supervisor persistence path:
  `apps/pylon/scripts/install-codex-supervisor-systemd.sh` installs
  `openagents-codex-supervisor.service`.
- Gate-fix commits `982c33f521` + `1cc0e9ba03` (both #6354); worker
  `hasAvailableCodexCapacity` (line ~299) + typed refusals; `state.ts`
  `loadOrCreateRuntimeState` (line ~204).
- Artanis token file + `oa_agent_yCqh…` prefix; Cloudflare unblock script path.
- Public endpoints (`/khala-tokens-served`, `/history`, `/model-mix`) live;
  `pylon_codex` ~61% of the 30-day mix.

Could **not** verify (flagged inline):

- The exact deploy Worker version `68da222b` for the gate fix (the recorded
  #6358 deploy used `95d3fcee-…`).
- The **live** `SUP_PYLON_REF` (refs drift; fetch via `provider go-online
  --json` at launch). `pylon.a1469b9cdf6965a57530` was a prior live value; the
  stale default `pylon.33afd48282a649047e3a` must not be trusted.
- §8 throughput figures are operator measurements, not re-measured here.
