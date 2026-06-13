# Pull TestFlight crash data (our infra, no Expo/EAS)

When a TestFlight build crashes and a tester taps **"Share With Developer"**, the
crash + symbolicated log lands in App Store Connect. Pull it yourself via the
**App Store Connect API** — no Expo, no EAS, no Xcode Organizer GUI needed.

## TL;DR
```sh
cd clients/mobile/AutopilotRemoteControl
ASC_ENV=../../../.secrets/appstoreconnect.env \
  bun scripts/testflight-crashes.mjs --limit 5          # concise (exception + app frames)
ASC_ENV=../../../.secrets/appstoreconnect.env \
  bun scripts/testflight-crashes.mjs --limit 1 --full   # full logText (whole .ips crash report)
```
`--app <ascAppId>` overrides the app (default `6779949704`, Autopilot Remote
Control). The script prints each submission's device/OS/comment and the
symbolicated crash log.

## Auth (one-time)
App Store Connect API key (ES256 JWT). Secrets live in workspace
`.secrets/appstoreconnect.env` (git-ignored):
```
ASC_API_KEY_ID=<key id>           # e.g. matches AuthKey_<id>.p8
ASC_API_ISSUER_ID=<issuer uuid>
ASC_API_PRIVATE_KEY_PATH=<abs path to AuthKey_<id>.p8>
```
The same key uploads builds (`xcrun altool`) and pulls crashes. Never print the
key/issuer or commit the `.p8`.

## The API path that actually works (gotchas)
- Crash submissions are **collection-queryable only at the APP level**:
  `GET /v1/apps/{appId}/betaFeedbackCrashSubmissions` ✅
  - The **build** relationship `…/builds/{id}/betaFeedbackCrashSubmissions` is
    **404** (doesn't exist), and `GET /v1/betaFeedbackCrashSubmissions` collection
    is **403** (instance-only). Use the app-level collection.
- Each submission has a `crashLog` relationship:
  `GET /v1/betaFeedbackCrashSubmissions/{id}/crashLog` →
  `data.attributes.logText` is the **full symbolicated `.ips` report**.
- `diagnosticSignatures` (MetricKit / Xcode Organizer) is **statistical** — it
  needs volume and won't show a single tester's crash. Use crash submissions.
- JWT must be **ES256 with the signature in JOSE form (raw r‖s)**, not DER.
  In Node/Bun: `createSign("SHA256").sign({ key, dsaEncoding: "ieee-p1363" })`.
  JWT claims: `iss`=issuer, `aud`="appstoreconnect-v1", short `exp` (≤20 min).

## Reading the log
- `Exception Type: EXC_CRASH (SIGABRT)` + a `Last Exception Backtrace` with app
  frames = an **uncaught exception** (native or JS-thread). Read the topmost
  `Autopilot …` frames — they name the file:line.
- A crash in `RCTJSThreadManager runRunLoop` / `hermesvm` = thrown from JS or an
  RN native module at startup.
- A crash in `ExpoUpdates*` / `FileDownloader.parseManifestResponse` /
  `Manifest.swift requiredValue(forKey:)` = **our OTA server returned a manifest
  missing a required expo-updates key** → fix the manifest in `apps/oa-updates`
  (it's server-side; the installed build recovers on next launch once the
  manifest is valid). See that app's manifest endpoint.

## Worked example (2026-06-13, build 13 / drawer)
`requiredValue(forKey:)` threw in `ExpoUpdatesUpdate.update` while parsing the
manifest from `updates.openagents.com` → SIGABRT on launch. Root cause was our
OTA manifest, not the app's JS/native code.
