# Owned avatar video pipeline — real-time lip-sync for Sarah from our own footage

Date: 2026-07-09
Status: design spec (owner-directed: "let's design it ourselves") — build lanes at §8
Trigger: LiveAvatar's custom-avatar policy allows first-person likenesses only,
so the owner's Midjourney-animated character footage cannot become a LiveAvatar
avatar. We design the pipeline ourselves. This is also the endgame the
LiveAvatar assessment already named: the LITE-mode path taken all the way —
zero per-minute platform credits, full pipeline sovereignty.
Related: `2026-07-09-liveavatar-integration-assessment.md` (§3 LITE),
epic #8598 (avatar surface), #8599/KHS-6 (pre-recorded audio path lands free
once we own the renderer). Research basis: mid-2026 open-source survey
(sources inline).

## 0. The one-paragraph design

Keep everything we already built — the `/sarah` Effect Native surface, the
avatar-session seam (mint/status/stop/events), the owned Gemma/Khala brain,
the SSE card bus — and replace only the vendor behind it: a GPU session
service that plays our catalogued **idle clips** when Sarah is silent and, when
her TTS speaks, synthesizes ONLY the mouth region of each frame
(**MuseTalk-class 256×256 inpainting**, pasted back into the untouched 720p
frame) in real time, streamed to the browser over WebRTC. Identity outside the
mouth is pixel-perfect **by construction** because every non-mouth pixel is our
own footage. TTS is **CosyVoice-class self-hosted streaming** (~150 ms first
packet, zero-shot voice clone). The conversation loop is the same
turn cycle LiveAvatar LITE documents (speak-chunks / interrupt / listening
states) — ours to run.

## 1. What we have (the asset inventory)

From `~/Downloads/sarah/README.md` — ten 8-second clips, one still:

- Uniform format: H.264 1280×720 @ 24 fps, yuv420p, 8.000 s each — uniform
  enough that stitching already copies streams without re-encode.
- A catalogued **expression state library**: best quiet idle (clip 6, −46.9 dB),
  idle/filler (clip 3), listening/animated with smile ending (clip 4), subtle
  smile transition (clip 5), strong talking candidates (clips 8, 0), plus two
  stylized UI-overlay shots (7, 9) usable as intro/outro flair.
- The character is **fully synthetic** (Midjourney-designed, animated) — no
  likeness/consent constraint anywhere in our pipeline, and a brand asset we
  own outright.

This inventory is exactly the input the architecture below wants: a fixed
framing, a consistent subject, and pre-graded emotional states.

## 2. Architecture

```text
                     ┌────────────────────────────────────────────────┐
 Sarah brain         │   Avatar Render Service (GPU, Cloud Run/GCE)   │
 (Khala gateway,     │                                                │
  llm-openai-compat) │  ┌──────────┐   PCM 24k   ┌─────────────────┐  │
   reply text ──────▶│  │ CosyVoice │────────────▶│ Frame scheduler │  │
                     │  │ streaming │             │  (turn manager) │  │
                     │  │ TTS+clone │             │                 │  │
                     │  └──────────┘              │ idle state:     │  │
                     │                            │  loop clips 6/3 │  │
                     │  ┌────────────────────┐    │ listening:      │  │
                     │  │ MuseTalk 1.5       │◀───│  clips 4/5      │  │
                     │  │ audio→mouth crop   │    │ speaking:       │  │
                     │  │ 256² inpaint       │───▶│  inpainted      │  │
                     │  │ paste-back to 720p │    │  frames         │  │
                     │  └────────────────────┘    └───────┬─────────┘  │
                     │                                    │ 24fps      │
                     │                            ┌───────▼─────────┐  │
                     │                            │ WebRTC egress    │  │
                     │                            │ (LiveKit/aiortc) │  │
                     └────────────────────────────┴───────┬──────────┘
                                                          │
                        /sarah page: same avatar-session seam
                        (mint → attach video → SSE cards)  ▼
                                                    <video> element
```

Key decisions and why:

1. **Mouth-region inpainting, not full-frame generation.** For fixed-framing
   footage with a curated state library, MuseTalk-class inpainting is
   structurally correct: it synthesizes a 256×256 lower-face crop per frame
   and composites it back, so hair/wardrobe/lighting/background are our
   pixels, identity drift is impossible outside the mouth, and GPU cost is one
   small U-Net pass per frame. Full-frame models (Ditto, SoulX-FlashHead)
   regenerate every pixel — they buy audio-driven head motion at the cost of
   identity shimmer and they *fight* our clip library instead of exploiting
   it. They're the v2 A/B, not the v1.
2. **The idle/speak state machine is the product feel.** Silent → loop clip 6/3
   (seamless because the README already verified clean joins); user speaking →
   listening clips 4/5 (nods/smile); Sarah speaking → the scheduler pulls
   frames from a talking-pose clip and MuseTalk re-lips them against the live
   TTS PCM; interrupt → crossfade back to listening. This mirrors what
   LiveTalking ships in production (idle-video + speak compositing) and what
   LiveAvatar LITE's own docs describe (`agent.start_listening` /
   `agent.speak` chunks / `agent.interrupt`).
3. **One preprocessing pass, then cheap forever.** MuseTalk's honest real-time
   numbers depend on precomputed reference embeddings per avatar — a one-time
   job over our ten clips (this is exactly why fixed footage wins: Ditto's
   paper measured MuseTalk at RTF 2.248 *including* preprocessing; LiveTalking
   measures 42 FPS on an RTX 3080Ti / 72 FPS on a 4090 *after* it, because
   production setups preprocess once).
4. **Same seam, swappable vendor.** The browser contract stays our
   `/sarah/api/avatar/*` mint/status/stop + SSE events + a WebRTC join. v1 can
   even keep LiveAvatar as a fallback renderer behind the same seam while the
   owned service matures (env-selected renderer).

## 3. Component selection (from the mid-2026 survey; licenses checked)

| Slot | v1 pick | License | Measured performance | Alternates / ladder |
|---|---|---|---|---|
| Lip sync | **MuseTalk 1.5** ([TMElyralab/MuseTalk](https://github.com/TMElyralab/MuseTalk)) | MIT, models commercial-OK | 42 FPS @ RTX 3080Ti, 72 FPS @ 4090 inside LiveTalking (256² crop → 720p paste-back) | v2: **Ditto** (Apache-2.0, TensorRT, 385 ms first-frame, adds head motion); **SoulX-FlashHead-Lite** (Apache-2.0, 96 FPS @ 4090, 3 streams @ 25 FPS — young); **LatentSync 1.6** offline-only for marketing renders (verify its LICENSE file — sources disagree Apache vs OpenRAIL++) |
| Pipeline skeleton | **LiveTalking pattern** ([lipku/livetalking](https://github.com/lipku/livetalking), Apache-2.0, 8.3k★, v2.0.4 06/2026) — adopt it directly or port its frame-scheduler/sync/idle-compositing design into an owned Bun/Effect+Python service | Apache-2.0 | Two years of production use; WebRTC+RTMP egress; pluggable lip-sync + TTS seams | [OpenAvatarChat](https://github.com/HumanAIGC-Engineering/OpenAvatarChat) as architecture reference (handler graph, ~2.2 s e2e); Duix/HeyGem rejected (offline toolkit + revenue-carve-out license) |
| TTS | **CosyVoice 2/3** ([FunAudioLLM/CosyVoice](https://github.com/FunAudioLLM/CosyVoice)) | Apache-2.0 | ~150 ms first packet, bidirectional streaming, 24 kHz, zero-shot clone from seconds of audio; already a LiveTalking plugin | **Chatterbox/Turbo** (MIT; vendor-claimed sub-200 ms — verify ourselves); interim managed: **Google Chirp 3 HD** `streaming_synthesize` (instant custom voice is allow-list gated); Kokoro as no-clone fallback. Disqualified for cloning: F5-TTS weights (CC-BY-NC), XTTS (CPML), Fish S1-mini (NC) |
| Voice | Clone from a curated Sarah read (we record/generate the reference audio we own) | — | — | The synthetic character means the voice is also fully ours to define |
| Egress | WebRTC — LiveKit self-hosted (we hold deep LiveKit reference expertise in `projects/livekit/`) or aiortc as LiveTalking ships | Apache-2.0 | — | The /sarah page already attaches remote WebRTC video (same UX as today) |

Rejected on license (do not revisit without legal): Sonic (non-commercial),
FLOAT (CC-BY-NC-ND), Hallo3 (CogVideoX conditional), LivePortrait's bundled
InsightFace models (research-only; MediaPipe swap required if ever used).

## 4. Latency budget (voice-to-voice target p50 ≤ 800 ms on the avatar lane)

| Stage | Budget | Basis |
|---|---|---|
| ASR (user speech → text) | 60–120 ms | production voice-agent practice |
| Brain first token (Khala gateway → Gemma) | 150–500 ms | our measured 0.17 s first byte + thinking variance |
| TTS first packet (CosyVoice) | ~150 ms | published streaming latency |
| **Video time-to-first-frame** | **150–300 ms** | MuseTalk per-frame (no chunking) fits; first inpainted frame follows first audio chunk |
| WebRTC transport | 30–80 ms | standard |

The mouth only needs to move when audio plays, and audio pacing is ours — the
scheduler holds a small (~200 ms) jitter buffer so lips never lead the sound.

## 5. GPU + cost

- v1: one **NVIDIA L4** (GCP g2-standard-4/8) per concurrent stream is
  realistic given the measured 3080Ti numbers (L4 ≈ same class for this
  workload). Spot/on-demand g2 ≈ $0.2–0.7/hr — versus LiveAvatar FULL at
  2 credits/min. Break-even is measured in *hours per day* of conversation.
- Scale-to-zero: the render service runs on Cloud Run GPU (or a GCE MIG at
  min 0) and cold-starts on session mint; idle loops cost nothing when nobody
  is talking to her.
- Preprocessing (one-time per clip set) and any LatentSync marketing renders
  run as batch jobs, not standing capacity.

## 6. What we reuse from tonight's stack (nothing is thrown away)

- The **avatar-session seam** (`mint/status/stop/events`) — the browser and
  EN surface don't change; a `renderer: liveavatar | owned` field on mint
  selects the backend during the transition.
- The **brain** — same Khala-gateway completions; the render service consumes
  the same streaming text the LiveAvatar custom-LLM hook consumes today.
- **KHS-6 semantic cache → true pre-recorded clips**: owning the renderer
  makes the owner's "pre-recorded audio to save on generation" trivial — a
  cache hit can play a pre-rendered audio+lip take with zero GPU inference
  (the exact capability FULL-mode LiveAvatar could not give us).
- **Turn persistence, isolation contracts, pricing guard** — all upstream of
  the renderer; untouched.

## 7. Honesty flags (from the research, kept visible)

- MuseTalk's "real-time" claim assumes preprocessed reference embeddings —
  true for us (fixed clips), but never quote the 30 fps figure without that
  caveat; Ditto's end-to-end accounting put unpreprocessed MuseTalk at
  RTF 2.248.
- Chatterbox latency and its ElevenLabs blind-test numbers are vendor-run;
  measure before relying on them.
- LatentSync's license is reported inconsistently (Apache-2.0 vs OpenRAIL++)
  — read the LICENSE in the repo at adoption time.
- Known MuseTalk failure modes: teeth smearing, occasional single-frame
  jitter, chin seam under rotation — mitigated by fixed framing, and clip
  selection can avoid strong head turns during speech.
- Quality ceiling honesty: 256² mouth crops at 720p are good, not
  LatentSync-512² good; the v2 ladder exists for a reason.

## 8. Build lanes (OAV-*)

- **OAV-1 — offline proof (no streaming):** preprocess the 10 clips; render
  one Sarah reply (CosyVoice-cloned audio + MuseTalk over a talking clip) to
  MP4; side-by-side vs LiveAvatar for a quality go/no-go. One GPU day.
- **OAV-2 — the render service:** LiveTalking (or a ported scheduler) on one
  L4: idle/listen/speak state machine over the clip library, MuseTalk
  backend, WebRTC egress, `speak/interrupt/listening` control API mirroring
  the LITE cycle.
- **OAV-3 — TTS ownership:** CosyVoice deployment + Sarah voice clone from
  owned reference audio; Chirp 3 HD as the managed interim behind the same
  PCM-stream seam.
- **OAV-4 — seam integration:** `renderer: owned` on the avatar mint; the
  render service consumes the brain's streaming text; SSE cards unchanged;
  A/B against LiveAvatar on staging.
- **OAV-5 — pre-rendered takes:** KHS-6 cache hits map to pre-rendered
  audio+video takes (zero-inference answers); opener pre-rendered.
- **OAV-6 — quality ladder:** Ditto A/B (head motion), SoulX-FlashHead-Lite
  trial, LatentSync offline renders for marketing, fine-tune evaluation on
  our footage.

Exit for the program: a full Sarah conversation on `/sarah` rendered entirely
by owned infrastructure — no LiveAvatar session, no per-minute platform
credits, first frame ≤ 300 ms after first audio — with the LiveAvatar seam
retained as a fallback until the owner retires it.
