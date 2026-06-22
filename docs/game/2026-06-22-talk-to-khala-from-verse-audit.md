# Talk to Khala from a Verse textbox — audit & minimal path

*2026-06-22. What's needed so a user can type a prompt into a textbox **in the
Verse world** and talk to Khala — streamed response, with the in-world
crackling-energy effect firing. Most of the hard parts already exist; the gap is a
small amount of UI + one render wiring.*

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

## What's missing (the actual gaps)

1. **An in-world textbox.** There is **no text input inside the rendered 3D world**
   today — the desktop chat is a sidebar pane; the Verse plays behind it. Need a
   Foldkit input surface anchored in/over the scene (HUD panel or a bubble at the
   avatar).
2. **Textbox → `khalaTurn`.** Wire submit → `khalaTurn({prompt, model, turnId})`,
   reusing the existing RPC + token resolution. (Trivial — the RPC exists.)
3. **Immediate in-world effect from the response.** Today the crackling effect only
   appears when the public-activity-timeline poll picks up the receipt (~5–10s later).
   For a responsive feel, drive a **local** scene effect directly from the
   `khalaTurn` receipt (a Khala-response node + crackling arc near the avatar),
   mirroring the existing inference-layer mapper. This is the main new wiring.
4. **Live tokens in-world.** `khalaToken` deltas already stream to the app; no 3D
   consumer renders them. Render into a response bubble/panel that appends live.

## Identity / credits

- **Desktop MVP: solved** — the owner's agent token + balance is reused; no per-user
  flow needed for a first "talk to Khala from Verse."
- **Web Verse: future** — there is **no web Verse client today** (Verse is desktop-
  only; the web app renders read-only Tassadar proof snapshots, not live play). A web
  user would need an authenticated identity + their own funded balance (the LN→credit
  bridge / team-credit pool) — out of scope for the first demo.

## Minimal path (desktop MVP)

1. Add a Foldkit **in-world textbox** (HUD/bubble) — `src/ui/verse-khala-input.ts`,
   mounted in `src/ui/view.ts`.
2. On submit → `khalaTurn({prompt, model:"openagents/khala-mini", turnId})`.
3. Render streamed `khalaToken` deltas into an in-world response bubble/panel.
4. On receipt, drive a **local** crackling-arc effect from the Khala nexus to the
   avatar (reuse the M5 inference-layer mapper + `three-effect` `createCracklingArc`),
   keyed to the receipt ref (evidence-bound). The public-timeline projection still
   fires too, for other viewers.

**Hardest gap:** #3 (immediate local effect from the receipt) — the render
infrastructure exists, the wiring doesn't. Everything else is small. **Not blocking
for desktop:** identity/credits (owner token works).

> Quality note: per `docs/inference/2026-06-22-verified-work-must-execute-the-artifact.md`,
> a code-generating Khala turn from Verse should still be gated by the executed
> acceptance suite before its output is shown as "verified" — talking to Khala is the
> easy part; trusting its output is the QC part.

## One line

The call path, auth, streaming, and render all exist; "talk to Khala from Verse" is a
textbox + one render wiring on the desktop app — a days-not-weeks MVP. A web/multi-user
Verse is the larger, later piece (per-user identity + credits).
