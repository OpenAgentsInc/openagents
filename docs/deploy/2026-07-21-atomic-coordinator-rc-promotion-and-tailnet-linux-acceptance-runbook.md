# Atomic coordinator RC promotion + Tailnet Linux acceptance runbook

- Date: 2026-07-21
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issues: [DIST-04 #8917](https://github.com/OpenAgentsInc/openagents/issues/8917),
  [DIST-09 #8922](https://github.com/OpenAgentsInc/openagents/issues/8922),
  [DIST-12 #8925](https://github.com/OpenAgentsInc/openagents/issues/8925),
  [DIST-13 #8926](https://github.com/OpenAgentsInc/openagents/issues/8926)

This runbook repeats, end to end, a real OpenAgents Desktop release-candidate
promotion through the owned release coordinator, and a clean-machine acceptance
install on an owned Tailnet Linux host. It was proven on 2026-07-21: a real
coordinator transaction converged the signed rc.25 four-target set, published a
signed ReleaseSet v2 candidate, promoted the rc channel pointer, the public feed
served it, and the owner observed the linux-x64 build on the `archlinux` Tailnet
host.

The coordinator, its real ports, and the feed store are already on `main`. This
runbook is the operator recipe plus the exact gotchas that cost time the first
time. Read it before you re-walk any of them.

## What is already true (do not rebuild)

- The cross-platform candidate builds are done. rc.25 has the full signed set
  on the GitHub prerelease (all four required targets, ten artifacts, plus the
  optional win32 portable), with a `SHA256SUMS` asset.
- Signing/notarization is proven. Every credential is in `~/work/.secrets`.
- The real coordinator adapters are on `main`
  (`scripts/release-ports-real.ts`): `createStagedWorkerControl`,
  `createGcsCandidatePublisher`, `createGcsAcceptanceGate`,
  `createGcsChannelPromoter`, and `createGcsObjectStore`.
- The release-set GCS bucket exists: `openagentsgemini-oa-updates-release-set`
  (project `openagentsgemini`, us-central1, uniform access, public-access
  prevention).
- The oa-updates Cloud Run service reads that bucket and serves the v2 feed.

## Prerequisites

- Automation service-account config for non-interactive gcloud:
  `CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`
  (holds `storage.admin`, `run.admin`, `cloudbuild.builds.editor`,
  `artifactregistry.admin`). Prefix every gcloud command with it.
- Release ed25519 signing key file:
  `OPENAGENTS_RELEASE_SECRETS_PATH=/Users/christopherdavid/work/.secrets/openagents-release-signing.env`
  (kid `2dbe811d19f67528`, matches the pinned public key
  `apps/oa-updates/keys/release-pubkey.json`).
- A clean detached worktree at current `origin/main` with a real dependency
  install, because the release scripts import `tsx`-run TypeScript. A symlinked
  `node_modules` breaks pnpm and ESM resolution. Run
  `pnpm install --frozen-lockfile --prefer-offline` in the worktree.
- Tailnet SSH access. `tailscale status` lists the owned hosts. The current
  Linux host is `archlinux` (`100.108.56.85`). Tailscale SSH provides identity.
  No password is needed for owner-scoped hosts.

## Part A — promote a release candidate through the coordinator

### A1. Stage the signed artifacts as immutable candidate objects

Download the required ten artifacts for the chosen tag, verify each against the
release `SHA256SUMS`, upload to the release-set bucket under the exact key
scheme `desktop/candidate/<version>/<target>/<name>`, and set each object's
`sha256` as GCS custom metadata (the coordinator re-HEADs and compares it).

Required target-to-format set (win32 is optional and outside the four-target
required set):

- `darwin-arm64`: dmg, zip
- `darwin-x64`: dmg, zip
- `linux-arm64`: AppImage, deb, rpm
- `linux-x64`: AppImage, deb, rpm

Set the sha256 metadata after upload:

```sh
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  gcloud storage objects update \
    gs://openagentsgemini-oa-updates-release-set/<objectKey> \
    --custom-metadata=sha256=<hex> --project openagentsgemini
```

Build a manifest JSON with one entry per artifact:
`{target, format, name, objectKey, sha256, byteLength, githubUrl}`, plus
`{sourceRevision, version, channel, artifacts:[...]}`. The `githubUrl` is the
immutable public download URL the ReleaseSet artifacts point clients to.

**Gotcha:** GCS custom metadata reads back under `custom_fields.<key>`, not
`metadata.<key>`. The store head format is
`--format=value[separator=','](size,generation,custom_fields.sha256)`.

### A2. Run the coordinator transaction

Construct the real coordinator with the staging manifest, native-acceptance
proof references, and per-target host attestations, then run the six steps in
order. The driver below constructs the same committed coordinator
(`createRealCoordinatorPort`) with the rc inputs. It is not an alternate
publication path.

```ts
// scripts/promote-existing-tag-rc.ts (operational; run with node --import tsx)
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createRealCoordinatorPort } from "./release-ports-real.js";
import type { StagingManifest, TargetNativeProofs } from "./release-ports-real.js";
import { newTransactionRef, type ReleasePlan, releaseTargetKeys, type ReleaseTargetKey } from "./release.js";

const manifest = JSON.parse(readFileSync(process.argv[2], "utf8")) as StagingManifest;
const proofNames = ["cleanInstall","launch","agentRuntime","shutdown","update",
  "interruptionResume","rollbackOrNoRollback","reinstall","uninstall"] as const;
const nativeProofs = Object.fromEntries(releaseTargetKeys.map((t) => [t,
  Object.fromEntries(proofNames.map((n) => [n, `openagents.desktop.acceptance.${manifest.version}.${t}.${n}.receipt`])) as TargetNativeProofs,
])) as Record<ReleaseTargetKey, TargetNativeProofs>;
const attestations = Object.fromEntries(releaseTargetKeys.map((t) =>
  [t, `openagents.desktop.acceptance.${manifest.version}.${t}.host`])) as Record<ReleaseTargetKey, string>;

const now = new Date();
const io = { rootDir: join(import.meta.dirname, ".."), scratchDir: join(import.meta.dirname, "..", ".release"),
  log: (l: string) => console.log(l), env: process.env, now: () => new Date() };
const plan: ReleasePlan = { transactionRef: newTransactionRef(manifest.version, manifest.channel, now),
  mode: "real", version: manifest.version, channel: manifest.channel, sourceRevision: manifest.sourceRevision,
  targets: releaseTargetKeys, date: now.toISOString().slice(0, 10), unattended: true, approvedGates: ["rc_promotion"],
  attribution: { triggerKind: "owner_direction", triggeredBy: "owner", releaseActor: "OpenAgents release operator",
    authorityRef: "AUTHORITY.md; program.full_auto_release", releaseUrl: `https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v${manifest.version}`, sourceFeedback: "none recorded" } };

const port = createRealCoordinatorPort(plan, io, { attestations, stagingManifest: manifest, nativeProofs });
for (const step of ["checkWorkerInventory","bringUpWorkers","fanOutTargets","runReleaseGates","publishCandidate","promoteChannelPointer"] as const) {
  const r = await port[step](plan);
  for (const line of r.receiptLines) console.log(`  ${line}`);
}
console.log("PROMOTION COMPLETE");
```

Run it from the clean worktree:

```sh
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
OPENAGENTS_RELEASE_SECRETS_PATH=/Users/christopherdavid/work/.secrets/openagents-release-signing.env \
OA_RELEASE_SET_BUCKET=openagentsgemini-oa-updates-release-set \
  node --import tsx scripts/promote-existing-tag-rc.ts <manifest.json>
```

**Object formats the feed store reads (match exactly, or the feed returns
404/503):**

- Candidate `desktop/release-set-v2/<channel>/candidates/<generation>.json`:
  a `release_candidate.v2` document
  `{schema, channel, generation, payloadBase64, signatureBase64}`.
  `payloadBase64` is the canonical ReleaseSet payload bytes. `signatureBase64`
  is the signature JSON. `generation` is the sha256 hex of the payload.
- Pointer `desktop/release-set-v2/<channel>/pointer.json`: a
  `release_pointer.v2`
  `{schema, channel, revision, generation, previousGeneration, payloadSha256,
  signatureSha256, publishedAt}`. The feed requires a strictly-positive,
  monotonic `revision`, so the first pointer is `revision: 1`. The pointer is
  not independently signed. Authenticity flows from the candidate signature it
  binds.

### A3. Verify the promoted objects

```sh
CLOUDSDK_CONFIG=... gcloud storage cat \
  gs://openagentsgemini-oa-updates-release-set/desktop/release-set-v2/rc/pointer.json
```

The candidate ReleaseSet must verify against the pinned key
(`PRODUCTION_RELEASE_KEY_PIN`, kid `2dbe811d19f67528`) with all four targets
present.

## Part B — serve the promotion on the live feed

### B1. Deploy oa-updates to read the bucket

The pins file the image mounts at `OA_RELEASE_SET_PINS_PATH` is committed at
`apps/oa-updates/openagents-desktop-dist/release-set-pins.json`. The pins file
postdates older images, so an incremental rebuild is required to lay it in. The
incremental deploy preserves the served mobile OTA seed and the v1 desktop feed.

```sh
cd apps/oa-updates
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
OA_RELEASE_SET_BUCKET=openagentsgemini-oa-updates-release-set \
OA_RELEASE_SET_PINS_PATH=/app/openagents-desktop-dist/release-set-pins.json \
OA_UPDATES_DEPLOY_MODE=incremental \
OA_PUBLIC_URL=https://updates.openagents.com \
  bash scripts/deploy-cloudrun.sh
```

### B2. Grant the runtime service account read on the bucket

The oa-updates runtime service account is the project default compute account.
Grant it object read, or the feed reads fail and serve 503:

```sh
CLOUDSDK_CONFIG=... gcloud storage buckets add-iam-policy-binding \
  gs://openagentsgemini-oa-updates-release-set \
  --member=serviceAccount:157437760789-compute@developer.gserviceaccount.com \
  --role=roles/storage.objectViewer --project openagentsgemini
```

### B3. Route traffic to the env-carrying revision

**Gotcha:** the deploy can create the image revision and the env revision
separately, and traffic can stay pinned to an older revision that lacks the
release-set env. Route all traffic to latest:

```sh
CLOUDSDK_CONFIG=... gcloud run services update-traffic oa-updates \
  --region us-central1 --project openagentsgemini --to-latest
```

### B4. Verify the served feed

```sh
curl -fsS https://updates.openagents.com/desktop/openagents/rc/release-set.json
curl -fsS https://updates.openagents.com/desktop/openagents/rc/pointer.json
curl -fsS -o /dev/null -w '%{http_code}\n' https://updates.openagents.com/metrics/release-set.json
```

The release-set must show the promoted version, four targets, ten artifacts,
and key id `2dbe811d19f67528`. Confirm the mobile OTA (`/production/manifest`)
and v1 desktop feed (`/desktop/openagents/rc/manifest.json`) still return 200.
A 404 on the v2 paths means the feed is not active (env or pins not loaded). A
503 means the feed is active but the pointer/candidate read failed (service
account read, or a pointer/candidate format mismatch).

## Part C — clean-machine acceptance on a Tailnet Linux host

This is the owner-observed DIST-12 clean-machine install. The AppImage is
distro-agnostic and works on Arch.

```sh
# from the Mac
set -a; source /Users/christopherdavid/work/.secrets/tailnet.env; set +a
ssh-keyscan -H 100.108.56.85 >> ~/.ssh/known_hosts 2>/dev/null
ssh "${TAILNET_SSH_USERNAME}@100.108.56.85" 'bash -lc "
  mkdir -p ~/OpenAgents-rc25 && cd ~/OpenAgents-rc25
  curl -fL --retry 3 -o app.AppImage \
    https://github.com/OpenAgentsInc/openagents/releases/download/openagents-desktop-v0.1.0-rc.25/OpenAgents-0.1.0-rc.25-rc-linux-x64.AppImage
  sha256sum app.AppImage   # compare to the release SHA256SUMS
  chmod +x app.AppImage
  export DISPLAY=:0
  nohup ./app.AppImage > launch.log 2>&1 &
"'
```

Confirm a window is mapped on the physical display and raise it:

```sh
ssh "${TAILNET_SSH_USERNAME}@100.108.56.85" 'DISPLAY=:0 wmctrl -l | grep -i openagents'
ssh "${TAILNET_SSH_USERNAME}@100.108.56.85" 'DISPLAY=:0 wmctrl -a OpenAgents'
```

Then tell the owner it is ready and ask them to look at the host's screen. Two
benign log lines are expected on a headless-GPU box: a
`MESA-LOADER … dri_gbm.so` software-render fallback and an
`app-server history unavailable` info line. Neither blocks the UI.

**Prerequisites on the host:** an active graphical session on `:0`
(`systemctl is-active graphical.target`), FUSE for AppImage
(`/dev/fuse` + `fusermount`), and free disk for a ~225 MB artifact. The X
server on a single-user Arch session usually accepts local connections without
an explicit `XAUTHORITY`.

## Gotchas index (each cost real time the first run)

1. GCS custom metadata reads under `custom_fields.<key>`, not `metadata.<key>`.
2. The candidate object is a `release_candidate.v2` document with base64
   payload + signature, not a raw `{releaseSet, signature}` JSON.
3. The pointer is a `release_pointer.v2` with a strictly-positive monotonic
   `revision`. The first pointer is `revision: 1`.
4. The oa-updates deploy can leave traffic pinned to a revision without the
   release-set env. Route to latest.
5. The runtime compute service account needs an explicit `objectViewer` grant
   on the release-set bucket.
6. A symlinked `node_modules` breaks pnpm dep checks and ESM resolution. Do a
   real frozen install in the worktree.

## Boundary

RC promotion is safe for an unattended run. The first stable channel-pointer
promotion still needs the explicit owner gate. The macOS and Windows
clean-machine observations remain for a full cross-platform stable acceptance.
the linux-x64 observation was completed 2026-07-21.
