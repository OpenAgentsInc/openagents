# Release signing & GCP-only deploy runbook

Date: 2026-06-15. The OpenAgents **release/provenance** key signs every artifact +
authoritative manifest so Pylons/Autopilot/users can verify it came from our infra
and **fail closed** otherwise (plan: `docs/ota/2026-06-15-ota-autoupdate-plan.md`
§6b). This runbook is how the key is held, backed up, used to sign deploys, and
rotated. **The signed artifacts publish only to our Google Cloud infra**
(`updates.openagents.com`, the `oa-updates` Cloud Run service, project
`openagentsgemini`).

## The key (ed25519)

- **kid:** `2dbe811d19f67528`  ·  **alg:** ed25519  ·  created 2026-06-15.
- **Public key (pinned by clients):** `apps/oa-updates/keys/release-pubkey.json`
  — committed, public, safe. Clients embed this and reject anything not signed by
  it. (Public `x`: `P9steasTKRx6gr9QQlbah4kXm17aAh2wLHLAL-Txwak`.)
- **Private key (SECRET):** `~/work/.secrets/openagents-release-signing.env`
  (mode 600, gitignored, NEVER committed/printed). Holds the JWK seed `d`.

## Backup & recovery (device loss / compromise)

- **Canonical backup is GCP Secret Manager** (in our cloud, access-controlled,
  survives this device): secret `openagents-release-signing-key`, project
  `openagentsgemini`. The current local key was pushed there (version 1) and the
  copy is **hash-verified** equal to the local file.
- **Recover** on a fresh machine:
  ```sh
  gcloud secrets versions access latest --secret=openagents-release-signing-key \
    --project=openagentsgemini > ~/work/.secrets/openagents-release-signing.env
  chmod 600 ~/work/.secrets/openagents-release-signing.env
  ```
- **Re-back-up** after generating/rotating a key:
  ```sh
  gcloud secrets versions add openagents-release-signing-key \
    --project=openagentsgemini --data-file=~/work/.secrets/openagents-release-signing.env
  ```
- **On device COMPROMISE:** the local key is burned — **rotate** (below). Backup
  recovers from *loss*; rotation answers *theft*.

## Sign a deploy

```sh
# loads the key from env -> .secrets -> GCP Secret Manager (in that order)
bun apps/oa-updates/scripts/sign-release.ts <artifact-or-manifest>  > <artifact>.sig.json
# verify (the reference fail-closed check clients embed)
bun apps/oa-updates/scripts/verify-release.ts <artifact> <artifact>.sig.json   # exit 0 ok / 1 reject
```
- In **our GCP infra** (Cloud Run / CI), the signer needs no local file: mount the
  Secret Manager secret as `OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D` (+ `_KID`)
  and `sign-release.ts` uses it. Verified working both ways (local file + env).
- Tampering is rejected (sha256 + ed25519), wrong/again `kid` is rejected — tested.

## Publish (GCP only)

Build → sign → publish the artifact **and** its `.sig.json`/signed manifest to
`updates.openagents.com` (the `oa-updates` Cloud Run service, project
`openagentsgemini`, `us-central1`; see `scripts/deploy-cloudrun.sh`). Nothing is
served from anywhere but our GCP. Clients fetch over TLS, then **verify the
signature against the pinned `release-pubkey.json` and fail closed** — host/TLS is
never the trust boundary, the signature is. (Wiring the signature into the
published feed manifests is tracked in #5043 / the OTA epic #5039.)

## Rotation / compromise procedure

1. Generate a new keypair (new `kid`), write to `.secrets`, push to GCP Secret
   Manager (a new secret version or a new secret for the new kid).
2. Add the new public key to the pinned key set (clients pin a long-lived **root**
   that signs rotating subkeys — JWKS-style — so rotation doesn't require
   reflashing every client; until that exists, ship the new `release-pubkey.json`
   in the next client release).
3. Re-sign the current `stable`/`canary` releases with the new key; publish.
4. Mark the old `kid` revoked in the key set + transparency log; clients reject it.
5. Never reuse a compromised key.

## Custody note

Long-term the private key should live **only** in KMS/HSM and never touch a laptop
(plan §6b). This local-`.secrets` + Secret-Manager-backup setup is the working
bootstrap so deploys can be signed today; migrating signing into KMS (sign-via-API,
key never exported) is tracked under the OTA epic (#5044).
