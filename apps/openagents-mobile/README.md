# OpenAgents mobile

The current OpenAgents iOS and Android source has two plain React Native
surfaces. The released app can mirror an admitted Omega desktop. The source
also implements a managed Sarah voice session with the protected mobile device
identity. The live service and physical-device release proof are pending. Expo
provides the native host, local build tools, camera access, microphone access,
secure storage, and the owned OTA feed. Effect Native is not part of the
mounted UI.

- Display name: `OpenAgents`
- iOS bundle identifier / Android application ID: `com.openagents.app`
- App entry: `src/app.tsx`
- Mounted screens: `src/screens/omega-home-screen.tsx` and
  `src/screens/sarah-voice-screen.tsx`
- Device bridge protocol: `openagents.omega.device_bridge.v1`
- Sarah voice protocol: `openagents.sarah.voice.v1`

The owner-directed reset on 2026-07-27 removed the previous general
conversation, Effect Native, Sync, managed-sandbox, Full Auto, notification,
settings, and mobile command surfaces. The managed Sarah voice surface uses
only the protected device identity and a normal OpenAgents session. Historical
receipts remain evidence only for the commits that they identify.

## What exists today

The app opens a signed device identity from Expo SecureStore and connects to an
Omega desktop bridge over WebSocket. A person can:

- scan the short-lived QR bootstrap shown by Omega desktop, with the in-app
  scanner or with the phone camera app. The QR can carry the raw bootstrap
  JSON or an `https://openagents.com/pair#<base64url>` Universal Link. The two
  formats decode through the same pairing schema
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

The mounted production screen supplies the cached endpoint, an optional QR
bootstrap, and each pairing Universal Link the OS gives the app. The watcher
in `src/screens/omega-pairing-link.ts` reads the initial URL and each later
link event. The screen does not currently supply signed announcement
discovery, manual MagicDNS, or a relay observation feed. Simulator development
can provide a bootstrap through either:

- `EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP` for inline JSON
- `EXPO_PUBLIC_OMEGA_PAIRING_BOOTSTRAP_URL` for a JSON endpoint.

## Sarah voice

The home screen has a `Talk to Sarah` action. The Sarah screen opens the
protected Nostr device signer from SecureStore. On a fresh install, it uses
that signer to request a short normal OpenAgents bearer session. It can use the
bounded Sarah NIP-98 challenge when automatic account sessions are not
available. The app does not contain an OpenAI API key.

The API reserves normal OpenAgents credit and returns a one-use gateway ticket.
The ticket stays in WebSocket headers. It does not enter the URL.

Expo Audio captures mono 24 kHz PCM. The local native audio module plays the
managed gateway PCM stream. The screen shows live user and Sarah transcripts.
It also has mute, interrupt, end, and retry controls.

The microphone is active only when the app is in the foreground and the screen
shows `Listening`. Capture stops while Sarah speaks. Capture and playback stop
when the app leaves the foreground or the screen closes. The screen does not
persist audio or transcript text.

The mobile profile is `mobile_voice_only`. This profile has no tools. Sarah
cannot use this surface to control files, an editor, a shell, Git, URLs,
payments, accounts, or device actions. The mobile client also rejects an
unexpected tool frame and closes the session.

## Capability boundary

The Omega surface is a read-only desktop mirror. It does not render a transcript
composer because the mounted surface does not open the signed relay command
lane. The Sarah surface is voice-only and has no device command authority. The
app does not currently:

- create, append to, steer, interrupt, or approve desktop work
- synchronize general account conversations
- control managed sandboxes, terminals, files, diffs, Git, or Full Auto
- register or consume push notifications
- run a model, shell, cloud SDK, or desktop authority on the phone.

The managed gateway runs Sarah outside the phone. The phone has no provider
credential. A live service journey and physical-device reliability require
candidate-bound evidence before a TestFlight upload.

## Architecture

- `src/app.tsx` loads the bundled faces. It then mounts `OmegaHomeScreen` or
  `SarahVoiceScreen` inside the safe-area provider.
- `src/screens/omega-home-screen.tsx` owns QR scanning, connection notices,
  activity ordering, selection, and the clock the relative stamps read against.
- `src/screens/sarah-voice-screen.tsx` owns the foreground voice lifecycle,
  microphone permission, capture, playback, transcripts, and user controls.
- `src/sarah-voice/` owns strict protocol frames, managed session
  authentication, reconnect limits, and the protected bearer-session vault.
- `modules/expo-realtime-audio/` owns native PCM playback for iOS and Android.
  Expo Audio owns capture. Neither path has a background mode.
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
- the exact microphone permission and the absence of background audio modes
- the Hermes Web Crypto fallback
- the Metro-reachable source graph staying free of Node built-ins
- bridge framing, admission, mirror updates, revocation, storage, and signer
  cleanup
- Sarah session authentication, ticket handling, credit errors, reconnect,
  sequence checks, transcript updates, unsupported-tool refusal, audio bounds,
  and background cleanup.

These tests do not replace a signed Omega host run, a live managed Sarah
gateway run, or a physical-device run.

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

Sarah voice changes the native permission and native audio module. Do not ship
this change as an OTA update. Use a new store build.

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
