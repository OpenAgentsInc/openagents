# OpenAgents Updates & Deployment Infrastructure — Build-Our-Own Audit

Date: 2026-06-13
Status: audit / design proposal. No code or invariant changes here. Proposes a
new Cloud capability ("OpenAgents Updates") and a phased plan to own the
over-the-air (OTA) update path — and, optionally later, native binary builds —
instead of depending on Expo Application Services (EAS).

Repo home: `cloud` (this is private managed-infra work that extends OpenAgents
Cloud). Client-side config changes land in the public `openagents` monorobo
(`clients/mobile/AutopilotRemoteControl`).

---

## 1. Why (motivation)

We are shipping the Autopilot Remote Control mobile app and building the M6
"self-driving loop" (a message from the phone → Pylon → a coordinator agent fans
out coding agents → the result is auto-shipped back to the phone as an OTA update
or a new build). Today the ship step rides on **EAS**, and that creates two hard
external dependencies we don't control:

1. **EAS quotas / limits.** Build minutes, update bandwidth, and concurrency are
   metered by a third party. The self-driving loop is designed to ship *often*
   (every accepted intent), so a per-update/per-build external quota is a
   structural throttle on the core product loop.
2. **Account/org coupling.** The Expo MCP `build_run` (server-side git build)
   requires a connected GitHub repo, which in turn wants a GitHub **org**. The
   owner does not want GitHub in the deploy path at all (no repo connection, no
   push triggers). We already proved the loop can be **GitHub-free** by shelling
   out to the local-upload `eas build` CLI — but that still routes through EAS.

The strategic fix: **own the OTA update channel.** OTA updates are the frequent,
high-leverage case (JS-only changes — the bulk of what the loop ships), and the
Expo Updates client speaks an **open, documented HTTP protocol** to whatever
server `updates.url` points at. Replicating that server is cheap and removes the
third-party dependency for the 90% case. Native binary builds are the expensive
10% and can stay on the EAS CLI near-term.

This audit is the "what would it take to clone it" answer, grounded in a deep
read of our Expo clone at `projects/repos/expo`.

---

## 2. TL;DR & recommendation

- **Build "OpenAgents Updates" — a self-hosted EAS-Update-compatible OTA server —
  on OpenAgents Cloud.** It is just: an HTTP **manifest endpoint** + a
  content-addressed **asset store** + a small **publish path** + optional
  **code signing**. The `expo-updates` client is server-agnostic; pointing
  `updates.url` at our endpoint needs **no client fork**, only config.
- **The protocol is fully specified** (Expo Updates v1) and reverse-engineered
  below from the client source. There is even an upstream reference
  implementation pattern (`custom-expo-updates-server`) confirming this is a
  supported, documented integration — the `expo-updates` runtime is MIT.
- **Reuse Cloud primitives we already have:** content-addressed `sha256` artifact
  storage (`oa-workroomd` ARTIFACT_CLOSEOUT), the signer-ref + signed-receipt
  model (`oa-node` SIGNED_UPDATES), capability gateways / managed ingress, and
  the refs-only receipt discipline. An OTA server is largely an assembly of these.
- **Build binaries LOCALLY on our own hardware — no Expo cloud build at all.**
  We do **not** need EAS's hosted build service. Near-term: `eas build --local`
  runs the entire build on this Mac (no build minutes, no GitHub, no server-side
  git) and emits an `.ipa`. End state (fully Expo-cloud-free): `expo prebuild`
  generates the native `ios/`/`android/` projects and we build with
  `xcodebuild`/`fastlane gym` (iOS) + Gradle (Android), sign with our Apple cert,
  and **send the binary straight to Apple** via the App Store Connect API
  (`fastlane pilot` / `xcrun altool` / direct ASC upload). Keep it simple: our
  local+cloud infra builds and submits; Apple is the only external dependency.
- **Phase it:** (1) OTA MVP (unsigned JSON manifest + asset store + publish CLI),
  (2) code signing + rollback + channels, (3) multipart + bsdiff + CDN + metered
  receipts, (4) **local binary builds** — `eas build --local` first, then a
  fully Expo-cloud-free `prebuild → xcodebuild/fastlane → ASC upload` path.
- **Deployment:** the MVP server (`apps/oa-updates`, Bun) is proven locally
  (real `expo export` served over HTTP). Initial public test → **GCloud Cloud
  Run** (`gcloud run deploy`, containerized; stable `*.run.app` URL usable
  directly as `updates.url`), with **`updates.openagents.com`** CNAME'd to it
  (owner/DNS step — the current Cloudflare token is zone-read-only). Prod target
  stays GCloud; later port the server to Cloud Rust per §5 for the production
  service. A local Cloudflare quick-tunnel is an acceptable throwaway for a first
  on-device test if Cloud Run isn't ready.

This directly de-risks M6: the coordinator's OTA arm (CL-38) publishes to **our**
server with no Expo quota, no GitHub, instant, and **signed** — and signing is
what makes an autonomous push to a live device safe (a forged update can't be
loaded), reinforcing CL-41 loop-safety.

---

## 3. How EAS Update actually works (Expo Updates v1 protocol)

Reverse-engineered from `projects/repos/expo` — the `expo-updates` native client
(`packages/expo-updates/android/.../FileDownloader.kt`,
`ios/.../FileDownloader.swift`) and the spec at
`docs/pages/technical-specs/expo-updates-1.mdx`. The server is whatever
`updates.url` resolves to; the client drives the contract.

### 3.1 Manifest request (client → server)

`GET <updates.url>` with headers:

Required:
- `Expo-Platform: ios | android`
- `Expo-Protocol-Version: 1`
- `Expo-API-Version: 1`
- `Expo-Updates-Environment: BARE`
- `Accept: multipart/mixed,application/expo+json,application/json`
- `EAS-Client-ID: <uuid>` (stable per install)
- `Expo-JSON-Error: true`

Conditional / stateful:
- `Expo-Runtime-Version: <string>` — the build's runtime version (see §3.6).
- `Expo-Channel-Name: <channel>` — e.g. `production` / `preview` (see §3.5).
- `Expo-Current-Update-ID: <uuid>` / `Expo-Embedded-Update-ID: <uuid>` — what's
  currently running / embedded, lowercased.
- `Expo-Recent-Failed-Update-IDs: <sfv list>` — for server-side rollback logic.
- `Expo-Expect-Signature: sig, keyid="main", alg="rsa-v1_5-sha256"` — present iff
  the build is configured for code signing (see §3.4).
- `Expo-Extra-Params: <sfv dict>` — params the client persisted from prior
  `expo-server-defined-headers` responses.

### 3.2 Manifest response (server → client)

Content negotiated to `application/json`, `application/expo+json`, or
`multipart/mixed`. Response headers:
- `expo-protocol-version: 1` (required), `expo-sfv-version: 0` (required)
- `expo-manifest-filters: <sfv dict>` — lets the client filter stored updates by
  `metadata`.
- `expo-server-defined-headers: <sfv dict>` — headers the client stores and
  re-sends.
- `expo-signature: sig="<b64>", keyid="main", alg="rsa-v1_5-sha256"` — when signed.
- `cache-control: private, max-age=0` (manifests are dynamic).

**Manifest JSON** (`expo-updates-1.mdx`):
```ts
type Manifest = {
  id: string                         // UUID, unique per manifest
  createdAt: string                  // ISO-8601
  runtimeVersion: string             // must match the build to be loadable
  launchAsset: Asset                 // the JS bundle entry point
  assets: Asset[]                    // images/fonts/etc.
  metadata: { [k: string]: string }  // filterable
  extra: { [k: string]: any }        // e.g. { eas: { projectId } }
}
type Asset = {
  hash?: string        // base64url(sha256) of the bytes — client verifies
  key: string          // stable asset id
  contentType: string  // e.g. application/javascript
  fileExtension?: string
  url: string          // where to fetch the bytes
}
```

**Multipart/mixed** form (parts are individually signable):
```
--boundary
Content-Disposition: form-data; name="manifest"
Content-Type: application/json
expo-signature: sig="…", keyid="main", alg="rsa-v1_5-sha256"

{manifest JSON}
--boundary
Content-Disposition: form-data; name="directive"
Content-Type: application/json
expo-signature: …

{directive JSON}
--boundary
Content-Disposition: form-data; name="extensions"
Content-Type: application/json

{ "assetRequestHeaders": { "<assetKey>": { "Authorization": "…" } } }
--boundary
Content-Disposition: form-data; name="certificate_chain"
Content-Type: application/json

{PEM chain}
--boundary--
```

**Directives** (in lieu of a manifest):
- `{ "type": "noUpdateAvailable" }` — nothing newer for this runtime/channel.
- `{ "type": "rollBackToEmbedded", "parameters": { "commitTime": "<iso>" } }` —
  tell the client to fall back to the build's embedded bundle (our "undo" lever).

### 3.3 Asset download + verification

`GET <asset.url>` (per `launchAsset` and each `assets[]` entry), with the same
`Expo-*` identity headers + `Accept: <asset.contentType>` + optional
`assetRequestHeaders[key]`. Optional bsdiff patching via `A-IM: bsdiff` request /
`im: bsdiff` + `expo-base-update-id` response. The client computes
`base64url(sha256(bytes))` and **rejects** the asset if it doesn't equal
`asset.hash`. So assets are immutable + content-addressed — exactly the
`oa-workroomd` `artifacts/sha256/<digest>` model. Serve with
`cache-control: public, max-age=31536000, immutable`.

### 3.4 Code signing

- Build embeds `codeSigningCertificate` (PEM) + `codeSigningMetadata`
  (`{ alg: "rsa-v1_5-sha256", keyid: "main" }`).
- Client sends `Expo-Expect-Signature`; server returns `expo-signature` over the
  **manifest body bytes** (RSA-SHA256, base64). Client verifies against the
  embedded cert / chain before loading.
- Property we want: **a build will only load updates signed by our key.** Even
  our own update server cannot push a manifest the app accepts without the
  signing key. This is the cryptographic backstop for autonomously shipping to a
  live device (CL-41).

### 3.5 Channels → branches

`Expo-Channel-Name` is **not** part of the core protocol — it is a request header
the server uses to route. EAS maps `channel → branch → latest update`. We
replicate trivially: store updates per `(branch, runtimeVersion, platform)` and
keep a `channel → branch` table (e.g. `production → production`,
`preview → preview`). `eas.json` already sets these channels on our build
profiles.

### 3.6 runtimeVersion / fingerprint (the safety gate)

The client only loads a manifest whose `runtimeVersion` **matches the build's**.
With `runtimeVersion: { policy: "fingerprint" }` (already set in our
`app.config.ts`), the value is a hash of the native layer + deps, written at build
time and read from a fingerprint file (`file:fingerprint` sentinel). Our build #4
reports `runtimeVersion: "d36a2a5bb2320ce37a89c965823c91887d7d6bdb"`. The publish
side must stamp updates with the **same** fingerprint, or the app silently won't
load them. This is exactly the OTA-vs-rebuild signal CL-37 classifies: matching
fingerprint ⇒ OTA-eligible; changed ⇒ must rebuild.

---

## 4. What's cheap vs expensive to replicate

| Capability | Cost to own | Verdict |
| --- | --- | --- |
| **OTA manifest endpoint** (§3.1–3.2) | Low — one HTTP route + content negotiation | **Own it (Phase 1)** |
| **Asset store + serving** (§3.3) | Low — content-addressed blob store + CDN; we already do `artifacts/sha256/<digest>` | **Own it (Phase 1)** |
| **Publish path** (export bundle+assets, hash, upload, write manifest row) | Low–Med — `expo export` already produces this locally | **Own it (Phase 1)** |
| **Code signing + rollback directives** (§3.4, §3.2) | Med — RSA signing over manifest, key custody (reuse SIGNED_UPDATES signer-ref) | **Own it (Phase 2)** |
| **Channels/branches/rollouts** (§3.5) | Low — small metadata table | **Own it (Phase 2)** |
| **Multipart + bsdiff patches + CDN tuning** | Med | **Phase 3** |
| **iOS/Android binary BUILD** (compile signed `.ipa`/`.aab`) | Med — needs a Mac w/ Xcode (we have one) + Apple certs; **`eas build --local` now**, `prebuild`+`xcodebuild`/`fastlane` later | **Build locally on our hardware — no Expo cloud** |
| **Store SUBMIT** (App Store Connect / Play) | Low–Med — it's just the ASC/Play API (we already have an ASC API key) | **Own it — direct ASC upload (`fastlane pilot`/`xcrun altool`/API)** |

The asymmetry: **OTA is an HTTP+blob problem; binary builds are a
local-toolchain problem.** Both are ownable on our own machines — OTA on Cloud,
binaries on a local Mac — with **Apple as the only external dependency**. OTA
still carries the frequent case (every JS-only change the loop ships); local
builds handle the infrequent native/fingerprint change.

---

## 5. Proposed architecture — "OpenAgents Updates" on Cloud

A new Cloud service (working name `oa-updates`, or a capability of `oa-node`'s
gateway surface) exposed at e.g. `https://updates.openagents.com`.

### 5.1 Endpoints
- `GET /:projectId/manifest` — reads the `Expo-*` headers, resolves
  `channel → branch`, finds the latest published update for
  `(branch, runtimeVersion, platform)`, and returns a signed multipart/mixed
  manifest — or a `noUpdateAvailable` / `rollBackToEmbedded` directive.
- `GET /assets/:sha256` (or signed CDN URLs) — serves an immutable asset blob;
  `cache-control: public, max-age=31536000, immutable`.
- `POST /:projectId/publish` (authenticated, operator/coordinator only) — accepts
  a published update (manifest + asset digests) and inserts the rows; or this is
  done by a CLI that writes directly to the store (see §7).

### 5.2 Data model
- `Update { id, projectId, platform, branch, runtimeVersion, createdAt,
  launchAsset, assets[], metadata, extra, signature? }`
- `Branch { projectId, name }` and `ChannelMapping { projectId, channel, branch }`
- `Rollback { projectId, branch, runtimeVersion, embeddedCommitTime }` (directive)
- Assets stored **content-addressed by `sha256`** (reuse the
  `artifacts/sha256/<digest>` convention from `oa-workroomd` ARTIFACT_CLOSEOUT).

### 5.3 Storage / runtime mapping to existing Cloud primitives
- **Assets** → content-addressed object store. Cloud already content-addresses
  artifacts (`docs/oa-workroomd/ARTIFACT_CLOSEOUT.md`). Back it with the existing
  object store (GCS today; R2/S3 + CDN recommended for global asset serving).
- **Manifest metadata** → a small DB (the file-backed JSON/JSONL MVP pattern is
  fine to start; graduate to sqlite/Postgres/D1 for the latest-per-branch query).
- **Signing** → reuse the **signer-ref** model from
  `docs/oa-node/SIGNED_UPDATES.md` (`local-keychain://openagents/cloud/release`
  style refs; key material never stored in repo/receipts). Sign the manifest
  body; publish the cert chain; embed the cert in the app.
- **Serving auth / ingress** → the managed-ingress + capability-gateway model
  (`docs/oa-workroomd/MANAGED_PREVIEW_INGRESS.md`,
  `LINK_LOCAL_GATEWAYS.md`) already gives us endpoint tokens (stored as `sha256:`
  digests) and audited access — directly reusable if we want private/gated
  updates.
- **Receipts** → emit refs-only `update_published` / `update_rolled_back`
  receipts (mirrors `update-receipts.jsonl` in SIGNED_UPDATES and the
  resource-usage-receipt model). These are exactly the per-intent ship receipts
  CL-41 wants.

### 5.4 Contract
Add an `openagents.mobile_update.v1` contract to
`openagents-cloud-contract` (manifest row + publish receipt), consistent with the
repo's "typed schema, refs-only, no raw secrets" discipline.

---

## 6. Client changes (minimal — no fork)

In `clients/mobile/AutopilotRemoteControl/app.config.ts`:
- `updates.url` → `https://updates.openagents.com/<projectId>/manifest` (instead
  of `https://u.expo.dev/<projectId>`).
- Add `updates.codeSigningCertificate` + `updates.codeSigningMetadata`
  (`{ alg: "rsa-v1_5-sha256", keyid: "main" }`) once Phase 2 signing is live.
- Keep `runtimeVersion: { policy: "fingerprint" }` and the `eas.json` channels.

That's the whole client change. `expo-updates` is server-agnostic and MIT; it
already speaks the protocol in §3. A build pointed at our URL fetches manifests
from us. (Note: changing `updates.url` is a native/config change → it itself
requires one rebuild to take effect; after that, JS OTAs flow from our server.)

---

## 7. Publish path (our `eas update` equivalent)

The publish side is small and we already validated its core locally:
1. `npx expo export --platform ios --output-dir dist` — produces the Hermes/JS
   bundle + assets + `metadata.json` (we ran this; it emitted
   `_expo/static/js/ios/index-*.hbc` + assets). Same for android.
2. Hash each asset + the launch bundle: `base64url(sha256(bytes))`.
3. Upload blobs to the content-addressed store (`/assets/<sha256>`).
4. Compute the **runtimeVersion fingerprint** (must equal the build's — use
   `expo-updates` fingerprint generation) and stamp the manifest.
5. Insert an `Update` row for `(branch, runtimeVersion, platform)`; sign the
   manifest body (Phase 2).
6. Emit an `update_published` receipt.

Wrap as a CLI `oa-update publish --branch <channel> --platform ios|android`. The
M6 coordinator's CL-38 arm shells out to this instead of `eas update` — fully on
our infra, no Expo, no GitHub.

---

## 8. Native builds — build locally, no Expo cloud

Owner directive: **do everything on our local + cloud infra and just send
binaries to Apple. No dependency on Expo's cloud build.** This is very doable —
we have a Mac with Xcode, an Apple Developer account, and an ASC API key. Two
steps, simplest first:

- **Step A — `eas build --local` (validate the local toolchain now).** Runs the
  full EAS build pipeline **on this Mac** instead of EAS servers: no build
  minutes, no GitHub, no server-side git. Produces a signed `.ipa` locally. Still
  uses the `eas` CLI as a convenient wrapper (and can use local
  credentials), but **nothing runs on Expo's cloud**. Lowest-effort way to prove
  we can build on our own hardware. Command:
  `eas build --platform ios --profile production --local`.
- **Step B — fully Expo-cloud-free (`prebuild → xcodebuild/fastlane`).** Drop the
  `eas` wrapper entirely: `npx expo prebuild --platform ios` generates the native
  `ios/` Xcode project; build + sign with `fastlane gym` (or `xcodebuild
  -archive -exportArchive`) using our distribution cert + provisioning profile;
  Android via `expo prebuild --platform android` + `./gradlew bundleRelease`.
  This removes EAS from the build path completely.
- **Submit — straight to Apple.** Upload the `.ipa` to App Store Connect /
  TestFlight via the **ASC API directly** (`fastlane pilot upload`, `xcrun
  altool --upload-app`, or the ASC REST API) using the API key we already hold
  (id `4XNA2QCAQ7`). No `eas submit` needed.

Net: **OTA on Cloud + local binary builds + direct ASC upload = zero Expo-cloud
dependency.** Apple is the only external party (unavoidable for iOS). Keep it
simple — `eas build --local` is the pragmatic first rung; graduate to
prebuild+fastlane when we want the `eas` CLI fully out of the loop. Credential
custody (the Apple distribution cert + provisioning profile + ASC key) moves to
our keychain/secret store under the SIGNED_UPDATES signer-ref discipline.

---

## 9. How this de-risks the self-driving loop (M6)

- **CL-37 (classify):** compares the merged tree's runtime fingerprint to the
  deployed build's. Unchanged ⇒ OTA; changed ⇒ rebuild. (Independent of who hosts
  the server.)
- **CL-38 (auto OTA):** publishes to **OpenAgents Updates** via `oa-update
  publish` — no Expo quota, no GitHub, instant, **signed**. The phone pulls it on
  next launch.
- **CL-39 (auto rebuild):** runs a **local build** on our Mac (`eas build
  --local` now; `prebuild`+`xcodebuild`/`fastlane` later) + direct ASC upload —
  no Expo cloud, no GitHub. Swappable build backend without touching loop logic.
- **CL-41 (loop safety):** code signing means an autonomous push to a live device
  cannot be forged; the signer-ref + `update_published` receipt give an auditable
  trail. Owning the server also means we set our own rate/rollback policy instead
  of inheriting Expo's.

---

## 10. Proposed CND rungs (add to `docs/ISSUES.md`)

- **CND-OTA-1 — Updates manifest endpoint (MVP, unsigned JSON).** `GET
  /:projectId/manifest` resolving channel→branch→latest for runtime+platform;
  `noUpdateAvailable` when none. Acceptance: a dev build with `updates.url` →
  ours loads a published JS update.
- **CND-OTA-2 — Content-addressed asset store + serving.** `sha256` blobs,
  immutable cache; reuse the artifact-closeout addressing.
- **CND-OTA-3 — Publish CLI (`oa-update publish`).** Wrap `expo export` + hash +
  upload + manifest insert + fingerprint stamping.
- **CND-OTA-4 — Code signing + cert embedding.** RSA-SHA256 manifest signature
  (signer-ref model), `codeSigningCertificate` in the app; reject unsigned.
- **CND-OTA-5 — Channels/branches + rollback directive + rollouts.**
- **CND-OTA-6 — Multipart/mixed + bsdiff patches + CDN + metered receipts.**
- **CND-OTA-7 — local binary builds (no Expo cloud).** Step A: validate `eas
  build --local` on our Mac. Step B: fully Expo-cloud-free `expo prebuild →
  xcodebuild/fastlane gym` (iOS) + Gradle (Android) + direct ASC upload
  (`fastlane pilot`/`xcrun altool`/ASC API). Move cert/profile/ASC-key custody
  into our secret store.
- **CND-OTA-8 — `openagents.mobile_update.v1` contract** in
  `openagents-cloud-contract` (manifest row + publish/rollback receipts).

---

## 11. Risks & open questions

- **Fingerprint parity.** Publish must stamp the exact runtimeVersion the build
  embedded, or the app silently ignores the update. Use `expo-updates`'
  fingerprint generation on both sides; add a publish-time guard that refuses to
  publish for a runtimeVersion no live build reports.
- **Key custody.** The manifest signing key is the trust root for autonomous
  shipping. Keep it in the SIGNED_UPDATES signer-ref model (keychain/KMS ref, not
  in repo); rotate via cert chain.
- **Asset CDN / global latency.** Manifests are tiny + dynamic; assets are large
  + immutable → front assets with a CDN (R2 + Cloudflare, or GCS + CDN).
- **Multi-tenant vs single-app.** Start single-app (Autopilot Remote Control,
  one projectId); generalize later if other OpenAgents apps need it.
- **ToS.** `expo-updates` runtime is MIT and the protocol + a custom-server
  pattern are documented by Expo, so pointing it at our own server is a supported
  integration. We are not reusing EAS's hosted service, only the open client
  protocol.
- **Embedded fallback.** Builds still ship an embedded bundle; `rollBackToEmbedded`
  is our instant kill-switch if a bad OTA lands.

---

## 12. References

Expo client / protocol (our clone, `projects/repos/expo`):
- `packages/expo-updates/android/src/main/java/expo/modules/updates/loader/FileDownloader.kt`
  — request/response headers, multipart parsing, asset hash verification.
- `packages/expo-updates/ios/EXUpdates/.../FileDownloader.swift` — iOS equivalent.
- `packages/expo-updates/.../RemoteUpdate.kt` — directive types
  (`noUpdateAvailable`, `rollBackToEmbedded`).
- `packages/expo-updates/.../SignatureHeaderInfo.kt`,
  `CodeSigningConfiguration.kt` — `expo-signature` (`sig`/`keyid`/`alg`).
- `packages/expo-updates/.../ResponseHeaderData.kt` — `expo-protocol-version`,
  `expo-server-defined-headers`, `expo-manifest-filters`.
- `docs/pages/technical-specs/expo-updates-1.mdx` — the manifest schema +
  multipart format + directives.
- Upstream reference pattern: `expo/custom-expo-updates-server`.

Cloud primitives to reuse (this repo):
- `docs/oa-workroomd/ARTIFACT_CLOSEOUT.md` — content-addressed `sha256` storage.
- `docs/oa-node/SIGNED_UPDATES.md` — signer-ref + signed-receipt + rollback model.
- `docs/oa-workroomd/MANAGED_PREVIEW_INGRESS.md`,
  `docs/oa-workroomd/LINK_LOCAL_GATEWAYS.md` — endpoint tokens + audited serving.
- `docs/contracts/` — the typed, refs-only contract conventions.

Loop context (public `openagents` repo):
- `docs/autopilot-coder/2026-06-13-autopilot-clients-roadmap.md` — M6 + CL-37/38/39/41.
- `clients/mobile/AutopilotRemoteControl/app.config.ts` (`updates.url`,
  `runtimeVersion: fingerprint`), `src/coordinator/ship-mode.ts` (CL-37).
