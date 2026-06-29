# Khala Liveness Heartbeat — Runbook

Updated: 2026-06-26

> **Status:** operational runbook, honest-scope. Documents the scheduled liveness
> heartbeat that fires ~50k tokens across diverse configs at the **live production**
> Khala endpoint every 15 minutes, verifies completion success, checks public
> counter health with internal dogfood included in the aggregate, and writes
> a public-safe status. For future agents/operators. Not a product promise;
> heartbeat tokens are **internal dogfood** (not external demand) and are now
> **demand-tagged `internal`** at the source (#6298), so the public counter can
> count all real served tokens while the captured trace corpus does not mistake
> them for external real-user traffic. See
> **Demand-origin self-tag** below.

## What it is

`scripts/khala-heartbeat.sh` is a self-contained bash heartbeat. Each run:

1. Picks a bearer key from a rotating pool (gitignored secrets file).
2. Reads the public tokens-served counter (`GET /api/public/khala-tokens-served`).
3. Fires a battery of **diverse configs** against `POST /api/v1/chat/completions`
   (`model: openagents/khala`):
   - **short** (max_tokens 64, temp 0.2, non-streaming)
   - **medium** (max_tokens 512, temp 0.7, non-streaming)
   - **content-array** — OpenCode-style `content: [{type:"text",…}]` shape (exercises
     the tool-compat path that was the first OpenCode blocker)
   - **streaming** (SSE, max_tokens 400 — verifies the stream path returns chunks)
   - **long-gen burst, looped to a token target** — waves of `KHALA_HEARTBEAT_LONG_CONC`
     (default 6) concurrent max_tokens 8000 requests, repeated until
     `KHALA_HEARTBEAT_TOKEN_TARGET` (default 50000) tokens are served (capped at
     `KHALA_HEARTBEAT_MAX_WAVES`, default 15). The model stops ~1.5k tokens/request
     well under max_tokens, so the loop is what reliably reaches ~50k (typically
     ~30 long-gens, ~63k served). Also the load probe (6-wide concurrency).
4. Re-reads the counter and verifies it is readable and did not move backward.
   The public counter is all-demand and counts `internal` rows; the heartbeat
   still treats movement as a health signal rather than an external-demand
   claim. To require positive movement as an explicit proof, set
   `KHALA_HEARTBEAT_EXPECT_PUBLIC_COUNTER=1`.

**Why ~50k in diverse configs:** a single ping proves "the box answers"; ~50k across
short/long/streaming/content-array/concurrent proves the **real** paths customers use
(tool-calling shapes, streaming, long decode, concurrency) and that terminal
usage frames are non-empty. Exact accounting is verified from `token_usage_events`
for internal probes, and the public counter is the aggregate all-demand
served-token projection, not an external-market-demand claim.

## Demand-origin self-tag (#6298) — keep internal traffic out of the corpus

Default-on free-tier trace capture (#6293) records completed `openagents/khala`
sessions for the data-market / training corpus. Our own dogfood — the heartbeat,
the 500 canary, and the Harbor/Terminal-Bench run — would otherwise land in that
corpus mixed in with (and often outnumbering) genuine external free-tier users.

To keep the corpus clean **with no manual curation**, every internal caller
**self-tags its demand origin** with two request headers. The chat path resolves
the SAME attribution it already writes to the token ledger
(`token_usage_events`, migration 0232) and threads it onto the captured trace
(`agent_traces.demand_kind` / `demand_source`, migration 0236), so the trace and
the ledger always agree, and the corpus read excludes internal-dogfood by
default.

- `x-openagents-demand-kind`: one of `external | internal | internal_stress |
  own_capacity | unlabeled`. Internal dogfood callers send `internal`.
- `x-openagents-demand-source`: a bounded attribution token (a short
  `[A-Za-z0-9_.:-]` slug), the canonical name of the internal source.

Canonical internal sources (send these, exactly):

| Internal caller | `x-openagents-demand-kind` | `x-openagents-demand-source` |
| --- | --- | --- |
| `scripts/khala-heartbeat.sh` | `internal` | `heartbeat` |
| `scripts/khala-canary.sh` | `internal` | `canary` |
| Harbor / Terminal-Bench run | `internal` | `harbor_terminal_bench` |

Both scripts add these headers to their shared `AUTH` curl array, so **every**
completion they issue is tagged. Anything missing/unparseable defaults to
`unlabeled` (fail-soft): tagging never breaks a completion or a capture. Public
Khala token-counter projections count every real demand kind, including
`internal`, `internal_stress`, `own_capacity`, `external`, and unlabeled. The
trace corpus read is the path that excludes internal dogfood by default.

The Harbor/Terminal-Bench path tags itself through the Terminus agent's
extra-headers. The progress pusher
(`apps/openagents.com/scripts/gym-harbor-progress-push.ts`) is metadata-only and
issues no completions; it is the Harbor **agent's** `openagents/khala` calls that
must carry the headers, via the Terminus `llm_call_kwargs` extra-headers:

```sh
harbor run \
  --agent terminus-2 \
  --model openai/openagents/khala \
  --agent-kwarg api_base=https://openagents.com/api/v1 \
  --agent-kwarg 'llm_call_kwargs={"extra_headers":{"x-openagents-demand-kind":"internal","x-openagents-demand-source":"harbor_terminal_bench"}}'
```

(See `docs/gym/2026-06-25-khala-terminal-bench-through-openagents-run.md` for the
full Harbor invocation.) **Future internal load must self-identify the same
way:** pick a stable `internal` source slug and send both headers so it never
pollutes the external corpus.

Corpus read: `GET /api/traces` (owner session) excludes internal-dogfood
(`internal` + `own_capacity`) by default. `?demand_kind=internal` (repeatable /
comma-separated) filters to named kinds; `?demand_kind=all` returns every kind.
The response also carries `demandSegments` — a `{external, internal,
own_capacity, unlabeled}` count over all of the owner's traces.

## Verdict / exit codes

- `0` **ok** — configs returned success with non-empty usage/chunks, and the
  public counter endpoint was readable and monotonic. `status.json` includes
  `publicCounterCheck:"observed"` when movement is seen, or `"required"` when
  `KHALA_HEARTBEAT_EXPECT_PUBLIC_COUNTER=1`.
- `2` **degraded** — everything that ran was quota-limited (`402`/`429`): the endpoint
  is alive but the key is tapped out for the UTC day (rotate / add keys).
- `1` **down** — a hard failure on a config, an HTTP 200 with empty usage,
  unreadable public counter endpoint, backward public counter movement, no keys
  configured, or (only when public-counter movement is explicitly required)
  no public counter movement after served tokens. Writes to `FAILURES.log`.

## Files (all under `~/work/.khala-heartbeat/`, gitignored)

- `heartbeat.jsonl` — append-only log, one public-safe JSON line per run (ts, state,
  ok/fail/quota counts, summed tokens, counter before/after/delta, counter-check
  mode, demand kind, per-config http).
- `status.json` — the latest run only (what to check first).
- `FAILURES.log` — only failure/degraded notes (tail this when triaging).
- `.keystate` — round-robin key index.
- `launchd.out` / `launchd.err` — scheduler stdout/stderr.

**No prompts, completions, or keys ever land in any file.**

## Keys & quota math (important)

Heartbeat keys are **free-tier** `oa_agent_` keys stored in
`~/work/.secrets/khala-heartbeat.env` (gitignored):

```sh
# ~/work/.secrets/khala-heartbeat.env   (NEVER commit; NEVER print the values)
KHALA_HEARTBEAT_KEYS=oa_agent_xxx,oa_agent_yyy
```

The free tier caps **both** tokens and requests per UTC-day per key:
**2,500,000 tokens** (`FREE_TIER_MAX_TOKENS_PER_DAY`) and **2,000 requests**
(`FREE_TIER_MAX_REQUESTS_PER_DAY`), #6232. A ~50k/run heartbeat is ~34 requests/run
(≈30 long-gens + 4), so at ~96 runs/day that is ≈ **~5–6M tokens/day AND ~3.3k
requests/day** — **one key cannot cover 24/7 on either quota** (it 402s after ~12h).
Provision **≥2 keys** and the script round-robins them (≈2.5–3M tokens + ~1.7k req per
key/day, under both caps). Mint keys with `POST /api/keys/free` (returns `oa_agent_…`),
but note minting is **rate-limited to 25/IP/day** — mint when you have headroom (or from
another IP) and append to the pool. A funded key avoids the free caps but spends real
balance (~$0.24/Mtok blended, ~$1.4/day for the full heartbeat); free + rotated is the
no-spend default.

To lower the per-run load, drop `KHALA_HEARTBEAT_TOKEN_TARGET` (e.g. `25000` ≈ ~17
long-gens, which one key sustains 24/7 on both quotas) — but the owner asked for
**≥~50k/run**, so the default target is `50000`.

## GLM REAP smoke receipt expectation

`scripts/khala-glm-reap-production-smoke.mjs` checks the armed live GLM lane
through `openagents/khala` and normally requires a dereferenceable billable
receipt ref on both non-streaming and streaming responses. Use a normal funded
or otherwise billable `OPENAGENTS_AGENT_TOKEN` when the goal is a full
`nonstream_receipt_ref_present` PASS.

Some operator test tokens are explicitly zero-debit. Those tokens can prove GLM
serving, routing, model hiding, and public counter movement, but they do not
mint a billable receipt. For that known shape only, run the smoke with:

```sh
OPENAGENTS_KHALA_GLM_REAP_OPERATOR_EXEMPT_ZERO_DEBIT=1 \
  node scripts/khala-glm-reap-production-smoke.mjs --approve-live-spend
```

or pass `--operator-exempt-zero-debit`. In that mode a missing billable receipt
is accepted only when the response exposes the explicit no-debit marker
(`openagents.billing.mode = "no_debit"`) and the receipt check is reported as
`skipped (operator-exempt, no billable receipt)`. Do not use that flag for a
billable token; a missing receipt on a billable token must still fail.

## Schedule (every 15 minutes, no GitHub Actions)

Runs via a macOS **launchd LaunchAgent** on this Mac (the no-GitHub-Actions invariant
keeps unattended execution on our own machines). Plist:
`~/Library/LaunchAgents/com.openagents.khala-heartbeat.plist` with `StartInterval 900`.

Install / reinstall:

```sh
launchctl unload ~/Library/LaunchAgents/com.openagents.khala-heartbeat.plist 2>/dev/null
launchctl load   ~/Library/LaunchAgents/com.openagents.khala-heartbeat.plist
launchctl kickstart -k gui/$(id -u)/com.openagents.khala-heartbeat   # run once now
```

(The exact plist is reproduced at the bottom of this runbook so any agent can recreate
it.) On Linux hosts use cron instead: `*/15 * * * * /path/to/scripts/khala-heartbeat.sh`.

## How a future agent checks it

```sh
cat ~/work/.khala-heartbeat/status.json | jq .          # latest verdict
tail -5 ~/work/.khala-heartbeat/heartbeat.jsonl | jq .  # recent history
tail ~/work/.khala-heartbeat/FAILURES.log               # only if triaging
launchctl list | grep khala-heartbeat                   # is it scheduled?
bash ~/work/openagents/scripts/khala-heartbeat.sh       # run once on demand
```

## Triage

- **degraded (quota)** → rotate is automatic; if it persists, add another key to the
  pool (`KHALA_HEARTBEAT_KEYS`) — the day's quota is spent, not an outage.
- **down + counter didn't move** → only possible when
  `KHALA_HEARTBEAT_EXPECT_PUBLIC_COUNTER=1`; the gateway served tokens but the
  **recorder/counter** regressed. Check
  `GET /api/public/khala-tokens-served` and the served-tokens recorder. In the
  default internal heartbeat mode, counter movement is expected when tokens are
  actually served, but the check remains monotonic unless explicitly required.
- **down + counter regressed/unreadable** → the public counter surface itself is
  unhealthy; check the live-at-read scalar endpoint and sync summary path.
- **down + http 5xx / timeouts** → the gateway or a backing lane (Fireworks DeepSeek
  V4 Flash primary; Gemini Flash / GPT-OSS Hydralisk / GLM-REAP fallbacks) is
  unhealthy. Check `GET /api/v1/models` and the lane health.
- **down + http 401** → a key in the pool was revoked; replace it.
- Do **not** spam `NEEDS_OWNER.md`; the heartbeat writes its own `FAILURES.log`.
  Surface to the owner only on a sustained outage that needs an owner action.

## 500 RED-ALERT synthetic canary (the fast outage detector — AAR 2026-06-25)

The 15-minute heartbeat above proves "the real customer paths + accounting work",
but it is too coarse and too heavy to be a *fast outage detector*. The
**2026-06-25 gateway-wide 500 outage** (every `POST /api/v1/chat/completions`
returned 500 for ~10+ minutes after the worker shipped ahead of migration
`0234`) was **not auto-detected — the owner noticed manually**. AAR:
`docs/incidents/2026-06-25-khala-500-completions-outage-aar.md`.

`scripts/khala-canary.sh` is the tight loop that closes that detection gap. Each
run (every ~90s) fires **ONE** small real `openagents/khala` completion and:

- **UP** (exit 0): http 200 with non-empty usage, and the public counter endpoint
  was readable and monotonic. In default internal mode the public counter delta
  is a liveness/accounting signal, not an external-demand claim.
- **DEGRADED** (exit 2): http 402/429 — endpoint alive, the canary key is just
  quota/rate-limited (rotate/add keys). **No RED ALERT.**
- **DOWN** (exit 1): http 500 / any non-200, http 200 with empty usage, unreadable
  public counter endpoint, backward public counter movement, or (only with
  `KHALA_CANARY_EXPECT_PUBLIC_COUNTER=1`) no public counter movement after
  served tokens.

On a **healthy→down transition only** (edge-triggered, so a sustained outage does
NOT spam), DOWN:

1. writes a prominent block to `~/work/.khala-heartbeat/RED-ALERT.log` (with the
   exact first-investigation steps — including `bun run check:pending-migrations`,
   the single most likely cause given the AAR),
2. appends ONE dated `RED-ALERT:` line to `~/work/NEEDS_OWNER.md`,
3. exits non-zero so a scheduler/agent watcher reacts.

Recovery (down→up) is logged to `RED-ALERT.log` (no NEEDS_OWNER spam).

**It should trigger an agent investigation.** When the canary is DOWN, an agent
(or the owner) should immediately: (a) run
`cd apps/openagents.com && bun run check:pending-migrations` — if anything is
pending, that IS the outage class from the AAR, apply it with
`cd workers/api && wrangler d1 migrations apply openagents-autopilot --remote`;
(b) check `GET /api/v1/models` + lane health for a 5xx backing lane; (c) review
the most recent deploy (the ONLY sanctioned deploy is `deploy:safe`).

Secret-safe like the heartbeat: keys come from the gitignored secrets file (by
default `~/work/.secrets/khala-heartbeat.env`; point at a dedicated ops pool with
`KHALA_CANARY_ENV` + `KHALA_CANARY_KEYS`). To make the canary require positive
public-counter movement, set
`KHALA_CANARY_EXPECT_PUBLIC_COUNTER=1`. No
key/prompt/completion/raw-IP ever lands in any file.

Check it:

```sh
cat ~/work/.khala-heartbeat/canary-status.json | jq .   # latest tick
tail ~/work/.khala-heartbeat/RED-ALERT.log              # any fired alerts (+ recoveries)
tail -20 ~/work/.khala-heartbeat/canary.jsonl | jq .    # recent ticks
bash ~/work/openagents/scripts/khala-canary.sh          # run once on demand
```

### Ops free-key pool convention (so the canary/ops are never keyless)

The canary and the heartbeat reuse pre-provisioned **free-tier** keys rather than
minting on every run. Keep a small **stable** ops pool in the gitignored secrets
dir so an agent is never keyless (AAR 2026-06-25 secondary pain: the responder
hit `free_key_mint_rate_limited` trying to mint a fresh key mid-incident):

```sh
# ~/work/.secrets/khala-ops-keys.env   (NEVER commit; NEVER print the values)
# A handful of long-lived free-tier oa_agent_ keys reserved for ops/canary use.
KHALA_CANARY_KEYS=oa_agent_aaa,oa_agent_bbb
```

Then point the canary at it: `KHALA_CANARY_ENV=~/work/.secrets/khala-ops-keys.env`
in the canary LaunchAgent's `EnvironmentVariables`. If absent, the canary falls
back to `KHALA_HEARTBEAT_KEYS`. The per-IP mint cap is now **200/day and
env-overridable** (`FREE_KEY_MAX_MINTS_PER_IP_PER_DAY`), so an agent can also mint
a fresh ops key during an incident — but a stable pre-provisioned pool is the
no-mint default.

### The canary LaunchAgent plist (reproduce if missing)

`~/Library/LaunchAgents/com.openagents.khala-canary.plist`, `StartInterval 90`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.openagents.khala-canary</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>/Users/christopherdavid/work/openagents/scripts/khala-canary.sh</string>
  </array>
  <key>StartInterval</key><integer>90</integer>
  <key>RunAtLoad</key><true/>
  <key>StandardOutPath</key><string>/Users/christopherdavid/work/.khala-heartbeat/canary-launchd.out</string>
  <key>StandardErrorPath</key><string>/Users/christopherdavid/work/.khala-heartbeat/canary-launchd.err</string>
  <key>EnvironmentVariables</key><dict>
    <key>PATH</key><string>/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    <key>KHALA_CANARY_ENV</key><string>/Users/christopherdavid/work/.secrets/khala-ops-keys.env</string>
  </dict>
</dict></plist>
```

Install / reinstall:

```sh
launchctl unload ~/Library/LaunchAgents/com.openagents.khala-canary.plist 2>/dev/null
launchctl load   ~/Library/LaunchAgents/com.openagents.khala-canary.plist
launchctl kickstart -k gui/$(id -u)/com.openagents.khala-canary   # run once now
```

On Linux hosts use cron: `* * * * * /path/to/scripts/khala-canary.sh` (1-minute
granularity; the script self-bounds each tick well under a minute).

## The LaunchAgent plist (reproduce if missing)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>com.openagents.khala-heartbeat</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>/Users/christopherdavid/work/openagents/scripts/khala-heartbeat.sh</string>
  </array>
  <key>StartInterval</key><integer>900</integer>
  <key>RunAtLoad</key><true/>
  <key>StandardOutPath</key><string>/Users/christopherdavid/work/.khala-heartbeat/launchd.out</string>
  <key>StandardErrorPath</key><string>/Users/christopherdavid/work/.khala-heartbeat/launchd.err</string>
  <key>EnvironmentVariables</key><dict><key>PATH</key><string>/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string></dict>
</dict></plist>
```

Dependencies: `curl`, `jq` (the script also uses `jq` for JSON), Bash. The plist
`PATH` includes Homebrew so `jq` resolves under launchd.
