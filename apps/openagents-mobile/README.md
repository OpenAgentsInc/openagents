# OpenAgents mobile

The current OpenAgents iOS/Android app is one plain React Native surface for
mirroring an admitted Omega desktop. Expo provides the native host, local build
tooling, camera access, secure storage, and the owned OTA feed. Effect Native is
not part of the mounted UI.

- Display name: `OpenAgents`
- iOS bundle identifier / Android application ID: `com.openagents.app`
- App entry: `src/app.tsx`
- Mounted screen: `src/screens/omega-home-screen.tsx`
- Device bridge protocol: `openagents.omega.device_bridge.v1`

The owner-directed reset on 2026-07-27 removed the previous conversation,
Effect Native, auth, Sync, managed-sandbox, Full Auto, notification, settings,
and mobile command surfaces. Historical receipts remain evidence only for the
commits they pin.

## What exists today

The app opens a signed device identity from Expo SecureStore and connects to an
Omega desktop bridge over WebSocket. A person can:

- scan the short-lived QR bootstrap shown by Omega desktop
- retain the admitted grant, last successful endpoint, and resume cursor in
  device-only secure storage
- reconnect through the cached endpoint after reopening the app
- read the link state from one status dot, green for a direct connection and
  red when the desktop is unreachable (the view can render relay state, but the
  mounted screen supplies no relay observation feed)
- browse projected desktop threads and runs, ordered by the desktop's own last
  activity time and dated with it
- open a thread to read its bounded transcript, tool calls, and executor
  disclosure, rendered as Markdown in the desktop's own faces.

The bridge validates bounded protocol frames, signs the hello proof, binds a QR
proof to the pairing-secret digest, applies ordered snapshot/delta updates, and
requests a fresh snapshot on generation or sequence gaps. A stalled candidate
has a five-second admission deadline and is closed before the dial ladder
continues.

The mounted production screen supplies the cached endpoint and an optional QR
bootstrap. It does not currently supply signed announcement discovery, manual
MagicDNS, or a relay observation feed. Simulator development can provide a
bootstrap through either:

- `EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP` for inline JSON
- `EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP_URL` for a JSON endpoint.

## Capability boundary

This app is a read-only desktop mirror. The transcript composer is not rendered
at all, because the mounted surface does not open the signed relay command
lane. A disabled control still offers to do something, and this one cannot, so
the owner directed on 2026-07-27 that the surface show nothing rather than an
affordance that refuses. The app does not currently:

- create, append to, steer, interrupt, or approve desktop work
- authenticate an OpenAgents account or synchronize account conversations
- control managed sandboxes, terminals, files, diffs, Git, or Full Auto
- register or consume push notifications
- run an agent, model, shell, cloud SDK, or desktop authority on the phone.

The bridge client contains transport support beyond what the screen currently
wires. That code is not a product claim. Signed commands, live-host journeys,
and physical-device reliability require separate implementation and
candidate-bound evidence.

## Architecture

- `src/app.tsx` loads the bundled faces, then mounts one `OmegaHomeScreen`
  inside the safe-area provider.
- `src/screens/omega-home-screen.tsx` owns QR scanning, connection notices,
  activity ordering, selection, and the clock the relative stamps read against.
- `src/screens/omega-bridge-session.ts` owns the bridge mount lifecycle alone,
  so its ownership race has a seam a test can reach (#9264). The screen module
  cannot be imported under the test environment, because `expo-camera` needs a
  React Native runtime that the node test host does not provide.
- `src/screens/omega-home-view.tsx` renders pairing, activity, transcript, and
  connection states with plain React Native components.
- `src/ui/` contains the small button, screen, surface, text, theme, and
  Markdown primitives used by that view. `assets/fonts/` carries the desktop's
  own IBM Plex Sans and Lilex, with their licences, so a message reads the same
  on both screens.
- `src/workroom/omega-device-bridge-client.ts` owns the Effect-based signed
  bridge protocol, endpoint ladder, grant/cursor storage, and mirror reducer.
- `src/workroom/issue31-device-key-vault.ts` owns the device key in Expo
  SecureStore and disposes signer material when the bridge closes.
- `src/crypto-random-values.ts` installs the Expo CSPRNG fallback needed by
  Hermes without replacing a platform implementation.

The device bridge is the environment boundary. The phone renders its bounded
projection. It does not become execution authority.

## Run it

Install workspace dependencies from the repository root:

```sh
pnpm install
```

Start the Expo development server:

```sh
pnpm --dir apps/openagents-mobile run dev
```

Use a custom development build or simulator. A physical phone is required for
the production QR scanner path.

## Validate it

```sh
pnpm --dir apps/openagents-mobile run typecheck
pnpm --dir apps/openagents-mobile run test
```

The current focused suite covers:

- the exact app identity, owned OTA origin, channel, and runtime policy
- the Hermes Web Crypto fallback
- the Metro-reachable source graph staying free of Node built-ins
- bridge framing, admission, mirror updates, revocation, storage, and signer
  cleanup.

These tests do not replace a signed Omega host run or a physical-device run.

## Device builds

Mobile builds are local only. Never use `eas build`, `eas submit`, or
`eas update`.

```sh
pnpm --dir apps/openagents-mobile run prebuild:ios
open apps/openagents-mobile/ios/OpenAgents.xcworkspace

pnpm --dir apps/openagents-mobile run prebuild:android
apps/openagents-mobile/android/gradlew \
  -p apps/openagents-mobile/android :app:assembleDebug
```

Use Apple Team `HQWSG26L43` for iOS signing. Follow the mobile release entry in
`docs/DEPLOYMENT.md` before producing or uploading a release artifact.

## OTA updates

`app.json` points Expo Updates at the owned
`https://updates.openagents.com/openagents-mobile/manifest` feed on the
`openagents-production` channel with fingerprint runtime compatibility. Expo
checks the feed on app load. There is no custom three-second polling loop in
the current app.

Publish only through the owned update path:

```sh
pnpm --dir apps/openagents-mobile run publish:ota
```

This command delegates to `apps/oa-updates/scripts/publish-ota.sh`. Read
`docs/DEPLOYMENT.md` and the indexed OTA runbook before publishing. Do not
describe an OTA export as installed-device evidence until the target device has
actually loaded and verified it.
