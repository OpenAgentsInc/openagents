Here’s what Pylon v0.1 (“Testnet Alpha”) should **aim to achieve tomorrow**: prove the market loop works end-to-end with the smallest surface area, and generate **credible evidence** you can show developers + early buyers next week when you flip to mainnet.

## The 3 outcomes that matter

### 1) Prove **the loop**

**Buyer posts job → Providers pick up → Execute → Return result → Buyer pays (testnet) → Receipt logged → Stats update.**

If you can do that reliably, you have a marketplace. Everything else is garnish.

**Success criteria**

* ≥ 20 distinct providers complete ≥ 1 job
* ≥ 200 jobs completed total (even if tiny)
* Median job end-to-end latency < 30s for “small jobs” (you can pick the job types to make this true)
* ≥ 95% job success rate *for the “happy path”* (timeouts / provider churn are expected—just measure it)

### 2) Prove **trust primitives**

Tomorrow is not about “decentralization vibes.” It’s: can a buyer trust the system enough to send real money next week?

**Minimum trust story (alpha-grade)**

* Job envelopes are signed (buyer)
* Provider identity is consistent (pubkey)
* Results are signed (provider)
* Payment is linked to job id deterministically
* Receipts are queryable (by job id, provider pubkey)

Even if verification is weak at first, you want **auditability**.

**Success criteria**

* For every completed job: `job_id → request_event → result_event → payment_receipt_event`
* One command that reconstructs a job’s provenance trail

### 3) Prove **a first real workload pattern** (RLM-shaped)

You don’t need “Recursive Language Models” fully implemented tomorrow. You need a **compelling baby step** that clearly maps to RLM fan-out.

Think: “one smart coordinator” + “many tiny subcalls.”

**Success criteria**

* One demo pipeline that fans out 20–200 subtasks and fans back in
* The subtasks are small enough that lots of random Macs can do them
* The aggregation step produces something visibly useful

If you get *that* working, your narrative snaps into focus.

---

## What the MVP should include (and exclude)

### Must-have (v0.1)

**Node (provider)**

* `pylon node up` (connects to your relay, announces capabilities, starts polling/subscribing)
* Executes **one** job type initially (keep it narrow)
* Reports: start/stop, heartbeat, queue state, job timings, failures
* Produces signed results + minimal receipt metadata

**Buyer CLI**

* `pylon job submit <type> --input ...`
* `pylon job status <job_id>`
* `pylon job results <job_id>`
* `pylon pay --job_id <id>` (testnet)

**Stats**

* Global: online nodes, jobs/min, success rate, p50/p95 latency, top providers by completed jobs
* Per provider: uptime, completed, failure reasons, median runtime
* Show this somewhere dead simple (terminal dashboard or a tiny web page)

**Nostr**

* Define 3–5 event kinds (or tags) and stick to them.
* Everything is append-only, queryable, replayable.

### Explicitly NOT in v0.1

* Multi-job-type marketplace
* Sophisticated verification / fraud prevention
* Fancy UI
* Multi-relay routing
* “General compute” (start with one job format)

---

## The first “RLM-ish” use case to aim for tomorrow

Pick something that is:

* embarrassingly parallel
* cheap to run on CPU/Apple NPU
* yields a satisfying aggregated artifact

### Option A (best for launch): **Repo Map / Codebase Index Fan-out**

**Goal:** produce a structured “repo map” + retrieval index.

Flow:

1. Buyer submits: `(repo_url OR local snapshot) + instructions`
2. Coordinator splits into per-file tasks:

   * summarize file in 10–20 lines
   * extract exports/symbols
   * list dependencies/imports
3. Providers run on small local model (Apple FM / MLX) or even deterministic parsing.
4. Aggregator builds:

   * `repo_map.json` (symbols → files)
   * `overview.md` (architectural summary)
   * optional embeddings index

Why it’s perfect:

* It looks like agentic code search.
* It’s directly relevant to your “coding agents from phone” story.
* It’s clearly a building block toward RLM: “treat prompt as environment,” fetch snippets, recurse.

### Option B: **Document “RLM environment loader”**

Input: huge doc (100k–5M tokens). Fan-out:

* chunk summarize
* extract entities/facts
* answer a set of queries by retrieving only relevant chunks

This is closer to the RLM paper vibe, but harder to make visceral.

### Option C: **Testgen / lint pass**

Split by file; generate tests/lints suggestions; aggregate PR suggestions.
More complex and riskier for day 1.

**Recommendation for tomorrow:** Option A.

---

## Launch-day targets (numbers to shoot for)

You want numbers that make a good screenshot and validate feasibility.

**Supply**

* 50+ nodes online peak (even if many are flaky)
* 20+ “reliable” nodes (≥ 10 jobs completed each)

**Demand**

* 200–1,000 total microjobs completed (make them small)
* 1–3 “hero runs” that look like magic (e.g., repo map completed in 2 minutes)

**Performance**

* p50 job runtime: < 5s (tiny tasks)
* p95 end-to-end latency: < 45s (including pickup + execution + result publish)

**Reliability**

* < 10% hard failures on the happy path
* clear failure taxonomy (timeout, provider offline, exec error, model missing)

---

## What to instrument (so next week’s mainnet flip is sane)

Minimum telemetry fields per job:

* `job_id`
* `buyer_pubkey`, `provider_pubkey`
* timestamps: submitted, picked_up, started, finished, published
* durations: queue_wait, exec_time, publish_time
* bytes in/out
* result hash
* error code enum

Then you can produce:

* throughput charts
* reliability stats
* “top providers”
* and—most importantly—pricing heuristics for mainnet.

---

## Mainnet readiness checklist (what tomorrow should de-risk)

You flip to live BTC next week when:

* You can deterministically link payment ↔ job_id ↔ provider pubkey
* You have a refund path (even manual) for failed jobs
* You can rate-limit buyers (to avoid griefing providers)
* You have minimal anti-spam on relay (or whitelisting for alpha)
* You have one “golden path” use case that works 10 times in a row on camera

---

## What you should **aim to ship tomorrow** (concrete)

1. **Provider node** that can do one job type reliably (repo-file summary/extract).
2. **Buyer CLI** that can submit a repo-map run and watch it complete.
3. **Stats dashboard** with 5–8 core metrics.
4. **One hero demo script**: “spin up 30 nodes → submit job → watch map build → show artifact.”

If you hit those, “Testnet Alpha” is a win even if half the edge cases are on fire.

---

## The story you want the launch to prove

Not “we have a compute marketplace.”

Tomorrow should prove:
**“A swarm of random Macs can perform useful agent work as microjobs, coordinated over Nostr, with auditable receipts—and this structure naturally unlocks RLM-style fan-out.”**

That’s what makes next week’s “now we pay real sats” credible.

If you want, I can also give you:

* the exact event schema (kinds/tags) for `JOB_REQUEST`, `JOB_CLAIM`, `JOB_RESULT`, `PAYMENT_RECEIPT`, `NODE_ANNOUNCE`
* a day-1 pricing heuristic for mainnet (flat per job, per second, or per token proxy)
* and a launch runbook (what to monitor minute-by-minute, and what to kill-switch).
