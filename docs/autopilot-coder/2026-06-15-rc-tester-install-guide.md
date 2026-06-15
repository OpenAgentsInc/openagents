# Autopilot & Pylon v1.0-rc — tester install guide

Date: 2026-06-15. Audience: invited testers. These are **release candidates**
(`1.0.0-rc.2`, RC/canary channel only — not the public stable release). Both
ship **default-on auto-update**, so once you're on an RC you'll get newer RCs
automatically.

There are two ways to participate. Most testers want **Autopilot Desktop** (the
GUI app that runs a local node for you). Agents / power users can run **Pylon**
headless.

---

## A. Autopilot Desktop (macOS) — the app

The macOS build is **signed with our Apple Developer ID and notarized by Apple**,
so Gatekeeper opens it normally (no right-click-open workaround needed).

1. **Download here:** `AutopilotDesktop-1.0.0-rc.2-macos-arm64.dmg` from
   <https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/AutopilotDesktop-1.0.0-rc.2-macos-arm64.dmg>.
   The matching GitHub release is
   <https://github.com/OpenAgentsInc/openagents/releases/tag/autopilot-desktop-v1.0.0-rc.2>.
2. Open the `.dmg`, drag **Autopilot Desktop** to Applications.
3. Launch it. It bundles and starts a **headless Pylon node** for you — no
   separate install. The home screen is the live **pylon-network visualization**
   (the network's activity, online pylons, sats settled, training contributors).
4. If the app does not reach a working node/agent, use **Settings → First-run
   Health** on builds that include the #5064 health pane and report the shown
   blocker refs. On older rc.2 builds that do not show that pane, report the
   platform, app version, launch status, and visible error text.
5. Auto-update is **on by default** (checks at launch + every 6h). To opt out,
   set `AUTOPILOT_DISABLE_AUTOUPDATE=1` in the launch environment.

**Verify it's genuinely ours** (optional, recommended):
```sh
spctl -a -vvv -t exec "/Applications/Autopilot Desktop-canary.app"
# expect: accepted · source=Notarized Developer ID
codesign -dvv "/Applications/Autopilot Desktop-canary.app" 2>&1 | grep TeamIdentifier
# expect: TeamIdentifier=HQWSG26L43
```

Apple Silicon only for this RC. Intel (x64) and Linux desktop installers are
owner-gated until those builds are signed, notarized or packaged, and published.

---

## B. Pylon (headless CLI) — macOS & Linux

Pylon is a single self-contained binary (no Node/Bun install needed). It is
**signed with our ed25519 release key** (kid `2dbe811d19f67528`); the binary
verifies its own updates against that pinned key and **fails closed**.

Platforms: `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`.

1. Download `pylon-<platform>` from
   `https://updates.openagents.com/pylon/rc/<platform>/feed.json` →
   `releases[0].artifactUrl`.
2. Make it executable and run it:
   ```sh
   chmod +x pylon-darwin-arm64
   ./pylon-darwin-arm64 --help            # 28-command catalog
   ./pylon-darwin-arm64 status --json     # reports version 1.0.0-rc.2
   ./pylon-darwin-arm64                    # default: run the headless node
   ```
3. Auto-update is **on by default**: at startup Pylon checks the signed feed and,
   if a newer RC exists, downloads it, **verifies sha256 + the ed25519 signature
   against the pinned key**, atomically replaces itself, and relaunches. Opt out
   with `PYLON_DISABLE_AUTOUPDATE=1`. Manual check: `pylon update --check --json`.

**Verify a downloaded binary** (optional) against the published signature:
```sh
# each release ships pylon-<platform>.sig.json next to the binary
bun apps/oa-updates/scripts/verify-release.ts pylon-darwin-arm64 pylon-darwin-arm64.sig.json
# expect: OK: signed by OpenAgents (kid 2dbe811d19f67528)
```

### Or build Pylon from source
```sh
git clone https://github.com/OpenAgentsInc/openagents && cd openagents/apps/pylon
bun install
bun run build:rc-binaries 1.0.0-rc.2   # builds + signs all 4 platforms into dist/rc/
# or just run it directly:
bun src/index.ts --help
```

---

## What "release candidate" means here

- These are **RC builds for testing**, not the public stable release. Behavior,
  copy, and pricing may change before GA.
- The training-run launch itself is gated separately — installing an RC lets you
  exercise the app/node; the live paid run goes live when we announce it.
- Report issues in the Product Promises Forum
  (<https://openagents.com/forum/f/product-promises>) or, for concrete
  reproducible bugs, the strict bug form.

## For maintainers: publishing the RC artifacts

The artifacts are produced locally and published to our GCP feed (the
`oa-updates` Cloud Run service, `updates.openagents.com`, project
`openagentsgemini`):

- **Pylon:** `apps/pylon` → `bun run build:rc-binaries 1.0.0-rc.2`, then
  `bun apps/oa-updates/scripts/publish-pylon-release.ts --build-dir
  apps/pylon/dist/rc/1.0.0-rc.2 --channel rc --rollout 100`, then deploy
  `oa-updates` with `OA_PYLON_RELEASES_DIST=/app/pylon-dist`.
- **Autopilot:** `apps/autopilot-desktop` → `bun run build:canary`, codesign +
  notarize via `scripts/notarize-macos.sh`
  (`OA_ASC_ENV=~/work/.secrets/appstoreconnect.env`), then upload the
  `artifacts/` (`.app.tar.zst`, `.dmg`, `update.json`) to the desktop feed.

Signing custody + restore: `apps/oa-updates/docs/release-signing-runbook.md`.
RC channel only until the owner authorizes stable/GA.
