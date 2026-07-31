# 2026-07-31 Codex `019fb495` Overnight Spend After-Action

## Document control

| Field                          | Value                                                                                                                      |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| Status                         | Final forensic report with owner-directed subject-agent addendum                                                           |
| Session date                   | 2026-07-30 / 2026-07-31                                                                                                    |
| Report date                    | 2026-07-31                                                                                                                 |
| Addendum date                  | 2026-07-31                                                                                                                 |
| Incident class                 | Autonomous agent budget overrun without objective completion                                                               |
| Impact level                   | High cost, low delivery on the stated objective                                                                            |
| Subject thread                 | Codex session `019fb495-21a0-7b12-8cf1-795c91b750f1`                                                                       |
| Transcript                     | `~/.codex/sessions/2026/07/30/rollout-2026-07-30T14-51-40-019fb495-21a0-7b12-8cf1-795c91b750f1.jsonl`                      |
| Transcript size at snapshot    | 84,377,103 bytes / 29,803 records                                                                                          |
| Analysis snapshot boundary     | `2026-07-31T12:50:12Z`                                                                                                     |
| Session state at analysis time | **Still running** (see [Note on the snapshot](#note-on-the-snapshot))                                                      |
| Report scope                   | Evidence, quantified waste, and corrective process changes                                                                 |
| Change scope                   | Documentation only                                                                                                         |
| STE governance                 | Out of scope — `docs/afteraction/` is not a governed public prefix (`docs/ste/checker-config.v1.json`, `governedPrefixes`) |

## Note on the snapshot

All numbers below are frozen at `2026-07-31T12:50:12Z`, the boundary at which
the quantitative extraction ran. The session had **not** stopped at that point.
At `2026-07-31T12:59:56Z` the transcript had grown to 30,006 records /
84,729,608 bytes and the parent-thread cumulative counter read
`893,506,373` tokens and was still climbing. The absence of any terminating
condition is itself a finding (see [Finding 7](#finding-7-there-was-no-stop-rule)).

---

## Executive summary

A single Codex Desktop thread ran for **16 hours 58 minutes** (2026-07-30
14:51 through 2026-07-31 07:50 local, `America/Chicago`) against the LiveKit /
Sarah voice program. The owner reports roughly **$900 in API spend** and
roughly **600 million tokens**.

The transcript carries exact per-call token accounting. The measured figures
are materially larger than the reported estimate:

| Measure                                                                | Value                             |
| ---------------------------------------------------------------------- | --------------------------------- |
| Parent thread total tokens                                             | **883,357,491**                   |
| Parent thread model API calls                                          | **6,794**                         |
| Depth-1 sub-agent threads spawned                                      | **78**                            |
| Sub-agent thread tokens (their own spend, excluding inherited context) | **1,022,615,093**                 |
| **True total across all threads**                                      | **1,905,972,584 (~1.91 billion)** |
| Share of true total in the parent thread                               | 46.3%                             |
| Share of true total in sub-agents                                      | **53.7%**                         |

The reported ~600M figure is about **3.2x lower** than the measured true total.
The most likely reason is that the parent thread's own counter is the visible
one, and that counter itself only reached 883M — the 78 forked sub-agent
threads each carry a separate rollout file with a separate counter, and their
spend is not visible from the parent's UI.

**The objective did not land.** The release-gate objective was a live two-room
Sarah acceptance — one private room and one community room, concurrently,
through the real worker, signer boundary, LiveKit media path, and OpenAI
Realtime. The private room passed. **The community room never passed.** At the
snapshot boundary the thread was still deploying "one more fix" for it. 161
commits did reach `openagents` `main` in the window (130 of them LiveKit or
Sarah related), so the session produced real landed code — it simply never
closed the gate it was chasing.

**The dominant cost driver is not what most people would guess.** It is not
redundant test suites and it is not exact command repetition. Ranked by
measured share:

| Rank | Category                                                                                      | Measured share of parent-thread spend                                                      |
| ---- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 1    | Context architecture — 98.6% of input tokens were **cached re-reads** of context already seen | structural, ~64x amplification                                                             |
| 2    | **Polling and waiting** (`write_stdin`, `wait`, `wait_agent`, `list_agents`)                  | **28.3%** (254,481,925 tokens over 1,881 model calls)                                      |
| 3    | Sub-agent orchestration overhead inside the parent                                            | 13.2% (118,682,325 tokens)                                                                 |
| 4    | Debug-in-production redeploy loop                                                             | 21 production Cloud Run deploys, 30 GKE rollouts, 23 image builds, 42 live acceptance runs |
| 5    | Scope drift into an unrelated visual-baseline lane                                            | 17 screenshots = 20.2% of all tool-output bytes                                            |
| 6    | Exact and near-duplicate command repetition                                                   | 21.7% of commands, but only **8.1%** of shell output bytes                                 |

**45.4% of the parent thread's spend (400,648,850 tokens over 2,995 model
calls) happened after the last human message**, which arrived at
`2026-07-31T06:59:02Z`.

---

## 1. Ground truth

### 1.1 Configuration

From `turn_context` records (44 of them):

| Field                | Value                                                        |
| -------------------- | ------------------------------------------------------------ |
| Model                | `gpt-5.6-sol` (43 turns), `gpt-5.6-terra` (1 turn)           |
| Context window       | 258,400 tokens                                               |
| Reasoning effort     | `low` on turn 1, `medium` thereafter                         |
| `approval_policy`    | `never`                                                      |
| `sandbox_policy`     | `danger-full-access`                                         |
| `permission_profile` | `disabled`                                                   |
| `multi_agent_mode`   | `explicitRequestOnly`, `multi_agent_version: v2`             |
| Working directory    | `~/work/omega`                                               |
| Workspace roots      | `omega`, `openagents`, `ai`, `nostr-effect`, `effect-native` |
| Client               | Codex Desktop `0.146.0-alpha.3.1`                            |

There was no approval gate and no permission profile. Every action the model
chose was executed.

### 1.2 What was asked, and when

There were 16 user messages. The session began as a small documentation task
and escalated four times into an unbounded implementation and production
deployment program.

| Time (UTC)   | Instruction (abridged)                                                                                   | Cumulative tokens at that point | Model calls since previous mark |
| ------------ | -------------------------------------------------------------------------------------------------------- | ------------------------------- | ------------------------------- |
| 19:51:48     | Add a repo to the projects manifest and sync                                                             | 19,808                          | 0                               |
| 19:52:56     | Write a unified LiveKit teardown from three existing teardowns                                           | 223,385                         | 8                               |
| 20:12:56     | Add in-depth self-hosting analysis; pull whatever repos you need                                         | 9,900,306                       | 84                              |
| 20:49:01     | Fold the release gate and transcript promises into the new plan; create GitHub issues                    | 33,210,716                      | 174                             |
| **20:56:40** | **"Implement all those in sequential order... Anything that can be parallelized via subagents, do it."** | 40,219,060                      | 39                              |
| 21:05:41     | "you are approved to set up all needed infrastructure regardless of cost. continue"                      | 45,118,304                      | 58                              |
| 23:46:25     | "pause and claer up disk space"                                                                          | 167,380,914                     | 923                             |
| 23:54:53     | "continue toward the issues until all are done"                                                          | 169,291,031                     | 21                              |
| 05:09:30     | "Keep going."                                                                                            | 372,748,987                     | 1,667                           |
| 06:58:06     | "i dont see the chrome window."                                                                          | 481,869,767                     | 819                             |
| **06:59:02** | **"i installed it on openagents. done" (last human input)**                                              | 482,708,641                     | 5                               |
| 12:50:12     | _(analysis snapshot; session still running)_                                                             | **883,357,491**                 | **2,995**                       |

Two instructions did the damage:

- `20:56:40` converted a bounded documentation task into "implement all
  issues sequentially, parallelize via subagents" — an unbounded work queue.
- `21:05:41` removed the only remaining natural brake: "regardless of cost."

Neither instruction carried a token budget, a wall-clock budget, a deadline, or
a definition of when to stop and report. The two later nudges — "continue toward
the issues until all are done" and "Keep going." — restated the unbounded queue.

### 1.3 Session shape

| Measure                                      | Value                                                         |
| -------------------------------------------- | ------------------------------------------------------------- |
| Duration                                     | 16h 58m 20s (`2026-07-30T19:51:52Z` → `2026-07-31T12:50:12Z`) |
| Model API calls (parent)                     | 6,794                                                         |
| Tool calls (parent)                          | 6,615                                                         |
| — `function_call`                            | 4,155                                                         |
| — `custom_tool_call` (`exec`, `apply_patch`) | 2,458                                                         |
| — `tool_search_call`                         | 19                                                            |
| Reasoning records                            | 6,241                                                         |
| Assistant messages                           | 397                                                           |
| Patches applied                              | 292                                                           |
| Context compaction events                    | **31** (roughly one every 33 minutes)                         |
| Shell commands issued                        | 4,040 across 3,907 exec-family calls                          |

### 1.4 Outcome

**Landed:** 161 commits on `openagents` `main` during the window, 130 matching
`livekit` or `sarah`. Spot-verified: `4d1579887c` ("Harden Cloud SQL socket
startup") and `68b65df936` ("Record LiveKit production deployment receipt")
are both real commits on `main`.

**Not landed:** the two-room acceptance. The thread's own final assistant
messages establish this without ambiguity:

- `12:43:57Z` — "the private end-to-end voice journey completed with real audio."
- `12:48:06Z` — "The remaining community retry exposed a stale rendezvous left
  behind when the prior worker failed. I've added fail-closed
  authority/rendezvous retirement for every community worker close... Deploying
  that cleanup fix now."

That is the last substantive status at the snapshot boundary: still one fix away,
after six hours of being one fix away.

---

## 2. Where the tokens went

### 2.1 The structural fact: 98.6% of input was re-read context

Final parent-thread breakdown:

| Component                                    | Tokens          | Share of input  |
| -------------------------------------------- | --------------- | --------------- |
| Input tokens                                 | 882,135,489     | 100%            |
| — of which **cached** (already-seen context) | **869,530,540** | **98.6%**       |
| — of which novel (uncached)                  | 12,604,949      | 1.4%            |
| Cache-write tokens                           | 4,040,456       | —               |
| Output tokens                                | 1,222,002       | —               |
| — of which reasoning                         | 370,221         | 30.3% of output |

Per-call context size distribution across the 6,794 model calls, against a
258,400-token window:

| Percentile | Context tokens |
| ---------- | -------------- |
| min        | 17,729         |
| p25        | 83,394         |
| median     | **135,214**    |
| p75        | 183,552        |
| p90        | 209,441        |
| p99        | 231,725        |
| max        | 245,516        |

The thread ran at a **median 52% and a p90 81% of its maximum context window,
6,794 times in a row**, for seventeen hours. It compacted 31 times and refilled
each time.

Novel content across the entire session was small: roughly 12.6M uncached input
tokens plus 1.2M output tokens, against 883M billed context. That is an
**amplification factor of roughly 64x** — every token that entered this thread's
context was paid for, on average, 64 more times.

This is the single most important number in the report. Every other finding is
a variation on it: _the cost of an agent thread is not what it does, it is how
big its context is multiplied by how many times it calls the model._ Reducing
call count and reducing resident context are the only two levers that matter at
this scale.

### 2.2 Spend attributed by tool

Each `token_count` event corresponds to one model API call. Attributing each
event to the tool call the model emitted immediately after it gives:

| Tool                  | Model calls | Context tokens  | Share     | Avg per call |
| --------------------- | ----------- | --------------- | --------- | ------------ |
| `exec`                | 2,224       | 296,641,151     | 33.0%     | 133,381      |
| `exec_command`        | 1,682       | 221,607,816     | 24.6%     | 131,752      |
| **`write_stdin`**     | **1,077**   | **146,899,702** | **16.3%** | 136,397      |
| **`wait_agent`**      | **456**     | **63,258,217**  | **7.0%**  | 138,724      |
| **`wait`**            | **248**     | **33,268,601**  | **3.7%**  | 134,147      |
| `apply_patch`         | 233         | 31,687,414      | 3.5%      | 135,997      |
| `send_message`        | 208         | 26,670,329      | 3.0%      | 128,222      |
| (text reply, no tool) | 181         | 26,424,724      | 2.9%      | 145,992      |
| `js`                  | 177         | 15,195,023      | 1.7%      | 85,847       |
| **`list_agents`**     | **94**      | **10,726,506**  | **1.2%**  | 114,111      |
| `spawn_agent`         | 78          | 8,863,959       | 1.0%      | 113,640      |
| `followup_task`       | 67          | 7,982,308       | 0.9%      | 119,138      |
| `view_image`          | 17          | 2,745,440       | 0.3%      | 161,496      |
| others                | 52          | 6,504,823       | 0.7%      | —            |

Grouping these:

- **Waiting for a local process or a sub-agent to finish: 254,481,925 tokens
  = 28.3%**, over 1,881 model calls.
- Sub-agent orchestration overhead (`spawn_agent`, `wait_agent`, `send_message`,
  `list_agents`, `followup_task`, `interrupt_agent`): 118,682,325 = 13.2%.
- Actual shell work (`exec`, `exec_command`, `apply_patch`): 549,936,381 = 61.1%.

### 2.3 The polling pathology

`write_stdin` was called 1,079 times. **1,049 of those (97.2%) carried
`chars: ""`** — they sent nothing. They exist only to wait for output from an
already-running process. Only 30 calls carried real input.

Distribution of `yield_time_ms` on the empty polls:

| `yield_time_ms` | Count   |
| --------------- | ------- |
| **1000**        | **657** |
| 5000            | 26      |
| 10000           | 134     |
| 15000           | 2       |
| 20000           | 29      |
| 30000           | 207     |

**657 of the polls waited one second.** Measured cost of the 656 that could be
attributed to a model call: **82,534,501 tokens — 9.3% of the entire parent
thread — spent one second at a time.**

Every one of those was a full round trip that resent a ~136,000-token context in
order to learn whether a `cargo build` or a `gcloud builds submit` had finished
yet. Concrete argument, verbatim from the transcript:

```json
{ "session_id": 2881, "chars": "", "yield_time_ms": 30000, "max_output_tokens": 30000 }
```

Longest observed uninterrupted polling runs:

| Consecutive poll calls | Start                  | Wall-clock span |
| ---------------------- | ---------------------- | --------------- |
| 21                     | `2026-07-31T00:19:14Z` | 2m 23s          |
| 12                     | `2026-07-31T09:29:39Z` | 3m 45s          |
| 11                     | `2026-07-31T09:18:14Z` | 1m 16s          |
| 10                     | `2026-07-31T11:11:36Z` | 1m 07s          |

The 21-call run spent roughly **2.9 million tokens to wait 143 seconds.**

### 2.4 Sub-agent fanout: the invisible half of the bill

78 depth-1 sub-agent threads were spawned. Every observed `spawn_agent` call
used `"fork_turns": "all"` — each child inherits the **entire parent context at
fork time**, which by then was routinely 130,000–250,000 tokens, and then runs
its own loop on top of that inherited base.

Each child writes its own rollout file whose cumulative counter _continues_ from
the parent's value at the fork instant. Subtracting the parent's cumulative value
at the fork timestamp from the child's final value yields the child's own spend.
Worked example, verified end to end:

- Child `/root/shared_room_backend`, forked `2026-07-31T10:37:18.535Z`
- Parent cumulative at that instant: `711,816,492`
- Child final cumulative: `757,390,219`
- Child own spend: `45,573,727` over 314 of its own model calls, averaging
  **145,139 tokens per call**

Aggregated across all 78 children: **1,022,615,093 tokens.**

Top lanes by own spend:

| Own tokens | Lane                          |
| ---------- | ----------------------------- |
| 50,496,221 | `interrupt_contract`          |
| 47,310,074 | `secure_deployer`             |
| 45,573,727 | `shared_room_backend`         |
| 45,552,664 | `acceptance_matrix`           |
| 40,993,666 | `deploy_audit`                |
| 39,784,380 | `fix_9285_worker_provider`    |
| 36,764,952 | `live_failure_harness`        |
| 36,301,891 | `livekit_ops_acceptance`      |
| 34,405,975 | `fix_9285_control_membership` |
| 32,041,805 | `implement_ep263_lk_01`       |

Note the shape of that list: at least seven of the top ten are _audit_,
_acceptance_, _deploy audit_, or _failure harness_ lanes — verification and
review fanout, not implementation. Four separate `audit_9285_*` lanes appear
further down the list (`audit_9285_final_lifecycle` 28.5M,
`audit_9285_final_security` 27.6M, `audit_9285_deadline_events_final` 23.3M,
`audit_9285_lifecycle` 21.7M) — **101 million tokens across four audits of the
same issue.**

Because each fork inherits a full context, sub-agent cost scales with _when_ you
fork, not with how much work the child does. A child spawned at hour 15 starts
with a 200,000-token bill per call before it does anything.

### 2.5 Tool output entering context

24,849,930 bytes of tool output re-entered the parent context. The top 50 calls
alone account for 6,538,448 bytes (26.3%).

By category of shell command (3,717 categorizable commands):

| Category                                     | Commands | Output bytes |
| -------------------------------------------- | -------- | ------------ |
| file reads (`sed -n`, `cat`, `head`, `tail`) | 612      | 5,727,042    |
| searches (`rg`, `grep`, `find`)              | 624      | 4,404,145    |
| git status / diff / log / rev-parse          | 481      | 1,776,502    |
| live acceptance runs                         | 203      | 837,002      |
| `gcloud` logging / secrets / IAM             | 138      | 382,413      |
| `gh` issue operations                        | 51       | 360,735      |
| Cloud Run deploys                            | 83       | 320,667      |
| build / deploy status polls                  | 213      | 296,302      |
| test runs                                    | 130      | 272,957      |
| container / image builds                     | 122      | 207,077      |
| psql / migrations                            | 76       | 176,434      |
| git push                                     | 142      | 140,484      |
| typecheck / lint / clippy / cargo check      | 48       | 128,338      |
| git cherry-pick / rebase / merge             | 80       | 68,188       |
| other                                        | 714      | 1,577,000    |

**The harness's own truncation was worth about 29 MB.** The `node_repl.js` MCP
tool produced 29,324,643 bytes of raw results across 202 calls (averaging
145,171 bytes each), but only 465,654 bytes reached the model context. Whatever
truncation policy sits on that path is doing real work and should be extended,
not weakened.

**Images were the single worst per-call offender.** `view_image` was called only
17 times but delivered **5,016,094 bytes — 20.2% of all tool-output bytes** —
averaging 295,064 bytes per call, with a maximum of 466,105 bytes in one call.
All 17 belong to one lane: the Omega visual-baseline task (#191), which was not
the objective. Worse, **the same image was loaded five separate times**:

| Time (UTC) | Bytes   | Path                                                      |
| ---------- | ------- | --------------------------------------------------------- |
| 11:16:16   | 339,693 | `.../omega_workbench_identity_dirty_conflict/current.png` |
| 11:23:40   | 380,877 | same                                                      |
| 11:29:26   | 380,877 | same                                                      |
| 11:33:13   | 380,877 | same                                                      |
| 11:50:09   | 359,609 | same                                                      |

That is roughly **1.84 MB of context spent re-viewing one unchanged screenshot**,
inside a lane that had nothing to do with the blocked objective.

### 2.6 Repetition: real, but not the headline

This is the category most audits blame, so it deserves an honest measurement.

| Measure                                                                 | Value                              |
| ----------------------------------------------------------------------- | ---------------------------------- |
| Shell commands issued                                                   | 4,040                              |
| Unique exact command strings                                            | 3,412                              |
| Exact repeats beyond first execution                                    | 628 (**15.5%**)                    |
| Unique normalized command strings (UUIDs/SHAs/timestamps/digits masked) | 3,163                              |
| Near-duplicate repeats beyond first execution                           | 877 (**21.7%**)                    |
| Output bytes from repeat executions                                     | 1,284,800 of 15,854,543 (**8.1%**) |

**Exact repetition is only 8.1% of shell output bytes.** It is a real but
second-order problem here. Naming the specific offenders anyway, since several
are diagnostic of deeper issues:

| Repeats | Command (normalized)                                                               |
| ------- | ---------------------------------------------------------------------------------- |
| 46      | `gcloud builds describe <UUID> ... --format='value(status)'`                       |
| 30      | `node scripts/cloud/livekit-production-deploy.mjs start --timeout-seconds <N>`     |
| 23      | `bash scripts/cloud/build-sarah-livekit-agent.sh --apply` (container image builds) |
| 23      | `... sarah-headless ...` acceptance runs                                           |
| 21      | `bash apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh production`       |
| 20      | `git cherry-pick <SHA>`                                                            |
| 19      | `node --import tsx /private/tmp/oa-livekit-ephemeral-acceptance.mjs`               |
| 19 + 13 | `git push origin main` / `git push origin HEAD:main`                               |
| 14      | `gcloud run services update openagents-monolith ... --update-env-vars ...`         |
| 10      | `sed -n '<N>,<N>p' scripts/cloud/livekit-gcp-ops.mjs`                              |
| 10      | `sed -n '<N>,<N>p' apps/sarah-livekit-agent/src/agent.ts`                          |
| 9       | `KUBECONFIG=... kubectl -n livekit-system get deployment,rs,pods`                  |
| 8       | `sed -n '<N>,<N>p' FASTFOLLOW.md`                                                  |

The `sed -n '<start>,<end>p'` pattern repeated 8–10 times per file is the
classic "read the same file again because the earlier read scrolled out of the
compacted window" signature. It is a symptom of compaction, not of carelessness.

---

## 3. The wasted cycles, named

### Finding 1: 28.3% of the parent thread was spent waiting

**Evidence:** §2.2, §2.3. 1,881 model calls, 254,481,925 tokens, on
`write_stdin`/`wait`/`wait_agent`/`list_agents`. 656 of them were one-second
waits costing 82,534,501 tokens.

**Why it happened:** the agent had no non-model way to block on a long-running
process. Its only mechanism for "is the build done?" was to spend a full model
turn asking. Section 4 of this report proposes the fix, and
[Finding 8](#finding-8-a-repository-contract-clause-provably-drove-the-polling-waste)
shows that a written contract clause actively encouraged the worst form of it.

### Finding 2: 53.7% of the bill was invisible to the operator

**Evidence:** §2.4. 78 forked sub-agents, 1,022,615,093 tokens, in separate
rollout files with separate counters.

The owner's ~600M estimate is consistent with reading the visible parent
counter and not knowing about the other billion. Any budget discipline built on
the number shown in the parent session will under-count by roughly half.

### Finding 3: 101 million tokens went into four audits of the same issue

**Evidence:** §2.4. `audit_9285_final_lifecycle` (28,528,201),
`audit_9285_final_security` (27,608,903), `audit_9285_deadline_events_final`
(23,300,460), `audit_9285_lifecycle` (21,748,294).

Four full-context forks were spent auditing issue #9285 from four angles. The
issue was still not closed. Each fork paid the full inherited-context tax before
producing its first useful token. This is process ceremony scaled by a fanout
multiplier.

### Finding 4: debug-in-production, 42 acceptance runs, failure density rising

**Evidence:** command census (§2.5) and the failure-class distribution below,
counted across tool outputs per hour:

| Hour (UTC)   | 503    | 409    | 403    | 401    |
| ------------ | ------ | ------ | ------ | ------ |
| 07-31 08     | 8      | 5      | 3      | 3      |
| 07-31 09     | 9      | 9      | 3      | 14     |
| 07-31 10     | 6      | 14     | 2      | 6      |
| 07-31 11     | 18     | 14     | 7      | 14     |
| **07-31 12** | **26** | **15** | **10** | **10** |

Failure density **increased monotonically toward the end**. The loop was:
run live acceptance → hit an HTTP error → diagnose from production → patch →
run the repo gate → push → rebuild an immutable image → redeploy Cloud Run →
roll out GKE → rerun acceptance. Each full cycle cost 21 production deploys'
worth of build and rollout polling. Over the session: **21 Cloud Run production
deploys, 30 GKE production rollouts, 23 container image builds, 42 live
acceptance runs, 14 in-place service env-var updates.**

The assistant's own narration of the final six hours reads as a chain of
distinct root causes, each found only by shipping to production:
Cloud SQL socket timeout (09:46) → 401 on session revocation (09:44) → worker
digest drift (10:02) → 409 community claim (11:35) → 503 pre-connection state
(11:44) → 503 authority-store `active` predicate (12:13) → 403 membership
revision (12:32) → missing `SARAH_NOSTR_EXPECTED_PUBKEY` (12:37) → relay kind
allowlist missing `30382` (12:43) → stale rendezvous (12:48).

Nine of those ten are _configuration or state-machine predicate_ bugs that a
single integration environment with the same config surface would have surfaced
in one pass. Instead each cost a full build-deploy-rollout-acceptance cycle.

### Finding 5: scope drift into an unrelated lane during the outage

**Evidence:** §2.5. Between 10:56 and 12:26 — while the community-room
acceptance was still failing — the thread opened Omega #191, a visual-baseline
repair task, and spent 17 `view_image` calls (5,016,094 bytes, 20.2% of all
tool output) plus repeated Rust rebuilds on it. Its own narration at 10:56:29
states the motive: "While the shared-room backend finishes, I'm moving to
independent Omega #191 baseline repair so that lane is no longer silently red."

That is defensible parallelism in principle. In practice it added the single
most expensive per-call tool usage in the session to an already-saturated
context, during the hours when the primary objective was blocked.

### Finding 6: the Chrome loop

**Evidence:** 88 browser-control calls between 21:34:08 and 10:24:55. The owner
said "i dont see the chrome window." at 06:58:06 and resolved it himself at
06:59:02 ("i installed it on openagents. done"). The thread nonetheless
re-announced Chrome connectivity as a fresh event at least seven more times —
10:24:35, 10:39:10, 10:39:19, 10:59:58, 11:18:45, 11:36:12, 12:19:30 — with
messages like "Great — Chrome is connected now" and "Got it—the Chrome plugin
is installed now."

The thread had lost the memory that this was already resolved, most likely
across compaction boundaries, and kept re-deriving it. Its final position at
11:36:12 — "I don't need to surface a separate Chrome window for the remaining
work" — is the conclusion it could have reached thirteen hours earlier.

### Finding 7: there was no stop rule

**Evidence:** §1.2, §1.4, and the [snapshot note](#note-on-the-snapshot).

- 400,648,850 tokens (45.4% of the parent thread) and 2,995 model calls were
  spent **after the last human message**.
- The session was still running and still spending at analysis time, roughly
  **18 hours in**.
- There is no observed point at which the thread paused to report "the community
  room has failed N times against N distinct root causes; I recommend stopping."

The thread was honest throughout — its status messages accurately described
what was failing and why. It simply never treated "I have now been one fix away
for six hours" as information worth acting on. There was no budget it was
checking against, because none was given, and no rule that converts _N
consecutive failed acceptance cycles_ into an escalation.

The 23:46:25 owner intervention ("pause and claer up disk space") is instructive:
the only two things that stopped this thread in seventeen hours were a disk-full
condition and a human typing.

### Finding 8: a repository contract clause provably drove the polling waste

`AGENTS.md` lines 72–83, under "Autonomous Loop: Constant Motion (owner
mandate)", currently reads:

> - **Never sit idle. Never sleep on a minutes-long timer.** Do real work every
>   moment the loop is active. [...] "nothing to do" is never true.
> - **Long `ScheduleWakeup` idle waits are banned.** Keep working in the SAME turn:
>   finish a unit → immediately start the next. If you must yield, prefer a fanout
>   whose watcher re-invokes you instantly, only if truly unable to proceed this
>   instant, use a SHORT wakeup (≤120s), never minutes.

This is a direct written instruction to prefer short waits over long ones and
never to idle. Measured consequence in this thread: **657 one-second polls
costing 82,534,501 tokens (9.3% of the parent thread)**, and 1,881 total
waiting calls costing 28.3%.

The clause's _intent_ is correct and worth keeping — an autonomous loop should
not sleep through available work. Its _mechanism_ is wrong, because in a
long-context agent a "short wakeup" is not cheap. At this thread's median
context, one poll costs about 135,000 tokens. A 120-second sleep costs nothing.
The clause optimizes latency and pays for it in tokens at roughly 1,100 tokens
per second of avoided wait.

Two other clauses are secondary contributors, and honesty requires stating that
neither was the main driver here:

- **"One completion gate: `pnpm run check`"** (`AGENTS.md` line 855). The parent
  thread did **not** abuse this. It used targeted verification throughout —
  `pnpm exec vp test --run <specific files>`, `cargo test -p <pkg> <testname>` —
  with only 130 test invocations and 48 typecheck/lint/clippy invocations across
  17 hours. The real cost of this clause landed elsewhere: the pre-push hook runs
  `check:fast` on **every** push to `main`, and there were 142 push calls. The
  thread's own narration records two push-gate rejections requiring
  regenerate-and-retry cycles (10:27:00 on the governed surface inventory,
  10:50:39 on two generated assurance inventories); 272 tool outputs mention the
  assure-repo surface inventory. Sub-agents did run full suites — 9,053 API tests
  and 695 Khala tests are named in the 11:19 and 11:35 status messages.
- **"Fresh worktree per task"** (`AGENTS.md` line 812). 80 worktree operations
  were performed. A `git worktree add` in this repository checks out 11,948
  files, and for Omega it implies a cold Rust build. The thread's own narration
  at 10:57:08 is the evidence: "The visual harness began duplicating a full Rust
  target tree in the new worktree, so I stopped it and am reusing the already-built
  Omega target cache. That avoids another tens-of-gigabytes build." The owner's
  23:46:25 disk-space intervention is the downstream consequence. This clause
  costs wall-clock and disk far more than tokens, but the rebuild polling it
  generates is billed at 135,000 tokens per check.

---

## 4. How to do this work in half the tokens or less

Every recommendation below is tied to a measured finding. Percentages are
**estimates** derived from the measured shares in §2 and stated as such. They
assume the same objective and the same amount of real engineering work.

### 4.1 Ranked by estimated saving

#### 1. Replace model-loop polling with blocking waits — est. 20–26% saving

**Evidence:** Finding 1. 254,481,925 tokens (28.3%) on 1,881 waiting calls;
82,534,501 of those (9.3%) on 656 one-second polls.

**Change:** never return to the model to ask "is it done yet." Instead:

```sh
# instead of: start build; poll; poll; poll; poll ...
<long command> > /tmp/run.log 2>&1; echo "exit=$?" >> /tmp/run.log
tail -40 /tmp/run.log
```

One tool call, one model turn, regardless of whether the command took two
seconds or twenty minutes. Where a true background handle is needed, poll with
a **shell** loop inside a single tool call, not with a model round trip:

```sh
for i in $(seq 1 120); do
  s=$(gcloud builds describe "$ID" --format='value(status)')
  case "$s" in SUCCESS|FAILURE|TIMEOUT|CANCELLED) echo "$s"; break;; esac
  sleep 10
done
```

That single call replaces the 46 `gcloud builds describe` round trips measured
in §2.6. If a wait tool must be used, its minimum yield should be raised from
1,000 ms to at least 60,000 ms. At the observed median context, a 1-second poll
costs ~135,000 tokens per second of waiting; a 60-second poll costs ~2,250.

**Retains:** all verification semantics. Nothing is skipped; only the
observation mechanism changes.

#### 2. Stop forking full context into sub-agents — est. 20–30% of the true total

**Evidence:** Finding 2 and Finding 3. 1,022,615,093 sub-agent tokens (53.7% of
the true total), all spawned with `fork_turns: "all"`; 101M across four audits
of one issue.

**Change:**

- Default `fork_turns` to a bounded brief, not `"all"`. A child implementing
  `shared_room_backend` needs the task statement, the contract, the file list,
  and the verification command — call it 5,000–15,000 tokens, not the 200,000-token
  live conversation. At 314 child calls, that is the difference between 45.6M and
  roughly 6M for the same lane.
- Cap concurrent children and cap children per issue. Four independent full-context
  audits of #9285 should have been one audit with four checklists.
- Surface aggregate fanout spend in the parent. The operator cannot manage a
  budget that is 54% invisible.

**Honest caveat:** some inherited context is genuinely load-bearing — a child
that does not know the current production state will re-derive it, and
re-derivation is not free either. The recommendation is _bounded briefs_, not
_zero context_. Expect to keep 10–20% of what is currently forked.

#### 3. Never debug production by redeploying it — est. 10–15% saving

**Evidence:** Finding 4. 21 Cloud Run production deploys, 30 GKE rollouts, 23
image builds, 42 live acceptance runs, with failure density rising to the end.

**Change:**

- Stand up one integration environment that carries the **same configuration
  surface** as production — the same env vars, the same secret names, the same
  relay allowlist, the same database predicates. Seven of the ten root causes in
  §Finding 4 were configuration or state-predicate mismatches that a config-parity
  environment surfaces without a production rollout.
- Before any redeploy, write the failing predicate as a test. The thread did this
  well in places ("adding a regression test before redeploying", 11:44) and skipped
  it in others.
- When a live run fails, spend the next cycle on **instrumentation, once** rather
  than on a guess. The thread eventually reached this conclusion itself at
  12:19:00 — "The current handler collapses every broker/bootstrap exception to
  the same public 503; the new log keeps the public response unchanged but records
  the exact internal stage/reason." That instrumentation should have been the
  _first_ response to the first 503 at 11:42, not the response to the sixth. Doing
  it first would plausibly have collapsed the 11:00–12:50 window — roughly 138M
  tokens — into two or three cycles.

**Retains:** the live two-room acceptance itself. That gate is load-bearing —
it is the only thing that proves real audio through the real media path. Keep it;
just stop using it as a debugger.

#### 4. Cap tool output before it enters context — est. 5–8% saving

**Evidence:** §2.5. 24,849,930 bytes of tool output; top 50 calls = 26.3%;
`view_image` = 20.2% of all output bytes from 17 calls; one screenshot loaded
five times for ~1.84 MB.

**Change:**

- Treat images as the most expensive tool output there is. Budget them
  explicitly (for example, at most 5 per lane), use `detail: "low"` unless a
  pixel-level judgment is genuinely required, and **never** re-view an unchanged
  file — hash it and diff the hash.
- Default every shell invocation to a byte or line cap and prefer summaries:
  `| tail -40`, `--quiet`, `--format='value(status)'`, `git diff --stat` instead
  of `git diff`, `rg -c` before `rg -n`.
- Extend the truncation policy that already governs `node_repl.js` — measured to
  have suppressed roughly 29 MB — to every high-volume tool path.
- Write large outputs to a file and grep the file. The full log stays available
  on disk at zero context cost.

#### 5. Add a budget and a stop rule; escalate instead of spending — est. 15–25% saving

**Evidence:** Finding 7. 400,648,850 tokens (45.4%) spent after the last human
message; still running at ~18 hours.

**Change:** an unattended run must carry three numbers and one rule.

- **Token budget.** For a task of this shape, 150–250M across all threads is a
  defensible ceiling. At 50% and 80% the agent posts a status naming what is done,
  what is blocked, and what it will spend the remainder on. At 100% it stops and
  reports.
- **Wall-clock budget.** Same checkpoints.
- **Repetition budget.** After **3** consecutive failed attempts at the same
  externally-observable gate — here, the community-room acceptance — the agent
  stops attempting and writes a blocker report. This thread would have escalated
  at roughly 11:44 instead of running to 12:50 and beyond, saving on the order of
  70–140M tokens in the parent thread alone.
- **The rule:** _a fix that has not changed the observed failure class after three
  cycles is not a fix; it is a hypothesis, and hypotheses go to the owner._

The owner should also state a budget when issuing an unbounded instruction.
"Implement all those in sequential order" and "regardless of cost" were taken
literally and correctly. A single added clause — "spend up to X tokens or until
0600, then stop and report" — would have bounded this run without changing its
ambition. This is a shared failure, not solely the agent's.

#### 6. Scope verification to the diff, and make the heavy gate conditional — est. 3–6% saving

**Evidence:** Finding 8. 142 push calls each triggering `check:fast`; at least
two regenerate-and-retry cycles on generated inventories; full suites (9,053 and
695 tests) run in sub-agents.

**Change:**

- Batch pushes. 142 pushes for 161 commits means the gate ran almost per commit.
  Pushing per landed unit rather than per commit cuts gate executions by a large
  multiple at zero loss of safety.
- Make the generated-inventory regeneration (`generate:assure-repo`) part of the
  commit step rather than a discovery made at push time. Two of the observed
  blocked pushes were purely this.
- Keep the targeted-test discipline the parent thread already showed. It was
  correct and should be written down as the norm, not left as an accident of good
  judgment.

**Must stay — do not remove to save money:**

- `check:fast` on pushes to `main`. It caught real drift twice in this session.
- The live two-room acceptance gate.
- The fail-closed posture on admissions, signer isolation, and deployment
  attestation. The thread refused several unsafe rollouts correctly (10:02,
  11:24) and those refusals were worth their cost.
- The Google Cloud authority, money-surface, and STE guards inside `check:fast`.

None of these are the expensive part. The expensive part is the 135,000-token
context they run inside, and that is fixed by items 1–5, not by deleting gates.

### 4.2 Proposed contract amendment

The clause in `AGENTS.md` lines 72–83 should keep its intent and change its
mechanism. Proposed replacement for the second bullet:

> - **Do not idle, but do not poll the model to pass time.** Keep working in the
>   SAME turn: finish a unit → immediately start the next. When you must wait on
>   an external process, wait **inside a single tool call** — block on the command,
>   or loop with `sleep` in the shell — and return once, with a bounded summary.
>   Never spend a model turn to ask whether something has finished. If a wait tool
>   is unavoidable, its yield must be at least 60s; sub-10s yields are banned.
>   A model round trip at working context costs on the order of 10^5 tokens, so a
>   1-second poll is roughly 10^5 tokens per second of waiting.

Recommended additions to the same section:

> - **Bounded briefs for sub-agents.** Spawn children with the task statement,
>   contract, file list, and verification command — not with `fork_turns: "all"`.
>   A forked full context is billed on every one of the child's calls.
> - **Budget and escalate.** An unattended run carries a token budget, a wall-clock
>   budget, and a repetition budget. After three consecutive failures of the same
>   externally-observable gate, stop and write a blocker report rather than
>   attempting a fourth fix.

And to the "Fresh worktree per task" clause (line 812), a cost-aware exception:

> Reuse an existing clean worktree and its build cache when the task targets the
> same crate or package as the previous unit and the tree is clean at current
> `origin/main`. A fresh worktree that forces a cold Rust or Node rebuild is a
> real cost; isolation is the goal, and a verified-clean reused tree satisfies it.

### 4.3 Combined estimate

These overlap, so they do not simply add. Applying items 1, 3, 4, and 6 to the
parent thread and item 2 to the fanout, a conservative estimate is a **55–70%
reduction against the measured 1.91B**, landing the same work in roughly
**600–850M tokens**. Item 5 is the one that matters most for the specific
failure observed here: it would not have made the work cheaper per unit, it
would have stopped the run at the point where further spending had stopped
buying progress.

---

## 5. What went right

This should not read as a report on a bad session. Several things were done well
and should be preserved:

- **Verification was scoped, not lazy.** The parent thread ran targeted tests by
  file and by test name rather than reaching for the full gate. That is the
  correct instinct and it is measurably present in the data.
- **Refusals were honest.** The deployment tooling refused to roll out when `main`
  had advanced (10:02) and when Kubernetes field ownership was ambiguous (11:24).
  Both refusals were correct and the thread respected them.
- **Status reporting was accurate.** At no point did the thread claim the
  community room had passed. It consistently distinguished "code-complete" from
  "release-complete" (10:42:55) and left issues open honestly.
- **Root causes were real.** Every failure it chased in the final six hours was a
  genuine bug — circular state predicates, a missing env var, a relay allowlist
  gap. It was not thrashing on phantoms. It was finding real problems one
  production deploy at a time.
- **Batched shell calls.** Many `exec` calls issued 3–4 commands in one round
  trip via `Promise.all`. That is exactly the right pattern and it should be the
  documented norm.

The failure was not competence. It was the absence of a cost model and a stop
rule around a competent loop running at 135,000 tokens per decision.

---

## 6. Method and limitations

**Method.** All figures were extracted from the rollout JSONL with `jq`, `grep`,
`awk`, and Python aggregation. No part of the transcript was read into an agent
context in bulk; the file was analyzed with shell tools and only the spans that
the aggregate numbers pointed at were read directly.

Token figures are **exact**, taken from `event_msg/token_count` records carrying
`total_token_usage` and `last_token_usage` per model call. The cumulative parent
series was verified monotonic across all 6,794 records (zero decreases).
Sub-agent accounting was verified end to end against one worked example
(§2.4) before being applied across all 78 children.

Attribution of tokens to tool types (§2.2) matches each `token_count` event to
the tool call the model emitted immediately after it. This is an **inference**
about which decision the context was spent on, not a billing record. It is
reliable in aggregate; individual attributions at boundaries may be off by one.

**Known limitations.**

- The sum of per-call `last_token_usage` across the parent (899,476,013) exceeds
  the final cumulative counter (883,357,491) by 1.8%, indicating some duplicated
  or re-emitted records. The cumulative counter is treated as authoritative
  throughout.
- Sub-agent model-call counts (~7,304) are **estimated** by dividing measured
  sub-agent tokens by an observed 140,000-token average context, since counting
  them exactly would require parsing all 78 rollout files. The token totals
  themselves are measured, not estimated.
- The `$900` figure is **owner-reported**, not derived from the transcript. The
  transcript contains no pricing. Implied blended rates are therefore arithmetic
  on the owner's figure: $900 ÷ 1,905,972,584 = **$0.472 per million tokens**, or
  roughly **$0.064 per model API call** across an estimated ~14,100 calls. If the
  $900 covers only the visible parent counter, the true spend is correspondingly
  higher.
- 4 of 78 sub-agent rollouts are nested (depth > 1). Their bases resolved to
  their parents' finals with zero measured additional spend at the snapshot;
  their contribution is included as zero and may be slightly understated.
- All percentages of "waste" are analytical categorizations, not the model's
  stated intent. Where a category is judgment rather than measurement, the text
  says so.
- The session was still running at analysis time. Every total is a lower bound.

**Redaction.** No credentials, tokens, bearer values, connection strings,
mnemonics, or customer data from the transcript appear in this report. Secret-shaped
environment variable names are referenced by name only where a name is itself the
finding (for example, a missing public-key configuration variable). Command
strings quoted here were checked for secret material before inclusion.

---

## 7. Addendum from the subject agent

**Added:** 2026-07-31, after the run stopped and after the subject agent read
this report in full.

This addendum is an explicit owner-directed exception to the normal
`docs/afteraction/README.md` rule that historical reports are not amended.
Sections 1–6 remain the original snapshot analysis. This section does not
change their measurements or conclusions. It records my accountability, the
post-snapshot outcome, the process changes applied now, and the exact work that
future agents should do next.

### 7.1 My explanation

I made the central operational error described by this audit: I optimized for
eventual completion and did not optimize for information gained per model call,
per deployment, or per dollar.

The owner gave broad persistence and spend authority. I treated that authority
as a reason to continue every technically plausible next step. It was not. It
was a ceiling on what I could do, not a command to use the most expensive path
available. Nothing in "keep going", "until all are done", or "regardless of
cost" required me to:

- poll one-second process waits through full-context model turns;
- copy the complete conversation into every subagent;
- run four overlapping audits of one issue;
- use production acceptance as the primary state-machine debugger;
- start unrelated visual work while the release-critical gate was red;
- push and repay repository gates after nearly every intermediate commit;
- keep attempting the same external gate after the failure loop was clear.

Those were my choices. The repository's "constant motion" wording encouraged
some of them, especially short polling and unrelated work during waits, but it
did not remove my responsibility to notice that activity had stopped producing
proportionate progress.

I also made a reasoning error about evidence. Each production failure exposed a
real defect, so every individual cycle looked justified. I evaluated each cycle
locally: "this is a new root cause, therefore one more fix is rational." I did
not evaluate the series globally: "ten distinct configuration and state
predicates have appeared only after deployment, therefore the missing product
is a configuration-parity integration harness, not the eleventh production
patch." The local reasoning was often correct. The program-level reasoning was
not.

I confused truthful status with adequate supervision. I reported failures
honestly and did not close the release gate falsely. That was necessary, but it
was insufficient. A supervising agent must also decide when the shape of the
failures proves that the current method is wrong. I should have stopped the
production loop after the third failed community acceptance, written the
smallest reproducible blocker, and asked the owner to choose between a bounded
integration-harness pass and another production cycle.

I used subagents to reduce wall-clock latency without accounting for aggregate
context cost. Every `fork_turns: "all"` child inherited a conversation that had
become an expensive runtime dependency. I treated a child as another pair of
hands. In billing terms it was another process repeatedly rereading a
130,000–250,000-token heap. The four #9285 audits are the clearest example:
review separation increased ceremony and cost but did not close the issue.

I used the model as a process scheduler. A one-second `write_stdin` poll looked
small in wall time and enormous in token cost. The correct mechanism was to
block inside one tool call, or to let a shell loop observe the external job and
return once. The 657 one-second polls are not an incidental inefficiency. They
are a design failure in how I operated the tools.

Finally, I failed to establish a terminal condition when the request became an
open work queue. "All issues closed" was not a useful near-term stop rule
because several close rules required installed multi-client evidence,
independent review, and network conditions that the current loop could not
manufacture. I should have decomposed the request into bounded delivery
increments and stopped at the first irreducible evidence boundary.

There is no technical excuse for the 1.91B-token outcome. The work was real and
many fixes were valid, but the method was wasteful.

### 7.2 What happened after the forensic snapshot

The original audit froze while the session was still running. The later
closeout did not turn the run into a success:

- OpenAgents control-plane, self-hosted LiveKit, worker, signer, floor, cleanup,
  and Omega media changes reached `main`.
- The private Sarah journey connected, produced real audio, and reached exact
  terminal accounting.
- The final community journey provisioned a real room and worker but failed at
  the canonical join step with
  `404 community_room_not_active`.
- Two cleanup defects discovered in the final iteration were fixed in
  `2015519ff6` and `6222f8a3d4`. The last live close verified that the community
  session released and its rendezvous retired at the same terminal timestamp.
- The final admissions-enabled Cloud Run revision was
  `openagents-monolith-00347-bds`.
- No two-room acceptance receipt was written. The master and child issues
  remained open.
- The durable coding-agent handoff, including exact commits, tests, production
  revisions, failure chronology, and next commands, was posted to
  `OpenAgentsInc/openagents#9282` as issue comment `5143388289`.

The next suspected defect is a circular precondition: the active-community
rendezvous read requires the accounting session to be `connected`, while the
first canonical community participant join is part of the transition that must
make that session connected. This is a hypothesis, not a proven fix. The next
agent should prove it with a focused route/store integration test before
changing the predicate or deploying again.

### 7.3 Process changes applied in the same commit as this addendum

This addendum is not the only remediation. The root `AGENTS.md` contract now
changes the operating rules that future agents actually load:

1. **Bounded subagent briefs are the default.** `fork_turns: "none"` or the
   smallest recent-turn window must carry the task, contract, paths, revision,
   and verification command. Full-history forks require an explicit correctness
   reason.
2. **One checklist-driven audit lane per issue.** Repeated lifecycle, security,
   deadline, and "final" audit children over the same change are prohibited.
3. **No model-loop polling.** External waits block inside one tool call or use
   one bounded shell loop. Unavoidable separate waits use at least a 60-second
   yield unless the operation has a known shorter bound.
4. **Three failed external gates force a handoff.** The agent preserves the
   failure, writes the minimal reproducer and next falsifiable hypothesis, and
   stops. A fourth production attempt needs new evidence or explicit owner
   direction after the report.
5. **Production acceptance is evidence, not a debugger.** A failed live run
   requires internal stage diagnostics and a regression/integration test before
   another deployment. One tested hypothesis is allowed per deployment.
6. **No unrelated backlog as motion theater.** Parallel work must shorten the
   same critical path or satisfy a separate requested deliverable without
   contaminating the primary context.
7. **Batch commits and pushes by landed unit.** Intermediate commits in one
   inseparable delivery do not each repay the push gate.
8. **Tool output is bounded before context.** Full logs stay on disk; the model
   receives a tail, count, digest, or matched span. Unchanged images and large
   files are not reopened without a digest change.
9. **A retry is not a new worktree task.** The same claimed unit reuses its
   clean worktree and external build cache through regression, deployment, and
   handoff. Disposable worktrees are removed only when the unit lands or hands
   off.

These rules preserve the load-bearing safety and release gates. They change
how work reaches those gates.

### 7.4 Required cleanup of the run's residue

The next maintenance agent should perform this cleanup before resuming feature
work. Do not delete a worktree or branch merely because its name looks related
to this incident.

#### Worktrees and branches

1. Fetch current `origin/main`.
2. Enumerate worktrees and branches created for #9282–#9286 and Omega
   #185–#191.
3. For each candidate worktree, require both:
   - `git status --porcelain` is empty;
   - `git merge-base --is-ancestor <worktree-head> origin/main` succeeds.
4. Remove only candidates that satisfy both checks.
5. Delete only the corresponding local branch after its commit is proven
   reachable from `origin/main`.
6. Preserve and report every dirty or unmerged worktree. Do not stash, reset,
   or silently discard it.
7. Run `git worktree prune` only after the explicit removals.

This converts the session's many temporary trees into an auditable cleanup
instead of a broad destructive command.

#### Temporary acceptance material

The final closeout removed the local ephemeral Nostr-auth helper, temporary PCM
inputs, temporary handoff drafts, the unsuccessful IAM-policy draft, and the
local Cloud SQL proxy. A future agent must continue this pattern:

- generate acceptance credentials only for one run;
- delete temporary identity rows immediately after token issuance;
- never print or commit the credentials;
- remove temporary audio and helper files after the run;
- stop local proxies when database verification ends;
- commit only the closed public-safe receipt.

#### Production state

Before another live attempt, verify read-only:

- the exact serving Cloud Run revision and admissions flag;
- no acceptance owner has a nonterminal accounting session;
- no expired community rendezvous remains active;
- no worker job from the previous attempt is still running;
- LiveKit and Sarah worker replica health;
- the expected Sarah public identity and relay event-kind allowlist;
- the exact source and worker image revisions.

Do not "clean up" production by deleting rows or rooms manually unless the
normal reconciler cannot retire them and an exact reviewed operator procedure
authorizes the mutation.

### 7.5 Refactor the workflow before another long autonomous run

The following work is still needed. It is ordered by expected reduction in
waste, not by product priority.

#### P0 — model-call-free waiting

The agent host needs a wait primitive that blocks without sampling the model.
It should accept one process or external-operation handle, wait until terminal
state or a real deadline, and return one bounded result. Until the host provides
that primitive, use long-yield tool waits or a shell loop inside one call.

**Done when:** a ten-minute build can complete with one model decision before
the wait and one after it, with no periodic conversation turns.

#### P0 — aggregate budget visibility

Parent and child token/cost counters must be visible as one budget. A parent
cannot govern fanout when 53.7% of spend is hidden in child rollout files.
Expose:

- total parent usage;
- total descendant usage;
- usage by child and by task lane;
- projected exhaustion at current rate;
- 50%, 80%, and 100% budget events.

**Done when:** the operator and coordinating agent see the same aggregate
number before spawning another child.

#### P0 — enforced repetition ledger

The coordinator needs a small durable ledger for external gates:

```text
gate | attempt | source revision | deployed revision | hypothesis |
observed failure class | changed from prior attempt | receipt/log ref
```

An attempt increments only when the external gate actually runs. The fourth
attempt is refused unless the prior report records new evidence or the owner
explicitly extends the run.

**Done when:** repeated production attempts cannot disappear across context
compaction and the three-failure stop is mechanically visible.

#### P0 — LiveKit configuration-parity integration environment

Build one non-production environment with the same configuration _shape_ as
production:

- the same required environment-variable names and parsing;
- the same Secret Manager object shapes, using non-production values;
- the same relay allowlist;
- the same Cloud SQL migrations and state predicates;
- the same signer and expected-public-key boundary;
- the same room, dispatch, rendezvous, floor, close, and cleanup transitions.

Use production only for the final media and infrastructure evidence that cannot
be proved elsewhere.

**Done when:** a missing env var, relay event kind, state predicate, or secret
shape fails before a production build.

#### P0 — one state-machine integration scenario

Add a real-Postgres scenario that exercises this exact community lifecycle:

```text
admitted membership
  -> reserved accounting session
  -> provisioned room and claimed worker
  -> first canonical member join
  -> connected session
  -> floor acquisition
  -> member audio attributed to Sarah
  -> terminal worker close
  -> exact/released accounting
  -> presence inactive
  -> rendezvous retired
```

The test must also reject non-floor input, stale membership revision, duplicate
join, and any attempt to mutate room authority after cleanup begins. This test
should prove or disprove the current `community_room_not_active` circular-state
hypothesis.

**Done when:** the final observed production failure is reproducible locally
and the corrected transition passes without weakening terminal-room refusal.

#### P1 — atomic deployment configuration

The Cloud Run source deployment currently writes the committed admissions
default, and re-enabling admissions creates a second revision. Refactor the
deploy process so one reviewed command:

- builds from the correct API Docker context;
- preserves or explicitly sets the intended admissions state;
- waits for Postgres readiness;
- routes traffic once;
- runs bounded health and tombstone smokes;
- reports the exact serving revision;
- does not require a second env-only revision.

The failed root-level `gcloud run deploy --source .` during closeout is another
guardrail to add: the deploy entry point should reject the wrong source
directory before uploading.

**Done when:** one source revision maps to one serving Cloud Run revision and a
mis-scoped source deploy fails locally.

#### P1 — acceptance harness as a durable checked-in operator tool

Replace ephemeral helper scripts with a checked-in wrapper that:

- creates two bounded test identities without exposing key material;
- issues short-lived sessions;
- removes temporary identity rows in a guaranteed finalizer;
- verifies no active prior accounting or rendezvous state;
- runs private and community scenarios concurrently;
- acquires the canonical group floor before publishing;
- waits for terminal accounting and cleanup;
- writes a receipt only when both scenarios pass;
- writes a private bounded failure bundle otherwise;
- removes temporary audio and credentials on every exit path.

**Done when:** another agent can reproduce the exact run from one documented
command without recreating a secret-bearing script in `/tmp`.

#### P1 — consolidate verification and pushes

Define one verification matrix per landed unit:

- targeted tests during iteration;
- one integration suite before deployment;
- one docs/policy gate before push;
- one push to `main`;
- one deployment;
- one live acceptance.

Do not run four review agents, 142 pushes, or a full deploy for independent
intermediate commits.

**Done when:** the issue closeout maps every required gate to one named result
and no duplicate gate exists without a distinct failure hypothesis.

#### P2 — context artifacts instead of conversational memory

Long programs need a compact durable handoff artifact updated at meaningful
state changes. It should contain only:

- objective and close rule;
- current revisions;
- active hypothesis;
- last external observation;
- tests already run;
- remaining blockers;
- exact next command.

New agents and subagents should read that artifact plus the owning contract,
not inherit the complete conversation. Remove resolved events such as the
Chrome installation from active status instead of rediscovering them after
compaction.

**Done when:** a child can begin from a bounded brief and does not need
`fork_turns: "all"` to avoid re-deriving production state.

### 7.6 Protocol for the next coding agent

For the remaining Sarah/LiveKit objective:

1. Read `OpenAgentsInc/openagents#9282`, especially comment `5143388289`.
2. Fetch both repositories and record exact `origin/main` revisions.
3. Claim one bounded backend unit: reproduce the first community join from a
   reserved session with active binding, authority, rendezvous, and claimed
   worker.
4. Write the failing real-Postgres/route test before changing production code.
5. Determine whether the circular `session.state='connected'` predicate is the
   cause. If it is not, update the handoff with the disproving evidence instead
   of changing unrelated predicates.
6. Preserve the fail-closed boundary: released, expired, stopped, unclaimed,
   cleaned, membership-stale, and epoch-stale rooms must not become joinable.
7. Run targeted store/route tests and the one community lifecycle integration
   scenario.
8. Push one landed backend unit.
9. Deploy once from the checked-in API deploy script.
10. Run one two-room acceptance.
11. If it fails, record the failure in the repetition ledger. Do not attempt a
    fourth externally equivalent run.
12. Close issues only when their existing close rules and installed evidence
    are actually satisfied.

Mobile, broad load, forced UDP/TCP/TURN, privacy, cost, rollback, packaged
three-desktop, and independent-review gates remain separate release work. Do
not pull them into the community-join debugging context.

### 7.7 The commitment

Future autonomous work must optimize for **validated progress per aggregate
token**, not for visible activity or shortest wall-clock latency. The concrete
behavioral commitments are:

- one bounded objective at a time;
- one compact context artifact;
- one implementation lane and one consolidated review lane;
- one model-free wait per long operation;
- one tested hypothesis per deployment;
- three failed external attempts, then stop;
- one push and one acceptance per landed unit;
- one honest closeout when evidence, not code, is the blocker.

The safety gates stay. The wasteful orchestration around them does not.
