# ReleaseSet v2 feed operations

This runbook covers the Desktop ReleaseSet v2 feed owned by `oa-updates`.
The feed is additive: Expo/mobile manifests and assets keep their existing
seed and route ownership, and Desktop publication writes only beneath
`desktop/release-set-v2/` in the configured Cloud Storage bucket.

## Public contract

For each `stable` and `rc` channel the service exposes:

- `.../<channel>/v2/pointer.json`: mutable, bounded-cache pointer;
- `.../<channel>/release-set.json` and `release-set.sig.json`: no-store
  compatibility aliases with `x-openagents-release-generation`;
- `.../<channel>/candidates/<sha256>/release-set.json` and
  `release-set.sig.json`: immutable candidate bytes.

The pointer generation is the SHA-256 of the exact canonical signed payload.
Consumers should fetch the pointer once, use its immutable candidate URLs,
verify both recorded digests, verify the Ed25519 envelope against a pinned
key, and then use only artifact URLs, sizes, and hashes in that signed set.
They must not combine payload and signature aliases carrying different
generation headers.

All public routes allow credential-free cross-origin `GET`, `HEAD`, and
`OPTIONS`. Immutable responses use one-year immutable caching; pointers use a
15-second bounded cache; current aliases and errors use `no-store`. JSON
payloads, envelopes, pointers, and storage documents have explicit size caps.
Artifact admission hashes response streams incrementally and refuses redirects,
credentialed URLs, a mismatched `Content-Length`, oversized bodies, or digest
mismatches.

## Publication and promotion

Configure Cloud Run with:

- `OA_RELEASE_SET_BUCKET`: the dedicated Google Cloud Storage bucket;
- `OA_RELEASE_SET_PINS_PATH`: a deployed JSON array of public Ed25519 pins.

The deploy script uses `gcloud run deploy --update-env-vars` and
`--update-secrets`, never their destructive `--set-*` forms. A Desktop-only
invocation therefore preserves the existing mobile seed configuration, and a
mobile-only invocation preserves the ReleaseSet bucket and pins.

Image selection is additive at the layer level too, not just env vars. The
script auto-selects between two build modes (`OA_UPDATES_DEPLOY_MODE=auto` by
default; `full` and `incremental` are also selectable explicitly):

- **full** (`--source .` against the ordinary `Dockerfile`) — used only when a
  seed is actually being republished (`OA_SEED_DIST`/`OA_SEED_RUNTIME`, the
  mobile Expo export, or `OA_DESKTOP_RELEASES_DIST`, the legacy Desktop v1
  archive tree). `publish-ota.sh` always sets `OA_SEED_DIST`, so mobile
  publication is unchanged and continues to bake a fresh export.
- **incremental** (`Dockerfile.incremental`, built by Cloud Build against the
  exact immutable digest of the currently ready Cloud Run revision) — used for
  every other deploy: a bare server code push, a ReleaseSet v2 bucket/pin
  config change, or staging a new v2 RC manifest under the git-tracked
  `openagents-desktop-dist/` tree. This is the path that matters for this
  feed: because the gitignored `dist/`, `desktop-dist/`, `pylon-dist/`, and
  `desktop-ota/` seed directories are normally empty or stale in an ordinary
  checkout, a `--source .` build in this case would silently blank whatever
  mobile/Desktop-v1/Pylon bytes are already baked into the live image. The
  incremental build never touches those layers; it only replaces the compiled
  server and the small always-git-tracked Desktop v2 descriptor tree.

The script refuses to run (rather than guess) when: the base image cannot be
resolved to an immutable `sha256` digest, the built image cannot be resolved
to an immutable digest, an unknown `OA_UPDATES_DEPLOY_MODE` value is given, or
`OA_UPDATES_DEPLOY_MODE=incremental` is combined with a seed publish (that
combination would silently drop the requested seed instead of shipping it).
Both branches and every refusal are covered by command-level dry-run tests in
`src/deploy-cloudrun.test.ts`.

The service account needs object read/create/delete on the bounded
`desktop/release-set-v2/` prefix. It does not need signing-key access.
Signing remains offline/coordinator-owned. Candidate admission takes exact
payload and signature bytes plus the DIST-04 public artifact verifier port.
It re-verifies the signature, complete target/format matrix, channel, and every
artifact before creating one immutable candidate object.

Promotion is compare-and-swap. The caller supplies the pointer revision it
observed; Cloud Storage `ifGenerationMatch` makes the write atomic across
instances. A stale caller fails without changing current. Promotion retains
the prior generation as the one rollback slot. Rollback is another CAS write,
increments the pointer revision, and may select only that retained generation.
It cannot name an arbitrary older release.

The first public candidate must be published by the real DIST-04 coordinator,
then downloaded and independently re-verified through the public candidate
URLs before promotion. This repository slice intentionally does not claim that
external receipt.

## v1 compatibility

The old `/desktop/openagents/<channel>/manifest.json`, `manifest.sig.json`, and
`release.json` endpoints remain separate v1 responses through
`2026-10-14T23:59:59Z`, the compatibility deadline in the shared ReleaseSet
contract. V1 is darwin-arm64 only and is never projected into v2. New targets
and all new publication use v2. Remove the v1 seed/routes after the deadline
only after client telemetry shows no eligible v1 clients.

## Retention and garbage collection

Retain the current and immediate previous candidate for each channel. The feed
can list candidates outside those two pointer slots, but deliberately does not
delete them: deleting one object while another process promotes its pointer
cannot be made atomic across objects. Apply an operator-reviewed deletion plan
only while the DIST-04 publication lease excludes promotion, after a successful
promotion smoke and the operational rollback window. Bucket lifecycle rules may
remove noncurrent object versions later, but must never race the application
retention rule. Mobile prefixes are outside the plan and must never appear in a
Desktop deletion operation.

## Key rotation

1. Generate the new signing key outside the service and protect the private
   half in the release signer.
2. Add its public pin to the deployed pin array while retaining the old pin.
3. Deploy and verify both pins are accepted with fixture candidates.
4. Sign and admit a candidate with the new key; independently verify every
   public candidate URL and signature.
5. Promote with CAS and observe at least one full rollback window.
6. Remove the old pin only after no retained current/previous candidate uses
   it. Never reuse a key id for different public material.

## Disaster recovery

Cloud Storage object versioning and audit logs are required on the feed bucket.
If a pointer is lost or corrupted, stop promotion, identify the last verified
candidate from audit records, re-run public candidate verification, and restore
the pointer with a storage-generation precondition. Do not hand-edit payload or
signature bytes. If candidate integrity is uncertain, leave the feed failed
closed (404/no-store) while mobile OTA continues independently.

For a bad current release, use the typed rollback operation with the observed
revision. If its retained candidate is absent or fails verification, do not
force a pointer: publish a newly signed corrective release through normal
candidate admission. Record channel, pointer revision, public generation,
target/artifact counts, and reason. Logs deliberately omit signed artifact
URLs, private bucket topology, access tokens, and signing material.
