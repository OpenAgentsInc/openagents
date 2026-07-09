# LiveAvatar (HeyGen) integration assessment — giving Sarah a face, tools, and popup components

Date: 2026-07-09
Status: assessment + recommended architecture (owner asked: can the embed be
programmatically connected; can she pop components and have tools; is LITE
mode needed)
Sources: the three installed agent skills
(`.agents/skills/liveavatar-{integrate,debug,feedback}` + their reference
guides), docs.liveavatar.com (llms.txt index, Custom LLM page, OpenAI
Realtime connector page), and the current Sarah stack in `apps/sarah`.
Tracking: #8594 (Sarah epic).

## 0. Verdict up front

- **The embed you tested is a sealed box.** Its own guide says it plainly:
  "One API call, one iframe. No SDK, no WebRTC, no event handling." No
  events out, no commands in, no tools, no UI hooks. It's fine as a
  talking head on a landing page; it is a dead end for connecting into our
  system. Keep it as a demo artifact, don't build on it.
- **Yes, everything you want is possible — on FULL Mode with the Web SDK
  plus the Custom LLM add-on.** That combination gives us: avatar video
  rendered into our own `/sarah` page (not an iframe), a live event stream
  (every user/avatar utterance as data events) that our existing card
  system can render popups from, programmatic control (make her speak,
  interrupt her), and — the key move — **LiveAvatar's brain replaced by
  OUR brain**: they call an OpenAI-compatible endpoint we host, which is
  the owned Sarah runtime (Gemma 4 on our Google inference + the
  deterministic deal-rule enforcement + the real tools). Tools work
  because the tool loop runs inside our endpoint, server-side — LiveAvatar
  never needs to know tools exist.
- **You do NOT need LITE mode for v1.** LITE means we bring our own
  STT + TTS + turn manager and stream raw PCM audio; we don't have a
  separable STT/TTS pipeline today (our current voice loop is an
  integrated OpenAI speech-to-speech model). LITE is the later
  cost/sovereignty play (1 credit/min vs 2, full pipeline ownership), not
  the fastest path to a working, tool-wielding avatar.

## 1. What the platform actually offers (from the skills + docs)

Three integration tiers:

| Tier | What it is | Events/control | Tools | Cost |
|---|---|---|---|---|
| **Embed** | iframe from `POST /v2/embeddings` (avatar_id + context_id) | None. No SDK, no events | Only what the pasted context prompt says — no real tools | 2 credits/min (FULL under the hood) |
| **FULL Mode** | LiveAvatar runs ASR → LLM → TTS → video; we run the session via `@heygen/liveavatar-web-sdk` | **LiveKit data channels**: receive `user.transcription`, `avatar.transcription`, `avatar.speak_started/ended`, `user.speak_started/ended`; send `avatar.speak_text`, `avatar.speak_response`, `avatar.interrupt`, listening controls | **Custom LLM add-on**: they call any OpenAI-compatible `/chat/completions` endpoint — ours — where we run the whole tool loop | 2 credits/min |
| **LITE Mode** | We run STT + LLM + TTS; they render video from our PCM 16-bit/24KHz audio over a WebSocket (`agent.speak` chunks) | Their WebSocket protocol (`agent.*`/`session.*`) — completely different from FULL's | Everything is ours by construction | 1 credit/min |

Connectors (LITE-priced bridges to hosted voice agents — ElevenLabs Agent,
OpenAI Realtime, Gemini Live): LiveAvatar runs the bridge for you. The
OpenAI Realtime connector looks tempting because our current voice loop IS
OpenAI Realtime — but the docs confirm it returns **no WebSocket endpoint
to us** and documents **no tool-call event forwarding**; you get voice +
temperature + a context prompt, and tool calls are "see OpenAI's docs"
with no stated delivery channel. Our S-5 realtime tool bridge would not
survive that bridge. Not recommended.

Session mechanics common to real integrations (not embed):
backend mints a session token with `X-API-KEY` (secret, Secret Manager
only), backend starts the session with `Bearer <session_token>`, frontend
gets only the `livekit_client_token`. 5-minute inactivity timeout →
keep-alive every 2–3 min. Sessions burn credits per minute until stopped —
teardown is mandatory. Free sandbox exists for both modes (~1-minute
sessions, dedicated sandbox avatar IDs).

## 2. The recommended architecture (FULL + Custom LLM, SDK in our shell)

```text
 Visitor mic ──► LiveAvatar ASR ──► POST /chat/completions ──► apps/sarah
                                     (our OpenAI-compatible      owned runtime:
                                      endpoint, bearer-guarded)   Gemma 4 (our Google
                                                                  inference) + pricing
                                                                  guard + REAL tools
                                                                  (intake, checkout,
                                                                  handoff, CRM)
                                          final text ◄────────────┘
 Avatar video+voice ◄── LiveAvatar TTS+render ◄── final text
        │
        ▼
 our /sarah page (zero-React shell, EN trajectory)
   ├─ <video> element — session.attach(video), OUR layout, no iframe
   ├─ LiveKit data-channel events → existing typed dispatch → cards
   └─ side-channel SSE from apps/sarah → tool-result cards
      (checkout link, intake recorded, handoff confirmation)
```

Why this shape wins:

1. **One brain.** Voice-Sarah and text-Sarah become the same owned runtime
   — Gemma 4 on our Google inference, the deterministic
   no-improvised-pricing guard, the same session index and receipts. The
   avatar surface stops depending on the OpenAI Realtime + Vercel AI
   Gateway path entirely (that loop stays as the non-avatar voice
   fallback until retired).
2. **Real tools without platform support.** LiveAvatar's hosted loop has
   no documented function calling — and it doesn't matter. Their Custom
   LLM add-on calls our `/chat/completions`; inside that handler we run
   the full tool loop (intake_capture, deal_rules_evaluate,
   checkout_link_create, human_handoff, CRM writes) and return the final
   utterance. To LiveAvatar it's just a chat completion. Tool *effects*
   reach the page via our own side-channel (below).
3. **Components popping is our existing card system.** The `/sarah` shell
   already has a typed dispatch with UI cards (`ui/card`). Two feeds
   drive it: (a) LiveKit data-channel events from the SDK
   (`user.transcription` / `avatar.transcription` / speak lifecycle) for
   captions, state, and timing; (b) a session-keyed SSE stream from
   `apps/sarah` that the completion handler publishes tool results to —
   when Sarah creates a checkout link mid-sentence, the card pops as she
   says it. We can also *push* her: `avatar.speak_text` lets the page
   make her react to UI actions ("I see you opened the pricing card —
   want the link?").
4. **It's the skill's own golden pathway.** The decision tree in
   `liveavatar-integrate` routes "has their OWN LLM but no STT/TTS" to
   exactly this: FULL + Custom LLM.

What we must build (small):

- **AV-1 — SDK spike (sandbox, free):** backend route on `apps/sarah`
  that mints the session token (context = KB Section A system prompt;
  `opening_text` = the hardcoded opener) and returns it; page loads
  `@heygen/liveavatar-web-sdk`, attaches video to our own element,
  renders transcription events as chat lines through the existing
  dispatch. Sandbox avatar first (`is_sandbox: true`), then the real
  avatar you built.
- **AV-2 — the brain endpoint:** `POST /sarah/api/llm/chat/completions`,
  OpenAI-spec compliant (streaming), guarded by a dedicated bearer we
  mint and register in LiveAvatar's secret store (their `LLM_API_KEY`
  secret + `base_url` pointing at us). Internally: the owned runtime turn
  (history from the session index keyed by a conversation ref we embed,
  Gemma call, pricing guard, tools). Rate/spend caps in the S-3
  token-guard pattern.
- **AV-3 — the tool-effect side channel:** session-keyed SSE (or reuse of
  the LiveKit room's data channel is NOT available to our backend in FULL
  mode — SSE from apps/sarah is the clean seam) publishing typed card
  events; the shell subscribes and renders. Cards: intake recorded, quote
  (with rule refs), checkout link, human-handoff confirmation.
- **AV-4 — metering + receipts:** avatar sessions are credit-metered per
  minute — record session start/stop + minutes in the session index,
  enforce a session cap + daily cap on the mint route (same discipline as
  S-3), always call session stop on page close (orphaned sessions burn
  credits for 5 minutes).

## 3. Do we need LITE mode?

**Not now. Probably later.** The honest comparison:

| | FULL + Custom LLM (recommended v1) | LITE (later) |
|---|---|---|
| STT/TTS | Theirs (works today) | Ours to build/buy (Google Cloud STT/TTS on our GCP is the natural fit) |
| Brain | Ours (Gemma via our endpoint) | Ours (direct) |
| Turn manager | Theirs | Ours (their WebSocket protocol: `agent.speak` PCM chunks, `speak_end`, listening states, interrupt handling) |
| Tools | Ours, inside the endpoint | Ours, native |
| Latency control | Coarse | Fine-grained |
| Cost | 2 credits/min | 1 credit/min |
| Build size | Small (3 seams) | Medium (full audio pipeline + PCM 24KHz discipline; "audio format is king — wrong format = garbled with NO error") |

LITE is the sovereignty-ladder move: halves the per-minute cost and makes
the entire conversational pipeline ours (matching the SM-4 owned-runtime
trajectory). It becomes attractive when (a) avatar minutes are material
enough that 1 vs 2 credits matters, or (b) we want owned STT/TTS anyway
for other surfaces. The migration is contained: the brain endpoint from
AV-2 is reused as-is; only the audio pipeline and turn manager are new.
Decide after v1 usage data — file it as a lane, don't build it now.

## 4. Answers to the specific questions

- **"Can we programmatically connect the embed?"** No. The embed has no
  event surface at all. The avatar you built is reusable though — the
  same `avatar_id` (and the context you wrote) work in FULL mode sessions;
  nothing is lost, we just swap the iframe for the SDK.
- **"How do I pop up components?"** SDK events (every utterance arrives
  as a typed data-channel event) + our SSE tool-effect channel → the
  existing `/sarah` card dispatch. The avatar lives in our DOM, so
  components are ordinary page elements around/over the video.
- **"Can she have tools?"** Yes — by making LiveAvatar call our
  OpenAI-compatible endpoint, where the owned runtime executes the real
  tools server-side. Platform-native function calling isn't documented
  and isn't needed.
- **"Do I need LITE mode?"** No for v1; revisit for cost/pipeline
  ownership once minutes are real (§3).

## 5. Gotchas the skills flag (so we don't rediscover them)

- No `context_id` = silent avatar, **no error thrown** (the #1 failure).
- `/sessions/start` auth is `Bearer <session_token>`, not the API key.
- `X-API-KEY` never reaches the frontend; the page only ever sees
  `livekit_client_token`.
- FULL and LITE event systems are different protocols (`avatar.*` on
  LiveKit data channels vs `agent.*` on WebSocket) — never mix.
- 5-minute inactivity timeout; keep-alive every 2–3 minutes; always tear
  down sessions or credits burn until timeout.
- Sandbox avatar IDs differ between embed and session APIs.
- (LITE, for later) PCM 16-bit 24KHz mono or garbled video with no error;
  same `event_id` across all chunks of one utterance; interrupt requires
  stopping your own send loop too.

## 6. Security/law notes for our implementation

- LiveAvatar API key → GCP Secret Manager, backend only (`apps/sarah` env
  via the monolith's secret map).
- The AV-2 brain endpoint is effectively a public LLM proxy if left open —
  it ships bearer-guarded from day one (dedicated secret registered on
  LiveAvatar's side), with the S-3 discipline: origin checks where
  applicable, per-session caps, daily caps, spend alerts.
- The KB/system prompt lives in our repo (`docs/sarah/SARAH_KNOWLEDGE_BASE.md`)
  and is pushed into the LiveAvatar context via API at deploy time — not
  hand-pasted drift. Regenerate context on KB change.
- All hard rules stay enforced in code where they already are (pricing
  guard, checkout tracing, suppression) — the avatar is a renderer, not
  an authority. Receipts discipline extends to avatar minutes.
