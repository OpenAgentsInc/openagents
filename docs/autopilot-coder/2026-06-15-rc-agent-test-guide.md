# Agent test guide — Autopilot & Pylon v1.0-rc

Date: 2026-06-15 (rev 2). **Audience: a tester's coding/automation agent**
(Claude Code, Codex, an Autopilot/Pylon node, etc.) — not a human. Copy-pasteable
script to install a release candidate, exercise it headlessly, and report
structured results. Every step emits machine-readable JSON to assert on. RC
channel only (`1.0.0-rc.2`).

> **No coding agent SDK required.** The entire core test — signature
> verification, every `--json` command, and booting the node — works on a vanilla
> machine with **zero** coding-agent SDKs installed (`@openai/codex-sdk`,
> `@anthropic-ai/claude-agent-sdk`, etc.). The **only** SDK-gated step is the
> optional managed-session spawn (§4). **If you don't have an SDK, skip §4 — a
> no-SDK run is a complete, passing test.** Nothing in the core path imports,
> requires, or trips on an agent SDK; a Pylon with no SDK simply doesn't advertise
> the coding-session lane.

Human installers: use `docs/autopilot-coder/2026-06-15-rc-tester-install-guide.md`.

---

## 0. What to report back

Return one JSON object:

```json
{
  "platform": "darwin-arm64",
  "pylonVersion": "1.0.0-rc.2",
  "signatureVerified": true,
  "commandsOk": ["help","bootstrap","context","status","balance","memories","update-check"],
  "commandsFailed": [],
  "sessionSpawned": "skipped-no-sdk",
  "notes": "free text"
}
```

- `sessionSpawned` is tri-state: `true` (spawned + verified), `false` (attempted
  with an SDK present but it failed — a real bug, capture details), or
  `"skipped-no-sdk"` (no SDK installed — **this is a PASS**, not a failure).
- A run with `signatureVerified: true`, all core commands in `commandsOk`, and
  `sessionSpawned: "skipped-no-sdk"` is a **fully successful** RC test.
- Capture stderr verbatim in `notes` for anything that errors. Don't hide
  failures — but also don't count an SDK-gated skip as a failure.

---

## 1. Get the signed Pylon binary

Platforms: `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`.

**Option A — download the RC artifact** (from our GCP feed, once published):
```sh
PLAT=darwin-arm64
BASE=https://updates.openagents.com/pylon/rc/$PLAT
curl -fsS "$BASE/feed.json" -o feed.json    # if this 404s, the feed isn't published yet -> use Option B
URL=$(python3 -c "import json;print(json.load(open('feed.json'))['releases'][0]['artifactUrl'])")
curl -fsS "$URL" -o pylon && chmod +x pylon
```

**Option B — build from source** (always works; needed while the feed is unpublished):
```sh
git clone https://github.com/OpenAgentsInc/openagents
cd openagents/apps/pylon && bun install
bun run build:rc-binaries 1.0.0-rc.2          # builds + signs all 4 platforms into dist/rc/
cp "dist/rc/1.0.0-rc.2/pylon-$PLAT" ../../pylon && chmod +x ../../pylon
```

## 2. Verify provenance (must pass — fail closed)

Signed with the OpenAgents ed25519 release key (kid `2dbe811d19f67528`):
```sh
# from a source checkout, against the pinned key:
bun apps/oa-updates/scripts/verify-release.ts \
  apps/pylon/dist/rc/1.0.0-rc.2/pylon-$PLAT \
  apps/pylon/dist/rc/1.0.0-rc.2/pylon-$PLAT.sig.json
# expect: OK: ... signed by OpenAgents (kid 2dbe811d19f67528)
```
A mismatch is a **hard fail** — stop. Set `signatureVerified` accordingly.

## 3. Exercise the agent surface (headless, all JSON, NO SDK needed)

Pylon is agent-steerable; `help --json` is the machine-readable catalog. Use an
isolated home so you never touch a real node, and **read stdout only** — these
commands print JSON to stdout; logs go to stderr, so `2>&1` will corrupt your
parse. Pipe with `2>/dev/null` (or capture stdout separately).

```sh
export PYLON_HOME=$(mktemp -d)/pylon
export PYLON_OPENAGENTS_BASE_URL=https://openagents.com
B=./pylon   # the binary under test

$B help --json      2>/dev/null   # catalog of commands
$B bootstrap --json 2>/dev/null   # assert .version == "1.0.0-rc.2"   (top-level)
$B context --json   2>/dev/null   # assert .schema starts "openagents.pylon.context"
$B status --json    2>/dev/null   # SEE SHAPE BELOW
$B balance --json   2>/dev/null   # assert .balance.balanceMsat is a number  (the #5038 fix)
$B memories --json  2>/dev/null   # assert valid JSON
$B update --check --json 2>/dev/null   # SEE NOTE BELOW
```

### `status --json` shape (assert the nested fields, not top-level)

`status` returns a public projection envelope. Assert:
```jsonc
{
  "kind": "status",
  "state": {
    "version": "1.0.0-rc.2",          // <-- .state.version, NOT top-level .version
    "runtime": { "lifecycle": "offline" }  // <-- .state.runtime.lifecycle
  }
}
```
(`bootstrap --json` is different — its `.version` IS top-level.)

### `update --check --json`

```jsonc
{ "status": "up-to-date" | "update-available", ... }
```
**If the live RC feed is not published yet**, this returns a clean error:
```jsonc
{ "status": "error", "error": "update feed https://updates.openagents.com/pylon/rc/<plat>/feed.json returned 404", "applied": false }
```
That 404 is a **feed-publishing state, not a binary failure** — the client route
is correct, the artifact just isn't served yet. Record it as
`update-check: feed-unpublished` and **do not** count it as a binary defect.
(Publishing the feed is a maintainer task — see the install guide's maintainer
section.) Set `commandsOk` to include `update-check` once the feed is live;
otherwise note `feed-unpublished`.

## 4. OPTIONAL — managed agent session (requires a coding agent SDK + credentials)

**Skip this entire section if you don't have a coding agent SDK installed.** The
RC is fully validated by §1–§3 without it. This step exercises a managed coding
session, which requires:

1. A coding agent SDK resolvable in the **node's** environment
   (`@openai/codex-sdk` for `--adapter codex`, or
   `@anthropic-ai/claude-agent-sdk` for `--adapter claude_agent`), **and**
2. valid credentials for that agent.

**Important — the standalone compiled binary does not bundle agent SDKs** (by
design — the binary stays SDK-free so the core works everywhere). So in the
compiled binary, `--adapter codex/claude_agent` will report `sdk_missing`. That is
the **expected SDK-free behavior, not a defect.** To exercise a session, run the
node from a **source checkout** (where `node_modules` provides the SDK) or any
environment where the SDK + creds are present.

`sessions spawn` talks to a **running node's** control API, so start a node first
(isolated home + a random control port to avoid colliding with a real node on the
default `4716`):

```sh
# from a SOURCE checkout (apps/pylon), with the SDK installed:
export PYLON_HOME=$(mktemp -d)/pylon
export PYLON_CONTROL_PORT=$(( 20000 + RANDOM % 20000 ))
WORK=$(mktemp -d)

bun src/index.ts node &                 # start the node (control API on $PYLON_CONTROL_PORT)
NODE_PID=$!
sleep 3                                  # let the control API come up

REF=$(bun src/index.ts sessions spawn --adapter codex \
  --objective "create a file pong.txt containing 'pong'" \
  --verify "test -f pong.txt" \
  --worktree "$WORK" 2>/dev/null | python3 -c "import sys,json;print(json.load(sys.stdin).get('sessionRef',''))")

# poll THAT session ref (sessions list includes external host sessions — filter to $REF):
for i in $(seq 1 60); do
  STATE=$(bun src/index.ts sessions list --json 2>/dev/null \
    | python3 -c "import sys,json;rows=json.load(sys.stdin);print(next((r['state'] for r in (rows if isinstance(rows,list) else rows.get('sessions',[])) if r.get('sessionRef')=='$REF'),'pending'))")
  [ "$STATE" = "completed" ] || [ "$STATE" = "failed" ] && break
  sleep 2
done
echo "session $REF -> $STATE"
kill $NODE_PID 2>/dev/null
```

Outcomes:
- No node running → `sessions spawn` returns a clean `{ "ok": false, "code": "no_token" }` (not a crash). Start the node first.
- No SDK in the node env → the session resolves `sdk_missing` (clean, expected) → record `sessionSpawned: "skipped-no-sdk"`.
- SDK + creds present → the session should reach `completed` with `pong.txt`
  created and the verify passing → `sessionSpawned: true`.

## 5. Default-on auto-update (don't fight it)

Pylon checks for updates at startup by default and self-replaces (verifying
against the pinned key). For a bounded test, opt out so the binary stays fixed:
```sh
export PYLON_DISABLE_AUTOUPDATE=1
./pylon                         # runs the headless node; SIGTERM to stop. No SDK needed.
```

---

## Autopilot Desktop (GUI) — agent notes

The macOS app is a notarized `.app`. An agent verifies provenance + Gatekeeper:
```sh
APP="/path/to/Autopilot Desktop-canary.app"
spctl -a -vvv -t exec "$APP"                       # expect: accepted / Notarized Developer ID
codesign -dvv "$APP" 2>&1 | grep TeamIdentifier    # expect: HQWSG26L43
```
The app bundles + runs the same Pylon node headlessly; the Pylon binary test above
covers node behavior. GUI interaction is a human task.

---

## Boundaries for the testing agent

- **Don't** publish to npm, deploy, or flip any promise/registry state — read/exercise only.
- **Don't** run on a real funded node home; always use a throwaway `PYLON_HOME`.
- The wallet `balance` is real but tiny (test sats); don't attempt payouts.
- **No SDK is required and none should be installed for the core test.** Report
  an SDK-gated skip as `"skipped-no-sdk"` (a pass), never as a failure.
