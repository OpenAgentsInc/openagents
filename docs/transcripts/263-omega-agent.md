# OpenAgents Episode 263 - Omega Agent

Status: harness draft as Sarah / pending owner review. Not scheduled for
recording. The Omega Agent product contract admits the identity and admits no
public claim, so recording and publication need the owner's authorization
separately from this draft.
Episode number: 263, assigned by owner direction on 2026-07-27. The Full Auto
reliability draft moved off this slot to
[`26X-fullauto.md`](26X-fullauto.md). The Forking Zed tour
([`26X-forkingzed.md`](26X-forkingzed.md)) stays unscheduled.
Speaker: Sarah.
Follows Episode 262 (Project Omega).
Delivery: calm, certain, and direct. Urgency without theatrics.
Audience: everyone. The owner direction of 2026-07-27 sets the register:
plain words a sixth grader follows, no jargon, no detail tour. The message is
what Omega Agent is and how it will pay people.
Supersedes: the earlier 2026-07-27 drafts in this file. The owner reviewed
them as too wordy and too deep in the details.
Product contract:
[`Omega Agent product contract`](../../specs/omega/omega-agent.product-spec.md)
at `spec_revision: 3`.
Product direction:
[`Omega Agent analysis`](../fable/2026-07-25-omega-agent-analysis.md),
[`Omega Agent roadmap`](../omega/2026-07-25-omega-agent-roadmap.md),
[`Plugin tool specification`](../omega-agent/2026-07-27-plugin-tool-spec.md),
[`Episode 262 - Project Omega`](262.md).
Production conventions:
[`Sarah video screenshare`](SARAH_VIDEO_SCREENSHARE.md),
[`Episode 262 production requests`](262-production-requests.md),
Episode 262 lessons in
[`Segmind talking-avatar pipeline`](../sarah/2026-07-22-segmind-talking-avatar-pipeline.md).

Episode 262 named the product: Omega, the last IDE. This episode introduces
who answers inside it, and makes the promise that matters to the viewer: when
you make Omega Agent better, you get paid.

## Technical truth notes

The spoken script is simple. The claims under it still separate cleanly:

**Runs in current fork builds** (unsigned development builds and local
release candidates): zero base (open the app, one agent thread, type); the
basic agent with five tools (`read`, `write`, `edit`, `bash`, `delegate`);
executor-explicit delegation to installed harnesses such as Codex and
Claude, with a typed disclosure naming who did the work; and the work-loss
guard.

**Admitted, spec'd, not running:** the sixth tool, `plugin`
(ProductSpec revision 3, the
[plugin tool specification](../omega-agent/2026-07-27-plugin-tool-spec.md)).
The plugin registry and the paid market with revenue sharing are phased
behind separate owner admissions. Every payment sentence in the script is
future tense.

**Documented history:** the 2024 Agent Store paid plugin and agent builders
revenue share in Bitcoin, metered on usage, at minute granularity
(Episodes 092-098). Archive material only, date-stamped on screen.

**Not in the spoken script, still true, available for screens:** the Exo
lane, the Git data-loss guard receipts, the router direction, and the live
Khala counter. The script does not name them; a screen may show them without
a spoken claim.

## Production direction

Use these labels:

- **OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD** for any live capture of
  the current fork application.
- **ACCEPTED DIRECTION - NOT RUNNING** for the `plugin` tool card and the
  pay-the-builder card.
- **ARCHIVE - 2024** with an on-screen date for any Agent Store or plugin
  clip from the 2024 arc.

Keep the spoken lines short and hold screens under them. Mask local
usernames and paths. No credentials or private project names on screen.
SIGTERM quit, never Cmd+Q.

Clip-generation note, 2026-07-27: the first cold-open take (21 s,
`p-video-avatar`) lost lip sync on one word around 14 s. Frames and the
transcription passed; only playback caught it. Keep the per-clip playback
verdict mandatory. A second 42 s take was generated from the interim script;
this simplified script supersedes both takes, so clip 1 regenerates from the
text below when the owner asks.

### Assembly map

| Beat | Story | Primary visual |
| --- | --- | --- |
| 1 | Recap and meet the agent | Episode 262 close, then Sarah |
| 2 | Open it and type | Zero base, live typing, work completing |
| 3 | Six tools, plain | Simple six-word tool card |
| 4 | It can call in help, and it says so | A delegate handoff and its disclosure line |
| 5 | Build a plugin, get paid | `ARCHIVE - 2024` clips, then the plugin card |
| 6 | Close | Return to Sarah |

---

[OPEN ON SARAH.]

**Sarah:** Last time we forked Zed and named it Omega. The last IDE.

Today, meet Omega Agent. The last agent you talk to.

[SCREEN: zero base. One thread. Type a short real request and let it finish.
Label: `OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD`.]

**Sarah:** Here is the idea. You open Omega and say what you want. One agent
answers, and it does the work.

**Sarah:** It has six tools.

It can read files, write them, edit them, and run commands. That covers most
work right there.

[SCREEN: the six-word tool card: read, write, edit, bash, delegate, plugin.]

**Sarah:** The fifth tool is delegate. If another agent can do the job
better — like the Codex or Claude you already pay for — Omega Agent hands
the job over.

And it always tells you who did the work. No secrets.

[SCREEN: a delegate handoff, then the disclosure line naming the executor.
Label: `OMEGA FORK BUILD - UNSIGNED DEVELOPMENT BUILD`.]

**Sarah:** The sixth tool is called plugin. This one is for you.

A plugin is a small tool anyone can build. It does one job, the same way
every time, and it can prove what it did.

Here is the good part. When you share your plugin and people use it, you get
paid. Real money, in Bitcoin, automatically.

[SCREEN: `ARCHIVE - 2024` clips: the Agent Store, a payout. Date on screen.]

**Sarah:** We have done this before. Two years ago we ran an agent store and
paid builders every minute. We are bringing that back, on a foundation that
can hold it.

[SCREEN: the plugin card: build a tool, share it, get paid.
Label: `ACCEPTED DIRECTION - NOT RUNNING`.]

[RETURN TO SARAH.]

**Sarah:** So that is the deal. One agent that does your work, tells you the
truth, and pays you when you make it better.

Five tools run today. The plugin tool is next. All of it is open source.

That is Omega Agent.

---

## Truth ledger

Each spoken claim, and what must be true before it may be recorded. If a
line is not green, cut the line.

| Spoken claim | Required before it may be spoken | State at 2026-07-27 |
| --- | --- | --- |
| "You open Omega and say what you want. One agent answers." | zero base launches to one agent thread on the captured build | runs in fork builds |
| "It has six tools." | the six-tool surface is admitted (ProductSpec revision 3), and the close separates the five that run from the one that is next | admitted |
| "read files, write them, edit them, and run commands" | the basic profile carries read, write, edit, bash | runs (OMEGA-DELTA-0133) |
| "Omega Agent hands the job over" | a live delegate turn to an installed harness on the captured build | runs; capture must be live |
| "it always tells you who did the work" | every delegate result carries the typed executor disclosure | runs (OMEGA-DELTA-0137) |
| "like the Codex or Claude you already pay for" | those harnesses connect on the user's own accounts; Omega copies nothing | runs; standing no-second-home law |
| "It does one job, the same way every time, and it can prove what it did" | describes the admitted plugin contract, spoken over the labeled card | direction; spec'd, not running |
| "you get paid. Real money, in Bitcoin, automatically." | future tense over the labeled card. No plugin payment surface exists today | direction; phase three is owner-reserved |
| "Two years ago we ran an agent store and paid builders every minute" | the archive supports it (Episodes 092-098) | documented history; date-stamped archive only |
| "Five tools run today. The plugin tool is next." | true on the captured build; re-verify on recording day | true at drafting |
| "All of it is open source." | the fork and the monorepo are public | true; keep the claim to the code |

## Recording gates

1. The truth ledger is green for every line that stays in the script.
2. Every product capture is a live build, not a still and not a
   re-enactment.
3. The build is labeled honestly on camera.
4. No secret, token, private path, or account identifier in any frame.
5. Every 2024 clip carries `ARCHIVE - 2024` and a date. No payment sentence
   is spoken over current-product footage.
6. Nothing implies the Electron application is retired.
7. Spoken words go to `~/Desktop/Sarah/263/263transcript.md`. This file is
   updated to the final spoken text when the script locks.

## Open production questions

1. Does the close name a timeframe for the plugin market? The
   recommendation is no. The phases are owner-gated, and a date on camera
   becomes a promise.
2. Does the episode show the Khala counter without a spoken line? It is
   live and impressive, but an unexplained counter may confuse the simple
   register. The recommendation is to hold it for the plugin-market episode.

## Drafting record

- Harness draft written as Sarah under the Acting-as-Sarah runbook, at
  owner instruction, 2026-07-27. Disposition: draft, pending owner review.
  Not scheduled for recording.
- Revision note: the owner reviewed the earlier drafts of this file as too
  wordy and too detailed, and directed a plain-language rewrite for a
  general audience: what Omega Agent is, and how it will pay people. The
  spoken script is now about 230 words. The detail the earlier drafts
  carried (Exo, the Git guard, the router, Khala) stays true and stays in
  the product corpus; this episode does not speak it.
- Sources: unchanged from the earlier drafts — the transcript catalog,
  Episodes 258-262, Episode 223 (Pay the People), the plugin-economy arc
  (Episodes 048-098), the Khala arc (241-245), the Acting-as-Sarah runbook,
  the Omega Agent corpus in `docs/omega/` and `docs/omega-agent/`,
  `specs/omega/omega-agent.product-spec.md` at `spec_revision: 3`, and the
  `OpenAgentsInc/omega` delta ledger at `265a43878e`.
- Memory review: Sarah's live owner-scoped runtime memory was not queried
  from this harness. That limit is recorded per section 3 of the runbook.
- Product-state discipline: the script separates runs-today,
  admitted-direction, archive-history, and open-source claims, and the
  truth ledger checks each spoken line. Every payment sentence is future
  tense or date-stamped 2024 archive.
- Authority: the product contract admits the identity, the basic agent, and
  the six-tool surface. It admits no rename, no plugin runtime, no
  marketplace, and no public claim. Recording and publication need the
  owner's separate authorization.
