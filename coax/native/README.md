# Coax Native starter

The OpenAgents book begins with this starter, then adds application behavior
in Coax. Its first installed target is iOS: UIKit owns the window, WebKit
runs Coax-generated Wasm, and the existing shared component DOM backend
draws the app. This is the WebKit/Wasm delivery profile of
[Coax Native](../native.md), not the proposed native-control renderer.

The reusable starter is `starter/app.coax`; OpenAgents builds its application
surface from `apps/openagents-ios/home/app.coax`. Its canonical resources live here and in
`components/` and `coax/web/`; generated copies are ignored. The iOS target
uses the existing bundle `com.openagents.app`, signing team, icons,
entitlements and 1.0.0 version line. Old world/wallet screens are excluded
from the replacement target. Their Keychain records are retained.

## Build

From the repository root on the Mac:

```sh
apps/openagents-ios/generate.sh
xcodegen generate --spec apps/openagents-ios/project.yml
coax/remote/remote.sh run --local -- xcodebuild \
  -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -destination 'generic/platform=iOS Simulator' \
  -derivedDataPath /tmp/coax-native build CODE_SIGNING_ALLOWED=NO
```

Updates are optional. To produce an offline-only starter, generate with
`coax/remote/remote.sh run -- bash coax/native/generate.sh --without-updates`.
That shell never checks, downloads or executes remote packages. The normal
OpenAgents configuration enables Updates and bundles its public key.

## Installed services

`coaxNative.call(op, args)` is the WebKit transport. Only the package's main
frame at `coax-native://app` may call it. Replies belong to the calling view's
epoch. At most eight services may be outstanding; activation waits until
requests finish.

| Operation | Arguments and reply |
| --- | --- |
| `ready` | First interactive module; returns build/runtime/update details |
| `updates.status`, `updates.check` | Returns `message`; check may offer native activation |
| `auth.state` | Returns `loggedIn` |
| `auth.emailStart` | `email`; returns opaque `pending` |
| `auth.emailVerify` | `pending`, `code`; returns `loggedIn` |
| `auth.keyLogin` | Uses the existing device Nostr identity; returns `loggedIn` |
| `auth.passkeyLogin` | Uses Apple's passkey sheet; returns `loggedIn` |
| `auth.logout` | Clears the existing native account session |
| `http` | `path`, `method`, `body`; returns `status`, `body`, `contentType` |

The shared native auth adapter retains device-bound refresh tokens and
Secure Enclave support. Tokens and key material never enter Wasm or JS.
HTTP is restricted by installed Coax policy to `/api/chat/*` and
`/api/auth/account` on openagents.com, GET/POST, 64 KiB request and 1 MiB
response. The browser glue redirects relative `/api/` fetches through it.
Chat can use the existing send/state/turn polling API. Streaming transport,
wallet payments, notifications and arbitrary new native SDKs are not exposed by this
profile. The OpenAgents-only build 1069 adds a read-only `wallet.balance`
service for its separate Extras module; it is not book functionality.
The OpenAgents `home/` module adds email-code login and read-only account
history over these services. Full conversation functionality remains
subsequent book work (#13512).

## Publish an update

Keep `Generated/runtime.txt` from the released native build. Do not regenerate
it from modified native sources and target an older installed binary.
Build only changed Wasm, then prepare resources. For example:

```sh
coax/remote/remote.sh run -- coax/web/wasm.sh coax/native/starter/app.coax \
  coax/native/starter/face.c apps/openagents-ios/Starter/build/app.wasm
coax/remote/remote.sh run -- coax/web/wasm.sh apps/openagents-ios/extras/app.coax \
  coax/native/starter/face.c apps/openagents-ios/Starter/build/extras.wasm
coax/web/tool.sh coax/native/prepare.coax
mkdir -p /tmp/native-release
coax/web/tool.sh coax/native/package.coax apps/openagents-ios/Starter \
  com.openagents.app "$(cat apps/openagents-ios/Generated/runtime.txt)" 1 \
  /tmp/native-release/manifest.json index.html native.js extras.js coaxial/dom-host.js \
  coaxial/shell.js build/app.wasm build/extras.wasm build/zeron/Geist.ttf \
  build/zeron/Geist-Medium.ttf build/zeron/Geist-SemiBold.ttf build/zeron/GeistMono.ttf
coax/web/tool.sh coax/native/publish.coax apps/openagents-ios/Starter \
  /tmp/native-release/manifest.json /tmp/native-release/latest.cnu \
  "$HOME/.openagents/native-updates/keys/openagents-ios.key" \
  "gs://openagentsgemini-native-updates/runtimes/$(cat apps/openagents-ios/Generated/runtime.txt)"
```

Choose a fresh, increasing generation. The publisher checks file hashes,
signs locally using CryptoKit Ed25519, uploads immutable artifacts first,
then promotes `latest.cnu`. An interrupted publish keeps the previous feed.
To roll back, publish old compatible resources with a higher generation.
The installed key is the public file here; keep its private key outside the
repository. A restricted backup is in Secret Manager
`coax-native-ios-update-signing-key` in `openagentsgemini`; the public Cloud Run
service does not mount it. Losing all copies requires another native build.

The client downloads asynchronously after launch, stages and verifies the
whole package, and activates on the next cold launch or the explicit Apply
action. There is no silent hot replacement during a login or request.
A trial becomes healthy after the module reports ready and remains in the
foreground for 30 seconds. An interrupted trial is quarantined on the next
boot; a terminated WebKit process falls back immediately. The previous
healthy package and embedded bundle remain recovery choices. This first
profile conservatively allows one unclean trial; filesystem selectors use
atomic writes. It does not roll back account data or native credentials.

## Hosting

`updates.openagents.com` maps to Cloud Run `oa-updates` in `us-central1`,
project `openagentsgemini`. Its readonly GCS volume is
`openagentsgemini-native-updates`, mounted `/site`. The pinned existing
Coax static server runs `serve /site --static --host-dir /site/. --cache 0`;
it scales 0–2 instances, 1 CPU, 512 MiB. The service is public but cannot
sign updates. Its pinned fallback URL is
`https://oa-updates-157437760789.us-central1.run.app`.
DNS retains `CNAME ghs.googlehosted.com`; Google provisions the custom-domain
TLS certificate. The fallback works while that certificate is pending.
The deployed service's dedicated identity `native-updates-reader` has only
bucket object-viewer access. Its reproducible revision is pinned by image
digest in [cloud-run.yaml](cloud-run.yaml).
Build 1069 and later use `/runtimes/<fingerprint>/latest.cnu` and that
directory's `objects/`. The original build 1068 keeps the root feed, so a
new native capability cannot replace its compatible update with one it
cannot run.

This replaces the historical `oa-updates` deployment from `4954eb2fc0`,
without executing or restoring its old JavaScript server. Broader rollout,
expiry, delegated-key, telemetry and native-renderer designs remain in
[Updates](../native-updates.md). App Store distribution rules still govern
what functionality may change remotely; this runtime is not a guarantee
that every future feature can avoid a store build.
