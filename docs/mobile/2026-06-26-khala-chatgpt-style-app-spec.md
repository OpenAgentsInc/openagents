# Khala Mobile App — ChatGPT-style spec (2026-06-26)

Status: active build spec. Supersedes the minimal voice-only scope in
`2026-06-26-khala-voice-app-spec.md` (that doc stays as the voice-runtime
reference; this doc is the full-app target).

## Goal

Sprint to a **fully working Khala mobile app** that dogfoods the Khala API for
real internal coding work, with a polished, familiar UI. Native SwiftUI, local
Xcode only (no Expo/EAS — see repo `CLAUDE.md` mobile policy). Bundle
`com.openagents.khala`, Team `HQWSG26L43`.

The UI should feel like the **ChatGPT iOS app** (reference screenshots provided
by the owner, 2026-06-26): a left slide-over drawer with chat history, a model
pill in the top bar, markdown chat bubbles, streaming responses, and a composer
with `+`, text field, mic, and a voice button. **Keep the existing cool voice
visualization** (the animated push-to-talk orb) as the voice-input affordance.

## Reference (ChatGPT iOS app patterns, from the owner's screenshots)

1. **Left drawer (hamburger):** app title + search; a short menu; a **Recents**
   section listing historical chats; bottom row with **New Chat** + settings.
   Opens as a slide-over that dims the main view.
2. **Empty chat:** top bar = hamburger · model pill · new-chat icon. Centered
   suggestion rows (icon + label). Composer pinned to the keyboard.
3. **Model pill / picker:** ChatGPT shows "5.5 Medium" → a dropdown of
   intelligence levels. **Khala is a SINGLE model** (`openagents/khala`) — see
   the single-model constraint below; we do NOT replicate fake variants.
4. **Chat view:** full-width assistant messages with rich **markdown** (bold,
   bullet lists, headings, **code blocks**), a response action row (copy, etc.),
   and the composer with mic + voice button.

## Hard constraints

- **Single model.** Khala is one model: `openagents/khala`. Do NOT add
  mini/pro/code variants or a fake "intelligence" picker. The top-bar pill reads
  **"Khala"** and (v1) is non-interactive or opens a tiny sheet (About / which
  backend is live), never a fabricated variant list.
- **Backend is simple:** hit the Khala OpenAI-compatible API and render what
  comes back. No server-side history sync in v1 — conversations persist
  **locally** on device.
- **Standard SwiftUI components** (`NavigationStack`, `List`, `ScrollView`,
  `TextField`, `Menu`, `.sheet`) **plus** the existing voice visualization.
- **Coding dogfood:** markdown + **code-block rendering with monospace + copy**
  is first-class (this is how we read Khala's coding answers).
- Reuse the existing pieces: `Net/KhalaClient.swift`, `Store/KeychainStore.swift`,
  `Voice/*` (SpeechRecognizer, VoiceController, PushToTalkButton, AnimatedBackground),
  `Views/SettingsView.swift`, the env-gated demo hooks (`KHALA_API_KEY`,
  `KHALA_DEMO_PROMPT`).

## API

- Base: `https://openagents.com/api/v1`, OpenAI-compatible.
- Endpoint: `POST /chat/completions`, model `openagents/khala`,
  `Authorization: Bearer <key>` (key from Keychain; mint free via
  `POST /api/keys/free`; Settings manages it).
- **Multi-turn:** send the full `messages` array (system optional + the
  conversation's user/assistant turns), not just the latest.
- **Streaming:** use `stream: true` (SSE, `data:` lines, `[DONE]` terminator) and
  append tokens live for the ChatGPT-style typing effect. Fall back to
  non-streaming if a turn errors.
- Errors: 402 → "free quota reached / add credit" message; network/timeout →
  inline retry; never crash.

## Architecture

- **App shell:** a root view hosting (a) the main `NavigationStack` chat surface
  and (b) a left **slide-over drawer** (custom overlay with drag-to-open + dim
  scrim, standard SwiftUI + a `DragGesture`/`offset` animation, reduced-motion
  safe). The hamburger in the top bar toggles the drawer.
- **Xcode project — file-system-synchronized group (IMPORTANT for parallel
  build):** migrate `Khala.xcodeproj` to an Xcode 16
  `PBXFileSystemSynchronizedRootGroup` for the `Khala/` source folder so new
  Swift files are picked up **without per-file `project.pbxproj` edits**. This
  lets the feature lanes add files in parallel without `pbxproj` merge
  conflicts. (Foundation lane does this first.)
- **Persistence:** `Conversation` + `Message` stored locally. Prefer **SwiftData**
  (`@Model`) if it's clean on iOS 17+; otherwise `Codable` JSON in Application
  Support. Drawer Recents and the chat view read from this store.

### Data model

```
Conversation { id: UUID, title: String, createdAt, updatedAt, messages: [Message] }
Message      { id: UUID, role: .system|.user|.assistant, content: String, createdAt }
```

- Title: derived from the first user message (truncated), renamable.
- New / rename / delete conversation; sorted by `updatedAt` desc in Recents.

## Screens & components (maps to issues below)

1. **Foundation / shell** — sync-group migration; `Conversation`/`Message` +
   local store; app shell with the slide-over drawer container; **app launches
   to a visible chat surface (NOT black)** — this is the regression gate.
2. **Drawer + history** — ChatGPT-style left drawer: "Khala" title, search
   (filters Recents), Recents = conversation list, bottom New Chat + settings
   gear; tap a chat to open it; swipe to delete; rename.
3. **Chat view + markdown** — message list (full-width assistant turns, compact
   user turns), **markdown rendering** incl. fenced **code blocks** (monospace,
   horizontal scroll, copy button), response action row (copy message,
   regenerate). Auto-scroll to latest.
4. **Composer + streaming** — `+`, multiline `TextField` ("Ask Khala"), mic
   (existing STT → fills/sends), voice button (→ existing push-to-talk orb /
   `AnimatedBackground` voice viz), send. Stream tokens into the assistant
   bubble live.
5. **Multi-turn Khala client** — extend `KhalaClient` to send full history with
   `stream: true` (SSE), single model `openagents/khala`, 402/error handling,
   cancellation.
6. **Top bar + empty state + settings** — hamburger · "Khala" pill · new-chat
   icon; empty-state greeting + (optional) suggestion rows; Settings (existing
   key mgmt) reachable from the drawer; retain the voice viz everywhere it makes
   sense.

## Voice (retain + integrate)

- Keep `VoiceController` / `PushToTalkButton` / `AnimatedBackground`. The voice
  button in the composer enters the push-to-talk orb; on release it transcribes
  and sends as a normal user turn into the active conversation (so voice and
  text share one transcript). The orb visualization quality stays.

## Build / run / verify

- Local Xcode only. `clients/khala-ios/Khala/Khala.xcodeproj`, scheme `Khala`,
  iOS 17+.
- Every lane must end with a **clean Debug build + a launch-render check on the
  iPhone 17 simulator** (screenshot shows the real UI, not black), using the
  env demo hooks where useful.
- The black-screen the owner saw is a **stale local build/DerivedData** symptom
  (origin/main renders correctly); after landing, resync the owner's working
  tree for `clients/khala-ios/Khala` and clear `~/Library/Developer/Xcode/DerivedData/Khala-*`.

## Out of scope (v1)

- Server-side conversation sync, multi-device history, accounts beyond the
  single API key, image generation, web search, file attachments. (Composer `+`
  can stub these as disabled/"coming soon".)

## Issue breakdown

Tracked as GitHub issues (one foundation issue, then parallel feature issues —
the sync-group migration makes the feature lanes `pbxproj`-conflict-free):

- **A. Foundation:** sync-group migration + data model + local store + app shell
  + drawer container + **non-black launch gate**.
- **B. Drawer + chat history (Recents)** + new/rename/delete/search.
- **C. Chat view + markdown/code-block rendering** + response actions.
- **D. Composer + streaming** + voice-viz integration.
- **E. Multi-turn streaming `KhalaClient`** (full history, SSE, single model).
- **F. Top bar + model pill ("Khala") + empty state + settings wiring.**
