# Khala Liveness Heartbeat — Runbook

Updated: 2026-06-25

> **Status:** operational runbook, honest-scope. Documents the scheduled liveness
> heartbeat that fires ~50k tokens across diverse configs at the **live production**
> Khala endpoint every 15 minutes, verifies the public counter records them, and
> writes a public-safe status. For future agents/operators. Not a product promise;
> heartbeat tokens are **internal dogfood** (not external demand) and should be
> tagged as such once demand-tagging (`docs/gym/ROADMAP.md` F1 / #6252) lands.

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
4. Re-reads the counter and asserts it **moved** (proves the served-tokens recorder
   is alive), then writes a status line.

**Why ~50k in diverse configs:** a single ping proves "the box answers"; ~50k across
short/long/streaming/content-array/concurrent proves the **real** paths customers use
(tool-calling shapes, streaming, long decode, concurrency) and that **accounting**
still records every served token. It is also honest dogfood traffic on the North Star.

## Verdict / exit codes

- `0` **ok** — configs succeeded and the counter moved.
- `2` **degraded** — everything that ran was quota-limited (`402`/`429`): the endpoint
  is alive but the key is tapped out for the UTC day (rotate / add keys).
- `1` **down** — a hard failure on a config, or the counter did **not** move while
  tokens were served, or no keys configured. Writes to `FAILURES.log`.

## Files (all under `~/work/.khala-heartbeat/`, gitignored)

- `heartbeat.jsonl` — append-only log, one public-safe JSON line per run (ts, state,
  ok/fail/quota counts, summed tokens, counter before/after/delta, per-config http).
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
- **down + counter didn't move** → the gateway served but the **recorder/counter**
  regressed (this is exactly the class of bug behind #6231 / the monotonic-counter
  fix). Check `GET /api/public/khala-tokens-served` and the served-tokens recorder.
- **down + http 5xx / timeouts** → the gateway or a backing lane (Fireworks DeepSeek
  V4 Flash primary; Gemini Flash / GPT-OSS Hydralisk / GLM-REAP fallbacks) is
  unhealthy. Check `GET /api/v1/models` and the lane health.
- **down + http 401** → a key in the pool was revoked; replace it.
- Do **not** spam `NEEDS_OWNER.md`; the heartbeat writes its own `FAILURES.log`.
  Surface to the owner only on a sustained outage that needs an owner action.

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
