# OTA auto-update plan — Autopilot + Pylon (and Rust binaries)

Date: 2026-06-15. Owner mandate: **every OpenAgents client/binary auto-updates
over the air, ON by default.** A contributor or agent should never run a stale
node; updates land without manual action. This doc is the plan for one uniform
OTA system across the three runtimes we ship: the **Electrobun** desktop app
(Autopilot), the **Bun-compiled** Pylon binary, and any **Rust** binaries
(psionic/executor/tap-ldk lineage). Researched against Electrobun's updater and
the Rust self-update ecosystem (`self_update`, `cargo-dist`/`axoupdater`).

## 0. The one rule

**Auto-update defaults to ON for every artifact, every channel, every platform.**
It is opt-*out*, never opt-in. The opt-out is a single, documented switch per
runtime (env var + persisted config), and even when off the client still *checks*
and surfaces "update available" — it just doesn't auto-apply. There is no build
that ships with auto-update disabled by default.

## 1. Current state (what already exists)

- **Autopilot (Electrobun) OTA is ~80% built.** `electrobun build --env=canary|
  stable` emits, per channel/platform, a `*-update.json` (version + content
  hash), a `.dmg` installer, a compressed `*.app.tar.zst`, and **incremental
  BSDIFF patches** (Electrobun's custom zig BSDIFF — patches as small as ~14 KB;
  clients >1 version behind fall back to the full `.tar.zst`). These publish via
  **`apps/oa-updates`** (`desktop:publish --channel stable`), a deployed Cloud
  Run service that fronts **`updates.openagents.com`**; desktop clients read
  `updates.openagents.com/desktop/<channel>/feed.json`. (Refs:
  `apps/autopilot-desktop/README.md`, `apps/oa-updates/src/desktop-release.ts`,
  `electrobun.config.ts`.)
- **`oa-updates`** already models channels (`stable`/`canary`), a desktop release
  manifest + feed, an asset store, and code-signing test scaffolding. It also
  handles the mobile app's Expo channel headers. It is the natural single
  backend for *all* OTA feeds.
- **Pylon OTA is only declared, not implemented.** `apps/pylon/src/bootstrap.ts`
  ships an `updatePolicy: { dashboardPolling, channel: "github-releases",
  sourceBuildFallback: "disabled" }` — a stated intent (GitHub-releases channel)
  with **no self-updater wired**. The TUI is gone (#5034), so `dashboardPolling`
  is vestigial.
- Pylon ships three ways today: (a) **bundled headless node inside the Autopilot
  `.app`** (#5027 — updates *with* the app), (b) the **npm package**
  `@openagentsinc/pylon`, (c) **from source** (the dev / agent-steerable path).
  None auto-updates as a standalone install yet.

## 2. Research summary

### Electrobun updater (Autopilot)
- Mechanism: per-channel flat manifests (`stable-macos-arm64-update.json`),
  zig-BSDIFF incremental patches with full-bundle fallback, "bring your own static
  host (S3 / R2 / GCS / GitHub Releases)." We already host via `oa-updates` →
  `updates.openagents.com`. ([Electrobun: Updates](https://blackboard.sh/electrobun/docs/guides/updates/),
  [Bundling & Distribution](https://blackboard.sh/electrobun/docs/guides/bundling-and-distribution/))
- Caveat from research: GitHub's `/releases/latest/download` only resolves
  non-prerelease, so **canary can't auto-update from GitHub Releases** — fine for
  us, we serve channels from `updates.openagents.com`, not GH.
- Gaps Electrobun's docs don't pin (treat as implementation tasks): exact check
  cadence, default-on flag, and signature verification of the downloaded bundle —
  we set those explicitly (§5, §6).

### Rust / standalone-binary self-update (Pylon binary + future Rust)
- **`self_update`** — general-purpose in-place updater; backends GitHub/GitLab/S3;
  signature verification via **zipsign** (ed25519 over the archive). Good model
  for a hand-rolled updater. ([self_update](https://crates.io/crates/self_update))
- **`cargo-dist` + `axoupdater`** — cargo-dist builds/publishes releases with a
  `dist-manifest.json`; `axoupdater` is a library you embed to check + self-apply
  (GitHub or Axo backends). Tighter, manifest-driven. ([axoupdater](https://crates.io/crates/axoupdater),
  [cargo-dist](https://crates.io/crates/cargo-dist))
- Common pattern we adopt for **all** binaries: a signed per-channel/per-platform
  JSON manifest → download asset → **verify signature + hash** → **atomic
  self-replace** (write-temp + rename, `self-replace`-style) → relaunch. Pylon
  (Bun) implements this directly; Rust binaries use `self_update`/`axoupdater`
  pointed at the same feed convention.

## 3. Target architecture — one feed, three runtimes

`updates.openagents.com` (served by `apps/oa-updates`) is the single OTA backend.
Uniform layout:

```
updates.openagents.com/
  desktop/<channel>/feed.json          # Autopilot — Electrobun-native (exists)
  pylon/<channel>/<platform>/feed.json # Pylon binary — new
  <rust-binary>/<channel>/<platform>/feed.json  # future Rust binaries
```

Every feed entry is a signed manifest: `{ version, channel, platform, arch,
url, sha256, signature, minVersion?, rolloutPercent?, yanked? }`. `channel ∈
{stable, canary}`. Signing key per artifact class (Apple Developer ID for the
`.app`; an OpenAgents **ed25519 release key** for binaries). Clients pin the
public key at build time and refuse unsigned/mismatched updates.

## 4. Per-runtime plan

### 4.1 Autopilot (Electrobun) — finish + default-on
1. **Signing (gating, NEEDS-OWNER):** codesign + notarize the `.app` (Developer
   ID + notary creds; `apps/autopilot-desktop/scripts/notarize-macos.sh`,
   `.secrets/appstoreconnect.env`) so an auto-applied update isn't
   Gatekeeper-quarantined. This is the #5027 remainder.
2. **Default-on + cadence:** enable the Electrobun updater by default; check on
   launch **and** on a background interval (default ~6 h); apply silently and
   activate on next launch (or prompt-then-relaunch for a foreground update).
   Opt-out: `OA_AUTOPILOT_AUTOUPDATE=0` + a Settings toggle (still checks, doesn't
   apply).
3. **Channels:** `stable` default for downloads; `canary` opt-in for the team.
   The **bundled Pylon node updates with the app** (no separate Pylon update on
   the desktop install).
4. **Announced==admitted:** pin the OTA/`.app` the launch announcement links to
   the build the run admits + pays (RESEARCH_PLAN §6).
5. **Linux:** same `build:canary|stable` channels; document the feed + applier.

### 4.2 Pylon binary — the new build
1. **Ship a standalone binary:** `bun build apps/pylon/src/index.ts --compile
   --target=bun-<platform> --outfile pylon` (the headless bundle already builds,
   #5037 — zero `@opentui`). Produces a single self-contained `pylon` executable
   per platform (darwin-arm64/x64, linux-x64/arm64). Keep the npm package + the
   app-bundled form; the **binary is the headline auto-updating artifact**.
2. **Self-updater (default ON):** a `pylon` background routine + a `pylon update`
   command. On node start and on an interval (default ~6 h) it fetches
   `pylon/<channel>/<platform>/feed.json`, compares versions, and if newer:
   download → **verify ed25519 signature + sha256** → write to a temp path →
   **atomic rename over the running binary** (`self-replace` semantics) →
   schedule relaunch at the next safe point (no in-flight session). Modeled on
   `self_update`/`axoupdater`. Opt-out: `PYLON_AUTOUPDATE=0` (and the existing
   `updatePolicy` config) — still checks, surfaces availability, doesn't apply.
3. **Replace the stub policy:** swap `updatePolicy.channel: "github-releases"` →
   the `updates.openagents.com/pylon/...` feed (GH Releases as a mirror only;
   the canary-prerelease caveat from §2 is why the primary feed is ours). Drop
   `dashboardPolling` (TUI gone).
4. **Safety for a node mid-work:** never swap during an active session/lease;
   drain → apply → relaunch; on relaunch the node re-adopts its `.pylon-*` home
   and resumes. Record an update receipt (version from→to, hash) in the node log.
5. **`pylon update --json`** surfaces current/available/applied for agent
   steerability (consistent with `pylon.agent_steerable_cli.v1`).

### 4.3 Rust binaries — uniform convention
For any Rust binary we ship (psionic/executor runtime, tap-ldk/ldk-node tooling):
embed **`axoupdater`** (if built via `cargo-dist`) or **`self_update`** (general),
pointed at the same `updates.openagents.com/<binary>/<channel>/<platform>/feed.json`
convention and the same ed25519 release key (zipsign-compatible). Default ON,
same opt-out env convention (`OA_<NAME>_AUTOUPDATE=0`). This keeps one mental
model + one signing trust root across Electrobun, Bun-compiled, and Rust.

## 5. Default-on rollout & safety policy (applies to all three)

- **On by default**, opt-out only; even opted-out clients check + surface.
- **Channels:** `canary` (team, fast) promotes to `stable` (everyone) after a
  soak. Stable is the default download channel.
- **Staged rollout:** `rolloutPercent` in the manifest; clients hash their
  install id mod 100 to gate adoption; ramp 1% → 10% → 100%.
- **Rollback / kill-switch:** `yanked: true` and a `minVersion` floor; a bad
  release is pulled from the feed and clients refuse it; `oa-updates` can serve a
  pinned-older stable.
- **No downgrade** unless `minVersion` forces it; monotonic version compare.
- **Health gate:** publish to `stable` only after the canary soak shows no crash
  spike (tie to crash/health telemetry where it exists).

## 6. Security

- **Signing:** Apple Developer ID + notarization for the `.app`; an OpenAgents
  **ed25519 release-signing key** for every binary manifest+asset (zipsign for
  Rust, the same key verified in Pylon's Bun updater). Public keys pinned in the
  client at build time.
- **Verify before apply:** signature **and** sha256 of the downloaded asset;
  reject on mismatch; TLS-only fetch from `updates.openagents.com`.
- **Atomic apply:** temp-write + rename so a crash mid-update never leaves a
  half-written binary/app; keep the previous version for one-step rollback.
- **No secrets in artifacts/manifests** (the standing rule): no tokens, wallet
  material, or private paths in any published bundle or feed.

## 7. Work breakdown (issues to file)

- **Epic — OTA auto-update, default-on, for Autopilot + Pylon.**
  - Autopilot: finish signing/notarization (#5027 remainder) + default-on updater
    + background cadence + Settings opt-out + Linux feed.
  - Pylon binary: `bun build --compile` per-platform binary + release pipeline.
  - Pylon self-updater: feed client, ed25519+sha256 verify, atomic self-replace,
    drain-then-relaunch, default ON + `PYLON_AUTOUPDATE` opt-out, `pylon update
    --json`.
  - `oa-updates`: extend beyond `desktop/` to serve `pylon/<channel>/<platform>/`
    (and a generic `<binary>/...`) feeds + signed manifests + rollout/yank.
  - Release-signing key: provision the OpenAgents ed25519 key (NEEDS-OWNER), wire
    signing into `oa-updates` publish + key pinning into clients.
  - Rust convention: document + (when a Rust binary ships) wire `axoupdater`/
    `self_update` to the same feed.
  - Product promise: `releases.ota_autoupdate.v1` (planned) — every client
    auto-updates by default; flips green when Autopilot + Pylon both self-apply a
    signed update end to end.

## 8. Open decisions (owner)

1. **Release-signing key** custody (ed25519) + Apple Developer ID/notary creds —
   both NEEDS-OWNER; OTA can't be trustworthy without them.
2. **Pylon binary vs npm as the primary install** — recommend the
   `bun --compile` binary as the headline auto-updating artifact (matches "Pylon
   binary" + the agent-steerable path), npm as a secondary.
3. **Check cadence** (launch + ~6 h proposed) and whether desktop updates apply
   silently-on-next-launch vs prompt-then-relaunch.
4. **Telemetry** to gate canary→stable promotion (crash/health signal source).

## Sources
- [Electrobun — Updates](https://blackboard.sh/electrobun/docs/guides/updates/)
- [Electrobun — Bundling & Distribution](https://blackboard.sh/electrobun/docs/guides/bundling-and-distribution/)
- [self_update (crates.io)](https://crates.io/crates/self_update)
- [axoupdater (crates.io)](https://crates.io/crates/axoupdater)
- [cargo-dist (crates.io)](https://crates.io/crates/cargo-dist)
- Repo: `apps/oa-updates/`, `apps/autopilot-desktop/README.md` +
  `electrobun.config.ts`, `apps/pylon/src/bootstrap.ts` (`updatePolicy`), #5027.
