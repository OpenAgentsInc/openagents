# OpenAgents mobile — v0.5.2 build 113: Sarah conversation surface, text-first (GL-3 #8649)

Selecting **Sarah** from the pill dropdown now opens a REAL conversation with
production Sarah — the same `/sarah` contracts the web surface uses — inside
the GL-2 Liquid Glass shell. The bundled demo loop stays as AMBIENT BACKGROUND
ONLY (muted, behind the conversation); it is presentation, never conversation
evidence.

## Verified production contracts (before any code)

- `POST /sarah/api/prospect/session` → `{prospectRef, threadId, minted}`
  (mint verified live; ref persisted on-device, sent explicitly in turn
  bodies — the mobile relationship never depends on cookies).
- `POST /sarah/api/eve/turn` `{message, prospectRef, threadId?}` →
  `{ok, reply, modelPath, threadId, toolResults…}` — the exact route the web
  composer (`sendTextTurn` in `apps/sarah/src/ui/main.ts`) uses; verified
  live (`modelPath=khala_gateway_live`, ~16s).
- `GET /sarah/api/avatar/events?ref=` — SSE (`data: {json}\n\n`,
  `: connected`, `: hb` every 25s). VERIFIED against the server source: the
  owned TEXT turn path does not publish to this bus today (it feeds the
  avatar/brain tiers via `publishSarahAvatarEvent`), so v1 renders turn
  replies from the POST result exactly like web, and binds the stream for
  typed liveness + cards. Gap filed as residue: text turns should publish to
  the conversation stream so pure-SSE transcripts work across surfaces.

## Architecture (one typed program, no parallel models)

- `src/screens/sarah-core.ts` — PURE: `SarahState` slice, typed intents
  (`SarahSessionReady/Unavailable`, `SarahDraftChanged`, `SarahTurnSubmitted`,
  `SarahStreamStatusChanged`, `SarahEventReceived`), bounded entries/cards/
  text clipping, SSE frame parser, and the EN view projection — catalog
  `Transcript` (pinToEnd) + `Card` + `Composer` (real bound TextInput) +
  typed Send button, all on honest `surface: "glass"` materials.
- `src/screens/home-core.ts` — the ONE Home program gains the `sarah` slice;
  handlers are pure state updates; the production turn client is the single
  injected effect seam (`HomeProgramOptions.sarahTurn`).
- `src/sarah/sarah-client.ts` — the EFFECTFUL half: prospect mint, turn POST
  (45s bound), session persistence via `expo-file-system` (the ONLY
  JS-reachable persistent store already linked in the shipping runtime —
  `ExpoFileSystem.framework` verified inside the build-112 archive; adding
  AsyncStorage would have broken the JS-only OTA rail), and the bounded SSE
  loop over `expo/fetch` streaming with exponential backoff (1s→15s cap),
  typed phases (`connecting/live/reconnecting/unavailable`), and a 40s
  heartbeat-silence watchdog (server heartbeats every 25s; an idle dead TCP
  socket must never masquerade as live — found on-device during the
  reconnect proof).
- Sarah mode hides the tap-only SwiftUI glass composer (demo surface) and
  never triggers the reply-video takeover (`ComposerPressed` guard).

## On-device bugs found by the proofs (fixed + regression-tested)

1. **Restore key collision** — after restart the turn counter reset, so the
   next reply overwrote a RESTORED bubble (both Sarah bubbles showed the new
   text). Fixed: `turnCounterFromEntries` resumes numbering past persisted
   keys; regression test asserts the restored bubble stays intact.
2. **Dead-socket liveness** — 14s of airplane-Wi-Fi left the status "live"
   (idle TCP looks connected for minutes). Fixed with the 40s heartbeat
   watchdog above.
3. **iOS multiline submit** — RN multiline TextInputs never fire
   `onSubmitEditing`; the typed Send button carries the live draft through
   the SAME `SarahTurnSubmitted` intent (StaticPayload re-rendered per
   keystroke).

## Tests (32 pass, typecheck clean)

Behavior contract `openagents_mobile.sarah_text_surface.v1` (enforced,
test-sweep) — oracle `tests/sarah-surface.test.ts`: typed turn round-trip
(submit → user+thinking → done reply); failure → typed failed entry, composer
alive; turn-bootstrap session adoption from `threadId prospect:<ref>`; typed
SSE transcript/card events with dedupe; stream phase tracking; restored
continuity marker; key-collision regression; Send-button draft payload; SSE
frame parser (partial frames, comments); real render-rn round-trip driving
the lowered TextInput handlers.

## Simulator pixel proofs (iPhone 17 Pro, Release; idb HID taps/typing — no
host cursor; committed under receipts/)

1. `2026-07-10-build113-sarah-surface-live.png` — Sarah selected from the
   dropdown: "Sarah — live" (real prospect session + SSE), Message Sarah
   composer + Send, ambient video behind.
2. `2026-07-10-build113-turn-thinking.png` — sent turn: user bubble, "Sarah
   is thinking…" placeholder, composer cleared to "Sarah is replying…".
3. `2026-07-10-build113-production-reply.png` — **production Sarah replies
   in the shell** ("Hi Chris, I'm Sarah, an AI sales employee for
   OpenAgents—we're an agentic infrastructure where AI agents perform
   autonomous work and earn Bitcoin…").
4. `2026-07-10-build113-restart-memory.png` — app killed + relaunched:
   "live · continued", transcript restored from disk, and Sarah answers
   "Yes, Chris, you are from the mobile team." — the persisted prospectRef
   carries the server-side relationship across restarts.
5. `2026-07-10-build113-offline-failed-turn.png` — Wi-Fi off: status
   "reconnecting", the sent turn degrades to the RED typed failure entry
   ("I couldn't reach Sarah — check your connection"), composer + Send alive.
6. `2026-07-10-build113-offline-stream.png` — sustained loss: status
   "Sarah — offline" (typed unavailable after bounded failures), surface
   fully usable.
7. `2026-07-10-build113-reconnected-live.png` — Wi-Fi restored: automatic
   recovery to "live · continued", no user action.

## Delivery — both rails

- **OTA (JS-only, instant):** bundle exported from this tree and seeded for
  build 112's EMBEDDED runtime `b0211cc7bdb65d42495ad2e0639db5eb16da721f`
  (read from the build-112 archive's `EXUpdates.bundle/fingerprint` — the
  definitive value; note the GL-2 OTA's `5c5dc315…` seed matched build 111
  only). Runtime-compat note: the new `expo-file-system` JS import is safe on
  the 112 binary because its native framework already ships there (verified);
  the fingerprint delta is packaging-level, not native-behavior-level.
  Channel `openagents-production`, owner `openagents-mobile`. Publish
  evidence on #8649.
- **TestFlight build 113** (version FROZEN at 0.5.2): prebuild re-run after
  the bump (archived `CFBundleVersion=113` verified), archive Team
  `HQWSG26L43`, manual-signing export, `altool` upload. Upload/VALID evidence
  on #8649.

## Residue (filed on #8649 at close)

- Owned text-turn → conversation-SSE publication (server-side; enables
  pure-stream transcripts + cross-surface live continuation).
- Voice/avatar tiers per #8610 capacity policy (never block text).
- Account linking → operator posture through server-owned policy only.
- Native SwiftUI glass TextField in the composer bar (GL-1/GL-4 lowering
  lane; the EN Composer + Send row is the honest v1).
- Android simulator proof (iOS proven here; Android fallback chrome exists).
