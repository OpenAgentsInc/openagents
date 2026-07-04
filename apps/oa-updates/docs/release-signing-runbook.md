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

## Apple Developer ID (macOS code signing / notarization)

Separate from the ed25519 release key above. The **Developer ID Application**
cert signs + notarizes the Autopilot Desktop and Khala Code Desktop
`.app`/`.dmg` artifacts so Gatekeeper accepts them (#5048 / Autopilot v1.0-rc
#5046, #8245 for Khala Code). It does **not** sign Pylon CLI binaries — those
use the ed25519 release key; headless Pylon needs no Apple signing.

- **Identity:** `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`, issued
  by Apple Developer ID CA (G2), team `HQWSG26L43`, valid 2026-06-15 → 2031.
  Created from an `openssl`-generated CSR (not Keychain Access) so the private
  key lives in `.secrets/` like the other release keys.
- **Local material:** `~/work/.secrets/developer-id/` (mode 700, gitignored) —
  `developerID_application.key` (private), `.cer`/`.pem` (issued leaf),
  `DeveloperIDG2CA.pem` (Apple intermediate), `developerID_application.p12`
  (key+cert+chain), `p12-password.env` (the `.p12` passphrase).
- **Keychain:** the `.p12` is imported into the login keychain, so `codesign` /
  `notarytool` resolve the identity. `security find-identity -v -p codesigning`
  must list `HQWSG26L43`.
- **Env wiring:** `.secrets/appstoreconnect.env` sets
  `OA_DEVELOPER_ID_APPLICATION="Developer ID Application: OpenAgents, Inc. (HQWSG26L43)"`
  (quoted — value has spaces/parens) alongside the `ASC_API_*` notary key.
  `apps/autopilot-desktop/scripts/notarize-macos.sh` reads both.
- **Khala Code Desktop:** run
  `bun run --cwd clients/khala-code-desktop release:macos -- --version <version> --channel rc`
  on the owner-controlled Mac. The script points
  `OA_DESKTOP_APP_PATH` at `Khala Code.app`, skips Autopilot's Apple-FM bridge
  preflight, re-creates the DMG from the stapled app, signs/notarizes/staples
  the DMG, and stages the `desktop/khala-code-desktop/<channel>/feed.json`
  product feed. Upload/GitHub release creation stay behind explicit owner env
  flags and must be paired with the clean-Mac first-run smoke receipt in
  `NEEDS_OWNER.md`.

### Backup & recovery (device loss)

Canonical backup is **GCP Secret Manager** (project `openagentsgemini`),
hash-verified equal to the local `.p12`:
- `developer-id-application-p12` — base64 of the `.p12`
- `developer-id-application-p12-password` — the passphrase

Recover on a fresh Mac:
```sh
mkdir -p ~/work/.secrets/developer-id && chmod 700 ~/work/.secrets/developer-id
gcloud secrets versions access latest --secret=developer-id-application-p12 \
  --project=openagentsgemini | base64 -d > ~/work/.secrets/developer-id/developerID_application.p12
PASS=$(gcloud secrets versions access latest --secret=developer-id-application-p12-password \
  --project=openagentsgemini)
security import ~/work/.secrets/developer-id/developerID_application.p12 \
  -k ~/Library/Keychains/login.keychain-db -P "$PASS" \
  -T /usr/bin/codesign -T /usr/bin/security -T /usr/bin/productsign
security find-identity -v -p codesigning | grep HQWSG26L43   # confirm
```
On device **compromise**: revoke the cert in the Apple Developer portal, issue a
new one from a fresh CSR, re-import, and rotate the GCP secret versions.

> The `.p12` was built with `openssl pkcs12 -export -legacy -certpbe
> PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1` — openssl 3 defaults produce
> a `.p12` the macOS Security framework can't import ("MAC verification failed").

## Custody note

Long-term the private key should live **only** in KMS/HSM and never touch a laptop
(plan §6b). This local-`.secrets` + Secret-Manager-backup setup is the working
bootstrap so deploys can be signed today; migrating signing into KMS (sign-via-API,
key never exported) is tracked under the OTA epic (#5044).
