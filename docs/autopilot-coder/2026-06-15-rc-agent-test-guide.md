# Agent test guide — Autopilot & Pylon v1.0-rc

Date: 2026-06-15. **Audience: a tester's coding/automation agent** (Claude Code,
Codex, an Autopilot/Pylon node, etc.) — not a human. This is a copy-pasteable
script for an agent to install a release candidate, exercise it headlessly, and
report structured results. Every step emits machine-readable JSON so the agent
can assert on it. RC channel only (`1.0.0-rc.1`).

If you are a human, use the install guide instead:
`docs/autopilot-coder/2026-06-15-rc-tester-install-guide.md`.

---

## 0. What to report back

At the end, return one JSON object:

```json
{
  "platform": "darwin-arm64",
  "pylonVersion": "1.0.0-rc.1",
  "signatureVerified": true,
  "commandsOk": ["help","status","context","balance","update-check"],
  "commandsFailed": [],
  "sessionSpawned": true,
  "notes": "free text — anything surprising"
}
```

Fail loudly: if any step errors, capture stderr verbatim in `notes` and set the
relevant field false. Do **not** paper over failures — this is a test.

---

## 1. Get the signed Pylon binary

Pick your platform: `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`.

**Option A — download the RC artifact** (preferred; from our GCP feed):
```sh
PLAT=darwin-arm64   # set yours
BASE=https://updates.openagents.com/pylon/rc/$PLAT
# the feed lists the current release + its signature
curl -fsS "$BASE/feed.json" -o feed.json
cat feed.json   # note releases[0].version / .artifactUrl / .sha256 / .signature / .kid
URL=$(python3 -c "import json;print(json.load(open('feed.json'))['releases'][0]['artifactUrl'])")
curl -fsS "$URL" -o pylon
chmod +x pylon
```

**Option B — build from source** (if no artifact link yet):
```sh
git clone https://github.com/OpenAgentsInc/openagents
cd openagents/apps/pylon && bun install
bun run build:rc-binaries 1.0.0-rc.1          # signs all 4 platforms into dist/rc/
cp "dist/rc/1.0.0-rc.1/pylon-$PLAT" ../../pylon && chmod +x ../../pylon
```

## 2. Verify provenance (must pass — fail closed)

The binary is signed with the OpenAgents ed25519 release key
(kid `2dbe811d19f67528`). Verify before trusting it:
```sh
# Option A: the feed gives sha256 + signature; check sha256 at minimum
echo "<sha256-from-feed>  pylon" | shasum -a 256 -c -    # expect: pylon: OK
# Full ed25519 check (from a source checkout, against the pinned key):
bun apps/oa-updates/scripts/verify-release.ts pylon pylon.sig.json
# expect: OK: ... signed by OpenAgents (kid 2dbe811d19f67528)
```
Set `signatureVerified` accordingly. A mismatch is a **hard fail** — stop.

## 3. Exercise the agent surface (headless, all JSON)

Pylon is agent-steerable: `help --json` is the machine-readable command catalog.
Run each and assert it returns valid JSON (no `requires a value`, no stack trace).
Use an isolated home so you don't touch any real node:

```sh
export PYLON_HOME=$(mktemp -d)/pylon
export PYLON_OPENAGENTS_BASE_URL=https://openagents.com

./pylon help --json            # -> command catalog (assert: array/obj of commands)
./pylon status --json          # -> assert .version == "1.0.0-rc.1", .lifecycle present
./pylon bootstrap --json       # -> assert .version == "1.0.0-rc.1"
./pylon context --json         # -> assert .schema starts "openagents.pylon.context"
./pylon balance --json         # -> assert .balance.balanceMsat is a number (the #5038 fix)
./pylon memories --json        # -> assert valid JSON (no "--json requires a value")
./pylon update --check --json  # -> assert .status in {up-to-date,update-available}
```

Record each in `commandsOk` / `commandsFailed`.

## 4. Exercise a real unit of work (optional but valuable)

Spawn a coding session against the local runtime and verify it with a check:
```sh
./pylon sessions spawn --adapter codex \
  --objective "create a file pong.txt containing 'pong'" \
  --verify "test -f pong.txt"
./pylon sessions list --json   # -> assert the session appears; note completed/passed
```
Set `sessionSpawned` true only if the session reaches a terminal state with the
verify passing. (Requires an adapter/runtime to be available; if not, note it and
leave `sessionSpawned` false — that's a valid result, not a failure of the build.)

## 5. Default-on auto-update (don't fight it)

Pylon checks for updates at startup by default and self-replaces (verifying
against the pinned key). For a bounded test, opt out so the binary under test
stays fixed:
```sh
export PYLON_DISABLE_AUTOUPDATE=1
./pylon                         # runs the headless node; Ctrl-C / SIGTERM to stop
```
To test the updater path itself, leave it on and confirm `update --check` reports
the feed correctly (step 3).

---

## Autopilot Desktop (GUI) — agent notes

The desktop app is a notarized macOS `.app` an agent generally can't drive via a
TUI. An agent CAN verify the artifact and that it would pass Gatekeeper:
```sh
APP="/path/to/Autopilot Desktop-canary.app"
spctl -a -vvv -t exec "$APP"                       # expect: accepted / Notarized Developer ID
codesign -dvv "$APP" 2>&1 | grep TeamIdentifier    # expect: HQWSG26L43
```
The app bundles the same Pylon node and runs it headless internally; testing the
Pylon binary above covers the node behavior. GUI interaction is a human task.

---

## Boundaries for the testing agent

- **Don't** publish to npm, deploy, or flip any promise/registry state — this is a
  read/exercise test of a release candidate.
- **Don't** run on a real funded node home; always use a throwaway `PYLON_HOME`.
- The wallet `balance` is real but tiny (test sats); don't attempt payouts.
- Report honestly. A failed command or unavailable adapter is useful signal —
  record it, don't hide it.
