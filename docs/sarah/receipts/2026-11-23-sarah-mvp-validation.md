# Sarah MVP Live Validation Receipt (2026-11-23)

- **Schema:** `openagents.sarah.mvp_validation.v1`
- **Date:** 2026-11-23
- **Code validated:**
  - `metaverse/voice-host/speech.c`, `client.c`, `build.sh`, `test.sh` (live Sarah voice on `en-US-Chirp3-HD-Leda` + Vertex Gemini)
  - `coax/web/sarah-lib.coax` (97/97 Coax checks passing) + `coax/web/sarah.coax` (`coax/web/tool.sh sarah`)
  - `docs/sarah/sarah-direction.json` (`openagents.sarah.avatar_direction.v2`)
- **Local media output (not committed to git):** `~/.openagents/sarah-mvp/2026-11-23/`
- **Owner playback verdict:** **PENDING** (SQ-1 law: `advance: false` on both video takes until the owner plays the MP4s in motion)

---

## 1. Live Voice Validation (`metaverse/voice-host`)

Command:

```sh
metaverse/voice-host/test.sh
```

Pre-existing build blockers found and fixed in `metaverse/voice-host/build.sh`:
1. `pay/voice-meter.coax` was missing from the build loop after the DDD.9 move (`VoiceMeter.h` not found).
2. Linking both `libpsionic-parakeet.a` and `libpsionic-whisper.a` on macOS collided on 6 shared `#export_c` symbols from `psionic/lib/{ops,tensor}.coax`; fixed by scoping Whisper to its 3 C API symbols (`_psionic_stt_{open,text,gpu}`).

Live measured result (`--route sarah --profile sarah`, `en-US-Chirp3-HD-Leda` + Google Chirp 3 STT + Vertex `gemini-2.5-flash`):

| Metric | Value | Notes |
| --- | --- | --- |
| Offline suite (`local+echo`, `sarah-offline`, barge-in, 2-session concurrency) | **PASS** | `sarah-offline` WAV written |
| Local Psionic Whisper suite (`psionic-hello`, `psionic-name`, `psionic-barge`) | **PASS** | `0–2 ms` from `TURN_END` |
| Live Sarah (`live-sarah`) STT recognition | **`152 ms`** | Google Cloud STT V2 `chirp_3` |
| Live Sarah Vertex Gemini first sentence | **`714 ms`** | `gemini-2.5-flash` (`sarah` route) |
| Live Sarah Chirp 3 HD `Leda` first audio (cold REST) | **`600 ms`** | `6 ms` when SHA-256 WAV cached |
| Live Sarah total `TURN_END` → first audible speech | **`1,466 ms`** | **Core challenge confirmed:** REST `text:synthesize` adds ~600 ms vs. bidirectional streaming Chirp 3 HD (`163–198 ms`) |
| Live barge-in (`live-gemini-barge`) | **PASS** | Interrupt + epoch bump verified |

---

## 2. End-to-End Media & Lip-Sync MVP (`coax/web/tool.sh sarah mvp`)

Command:

```sh
coax/web/tool.sh sarah mvp \
  --text "Hello! I'm Sarah. Our voice and lip sync pipeline is back online. Shall we begin?" \
  --out "$HOME/.openagents/sarah-mvp/2026-11-23" \
  --models "p-video-avatar,kling-v2-standard-avatar"
```

### 2.1 Portrait Generation (Google Vertex AI)

- **Core challenge found:** Legacy Vertex Imagen `:predict` endpoints (`imagen-3.0-*`, `imagen-4.0-*`) returned `404` in project `openagentsgemini`. **`gemini-2.5-flash-image`** (`:generateContent`, `responseModalities: ["IMAGE"]`, `aspectRatio: "16:9"`) is live and authenticated.
- **Safety-filter trap found:** Including `"no watermark"` in a Vertex image prompt triggered `SAFETY: The prompt is blocked due to requesting to remove watermarks`. Removed from `SR_DEFAULT_PORTRAIT_PROMPT` and `sarah-direction.json`.
- **Result:** `1,249,310` bytes 16:9 PNG generated in **`6,267 ms`** (`~/.openagents/sarah-mvp/2026-11-23/sarah-portrait.png`), matching historical Sarah's warm-amber / cool-cyan lighting, navy-black cyan-piped collar, and composed closed-lip neutral gaze.

### 2.2 Audio-First Master Pipeline (Google Chirp 3 HD + Coax `measure.coax` + Google STT V2 + Vertex Gemini Judge)

| Stage | Result | Measurement |
| --- | --- | --- |
| Google Cloud TTS Chirp 3 HD (`en-US-Chirp3-HD-Leda`, 48 kHz `LINEAR16`) | **PASS** | `1,540 ms` wall for `7,316 ms` speech (`RTF 0.21`) |
| Coax `measure.coax` EBU R128 loudness normalization | **PASS** | Raw `-23.7 LUFS` → Normalized **`-16.6 LUFS`**, peak **`-3.0 dBFS`** |
| Google Cloud STT V2 (`chirp_3`) verbatim gate | **PASS** | `1,263 ms` — `"Hello, I'm Sarah. Our voice and lip-sync pipeline is back online. Shall we begin?"` |
| Vertex Gemini (`gemini-2.5-flash`) multimodal audio judge | **8/10** | `naturalness: 8`, `warmth: 7`, `confidence: 9`, `overall: 8` (`11,321 ms`) — *"Excellent clarity and pacing. Minor, subtle AI cadence in longer phrases."* |

### 2.3 Pure-Coax GCS V4 Signed URLs + Audio-Driven Lip-Sync A/B (Segmind Async V2)

Both lip-sync models received the **exact same portrait PNG** and **exact same locked Google Chirp 3 HD `Leda` 24 kHz WAV**, uploaded to `gs://openagentsgemini-sarah-export/sarah-mvp/...` and signed locally for 6 hours via `coax/web/sarah-lib.coax` (`coax/lib/crypto/sha256-kernel.coax`) + `openssl dgst -sha256 -sign` (zero Node/npm/pyOpenSSL dependencies).

| Metric | Arm A: `p-video-avatar` (Pruna) | Arm B: `kling-v2-standard-avatar` (Kling V2) |
| --- | --- | --- |
| Request ID | `3555f9802289fccdd5a4593589c65aaa` | `e89752a29f75d10a60da4e1556d5cb85` |
| Render wall time | **`47 s`** (~5× faster) | `236 s` |
| Resolution & FPS | **`1920×1088 @ 24.0 fps`** (3.4 MB) | `1264×720 @ 30.0 fps` (2.7 MB) |
| Video / Audio duration | `8,041 ms` / `7,316 ms` (**`725 ms` trailing video pad**) | `7,466 ms` / `7,337 ms` (**`129 ms` delta — 5.6× tighter**) |
| Post-mux audio loudness | **`-16.6 LUFS` / `-3.0 dBFS`** (preserved input level) | **`-19.6 LUFS` / `-6.0 dBFS`** (re-encoded 3 dB quieter — needs full-band WAV remux!) |
| Post-mux Google STT V2 gate | **PASS** (verbatim) | **PASS** (verbatim) |
| Visual frame & 6-frame strip inspection | Crisp 1080p, lighting & collar faithful; **drifts toward a brighter/toothier wide smile** mid-speech despite `negative_prompt`. | **Closer to composed, restrained direction** (minimal upper teeth, natural lip articulation, smooth 30 fps 6-frame progression, zero boil); lower `720p` resolution on `standard`. |
| Owner playback verdict (SQ-1) | **PENDING** (`~/.openagents/sarah-mvp/2026-11-23/sarah-p-video-avatar.mp4`) | **PENDING** (`~/.openagents/sarah-mvp/2026-11-23/sarah-kling-v2-standard-avatar.mp4`) |
| Scoreboard artifact | [`mvp-p-video-avatar-20261123.md`](../scoreboards/mvp-p-video-avatar-20261123.md) | [`mvp-kling-v2-standard-avatar-20261123.md`](../scoreboards/mvp-kling-v2-standard-avatar-20261123.md) |

---

## 3. Core Challenges Validated & Immediate Next Steps

1. **Audio-first decoupling works cleanly:** Driving both Pruna and Kling from the **same locked Google Chirp 3 HD `Leda` WAV** eliminates video-vendor TTS lock-in, guarantees identical pronunciation/voice across live calls and produced clips, and gates bad audio before spending video credits.
2. **Remux required after lip-sync:** Kling attenuated audio by `-3.0 dB` (`-16.6` → `-19.6 LUFS`) and Pruna padded `725 ms` of trailing video. Next step: add an automatic post-render `ffmpeg` remux step in `coax/web/sarah.coax` that swaps the compressed AAC back to the full-band 48 kHz `-16 LUFS` master WAV and trims dead video tail.
3. **Expression control vs. resolution/speed tradeoff:** Pruna is fast (`47 s`) and crisp (`1080p`) but smiles too brightly; Kling obeys composed expression better at `30 fps` with tighter A/V sync (`129 ms`), but takes `236 s` and outputs `720p` on `standard`. Next A/B: `kling-v2-pro-avatar` (1080p) and `veed-fabric-1.0` once the owner watches these two MP4s.
4. **Live voice latency gap confirmed:** Cold REST Chirp 3 HD `Leda` took `600 ms` (`1,466 ms` total `TURN_END → first audio`) in `metaverse/voice-host`. Upgrading `speech_synthesize` in `metaverse/voice-host/speech.c` to streaming Chirp 3 HD recovers the proven `163–198 ms` TTFA.
5. **Original Midjourney assets not in GCS:** `gs://openagentsgemini-oa-artifacts` returned `404` (only Sep 4 SQL dumps remain in `openagentsgemini-sarah-export`). `coax/web/sarah.coax mvp --image <PATH>` lets the owner drop their local original portrait straight into this same pipeline without regenerating.
