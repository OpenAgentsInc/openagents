# Khala Voice App — Spec (v1)

> **Honest-scope header.** This is a direction document, not a product promise.
> It describes the intended first version of a brand-new native iOS app and the
> contracts it targets. Nothing here is a guarantee of a shipped feature, a
> ship date, or a paid offering. Live behavior is only what the running app and
> the live `openagents.com` API actually do. Route any user-facing product
> claims through `docs/promises/` before broadening copy.

- **Date:** 2026-06-26
- **Status:** spec-first scaffold (v1 in progress)
- **Owning surface:** `clients/khala-ios/Khala/` (native SwiftUI iOS app)
- **Replaces:** `clients/khala-ios/AutopilotRemoteControl` (Expo React-Native app,
  retired — see `docs/mobile/2026-06-26-autopilot-remote-control-retirement.md`)

## 1. Product

**Khala** is a minimal **push-to-talk voice client** for the public Khala API.

One screen. You hold a button, speak, and Khala answers. That is the whole app.

- **Name:** Khala
- **Bundle id:** `com.openagents.khala`
- **Platform:** iOS, native SwiftUI. **No Expo / React Native / EAS.**
- **Job:** collect voice (push-to-talk) → speech-to-text → send the transcript
  to the Khala API → show (and optionally speak) the response.

It is deliberately tiny. It does not carry any of the operator / remote-control
features of the retired app (see Non-goals).

## 2. Onyx lineage — what we are bringing back

The interaction model is descended from **Onyx**, OpenAgents' earlier
push-to-talk voice surface. The archived Onyx is a native (Rust/WGPUI) app, not
a React-Native one, but its *voice UX* is exactly the thing we want back:

Reference sources in the archive (study, do not copy verbatim):

- `~/work/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/`
  — the voice pipeline (`session.rs`, `audio_capture.rs`, `transcriber.rs`).
- `~/work/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/onyx/src/app.rs`
  — the voice status colors and the press/release state handling.

What we bring back from Onyx:

1. **Press-and-hold push-to-talk.** Press to start recording; release to stop
   and transcribe. A quick tap (below a minimum hold time, ~200ms in Onyx) is
   discarded as an accidental tap rather than sent. Onyx's `VoiceSession`
   state machine is the model:
   `RecordingStarted → TranscriptionStarted → TranscriptionComplete{text}`
   (with `RecordingDiscarded{reason}` and `TranscriptionError`).
2. **State-as-color feedback.** Onyx encodes voice state as color. We reuse the
   same semantics:
   - Recording — red (`hsl(0, 80%, 50%)`)
   - Transcribing — orange (`hsl(45, 90%, 50%)`)
   - Success — green (`hsl(120, 60%, 50%)`)
   - Idle — neutral dim (dark theme)
3. **A single, calm, dark screen** with a large central press-to-talk control.
   The archive itself has **no animated background**; the animated background is
   the *new* aesthetic direction for Khala (a quiet, "alive" backdrop that
   reacts to voice state — see Section 4). It must stay subtle and dark, not a
   marketing gradient.

(There was no React-Native/Expo Onyx app in the archive; the "translate Onyx's
UI/UX into SwiftUI" intent is satisfied by re-creating its press-and-hold voice
loop and color-coded state in SwiftUI, plus the new animated backdrop.)

## 3. Voice flow

```
[idle] --press-and-hold-->
  start mic capture (AVAudioEngine)
  state = recording (red), background reacts
--release-->
  stop capture
  if hold too short / no speech -> discard, back to idle
  else state = transcribing (orange)
  finalize on-device speech-to-text (Apple Speech / SFSpeechRecognizer)
  -> transcript text
  state = success (green) briefly
  send transcript to Khala API (POST /api/v1/chat/completions, model openagents/khala)
  -> show response text
  (optional) speak response with AVSpeechSynthesizer
back to [idle]
```

- **Mic capture:** `AVAudioEngine` input tap. Configure `AVAudioSession` for
  record/playback. Drive a live amplitude/RMS value from the input buffers to
  feed the animated background.
- **Speech-to-text: on-device, Apple `Speech` (`SFSpeechRecognizer`).** Use
  `requiresOnDeviceRecognition = true` where supported. "Swift speech-to-text
  for now" (owner) — we use Apple's recognizer rather than sending audio to a
  remote STT. The Khala API receives the **transcript text**, not the audio.
- **Send:** the recognized transcript becomes a single user message in a
  `chat/completions` request (Section 5).
- **Display:** render the assistant text in the response area.
- **Speak (optional, v1 nice-to-have):** `AVSpeechSynthesizer` reads the
  response aloud.

## 4. UI / UX (single screen)

- **Layout:** one full-screen dark view.
  - Animated background filling the screen.
  - Centered large circular **push-to-talk button** (press-and-hold).
  - A response/transcript text area above or below the button.
  - A small settings affordance (gear) for the API key (Section 6).
- **Push-to-talk button:** `DragGesture(minimumDistance: 0)` or a long-press
  gesture to detect press-down (start) and release (stop). The button color and
  a pulsing ring reflect the current state (idle/recording/transcribing/
  success/error) using the Onyx color semantics.
- **Animated background (new):** a subtle, dark, "alive" backdrop built with
  native SwiftUI drawing — e.g. `TimelineView` + `Canvas`, a `Shape` with an
  animated path, or `MeshGradient`/`AngularGradient` animation. It should:
  - idle: slow, low-amplitude ambient motion;
  - recording: react to live mic amplitude (breathe/expand with the voice);
  - transcribing: a steady "thinking" motion;
  - stay within the dark, operational aesthetic (no bright marketing look).
  - **No third-party graphics deps.** SwiftUI `Canvas`/`TimelineView` only for
    v1; a richer Metal/visualizer pass is a roadmap item.
- **Typography:** system font; keep it minimal and legible on dark.

## 5. Khala API contract

The app talks to the public Khala inference endpoint. Source of truth:
`apps/openagents.com/apps/web/public/AGENTS.md` (served at
`https://openagents.com/AGENTS.md`).

- **Base URL:** `https://openagents.com/api/v1` (bare `/v1` also works).
- **Endpoint:** `POST https://openagents.com/api/v1/chat/completions`.
- **Model:** `openagents/khala` (one public model — there are no
  mini/pro/code variants).
- **Auth:** `Authorization: Bearer <oa_agent_… key>`.
- **Request body (non-stream first):**

  ```json
  {
    "model": "openagents/khala",
    "messages": [{ "role": "user", "content": "<transcript>" }]
  }
  ```

- **Response:** OpenAI-style chat completion; read
  `choices[0].message.content`.
- **Streaming (later):** `"stream": true` returns SSE. v1 is **non-stream**;
  streaming is a roadmap item.
- **Quota / errors:** free tier is rate/token limited (per the live contract,
  currently ~2,000 requests/day · 2,500,000 tokens/day, UTC reset). Over quota
  returns **`402`**. The app should surface a clear "free quota reached / add
  credits or wait for reset" message on 402, and a generic retry message on
  other non-2xx.

**Honest disclosure (must be reflected in-app).** The free Khala API is
**captured by default**: traffic becomes a **redacted, private-by-default
(`owner_only`) trace that may be used to improve/train OpenAgents models**, and
the tokens count on the **public served-token counter**
(`GET /api/public/khala-tokens-served`). Capture grants no payout. Paying for
privacy (or confidential compute) opts out of capture. Canonical terms:
`GET https://openagents.com/api/public/free-tier-data-sharing` (also embedded in
the `POST /api/keys/free` mint response as `dataSharing`). The app must show a
short, honest note of this before/at first use of a free key.

## 6. Auth & storage

- **Get a key:** mint a free key in-app via
  `POST https://openagents.com/api/keys/free` → read `credential.token`
  (an `oa_agent_…` value). Show the data-sharing disclosure (Section 5) at mint
  time. Alternatively, the user can **paste an existing key**.
- **Store:** the key lives in the **iOS Keychain** (via `Security` framework).
  Never store it in `UserDefaults` or plist. The scaffold ships a small
  `KeychainStore` wrapper.
- **No account/login** in v1 beyond holding a bearer key.

## 7. Architecture

- Native **SwiftUI** app, no third-party dependencies required.
- Frameworks: `Speech` (STT), `AVFoundation` (mic capture + optional TTS),
  `Foundation`/`URLSession` (HTTP), `Security` (Keychain), `SwiftUI` (UI).
- Source layout under `clients/khala-ios/Khala/Khala/`:
  - `KhalaApp.swift` — `@main` app entry.
  - `ContentView.swift` — the single screen (button + response + background).
  - `Views/PushToTalkButton.swift` — press-and-hold control + state color.
  - `Views/AnimatedBackground.swift` — `TimelineView`/`Canvas` backdrop.
  - `Views/SettingsView.swift` — key mint/paste + disclosure.
  - `Voice/VoiceController.swift` — the state machine (idle→recording→
    transcribing→success/error), `AVAudioEngine` capture, live amplitude.
  - `Voice/SpeechRecognizer.swift` — `SFSpeechRecognizer` wrapper.
  - `Voice/SpeechSynthesizer.swift` — optional `AVSpeechSynthesizer` wrapper.
  - `Net/KhalaClient.swift` — `URLSession` POST to `/api/v1/chat/completions`
    and `POST /api/keys/free`.
  - `Store/KeychainStore.swift` — Keychain read/write for the API key.
  - `Resources/Info.plist` — usage strings + bundle id.
- **Permissions (Info.plist):**
  - `NSMicrophoneUsageDescription` — "Khala records your voice so you can talk
    to it push-to-talk."
  - `NSSpeechRecognitionUsageDescription` — "Khala turns your speech into text
    on-device to send to the Khala model."

## 8. Build & ship (local Xcode only)

Per the repo **mobile build/ship policy (owner mandate): NO Expo/EAS cloud**
(see repo-root `AGENTS.md` → "Mobile build/ship policy" and `docs/DEPLOYMENT.md`).
Khala is native Swift, so there is **no Expo prebuild / OTA path at all** — it
is pure local Xcode:

- **Build/run locally:** open `clients/khala-ios/Khala/Khala.xcodeproj` in Xcode
  and run on a simulator or device. (The project is generated via `xcodegen`
  from `clients/khala-ios/Khala/project.yml`; run `xcodegen generate` in
  `clients/khala-ios/Khala/` first if the `.xcodeproj` is absent — see the app
  README.)
- **Archive → TestFlight:** local `xcodebuild archive` + `-exportArchive`, then
  Apple-native TestFlight upload via `xcrun altool` (or Transporter), using the
  App Store Connect API key in workspace `.secrets/appstoreconnect.env` /
  `.secrets/asc_api_key.json`.
- **Signing:** Apple Team `HQWSG26L43` (OpenAgents, Inc.). A new App Store
  Connect app record + bundle id `com.openagents.khala` + provisioning profile
  are **owner-gated** (see NEEDS-OWNER in the retirement note / README).
- **Never** run `eas build` / `eas submit` / `eas update`. There is no
  `u.expo.dev` for this app.

## 9. Scope boundary

**v1 is JUST: voice → on-device text → Khala API → response (optionally spoken).**

**Non-goals for v1 (explicitly out):**

- No remote-control / operator features from the retired Autopilot Remote
  Control app (no Pylon pairing, node steering, projections, stream cursors,
  bearer-node material, MMKV state, drawer nav).
- No multi-turn conversation history persisted across launches.
- No streaming responses.
- No accounts, no payments UI, no credit purchase flow in-app.
- No Android (iOS only for v1).
- No third-party SDKs.

## 10. Roadmap (after v1)

- **Streaming** responses (`stream: true`, SSE) with incremental text + spoken
  output.
- **Text-to-speech** polish (voice selection, barge-in/stop).
- **Conversation history** (multi-turn context, on-device persistence).
- **Richer Onyx-style visualization** (Metal/Canvas audio-reactive visualizer
  beyond the v1 ambient backdrop).
- **Credits / paid privacy** surfacing (opt out of free-tier capture).
- Possible **Android** port.
