# OpenAgents mobile

The current OpenAgents iOS and Android app mounts a bounded Pro controller. It
reads the owner's reactive work projection from Convex and sends mutations only
through Pro's authenticated capability broker and durable device outbox. The
source still contains the earlier Omega desktop mirror and managed Sarah voice
implementations, but they are not the mounted root. Physical-device release
proof remains pending. Expo provides the native host, local build tools,
notifications, Live Activities, share intake, quick actions, camera access,
microphone access, and secure storage. Effect Native is not part of the mounted
UI.

- Display name: `OpenAgents`
- iOS bundle identifier / Android application ID: `com.openagents.app`
- App entry: `src/app.tsx`
- Mounted root: `src/controller/controller-root.tsx`
- Device bridge protocol: `openagents.omega.device_bridge.v1`
- Sarah voice protocol: `openagents.sarah.voice.v1`

The owner-directed reset on 2026-07-27 removed the previous general
conversation, Effect Native, Sync, managed-sandbox, Full Auto, notification,
settings, and mobile command surfaces. The managed Sarah voice surface uses
only the protected device identity and a normal OpenAgents session. Historical
receipts remain evidence only for the commits that they identify.

## Legacy Omega mirror and Sarah voice

The source can open a signed device identity from Expo SecureStore and connect
to an Omega desktop bridge over WebSocket. Those modules remain available for
future composition but are not mounted by `src/app.tsx`. They support:

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

The local native audio module captures mono 24 kHz PCM and plays the managed
gateway PCM stream. The screen shows live user and Sarah transcripts.
It also has mute, interrupt, end, and retry controls.

The microphone is active only when the app is in the foreground and the screen
shows `Listening`. Capture stops while Sarah speaks. Capture and playback stop
when the app leaves the foreground or the screen closes. The screen never
persists audio; final transcript records use the app-private store below.

Authenticated owner sessions use the `mobile_command_center` profile. Before
voice connects, the app bootstraps the canonical Sarah thread and the gateway
advertises only Sarah's server-owned, receipted coding-capacity and existing
Full Auto tools. Tool activity is visible in the transcript. Repository work
is delegated to coding workers; Full Auto pause, resume, and stop remain
pending until Desktop applies them. The phone still cannot control files, an
editor, a shell, Git, URLs, payments, accounts, or device actions. The
`mobile_voice_only` profile remains the fail-closed NIP-98 fallback.

Every final user, Sarah, and command-activity transcript item is appended to
the app-private `sarah/voice-transcripts.jsonl` file. Audio is never written.
The realtime client sends a heartbeat every 15 seconds and resets its bounded
reconnect budget after each provider-confirmed stable connection.

## Capability boundary

The mounted controller is an authenticated projection and command client, not
execution authority. Notifications, Live Activities, share intake, and quick
actions can navigate or collect local input; they cannot execute work. Every
mutation still passes Pro's capability checks and durable outbox. The app does
not currently:

- synchronize general account conversations
- run a managed sandbox, terminal, filesystem, Git operation, or model locally
- run a model, shell, cloud SDK, or desktop authority on the phone.

The managed gateway runs Sarah outside the phone. The phone has no provider
credential. A live service journey and physical-device reliability require
candidate-bound evidence before a TestFlight upload.

## Architecture

- `src/app.tsx` loads the bundled faces, durable command outbox, controller
  session, ambient surfaces, and `ControllerRoot` inside the safe-area provider.
- `src/controller/` owns signed session bootstrap, Convex projections,
  deep-link routing, screens, and the disposable screenshot launch adapter.
- `src/ambient/` owns strict notification/Live Activity contracts, atomic tap
  deduplication, durable share intake, and launcher shortcuts.
- `src/screens/omega-home-screen.tsx` owns QR scanning, connection notices,
  activity ordering, selection, and the clock the relative stamps read against.
- `src/screens/sarah-voice-screen.tsx` owns the foreground voice lifecycle,
  microphone permission, capture, playback, transcripts, and user controls.
- `src/sarah-voice/` owns strict protocol frames, managed session
  authentication, reconnect limits, and the protected bearer-session vault.
- `modules/expo-realtime-audio/` owns native PCM capture and playback for iOS
  and Android. Neither path has a background mode.
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

## Ambient surfaces

The Pro controller adds four native, projection-only entry surfaces:

- `expo-notifications` opens an exact owner workspace and work aggregate after
  an atomic SQLite notification-ID claim, so cold-start and live listeners
  cannot handle one tap twice;
- `expo-live-activity` generates the iOS widget extension and reconciles a
  bounded, generic status from the current work-shell generation;
- `expo-share-intent` generates the iOS Share Extension and Android intent
  filters, then copies shared text, URLs, and images into a durable local
  intake inbox;
- `expo-quick-actions` exposes fixed routes for the attention inbox, share
  inbox, and new-task entry point.

Push and Live Activity payloads never contain transcript text, summaries,
project names, or thread titles. These surfaces can navigate or collect local
intake only; commands still require the authenticated Pro capability broker and
durable client outbox.

Pro owns the one-command real-backend screenshot matrix. From the Pro checkout,
run `OPENAGENTS_REPO=../openagents pnpm mobile:screenshots` to build the release
iOS workspace, capture Home/Thread/Inbox/Review in light and dark on disposable
phone/tablet simulators, validate dimensions and color mode, and destroy the
seeded Convex workspace.

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
- Sarah session authentication, canonical owner bootstrap, ticket handling,
  credit errors, heartbeat/reconnect, sequence checks, durable transcript
  appends, brokered command activity, unsupported-device-tool refusal, audio
  bounds, and background cleanup
- strict ambient payloads, atomic notification claims, exact deep links,
  durable share intake, Live Activity reconciliation, Pro harness bootstrap,
  and the native SDK configuration.

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

## OTA updates (retired)

There is no OTA lane. `app.json` sets `updates.enabled: false` and configures
no update URL. The owned `https://updates.openagents.com/openagents-mobile/manifest`
feed was retired on 2026-08-05 (#9325) at owner direction — there are no
installed mobile users — and that route now 404s.

The `updates` key stays present and false rather than being deleted:
`expo-updates` is still linked into the binary and `updates.enabled` defaults
to true, so removing the key would leave the client enabled with no configured
URL.

Ship JS changes in a **new store build**. Never `eas update`, and never point
this app at a new update endpoint without an owner decision and a new
invariant. The in-app update row on the home screen still reads the
`expo-updates` runtime for build/runtime diagnostics; with updates disabled its
manual check cannot succeed.
