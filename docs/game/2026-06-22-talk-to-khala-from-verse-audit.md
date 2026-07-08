# Talk to Khala from a Verse textbox — audit & implementation status

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


*2026-06-22. What's needed so a user can type a prompt into a textbox **in the
Verse world** and talk to Khala — streamed response, with the in-world
crackling-energy effect firing. Most of the hard parts already exist; the gap is a
small amount of UI + one render wiring.*

**2026-06-23 status:** implemented on current `openagents` main for the desktop
Verse MVP. The remaining web/multi-user identity and credit flow is still future
work.

## Goal

Type in a textbox inside the 3D Verse → Khala answers (streamed) → the answer shows
in-world (bubble/panel) and the crackling-energy effect fires from the Khala nexus.

## What already exists (reuse, don't rebuild)

- **The Khala call path is production-ready** (`apps/autopilot-desktop`): the
  `khalaTurn` RPC (`src/shared/rpc.ts`) → `src/bun/khala-turn.ts` does a **streaming**
  `POST /v1/chat/completions` (SSE, the 524-safe pass-through), pushes live deltas
  back via `khalaToken` (`turnId`-correlated), and returns the terminal `openagents`
  receipt. Parsing in `src/shared/khala-cockpit.ts`.
- **Auth/credits resolved for the desktop owner.** The Bun host resolves the agent
  token (`resolveShellAgentToken`: `OPENAGENTS_SHELL_AGENT_TOKEN` →
  `OPENAGENTS_AGENT_TOKEN` → persisted `agent-credential.json`); the token never
  crosses to the webview. That token already carries a prod credit balance (khala is
  paid; 402 otherwise). So a desktop MVP is funded today.
- **The in-world render is mostly done (M5, #6013).** Inference receipts already
  project gateway→public-timeline→world→desktop scene as a crackling arc / gateway
  portal (`chat-world-visualization.ts`, `chat-world-multiplayer.ts`), evidence-bound.
- **Local-chat contract exists** (`packages/world-contract`): `send_local_message`,
  `WorldLocalChatMessageRow`, chat bubbles above avatars — a place for the prompt +
  response to live in-world.

## Original gaps and current status

1. **In-world textbox: implemented.** `src/ui/verse-khala-input.ts` renders a
   Foldkit HUD surface over the Verse scene with a one-line input, live response
   bubble, receipt line, and honest status text.
2. **Textbox → `khalaTurn`: implemented.** `SubmittedVerseKhala` schedules
   `RunVerseKhalaTurn({prompt, turnId})`, which calls the existing host-side
   `khalaTurn` RPC with `model:"openagents/khala-mini"`.
3. **Immediate in-world effect from the response: implemented.**
   `withVerseKhalaEffectLayer` overlays a local, receipt-bound crackling arc from
   the Khala nexus to the local avatar as soon as a real receipt lands. The later
   public-activity-timeline projection remains separate for other viewers.
4. **Live tokens in-world: implemented.** Bun pushes `khalaToken` deltas
   correlated by `turnId`; `GotVerseKhalaToken` appends active-turn deltas into
   the in-world response bubble and ignores stale turn ids.

## Identity / credits

- **Desktop MVP: solved** — the owner's agent token + balance is reused; no per-user
  flow needed for a first "talk to Khala from Verse."
- **Web Verse: future** — there is **no web Verse client today** (Verse is desktop-
  only; the web app renders read-only Tassadar proof snapshots, not live play). A web
  user would need an authenticated identity + their own funded balance (the LN→credit
  bridge / team-credit pool) — out of scope for the first demo.

## Implemented desktop MVP

The implemented path is:

1. `verseKhalaInputOverlay(model)` is mounted in the Verse pane in
   `src/ui/view.ts`.
2. `SubmittedVerseKhala` records an active turn id and emits
   `RunVerseKhalaTurn`.
3. `RunVerseKhalaTurn` calls the Bun host `khalaTurn` RPC; the host resolves the
   owner agent token without exposing it to the webview and streams deltas through
   `khalaToken`.
4. `GotVerseKhalaToken` appends active-turn deltas to the in-world Khala bubble.
5. `RespondedVerseKhala` lands the terminal text and public-safe Khala receipt.
6. `verseSceneVisualization` applies `withVerseKhalaEffectLayer`, which renders a
   local crackling arc only when the receipt is live and carries a receipt ref.
   No receipt means no effect.

**Still not blocking for desktop:** identity/credits, because the owner desktop
token path is reused. **Still future:** web Verse identity, per-user credits, and
multi-user web interaction.

> Quality note: per `docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md`,
> a code-generating Khala turn from Verse should still be gated by the executed
> acceptance suite before its output is shown as "verified" — talking to Khala is the
> easy part; trusting its output is the QC part.

## One line

The call path, auth, streaming, HUD input, response bubble, and receipt-bound
local render effect now exist for the desktop Verse MVP. A web/multi-user Verse
remains the larger later piece (per-user identity + credits).
