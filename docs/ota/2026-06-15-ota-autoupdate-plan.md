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

## 0b. The second rule: the update path is open source, in one repo

**The entire OTA path — the update service, the feed/manifest format, the
signature-verify, and both clients — is open source in the one public repo
`OpenAgentsInc/openagents`, and stays that way.** This is a deliberate *trust*
property, not an afterthought: the OpenAgents thesis is "don't read a blog about
whether it's safe — point your agent at our code." An updater that can replace a
running binary on a stranger's machine is the single most security-sensitive
thing we ship; it must be the *most* auditable. Closed-source auto-update is a
backdoor by reputation; open-source auto-update is a checkable contract.

Verified 2026-06-15:
- **`apps/oa-updates`** (the OTA service) is already in the **public** monorepo
  (`github.com/OpenAgentsInc/openagents`) — not in the private `cloud/` repo. A
  `grep` of `cloud/` finds **no** OTA/update/feed code. Good; keep it there.
- Both clients (Autopilot `apps/autopilot-desktop`, Pylon `apps/pylon`) are in the
  same public monorepo. So the *whole* update path is already auditable in one
  place.

Rules going forward:
- **One repo:** the public monorepo is the single home for the OTA service + both
  clients + the manifest/signing/verify code. Do **not** split any part of the
  update backend into `cloud/` or any private code repo.
- **What stays out of the repo is data, not code:** the ed25519 **private**
  signing key and GCP/Cloud Run deploy creds live in the secrets store / CI, never
  in any repo. The **public** verification key is committed (clients pin it). Code
  + manifests + the public key are open; only the private key and bindings are not.
- **Cloud-repo advice (broader than OTA):** OTA needs nothing from `cloud/`, so
  there is nothing to move. For the wider "make the backend open source / one
  repo" goal: the product backend (`apps/openagents.com` Worker) is *already*
  public; the only closed code is `cloud/` (managed-node/workroom/private-fleet/
  accounting). Recommendation — fold `cloud/`'s **infra/protocol** surfaces into
  the public monorepo behind clear boundaries and keep only genuinely-private
  *operational policy* + secrets out via env (not a parallel private code repo);
  fully publishing `cloud/` is a separate owner call because it also contains
  private fleet/accounting logic. **None of that blocks OTA** — the OTA backend is
  already where it belongs.

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
  desktop/<channel>/feed.json            # Autopilot .app — Electrobun-native (exists)
  pylon/<channel>/<platform>/feed.json   # Pylon JS/Bun binary — new
  psionic/<channel>/<platform>/feed.json # Psionic Rust binary + models — new (Rust path)
  <rust-binary>/<channel>/<platform>/feed.json  # other future Rust binaries
```

### 3.5 Artifact separation — what is (and is NOT) in each binary

This is the crux, and the answer is **three independently-updated artifacts**, not
one fat binary:

- **`bun build --compile` produces a JS/Bun binary only.** It embeds the **Bun
  runtime + Pylon's TypeScript** (the node orchestrator: control server, wallet
  client, sessions, CLI). It does **not** and cannot embed Rust. So the Pylon
  binary is the *orchestrator*, ~2 MB-class, pure JS/Bun.
- **Psionic (the Rust ML substrate) is NOT inside the Pylon binary.** Pylon
  *installs and connects to* Psionic as a **separate downloaded binary +
  model artifacts**: `src/psionic-install.ts` already fetches a release manifest
  (`openagents.psionic.model_artifact_manifest.v0.3`, `binary:{url,sha256,…}`),
  downloads, **verifies sha256**, and lands it in the Pylon home; `src/psionic-
  connector.ts` talks to it as a separate process. "Pylon packages Psionic" means
  Pylon *orchestrates* Psionic's install/connect/update — not static linking. The
  Rust lives in the Psionic binary, shipped on its own feed.
- **Other native bits are also separate processes, not in the bun binary.** The
  MDK wallet runs as a daemon (`wallet status` reports `daemonOnline`), not an
  embedded addon. Anything native (Psionic, wallet daemon, any `.node` addon)
  ships and updates outside the `bun --compile` artifact. (Open item §8: confirm
  no Pylon JS dep pulls a native addon that `bun --compile` can't embed; if one
  does, it ships alongside the binary like Psionic.)
- **Pylon is separate from Autopilot.** Autopilot is the Electrobun `.app`
  (cockpit) that **bundles a Pylon JS node** and updates it via the app's
  Electrobun OTA (#5027). Standalone Pylon is the `bun --compile` binary with its
  own self-updater. **In both cases Psionic is installed/connected at runtime
  separately** (the `.app` does not bundle the multi-GB Psionic Rust + models).

So three OTA lanes, each default-on and independent:
1. **Autopilot `.app`** → Electrobun OTA (carries the bundled Pylon JS node).
2. **Pylon binary** (JS/Bun) → its own self-updater.
3. **Psionic** (Rust binary + models) → its own feed, **auto-updated by Pylon**
   via the `pylon psionic` install/update path. Pylon coordinates it because
   Pylon owns the Psionic connector + version compatibility.

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
   per platform (darwin-arm64/x64, linux-x64/arm64). **This is the JS/Bun
   orchestrator only** — it embeds the Bun runtime + Pylon TS, **not** Rust/
   Psionic/native daemons (those are separate, §3.5). Keep the npm package + the
   app-bundled form; the **binary is the headline auto-updating artifact**.
   Confirm during the build that no JS dep needs a native addon `bun --compile`
   can't embed (§8); if one does, ship it beside the binary like Psionic.
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

### 4.3 Rust binaries — Psionic is the concrete case
The first (and most important) Rust binary is **Psionic** itself, and it already
has the seed of an OTA path: `src/psionic-install.ts` downloads a manifest-described
binary + models and verifies sha256. Finish it into a real auto-updater:
1. **Feed it from `updates.openagents.com/psionic/<channel>/<platform>/feed.json`**
   (the existing `model_artifact_manifest.v0.3` is the manifest seed — add
   `channel`, `rolloutPercent`, `yanked`, `minVersion`, and an **ed25519
   signature** alongside the sha256).
2. **Pylon auto-updates Psionic, default ON.** Pylon owns the connector + version
   compatibility, so the Pylon node checks the Psionic feed on start/interval and
   runs the existing install/verify path to upgrade the Psionic binary + models
   (drain any in-flight Psionic work first). Opt-out: `PYLON_PSIONIC_AUTOUPDATE=0`.
   `pylon psionic status --json` surfaces current/available.
3. **Build/sign Psionic releases** with `cargo-dist` + **`axoupdater`** (or
   `self_update`/zipsign) so a *directly-run* Psionic binary can also self-update,
   and so the same ed25519 trust root + channel/manifest convention covers it.

For any other Rust binary (executor runtime, tap-ldk/ldk-node tooling): same
recipe — `cargo-dist`/`axoupdater` (or `self_update`), the same
`updates.openagents.com/<binary>/<channel>/<platform>/feed.json` convention, the
same ed25519 key, default ON, `OA_<NAME>_AUTOUPDATE=0` opt-out. One mental model +
one signing trust root across Electrobun, Bun-compiled, and Rust.

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

## 6b. Provenance — everything is signed by our infra, verified fail-closed

> **Status 2026-06-15 — signing key provisioned + backed up.** The OpenAgents
> ed25519 release/provenance key exists: private in `~/work/.secrets/openagents-
> release-signing.env` (600, gitignored), **backed up to GCP Secret Manager**
> (`openagents-release-signing-key`, project `openagentsgemini`, hash-verified),
> public pinned at `apps/oa-updates/keys/release-pubkey.json` (kid
> `2dbe811d19f67528`). Signer + fail-closed verifier shipped
> (`apps/oa-updates/scripts/sign-release.ts` / `verify-release.ts`) and tested
> (sign→verify pass, tamper→reject, GCP-Secret-Manager key path works with no
> local file). Runbook: `apps/oa-updates/docs/release-signing-runbook.md`. Remaining
> = wire signatures into the published feeds + client pinning + KMS migration
> (OTA epic #5039).

Owner requirement: **users and Pylons must be able to auto-verify that artifacts
and authoritative responses come only from OpenAgents infra, and fail otherwise.**
The correct way to guarantee "from our infra" is **cryptographic signing, not
hosting/IP/DNS.** Where a service runs (Cloudflare vs GCP) is an availability
choice; *who signed it* is the trust boundary. Tying trust to a host/CDN is
brittle (DNS hijack, MITM, mirror); a pinned signature is checkable and
hosting-independent. So:

- **One signing root, the private key never leaves our infra.** Hold the
  OpenAgents **ed25519 release/provenance key in a cloud KMS / HSM** (GCP Cloud
  KMS, or Cloudflare's key store for the Worker) so only our deployed services
  (with KMS sign permission) can produce a valid signature — *that* is the literal
  "comes only from our infra" guarantee. Nothing outside our infra can mint a
  valid signature even if it spoofs the host. The **public** key(s) are committed
  and pinned in clients.
- **Sign every authoritative output, not just updates:** OTA manifests + binaries
  (§6), the **product-promise registry**, **training-run manifests**,
  **dispatched-work payloads**, **settlement / promise-transition receipts**, the
  **capability manifest / `AGENTS.md`**, and (ideally) generic API responses via a
  detached signature header. Each signs a canonicalized body + version +
  timestamp (anti-replay/anti-rollback).
- **Clients pin the public key and FAIL CLOSED.** Pylon and Autopilot embed the
  OpenAgents public verify key(s) at build time, verify the signature on
  everything authoritative, and **refuse/error on a missing or invalid
  signature** — never "trust because it came over TLS from a familiar URL." This
  is the same posture as auto-update being default-on: **verification is mandatory
  by default; there is no shipped build that skips it.**
- **Key set, rotation, transparency.** Publish a rotatable key set (JWKS-style):
  clients pin a long-lived **root** that signs short-lived subkeys, so we rotate
  without reflashing clients. Add a **transparency log** of signed releases
  (sigstore/rekor-style) so anyone can independently audit that an artifact was
  signed by our root — open + checkable, per the OpenAgents thesis.
- **Reuse what exists, unify under the root.** Pylon already carries a Nostr
  keypair; the worker has L402 credential signing and some ed25519/nacl;
  `oa-updates` already does desktop code-signing. Keep those, but make the
  **KMS-backed provenance root** the single trust anchor for "is this from
  OpenAgents," and pin it everywhere.

### GCP / hosting note
The backend can run on our GCP — `oa-updates` is already Cloud Run and compute is
on GCE. The product API currently runs on **Cloudflare Workers**; consolidating it
onto GCP (Cloud Run) is a possible migration but is an **ops/availability**
decision, **not** what provides verifiability. Verifiability comes from the
KMS-rooted signing + pinned fail-closed verification above, which holds regardless
of whether a given service sits on Cloudflare or GCP. Recommendation: decide
hosting consolidation separately; ship the signing-provenance layer now, because
that — not the datacenter — is what lets a Pylon prove a payload is ours.

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
  - **Provenance root + fail-closed verification (§6b):** hold the signing key in
    KMS/HSM (never exported); sign authoritative outputs beyond OTA (registry,
    run manifests, dispatched work, receipts, capability manifest); make Pylon +
    Autopilot pin the public key and **refuse unsigned/invalid** by default;
    publish a rotatable JWKS key set + a transparency log.
  - **Psionic (Rust) auto-update:** promote `psionic-install` into a default-on
    auto-updater (channel/signed/rollout feed at `updates.openagents.com/psionic/`,
    Pylon checks + upgrades the Psionic binary+models, drain-first); build/sign
    Psionic releases via `cargo-dist`/`axoupdater` so a direct Psionic binary
    self-updates too.
  - Rust convention: document + (when another Rust binary ships) wire
    `axoupdater`/`self_update` to the same feed.
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
5. **Native deps in `bun --compile`:** confirm no Pylon JS dependency needs a
   native `.node` addon the bun binary can't embed (the MDK wallet is a separate
   daemon, Psionic is separate — so likely clean). Anything that does ships beside
   the binary, like Psionic.
6. **Psionic↔Pylon version compatibility:** since they update independently,
   define a compatibility range (Pylon declares the Psionic versions it supports;
   refuse/auto-bump on mismatch) so an auto-update of one doesn't break the pair.

## Sources
- [Electrobun — Updates](https://blackboard.sh/electrobun/docs/guides/updates/)
- [Electrobun — Bundling & Distribution](https://blackboard.sh/electrobun/docs/guides/bundling-and-distribution/)
- [self_update (crates.io)](https://crates.io/crates/self_update)
- [axoupdater (crates.io)](https://crates.io/crates/axoupdater)
- [cargo-dist (crates.io)](https://crates.io/crates/cargo-dist)
- Repo: `apps/oa-updates/`, `apps/autopilot-desktop/README.md` +
  `electrobun.config.ts`, `apps/pylon/src/bootstrap.ts` (`updatePolicy`), #5027.
