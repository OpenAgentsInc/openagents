# docs/sarah

> **CURRENT — OWNER ORCHESTRATOR REBOOT (2026-07-18).** Sarah is active as
> `principal.sarah`, the authenticated owner's persistent orchestrator inside
> supported OpenAgents clients. Her first production surface is the ordinary
> OpenAgents mobile conversation UI on one stable owner-private Khala Sync
> thread, backed by cited business-context projections and hosted Khala.
> Normative current artifacts are
> `specs/openagents/sarah-owner-orchestrator.product-spec.md`, its companion
> AssuranceSpec, `AUTHORITY.md` revision 3,
> `docs/authority/SARAH_AUTHORITY.md`, and the Sol reboot accepted plan.
>
> The former public web page, `/sarah/api/*`, `apps/sarah`, prospect CRM,
> avatar/video/GPU stack, and separate client state remain removed. `GET
/sarah` stays a 404 tombstone. Everything below this banner is historical
> source material, not current architecture.

Sarah is OpenAgents' AI sales employee — and under the Sarah-first product
thesis, the front door to the whole product: a disclosed AI identity with a
live avatar, durable per-prospect memory, typed tool and pricing authority,
and (per the current roadmap) the conversational surface through which fleet
coding, standing employees, and the company brain arrive. She serves at
**https://openagents.com/sarah**, implemented in `apps/sarah` (Bun + Effect,
zero-React Effect Native UI), with the openagents.com API remaining the
system of record for CRM, credits, checkout, and receipts — Sarah is a
client, never a second authority.

Her stack in one pass: browser speech recognition → the speak bridge →
her brain on the persona-neutral Khala gateway lane (exact receipts, caps,
typed fallback — #8600) → sentence-streamed TTS (hydralisk-tts: Chirp 3 HD +
CosyVoice2 clone) → the owned realtime avatar (hydralisk-avatar: MuseTalk
lip-sync over owned footage, WebRTC) on our own GPU node, with the
Blueprint Map canvas (#8626) rendering what she knows and is learning, live.

Strategy lives in `docs/fable/` (notably the Sarah-first product thesis) and
sequencing in `docs/sol/MASTER_ROADMAP.md`. This folder is the Sarah-owned
implementation, quality, and contract record.

## Start here

- **`SARAH_CONTRACTS.md`** — the behavior-contract registry rendering: what
  Sarah may never do (cross-prospect leakage, improvised pricing,
  owner-ungated learning, silent sessions), each with owner statements
  verbatim and test oracles. Read before changing any Sarah surface.
- **`SARAH_KNOWLEDGE_BASE.md`** — what Sarah knows, generated from her typed
  Blueprint (regenerate via `apps/sarah/scripts/render-kb-from-blueprint.ts`,
  never hand-edit).
- **`2026-07-09-sarah-quality-next-steps-assessment.md`** — the quality
  program: the five-axis quality bar and the SQ-1..8 issue map (#8618–#8625).

## The avatar / voice program (OAV)

- **`2026-07-09-owned-avatar-video-pipeline-spec.md`** — the owned pipeline
  spec (OAV lanes, §9 is the binding enhancement/quality-tier policy: e.g.
  full-strength per-frame GFPGAN is banned as a default enhancer).
- **`2026-07-09-oav-quality-strategy.md`** — the living quality record:
  measured take history, root causes for the "sharper but less human"
  failure, license verdicts (MuseTalk MIT, GFPGAN/LatentSync/CosyVoice
  Apache-2.0. CodeFormer/KEEP/PGTFormer NON-commercial — never ship), and
  the round-3 short-clip program. Companion paper triage: **`research.md`**.
  local PDFs in workspace `projects/papers/`.
- **`2026-07-09-oav1-offline-proof-receipt.md`** /
  **`2026-07-09-oav2-render-service-closeout.md`** — the offline proof and
  the live render-service closeout (the real-session simulator gate lives in
  hydralisk and runs before render-service deploys).
- **`2026-07-09-pipecat-voice-infra-audit.md`** — verdict: do not adopt as
  foundation. Extract owned-ASR/VAD/turn-taking patterns (BSD-2, Daily
  optional).
- **`2026-07-09-liveavatar-integration-assessment.md`** — the vendor
  (HeyGen LiveAvatar) seam, retained as the fallback flag behind the owned
  renderer.

## Quality gates and ops

- **`QUALITY_SCOREBOARD.md`** + **`scoreboards/`** — one canonical,
  playback-first scoreboard per media take (`sarah-take-scoreboard.v1`).
  the cultural law: nothing advances on stills — owner playback and
  per-segment STT gate every take.
- **`GPU_MEDIA_RUN_CLOSEOUT.md`** — the GPU run closeout checklist
  (artifact-existence monitors, host disposition, cost, no secrets).
- **`2026-07-09-sq4-hardening-receipt.md`** — the realtime hardening record
  (session eviction/liveness, keepalive races, the deploy-time
  synthetic-prospect smoke).

## Brain, data, and surface

- **`2026-07-09-khs1-khala-gateway-migration.md`** — Sarah's inference
  migration to the Khala gateway, the persona-bleed incident, and the
  persona-neutral resolution (#8600).
- **`2026-07-09-blueprint-map-surface-audit.md`** — the Blueprint Map
  program (#8626, BM-1..5): the live graph of what she knows/is learning,
  the split desktop layout, and the arbiter-effect/EN GraphFigure lineage.
- **`EN-GAPS.md`** — Sarah's Effect Native catalog demand register (gaps go
  upstream, never local one-offs).
- **`MIGRATION.md`** + **`historical/`** — the consolidation from the
  private Vercel-era Sarah repo into this monorepo (historical reference
  only).

## Operating rules of thumb

1. Contracts outrank convenience: a stated owner expectation lands in the
   registry with an oracle in the same change.
2. Playback and live proofs outrank stills, fixtures, and metrics — takes
   and builds ship only with pixel/motion evidence.
3. Sarah's authority is interpretive: money, email, learning promotion, and
   public claims cross typed, owner-scoped gates she cannot bypass.
4. The GPU host (`sarah-avatar-gpu-1`) is production — coordinate via the
   closeout checklist before touching it. Offline experiments never share
   the serving slot with a live session.
