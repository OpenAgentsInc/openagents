# Sarah Voice A/B & Episode 298–Style Install Walkthrough Receipt (2026-11-24)

## 1. Owner Playback Verdicts (SQ-1)

1. **Lip-sync engine comparison on canonical real `Sarah.png` (`1456×816`):**
   - **`kling-v2-standard-avatar` (`1280×720 @ 30 fps`, `267 s`):** **REJECTED** — distorts Sarah's mouth geometry during speech (and attenuates muxed audio by `-3.0 dB` without remux).
   - **`p-video-avatar` (Pruna, `1920×1088 @ 24 fps`, `54 s`):** **ACCEPTED** — holds Sarah's real likeness, mouth shape, and lighting cleanly; earlier plain `en-US-Chirp3-HD-Leda` input sounded flat/boring because Chirp 3 HD has no style/acting prompt.

2. **3-way Voice Expressiveness A/B on settled real Sarah still (`1920×1088`) → Pruna `p-video-avatar`:**
   - **Arm A (`gemini-2.5-pro-tts`, voice `Leda` + acting prompt, trimmed `6.71 s`):** **GOOD** (audio judge `9/10`).
   - **Arm B (Pruna built-in `voice_prompt` mode, `Leda (Female)`, `8.04 s`):** **REJECTED** — opened with an exaggerated upward lilt.
   - **Arm C (`gemini-2.5-pro-tts`, voice `Sulafat` + lively conversational prompt + subtle `-26 dB` room bed, trimmed `6.71 s`):** **WINNER** (audio judge `9/10`, owner preferred).
   - *Note:* ElevenLabs was probed and returned `401 payment_issue` (incomplete invoice on account); Google `gemini-2.5-pro-tts` on `openagentsgemini` succeeded immediately with `HTTP 200`.

## 2. Full 60-Second Episode 298–Style Install Walkthrough

Produced a complete `60.04 s`, `1920×1080 @ 30 fps` walkthrough explaining how to install **OpenAgents Desktop** (macOS universal DMG on `openagents.com`; Linux/Windows one-line source build from `github.com/OpenAgentsInc/openagents`), **OpenAgents iOS** (TestFlight `testflight.apple.com/join/dvQdns5B`), **Tailscale direct WireGuard pairing** (port `47109`, zero cloud relays), and **Antigravity (`Gemini 4 Neon`) + local `Inkling 2 Mini`**.

### Layout (1:1 with `episode-298-cut6.mp4`)
- **Background:** Animated dark perspective grid (`1920×1080 @ 30 fps`).
- **Left stage (`x=53, y=45, 1286×870`, `r=22` rounded stroke + lower-left monospace pill):**
  - `0.00–7.80 s`: `openagents.com/app-preview.jpg` (native macOS + iOS workspace)
  - `7.80–14.40 s`: live `openagents.com` capture (`Download for Mac` + `Join the TestFlight`)
  - `14.40–18.80 s`: `openagents.com` expanded Linux & Windows `git clone` build commands
  - `18.80–21.30 s`: `github.com/OpenAgentsInc/openagents`
  - `21.30–29.30 s`: 3-up Episode 298 iPhone simulator footage (`05`, `03`, `04`)
  - `29.30–38.30 s`: 3-up Episode 298 Tailscale pairing footage (`01`, `10`, `01`)
  - `38.30–45.00 s`: 3-up Episode 298 Antigravity / `Gemini 4 Neon` picker + Diff + Hub (`02`, `06`, `08`)
  - `45.00–51.40 s`: `inkling-psionic-vs-llamacpp.mp4` (`Inkling 2 Mini` local on Psionic Metal)
  - `51.40–56.40 s`: 3-up Episode 298 closing showcase (`07`, `09`, `01`)
  - `56.40–60.04 s`: `openagents.com` closing call-to-action
- **Right vertical-slice Sarah overlay (`x=1380, y=134, 486×688`, `r=22`):**
  - Continuous `60.04 s` Pruna `p-video-avatar` one-take cropped vertically (`768×1088` at `x=586` → `486×688 @ 30 fps`) instead of a horizontal 16:9 box.
  - Mouth-region frame-difference check across the full 60 s confirmed zero late-clip freeze:
    - `seed=4242`: early `7.85` / mid `6.02` / late `6.87` (settled eyes-open closing frame)
    - `seed=7777`: early `7.34` / mid `7.15` / late `6.97`
- **Audio & Captions:**
  - `gemini-2.5-pro-tts` (`Sulafat`, grounded opening prompt, zero lilt) + subtle `-27 dB` crossfaded ambient room bed.
  - Final muxed MP4 measured **`-16.6 LUFS`**, **`LRA 2.9 LU`**, **`-3.1 dBFS` peak** (`ebur128`), and passed full 60 s post-mux Google STT V2 (`chirp_3`) round-trip.
  - Bottom synced captions placed under the left stage from exact Google STT V2 word timestamps.

### Local Media Outputs (Not Committed)
- `~/.openagents/sarah-mvp/2026-11-24-install-video/openagents-install-sarah-ep298.mp4` (`seed=4242`, primary)
- `~/.openagents/sarah-mvp/2026-11-24-install-video/openagents-install-sarah-ep298-seed7777.mp4` (`seed=7777`, alternate)
- `~/.openagents/sarah-mvp/2026-11-24-install-video/qa/final-8frame-grid.jpg`

## 3. CLI Upgrades Landed in `coax/web/sarah.coax`
- Switched `sarah synth` and `sarah mvp` default TTS to `gemini-2.5-pro-tts` (`--voice Sulafat`, `--tts-model gemini-2.5-pro-tts`, grounded `--prompt`; legacy `--tts-model chirp3-hd` preserved).
- Added leading-silence trim and automatic post-render Pruna trailing-pad trim + clean 48 kHz WAV remux (`--no-remux` opt-out) in `sarah lipsync`.
- Switched `sarah mvp` default `--models` to `p-video-avatar`.
- Added `coax/web/tool.sh sarah slice --in MP4 --out MP4 [--w 486] [--h 688] [--x 586]` to crop `1920×1088` Pruna renders into Episode 298–style vertical portrait overlays.
