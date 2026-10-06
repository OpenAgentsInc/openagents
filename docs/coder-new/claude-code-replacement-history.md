# Claude Code replacement history and current terminal survey

Research snapshot: October 6, 2026. This catalog is background for a new TUI
specification. It covers the retained `docs/transcripts/001.md`–`289.md`
archive and the removed, unreleased Gym preparation session linked by the
[archive index](../../docs/transcripts/README.md). The
[current terminal survey](#current-terminal-survey) adds a source-based
inventory of the implementations available for the new specification.
Companion research covers [typed plugins, Nostr, and payments](plugin-architecture-carry-forward.md)
and [lessons from DeepSeek Harness](deepseek-harness-lessons.md).

Chris repeatedly replaces the interface where he works, but the replacement
often keeps Claude Code as its executor. Attempts to own the execution loop
come later. The clearest recorded returns are a failed cancellation recalled
in episode 190 and a return from Coder to Claude Code during refactoring in
episode 285. Other episodes show partial migrations, deliberate wrappers,
working prototypes, or plans whose eventual outcome is not recorded.

## How to read the catalog

Three different dependencies recur:

- **Interface:** Claude Code's terminal, scrolling, chat history, and controls.
- **Harness:** Claude Code's agent loop, SDK, subprocess, tools, subagents,
  context management, and resumable sessions.
- **Provider:** Claude models, account capacity, authentication, subscription
  economics, and Anthropic-owned infrastructure.

Replacing one does not establish independence from the others. Using Claude
to help build a replacement also does not, by itself, prove that the new
product needs Claude to run. Each entry identifies the dependency actually
shown and the limit actually recorded.

All 289 numbered files were searched, with relevant passages read in context.
Episode order supplies the chronology. `Generated at` means transcription
generation, not publication; upload dates are used only where the source
provides them. Several later episodes replay earlier footage, so a replay
does not count as a new attempt. Short quotations below reproduce the retained
transcript wording; they have not been checked against the original audio.
Obvious transcription variants of Claude, Codex, and Tauri are normalized in
paraphrases. This is a history of attempts and revisions, not a claim that
every announcement shipped, every attempt failed, or every product is still
supported.

## The most direct evidence of incomplete exits

| Record | What it establishes |
| --- | --- |
| [169, 23:26](../../docs/transcripts/169.md#L403-L405) | The owner's earlier OpenAgents chat loses daily-driver status because Claude Code is better. |
| [188, 00:00 and 03:00](../../docs/transcripts/188.md#L13-L91) | A claim to have broken dependence on the terminal is followed by an explanation that the dashboard still calls Claude Code. |
| [190, 00:05](../../docs/transcripts/190.md#L19-L21) | A previous cancellation fails because there is no alternative; a later Codex switch is reported. |
| [225, 04:45 and 08:30](../../docs/transcripts/225.md#L27-L43) | Probe is described as a Claude Code replacement, while Chris still carries a laptop running Claude Code. |
| [254, 15:13–16:22](../../docs/transcripts/254.md#L327-L357) | The forced Desktop cutover still requires Claude Code for headless CLI work. |
| [285, 01:25:30–01:26:00](../../docs/transcripts/285.md#L355-L357) | Coder's refactor problems cause an explicit return to Claude Code because it is reliable. |

## Coding products before the first explicit Claude Code departure

### Onyx, repository maps, and the homegrown issue solver — episodes 152–169

**Attempt.** Build a coding workflow controlled from a phone: GitHub tools,
voice input, repository maps, relevant-file selection, reasoning, and an
issue-to-pull-request loop. The [phone coding demo](../../docs/transcripts/152.md)
and [coding-loop sequence](../../docs/transcripts/158.md) precede the later
Claude Code wrappers.

**Limit and outcome.** These are independent coding-agent predecessors, not
necessarily projects started to replace Claude Code. By
[episode 169, 23:26](../../docs/transcripts/169.md#L403-L405), Chris says the
OpenAgents v4 chat used to be his daily driver but he no longer recommends it:
Claude Code is better. This is the first clear displacement of an existing
homegrown workflow in the relevant history.

### OpenAgents v4 and MCP as alternatives to local coding tools — episodes 164–168

**Attempt.** Extend the web app with repository clones, shell commands, and
MCP tools so it can perform the local work that makes Cursor and Claude Code
useful. Chris tries implementing chat-history storage without returning to
Cursor and aims to steer CLI programs from the web interface.

**Limit and outcome.** Package installation and checks still happen manually
in another tool. Full-file editing and overflowing chat context require
further work; Chris explicitly describes OpenAgents as a supplement to Cursor
or Claude Code. Sources:
[164, attempted cutover and local-tool gap](../../docs/transcripts/164.md#L87-L191),
[165, command execution](../../docs/transcripts/165.md#L113-L119), and
[165, context-compaction gap](../../docs/transcripts/165.md#L769-L779).
This is a concrete attempt to close the capability gap, not proof of parity.

## Replacing the terminal while retaining Claude Code

### Commander — episodes 170–182, especially 179

**Attempt.** A visual fleet workroom replaces raw terminal windows with
multiple agent panes, history, and resumed chats. Chris explicitly reports
switching his daily workflow from Claude Code's TUI to Commander and wants
other Claude Code users working from this interface.

**Limit and outcome.** The displayed panes are still Claude Code sessions.
Some behavior, including slash commands, is not fully represented. This is a
reported interface migration, not a new execution engine. Sources:
[179, 00:00–01:10](../../docs/transcripts/179.md#L13-L21), with the earlier
[Commander introduction](../../docs/transcripts/170.md).

### Swift prototype, Zero Base, and the Tauri rebuild — episodes 183–187

**Attempt.** Start again from a fresh codebase. A Swift Claude Code wrapper
provides the initial concept; a Tauri desktop/mobile app is the new direction.
The goals are a useful multipane interface, more unattended agent time, and
voice/mobile control without staying tethered to a computer.

**Limit and outcome.** Episode 183 explicitly chooses to wrap Claude Code.
Episode 185's intended full switch means Claude Code through OpenAgents.
Streaming and polish are still being built, while episode 187 demonstrates
desktop/mobile synchronization. Sources:
[183, 00:27–03:28](../../docs/transcripts/183.md#L17-L99),
[185, 00:30–00:39](../../docs/transcripts/185.md#L37-L43), and
[187, 00:00–01:18](../../docs/transcripts/187.md#L13-L49).

### The web dashboard — episode 188

**Attempt.** Escape Claude Code's scrolling and sluggish terminal inspection
with a web dashboard: collapsible tools, context usage, historical sessions,
and concurrent issue-to-PR work. Chris says he has finally broken his
dependence on the terminal situation.

**Limit and outcome.** He also says the dashboard is calling Claude Code.
A local Bun script integrates its SDK and saves data to Convex. The previous
Effect-heavy architecture becomes too complex, so this web app is a faster
way to get orchestration working. Public release is still deferred. Sources:
[188, opening claim](../../docs/transcripts/188.md#L13-L31),
[executor and inspection](../../docs/transcripts/188.md#L91-L101), and
[simplification, SDK, and release limits](../../docs/transcripts/188.md#L121-L157).

## Switching providers and adding orchestration

### Failed cancellation, Codex, and Tricoder — episodes 190–193

**Attempt.** In the October 21, 2025 upload, Chris reports a switch to Codex.
He waited a month before announcing it because an earlier attempt to cancel
Claude Code drew him back: he had no alternative. Codex's inspectable Rust
code and simpler remote authentication make it attractive. Tricoder then
offers phone access to local coding agents with QR pairing over local Wi-Fi
or Tailscale.

**Limit and outcome.** The failed earlier departure is explicit, but its
implementation and exact date are not described. The later Codex switch is a
reported success, not proof of permanent independence. The phone demo retains
Claude Code as an alternative when Codex disappoints. Sources:
[190, failed cancellation and Codex rationale](../../docs/transcripts/190.md#L19-L41)
and [193, provider switch on the phone](../../docs/transcripts/193.md#L53-L67).

### Local-model orchestration and “Ditch the TUI” — episodes 194–198

**Attempt.** Replace the terminal-only workflow with a coherent application:
desktop history navigation, mobile control, web chat, and local Apple
Foundation Models for cheap orchestration. The proposal gradually shifts
some work away from expensive frontier models while preserving their strengths.

**Limit and outcome.** Claude Code and Codex remain delegated executors.
Local title/summary work is an initial reduction in model usage, not an
independent coding loop. The desktop and web interfaces demonstrate real
navigation and connectivity; packaged onboarding and broader behavior remain
unfinished. Sources:
[194, local orchestration](../../docs/transcripts/194.md#L147-L243),
[196, desktop interface](../../docs/transcripts/196.md#L25-L39), and
[198, retained executors](../../docs/transcripts/198.md#L71-L73)
([new web-interface demo](../../docs/transcripts/198.md#L213-L235)).
The repeated upgrade proposal across these episodes is one program.

### Autopilot as a persistent agent around coding agents — episodes 199–214

**Attempt.** A long-lived personal agent and desktop HUD keep work moving
between turns, expose state, and eventually draw on shared skills and swarm
compute. The first version wraps Claude Code; later iterations emphasize
Codex and structured guidance rather than repeated manual continuation.
Recursive language models are explored as another component.

**Limit and outcome.** Episode 199 deliberately avoids reinventing Claude
Code because it remains strong, and reports reliable overnight work through
the wrapper. The later Codex HUD is explicitly not yet polished enough to
replace daily Codex use. The RLM experiment is basic and untested. Episode 214
still describes an unfinished Codex GUI and a proposed mix of frontier and
network work. Sources:
[199, wrapper and overnight report](../../docs/transcripts/199.md#L107-L125),
[203, RLM limits](../../docs/transcripts/203.md#L371-L391),
[206, new Codex segment](../../docs/transcripts/206.md#L181-L217),
[208, replacement caveat](../../docs/transcripts/208.md#L147-L159), and
[214, unfinished GUI](../../docs/transcripts/214.md#L55).
Episode 206 also replays the older Claude-focused introduction; that replay
does not establish a new return to Claude.
Subscription economics also attract continued use: in
[214](../../docs/transcripts/214.md#L77), Chris reports that his former Claude
Code usage would cost $6,000 per month through the API while his subscription
cost $200. These are his reported figures, not an independently verified bill.

### Removing Anthropic after the access dispute — episodes 204–205

**Attempt.** After the reported token-access and terms dispute, Chris decides
to remove Claude and Anthropic integration rather than keep negotiating the
dependency. This is an explicit provider-removal decision within the
Autopilot period.

**Limit and outcome.** The transcript shows/reports cancellation of the Max
account and complete integration removal. It does not establish a permanent
cutover: later Probe, Khala, Desktop, and Coder episodes again show Claude
Code or Claude-model use. Sources:
[205, decision](../../docs/transcripts/205.md#L169-L179) and
[205, cancellation and removal](../../docs/transcripts/205.md#L273-L303).
Earlier passages about cloning agents in these episodes read other people's
posts; they are not additional personal implementation attempts.

## Building a replacement runtime and a cloud coding product

### Probe — episodes 218–219 and 225

**Attempt.** Build a lean, embeddable Rust coding runtime inspired by
OpenCode and Codex. The terminal demonstrates cloud/Codex, remote Qwen, and
Apple Foundation Model inference, with responsive exit behavior. Episode 225
calls Probe a drop-in replacement for Claude Code and Cursor and reports the
beginning of an internal workflow migration.

**Limit and outcome.** Small/local-model capability remains uneven; routing
work between models is prospective. Episode 225 says Probe uses Cursor under
the hood for now. In the same discussion, Chris still carries his laptop
running Claude Code and wants a proper mobile interface. Sources:
[218, independent-runtime intent](../../docs/transcripts/218.md#L23-L55),
[219, inference and terminal demo](../../docs/transcripts/219.md#L37-L73), and
[225, partial migration and continuing Claude use](../../docs/transcripts/225.md#L27-L43).
Episode 219's opening replays the launch material from 218.

### Free Autopilot, Pylon, and the training route — episodes 228–237

**Attempt.** Offer an asynchronous coding service, reusable traces, and an
open compute/training network. Episode 230 explicitly wants agents that do
not depend on closed third-party APIs; existing Claude Code/Codex capacity
can provide a temporary execution path. Autopilot 1.0 and Tassadar extend the
idea toward an owned frontier coding model.

**Limit and outcome.** The free beta initially targets public repositories
and slower asynchronous delivery. The training ambition is conditional;
the demonstrated run dashboard includes a placeholder loss curve. These
records do not establish a daily-driver replacement or trained-model parity.
Sources: [228, free coding beta](../../docs/transcripts/228.md),
[230, API independence](../../docs/transcripts/230.md#L82-L88), and
[237, conditional model goal and dashboard](../../docs/transcripts/237.md#L5-L11)
([dashboard details](../../docs/transcripts/237.md#L92-L102)).
The trace-training proposal in [215](../../docs/transcripts/215.md#L9-L27)
is part of this direction, not another completed coding client.

## Khala and the desktop replacement cycle

### Khala behind OpenCode, then the Khala CLI — episodes 242–244

**Attempt.** Put model routing and reusable orchestration behind an
OpenAI-compatible endpoint, first through an open OpenCode harness. Chris
wants daily work on Khala rather than alternating between bare Claude Code
and Codex interfaces. Episode 243 demonstrates document editing/committing
and concurrent OpenCode instances; episode 244 demonstrates Khala CLI v0.1.11.
Pylon can steer the owner's installed Claude Code and Codex sessions.

**Limit and outcome.** The first integration rejects content arrays and
drops streamed tool calls. An editing session must restart after the
surrounding tool context is interrupted. Chris still values Claude's
long-running context and subagents. Router-originated dispatch and capacity
discovery remain missing, and the intended capacity comes from the existing
coding engines. Sources:
[243, intent and initial failures](../../docs/transcripts/243.md#L62-L112),
[243, session interruption and successful work](../../docs/transcripts/243.md#L192-L222),
[244, existing engines](../../docs/transcripts/244.md#L65-L81), and
[244, dispatch gaps and CLI](../../docs/transcripts/244.md#L143-L208).

### Khala Code — episodes 245–246

**Attempt.** A desktop client replaces terminal multiplexing and provides a
familiar coding cockpit, with contracts and QA Swarm intended to make its
behavior reliable. Chris explicitly describes a drop-in replacement for the
usual harnesses.

**Limit and outcome.** It wraps Codex, with Claude Code support being added;
Chris is still split between this app and Codex. Invalid session IDs, missing
titles, poor exhausted-account errors, absent failover, an inert Plan control,
and unclear stop/follow-up/queue behavior obstruct dogfooding. Overnight fleet
work produces duplicate PRs and coordination problems. Most mined behavior
contracts still lack oracle tests. Sources:
[245, client introduction](../../docs/transcripts/245.md#L18-L33),
[246, split usage and interface defects](../../docs/transcripts/246.md#L13-L44),
[246, replacement goal](../../docs/transcripts/246.md#L59),
[246, fleet migration](../../docs/transcripts/246.md#L151-L167), and
[246, pending contracts](../../docs/transcripts/246.md#L224-L230).

### Electron Desktop and “Ready the Fleet” — episodes 248–250

**Attempt.** Fold Khala Code into a predictable OpenAgents Desktop, with
inspectable subagents, multiple accounts, and one conversation coordinating
Fable/Claude and Codex workers. The shell changes again: Chris reports trying
Tauri and Electrobun before adopting Electron. The immediate goal is a
workbench reliable enough to use to build the workbench itself.

**Limit and outcome.** Startup hits black screens and hydration problems.
Live fleet tests find missing New Chat behavior, stale content, an invented
launch command, a 400 error on the wrong request path, unpinned model selection,
missing history, graph flicker, and final provider/delegation failures. The
Claude SDK explicitly spawns actual Claude Code headlessly. Sources:
[248, reset and startup problems](../../docs/transcripts/248.md#L3-L143),
[249, shell history and subagent interface](../../docs/transcripts/249.md#L53-L109),
[250, live failures and headless Claude dependency](../../docs/transcripts/250.md#L248-L388),
and [250, incomplete streaming](../../docs/transcripts/250.md#L572-L618).
The archive does not establish a specific failure cause for every earlier
shell experiment.

### Codex-only Desktop MVP, assurance, and removing Bun — episodes 251–253

**Attempt.** Narrow the workroom to a local-first Codex wrapper that can
complete and resume tasks without sending the owner back to another Codex
UI. ProductSpec and Observer/AssuranceSpec define expected behavior and
automated QA. Separately, the product replaces Bun with Node to reduce
Anthropic infrastructure concentration.

**Limit and outcome.** A release build and packaging are reported, but
Claude and other engines are deliberately deferred from the initial MVP.
Removing Bun is a real infrastructure dependency change, not ownership of
Claude Code's agent loop. Sources:
[251, scope and deferred providers](../../docs/transcripts/251.md#L24-L84),
[252, assurance and release work](../../docs/transcripts/252.md#L49-L98), and
[253, runtime dependency and cleanup](../../docs/transcripts/253.md#L10-L56).

### The July 16 forced cutover — episode 254

**Attempt.** Chris announces firing Claude Code and Codex Desktop, switching
all interactive work to OpenAgents, and fixing it from inside itself. Khala
Code is explicitly folded into the Desktop name. The MVP's original scope
has already expanded to multiple coding engines.

**Limit and outcome.** The cutover encounters message persistence, broken
steer/queue controls, attachment restrictions, and worktree problems. Chris
defines failure as having to eject to Codex, yet explicitly keeps Claude
Code installed for headless CLI work. The app becomes more usable during the
session; parity is still described as almost there. Sources:
[254, departure and naming](../../docs/transcripts/254.md#L13-L75),
[254, expanded engines](../../docs/transcripts/254.md#L91-L119),
[254, failed controls and retained Claude CLI](../../docs/transcripts/254.md#L327-L357),
[254, attachments and worktrees](../../docs/transcripts/254.md#L385-L397),
and [254, closing status](../../docs/transcripts/254.md#L713-L753).
The source media filename supplies the July 16, 2026 date.

### FastFollow, Full Auto, and the Agent IDE release candidate — episodes 255–258

**Attempt.** Continuously study the strongest agents, close parity gaps, and
run unattended research/implementation. A super-harness should route across
providers so a job does not die on a usage limit. The release candidate adds
an editor; Chris reports switching away from Fable within one conversation.

**Limit and outcome.** Claude integration had been broken/unverified; routing
algorithms remain incomplete. The manually started FastFollow pilot first
produces analysis, encounters policy constraints, and still has broken
steer/queue behavior. Full Auto overnight execution remains a plan when the
recording ends. Later Cursor file-opening failures and
Codex Desktop crashes motivate the alternative, but OpenAgents' prevention
claims still need incident-scale evidence. Sources:
[255, Claude integration](../../docs/transcripts/255.md#L727-L797),
[255, routing goal and gaps](../../docs/transcripts/255.md#L1341-L1351),
[255, Full Auto limits](../../docs/transcripts/255.md#L1703-L1915),
[256, release candidate and reported switch](../../docs/transcripts/256.md#L12-L30),
[257, editor demo](../../docs/transcripts/257.md#L51-L77), and
[258, prevention caveat](../../docs/transcripts/258.md#L82-L120).

## Omega, Sarah, and another return to the terminal

### Omega, the Zed fork — episodes 262 and 264–265

**Attempt.** After acknowledging the lack of one stable product, the project
chooses Zed as a native editor foundation with agents and collaboration,
then adds verification, markets, and multiplayer. Later forensics demos show
an Omega workbench and substantial partial workflow migration.

**Limit and outcome.** Episode 262 is a final script that explicitly leaves
Desktop supported until Omega earns the cutover. Episode 264 aims to use
Claude Code exclusively through Omega and retains delegated Claude/Codex
engines. Episode 265 reports 60–80% of development moving into the interface,
with both direct Codex and an Omega/Luna agent path; incomplete source
checkouts and target ambiguity complicate verification. Sources:
[262, scope and release boundary](../../docs/transcripts/262.md#L14-L37),
[264, Claude-through-Omega and UI defects](../../docs/transcripts/264.md#L69-L101),
and [265, engine paths and partial migration](../../docs/transcripts/265.md#L64-L88).
The later [Omega demo in 267](../../docs/transcripts/267.md#L25-L29) shows
DeepSeek and Kimi alongside delegation to Codex: another provider-neutral
alternative that still permits existing harnesses underneath.

### Sarah, ACP delegation, and BEAM continuity — episodes 269–274

**Attempt.** Chris reports moving development to Sarah on OpenAgents.com.
Sarah becomes a central interface for local agents, Forge, voice, memory,
and GitHub work. The plan combines Devin through ACP with Codex and other
executors; BEAM clustering should keep delegations alive across code changes.

**Limit and outcome.** Sarah still coordinates external coding agents.
Server-memory delegations disappear on restart and leave local Claude Code
processes orphaned. The hot-load work demonstrates an improvement, but the
full resilience path remains work to test. Sources:
[269, reported switch](../../docs/transcripts/269.md#L21),
[270, centralized interface and Devin/Codex](../../docs/transcripts/270.md#L179-L181),
and [271, lost delegations](../../docs/transcripts/271.md#L33).
Episode 272 again sets the goal of not needing external agent interfaces,
but its live interaction has context and visibility failures; see
[272, self-improvement attempt](../../docs/transcripts/272.md#L50-L108).
Forge and mirroring work support the consolidation. In
[274](../../docs/transcripts/274.md#L23-L35), Chris describes collapsing a
dozen surfaces, including Claude, into one interface and a new OpenAgents CLI.
These are further interface/orchestration attempts, not evidence of a
separate Claude-independent coding loop.

### Coder and the deliberately small terminal — episodes 275–277

**Attempt.** Return to a TUI after trying other agents and harnesses that do
not provide the desired workflow. Coder offers an installable, immediate
text box with model choices, tools, and Forge integration. Episode 277
reduces the active interface to visible shell commands, fast startup, and
a simple Gemini-powered coding loop. Chris reports day-to-day use.

**Limit and outcome.** This is a real native-agent/TUI alternative, while
cloud, web, mobile, and broader suite behavior remain partly prospective.
Using other model providers diversifies inference; the later refactor-driven
return to Claude Code shows the daily-driver cutover is not durable. Sources:
[275, rationale, tools, and model choices](../../docs/transcripts/275.md#L15-L21),
[275, partial suite](../../docs/transcripts/275.md#L25-L31), and
[277, daily use and minimal shell TUI](../../docs/transcripts/277.md#L17-L25).
The Coder Cloud private beta in [276](../../docs/transcripts/276.md) is another
execution location, not evidence of a distinct agent loop.

### Coder delegation, the Claude Code study, CoderOS, and mobile — episodes 278–281

**Attempt.** Combine competing agents under Coder, study Claude Code's
architecture for an independent Rust implementation, and carry the same
work across Linux and phones. Episode 278 demonstrates concurrent Codex and
Claude delegation; the study becomes a proposed implementation backlog.

**Limit and outcome.** Provider neutrality still includes a Claude Code
adapter on the user's sign-in. A roadmap does not establish completed
lifecycle or recovery behavior. Mobile synchronization and trusted-device
flows are still being built and debugged. Sources:
[278, delegation demo](../../docs/transcripts/278.md#L19-L21),
[279, independent implementation direction](../../docs/transcripts/279.md#L35),
[279, retained Claude adapter](../../docs/transcripts/279.md#L55), and
[281, cross-device build session](../../docs/transcripts/281.md).
CoderOS in [280](../../docs/transcripts/280.md) extends the environment; it
does not itself remove the executor dependency. A model's consent statement
in the study is not vendor authorization or implementation evidence.
In [283](../../docs/transcripts/283.md#L18), Chris says the Claude Code study
has messed up Coder and Devin CLI is repairing it, another recorded obstacle
to keeping the replacement usable.

### Coder v0.5 refactoring and an explicit return — episode 285

**Attempt.** Continue using Coder as the daily coding interface while
refactoring it and exploring a new agent architecture.

**Limit and outcome.** At 01:26, Chris says Coder has problems during its
refactor and he has switched back to Claude Code: “but it's reliable.” This
is a direct operational relapse, not merely use of Claude as an optional
model or a reference. Source:
[285, 01:25:30–01:26:00](../../docs/transcripts/285.md#L355-L357).
This passage concerns the existing Coder; it should not be presented as proof
that the separate Bendcoder prototype caused the fallback.

## Trying to own the agent loop

### Bendcoder and the System One architecture — episodes 285–286

**Attempt.** Build a small open-source coding agent from scratch in Bend,
with C for missing host capabilities. Jev classifies state, generation
proposes an action, and a deterministic loop executes tools. An interactive
terminal and self-improvement loop are built during the session. The aim is
to escape large, cache-dependent harnesses and make reusable agent behavior
composable.

**Limit and outcome.** Language/FFI limitations, shallow reads, missing
patch execution, build failures, and verification problems appear during
the prototype. A reported end-to-end test initially proves a small hello-world
objective rather than general autonomous development. Episode 286 extends
the design through context, permissions, progressive tools, and background
work; it is design exploration, not a completed replacement. Sources:
[285, architecture and language choice](../../docs/transcripts/285.md#L17-L45),
[285, initial loop and test](../../docs/transcripts/285.md#L171-L177),
[285, missing self-improvement and compile failures](../../docs/transcripts/285.md#L337-L363),
and [286, architecture discussion](../../docs/transcripts/286.md).
By the close, the prototype makes autonomous edits, but Chris still calls it
a proof of concept and questions Bend's contribution; see
[285, closing assessment](../../docs/transcripts/285.md#L513-L531).

### Coder One: standalone loop, then probes and delegation — episode 287

**Attempt.** Start again with a minimal issue-to-PR agent: explicit state,
Jev judgments, one generated action, and deterministic execution. Chris
announces an intention to make Claude Code obsolete, then tests configurations
on Terminal-Bench. The approach evolves from a standalone generation loop
to probes, evidence-based briefings, and short delegates.
The initial issue-to-PR deliverable is itself described as a small sibling of
Coder, not its replacement; see
[287, initial scope](../../docs/transcripts/287.md#L114-L142).

**Limit and outcome.** The session explicitly does not yet establish general
replacement quality. It removes Gemini from the loop and reuses Claude Code
or Codex delegates, tuning what surrounds them. The closing comparison keeps
the same Opus model/pass rate while reducing observed cost and time on the
tested tasks. The cheapest-per-task chooser is retrospective headroom, not a
validated router. Sources:
[287, replacement intent](../../docs/transcripts/287.md#L68),
[287, general-quality caveat](../../docs/transcripts/287.md#L214), and
[287, final architecture and measurement limits](../../docs/transcripts/287.md#L608-L626).
This is progress toward controlling the harness, with a substantial delegate
dependency still present.

### Microluna, Gym, and Fire Loop — removed, unreleased preparation session

**Attempt.** Move from a front end around strong delegates to an owned small
model harness. Microluna becomes the intended terminal path; Gym provides
inspectable trajectories and judgments. The goal is to put Claude on the
shelf as a model to use when needed, rather than depend on its whole coding
product. Fire Loop adds state, evidence, and completion checks.

**Limit and outcome.** The terminal is described as outdated and Chris wants
to get back to using it as his only UI. Tests pass while seven requirements
remain unobserved; a later version changes the wrong frozen check. The sole
reported embedding-task win is corrected to 54 times cheaper and is from a
task tuned during development, with no established generalization. Fire Loop
also encounters broken finish logic. Sources from the
[last retained Git version](https://github.com/OpenAgentsInc/openagents/blob/1ebd5e20f7/docs/transcripts/288-prep.md):
[Microluna pivot](https://github.com/OpenAgentsInc/openagents/blob/1ebd5e20f7/docs/transcripts/288-prep.md#L63-L101),
[outdated terminal](https://github.com/OpenAgentsInc/openagents/blob/1ebd5e20f7/docs/transcripts/288-prep.md#L281-L299),
[intent and false completion](https://github.com/OpenAgentsInc/openagents/blob/1ebd5e20f7/docs/transcripts/288-prep.md#L365-L387),
[cost correction and limits](https://github.com/OpenAgentsInc/openagents/blob/1ebd5e20f7/docs/transcripts/288-prep.md#L397-L459), and
[Fire Loop](https://github.com/OpenAgentsInc/openagents/blob/1ebd5e20f7/docs/transcripts/288-prep.md#L461-L469).
This incomplete session was recorded under a number later reassigned and
removed from the tree. It is not the released episode 288, and the transcript
is not restored here.

### The archive's endpoint — episodes 288–289

The current [288](../../docs/transcripts/288.md) previews OpenAgents 1.0.0;
[289](../../docs/transcripts/289.md#L16-L18) presents a composable general
agent across terminal, desktop, and mobile, with Coder loaded for repository
work. These describe another consolidation of earlier capabilities. Neither
establishes that Claude Code dependence has permanently ended. The source
survey below examines later terminal work separately from this chronology.

## Earlier Claude exits that are not Claude Code exits

The first 145 episodes contain important versions of the same pattern, but
their target is Claude chat/Artifacts, ChatGPT, or a homegrown Sonnet interface.
They should not be mislabeled as Claude Code history.

| Earlier effort | Progress and remaining dependence | Sources |
| --- | --- | --- |
| OpenAgents chat/Connie | Builds the daily chat interface and reports canceling ChatGPT and Claude subscriptions while continuing to use Claude models through OpenAgents. | [067, intent](../../docs/transcripts/067.md#L13-L25); [089, cancellation and model use](../../docs/transcripts/089.md#L49-L73). |
| AutoDev and WANIX artifacts | Builds an open coding workbench and browser shell/compiler concept. Chris reactivates Claude Pro and says he will cancel once the OpenAgents version works. Later he explicitly aims to replace Claude Artifacts, but patch handling still requires manual copying. | [103, workbench plan](../../docs/transcripts/103.md#L29-L99); [106, subscription relapse](../../docs/transcripts/106.md#L13-L39); [112, Artifacts replacement](../../docs/transcripts/112.md#L13-L15). |
| AutoDev v2/v3 | Reports 75%, then 85%, of workflow migration, while still needing Claude cleanup and second opinions. Closing the mounted chat can stop work; review/builds remain manual. Later reports no Pro subscription and months of daily use, so this is partial progress with reversals, not blanket failure. | [115](../../docs/transcripts/115.md#L13); [117](../../docs/transcripts/117.md#L13); [118](../../docs/transcripts/118.md#L13); [122, continuity](../../docs/transcripts/122.md#L17); [123, second opinions and review](../../docs/transcripts/123.md#L13-L15); [137, no Pro](../../docs/transcripts/137.md#L13); [138, daily use](../../docs/transcripts/138.md#L13). |
| Onyx/Pylon/local models | Proposes moving away from a complex, exclusively Sonnet-backed OpenAgents interface to voice/mobile control and local models. Small-model quality and larger-model latency still require a mix with hosted APIs; coding migration remains prospective. | [139, mobile direction](../../docs/transcripts/139.md#L13); [144, local tools](../../docs/transcripts/144.md#L13); [145, migration and limits](../../docs/transcripts/145.md#L13). |

The retrospective in [127](../../docs/transcripts/127.md#L13) makes the
distinction explicit: ChatGPT, an owned interface, Claude Artifacts, then
Sonnet connected directly to GitHub. Faerie's earlier
[coding loop and failure analysis](../../docs/transcripts/032.md#L25-L75)
is a predecessor to all of these, not an attempt to leave Claude Code.

## Current terminal survey

This survey describes committed source as of October 6, 2026:

- **This repository:** OpenAgents at
  [`636354ddc0a4aa044f971fafd39b47978fa62658`](https://github.com/OpenAgentsInc/openagents/tree/636354ddc0a4aa044f971fafd39b47978fa62658).
  Implementation and retained-evidence links below pin this revision.
- **Sibling Coder repository:** `../coder`, inspected from a read-only
  checkout of the upstream repository at
  [`0b9916b42f9b8a261ffaf911b6630c9ca72660ad`](https://github.com/OpenAgentsInc/coder/tree/0b9916b42f9b8a261ffaf911b6630c9ca72660ad).
  Its workspace version is `0.5.0`. Links to that repository pin this revision.
  This does not include uncommitted changes in another local checkout.

Source establishes implemented paths and retained checks. This survey does
not run the apps, exercise real providers, or establish daily-driver parity
with Claude Code. Older READMEs and roadmaps sometimes describe earlier
behavior; the implementation takes precedence here.

### The names refer to different layers

| Area | What it owns | Entry points |
| --- | --- | --- |
| Coder terminal crates in this repository | Shared theme, editor, composer, terminal lifecycle, transcript components, and rendering helpers. | [`coder-terminal`](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/lib.rs#L1-L68), [`coder-ui`](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-ui/src/lib.rs). |
| Standalone Coder in this repository | A small conversational shell over Coder's shared turn runner; separate task-management CLI. | [`coder` dispatch and screen](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/main.rs#L335-L453). |
| Terminal app in `../coder` | A complete Coder session app, shared UI core and terminal renderer, local execution, delegated agents, session recovery, and attached clients. | [`coder-terminal-app` binary](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/Cargo.toml#L6-L95). |
| OpenAgents chat TUI | Full-screen OpenAgents threads with Coder runs, settings, plugins, and host integration. | [`openagents terminal` and bare `openagents`](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L1-L44). |
| OpenAgents plain TTY shell | An ordinary shell with explicit requests, inline proposals, and owner-confirmed commands. | [`openagents terminal shell`](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L1475-L1531). |
| Related native OpenAgents Terminal | A real shell and smart input in a native window or Verse sheet, with thread and workbench views. | [`openagents-terminal` binary](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-app/Cargo.toml#L1-L24), [`terminal-core`](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/application.rs#L51-L114). |

The two repositories' `coder-terminal` crates are distinct implementations.
Neither crate name alone identifies the full application. Likewise,
`openagents terminal` is a terminal-hosted chat screen, while
`openagents-terminal` is the native shell application. The `shell` subcommand
adds a third surface that keeps the ordinary shell's editor and output.

### Shared Coder terminal components in this repository

**Existing foundation.** `coder-ui` supplies a four-level white palette;
`coder-terminal` supplies the ratatui composer, grapheme-aware multiline
editor, key mapping, Markdown renderer, spinner, guarded terminal modes,
scrollback, and event transport. Color selection supports truecolor,
indexed color, and `NO_COLOR`. Sources:
[crate inventory](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/lib.rs#L1-L68),
[palette](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-ui/src/theme.rs#L18-L46), and
[terminal color selection](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/ladder.rs#L82-L133).

The component layer can render turns, notes, cards, grouped tools, file diffs,
delegations, run rails, and list overlays. Components are pure renderers;
they do not own conversations, choose providers, or authorize actions. The
OpenAgents TUI consumes these richer components. Their presence does not
mean the smaller standalone `coder` screen exposes them. Sources:
[component modules](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/components/mod.rs#L1-L33)
and [OpenAgents row composition](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/rows.rs#L1-L26).

**Bounds and evidence.** Scrollback defaults to 5,000 logical lines and
caches wrapping until the width changes. Live preview text has a 1 MiB
buffer and reports dropped bytes; control events use an unbounded queue so
command outcomes are not silently dropped. These are display bounds, not a
bound on the agent's context or the process's total memory. Existing checks
cover Unicode editing, history, cleanup, event ordering, narrow layouts,
and component snapshots. Sources:
[scrollback](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/scrollback.rs#L17-L66),
[event lanes](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/events.rs#L1-L27),
[preview limit](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/events.rs#L111-L129), and
[component tests](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/tests/components.rs#L264-L447).

The Rust Native adapter projects a limited view vocabulary onto this
terminal palette and emits revision-bound activations. It is not a native
control host or an authorization layer; unsupported properties and literal
Markdown rendering remain explicit limits. See the
[adapter boundary](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/native.rs#L1-L28) and
[supported rendering](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/native.rs#L49-L112).

### The smaller standalone Coder shell in this repository

**Interaction.** The screen has a multiline composer, streaming and completed
Markdown, proposal/verdict/progress/error rows, a spinner, scrolling, and a
detail toggle. Enter submits; Alt+Enter or Ctrl+J inserts a newline. Readline
editing and prompt history live in the shared editor. `/verbose`, `/v`, and
Alt+V are its only screen commands. Every other slash-prefixed submission
is rejected as an unknown command. Page Up and Page Down move ten rows.
Sources: [composer keys](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-terminal/src/keys.rs#L30-L122),
[screen controls](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/main.rs#L582-L629), and
[transcript rendering](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/main.rs#L656-L880).

**Queue and lifetime.** A busy turn accepts one pending draft in an `Option`;
a later submission replaces it. This is not a durable FIFO queue. Ctrl+C
exits the screen; empty Ctrl+D also exits. There is no screen-level
per-turn cancellation, thread picker, run rail, or full-screen run view.
The production event loop handles key events but does not integrate mouse
or bracketed-paste events. The header captures the agent label once at
startup, so it is not a live provider/account/failover display. Sources:
[application state](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/main.rs#L192-L215),
[header](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/main.rs#L526-L539), and
[queue and input handling](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/main.rs#L550-L629).

The `coder task` command family separately offers durable submission,
execution, correction, cancellation, recovery, inspection, artifact, and
archive operations. Submission is inert until explicitly executed with a
grant. That task CLI does not make the small conversational screen a
thread-management UI. See the [task CLI](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/task/cli.rs#L11-L39).

**Claude Code dependence.** The shared turn runner can select Microcoder,
Claude Code CLI, or Codex CLI; environment settings and available
authentication affect selection. Microcoder owns the loop and tries Codex,
Claude, then hosted Vertex transport. Its Claude transport still invokes
the `claude` executable as a tools-disabled model call. That removes
Claude's agent loop from that path, but retains its executable and login
dependency. A whole-task Claude CLI delegation retains the foreign harness
and session instead. Sources:
[delegate selection](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door.rs#L350-L384),
[selection conditions](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door.rs#L454-L520),
[Microcoder provider order](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door/microcoder.rs#L152-L163),
[provider construction](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door/microcoder.rs#L320-L345),
[Claude model transport](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/microcoder-loop/src/claude.rs#L1-L9), and
[whole-task delegation](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door.rs#L898-L938).

The current conversational Microcoder path records spend but sets unbounded
loop limits; older claims of a fixed dollar or step cap do not describe this
path. The CLI turn also disables the separate issue-to-PR flow. See
[current limits](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door/microcoder.rs#L629-L634)
and [CLI flow selection](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/delegate_door.rs#L932-L936).

### The complete terminal app in the sibling Coder repository

**Architecture and interface.** The sibling app uses ratatui/crossterm over
the renderer-free `coder-ui-core` cell grid. Its shared core also serves
other surfaces; the terminal binary does not depend on GPUI. The default
transcript follows the tail until scrolled. Its composer includes directory
and branch, slash suggestions, context estimates, token totals, active
children, and available CPU/RAM readings. A delegation panel sits below it.
Sources:
[renderer](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-terminal/src/render.rs#L1-L82),
[shared UI core](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-ui-core/coder_ui_core.rs#L1-L50), and
[transcript screen](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/screens/transcript.rs#L1-L156).

The app adds a Resume picker, Plugins and gated Gym screens, child and
delegation detail, a bounded read-only syntax-highlighted file viewer, and
an optional fleet/order console. Editing includes grapheme-aware selection,
soft wrapping, persistent prompt history, slash/path completion, mouse
selection, and platform clipboard/OSC 52 copying. Pasted or dropped image
paths produce multimodal attachments, limited to 4 MB per image, with HEIC
conversion. Sources:
[screen inventory](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/screens.rs#L1-L40),
[editor](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-ui-core/input.rs#L1-L40),
[history](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-terminal/src/composer/history.rs#L16-L63),
[completion](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-terminal/src/composer/complete.rs#L1-L170),
[file viewer](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/file_viewer.rs#L1-L129), and
[image attachment path](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/image.rs#L1-L85).

**Sessions beyond the screen.** Scrubbed JSONL journals retain entries,
turns, checkpoints, and child references. Durable context tracks projected
occupancy; outage handling preserves completed tool results and resumes
interrupted responses. `serve` runs the same session loop without a screen;
attached terminals, CLI clients, and phone clients submit to one writer.
Admission supports loopback, allowlisted devices, expiring one-use pairing,
and registered account devices, with revocable submit/drive grants.
Sources:
[journal](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/history.rs#L1-L38),
[checkpoint and child records](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/history.rs#L135-L186),
[context projection](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/context.rs#L112-L188),
[recovery](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/outage.rs#L57-L130),
[shared writer](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/writer.rs#L1-L15), and
[device admission](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/admission.rs#L11-L53).

A libghostty-vt owner parses child-terminal output and publishes snapshots.
`drive` gives one client keyboard/resize authority; `panes` exposes the
child fleet through tmux. Native writer children normally receive isolated
Git worktrees; read-only children share the selected directory. Native
workers have durable admission and recovery machinery that foreign
adapters do not share. Sources:
[terminal owner](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/terminals.rs#L1-L38),
[drive protocol](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/drive.rs#L1-L27), and
[child isolation](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/isolation.rs#L1-L49).
See also the [native/foreign recovery boundary](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/delegate.rs#L1-L10).

Cross-screen survival requires a living writer. An attached terminal is a
client; an ordinary interactive terminal owns its loop. `serve` runs until
stopped, and `--with-parent` also ends it when stdin closes. Shutdown records
unfinished calls and child references; a retained `left_running` label is
not proof that a child continues uninterrupted after its owner exits.
Sources:
[attached client](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/attach.rs#L1-L19),
[writer lifetime](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/writer.rs#L238-L265), and
[shutdown records](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session_shutdown.rs#L9-L66).

While a turn is busy or recovering, submitted prompts wait in FIFO order
and run as separate turns, rather than immediately steering the current
turn. The pending queue lives in memory and is not restored by the journal.
`/queue cancel` clears it. Once notices and child views have handled their
keys, two Esc presses request cancellation of an active turn; child task
stopping has a separate `TaskStop` route. Ctrl+O releases or recaptures the
mouse for the enclosing terminal; it has a different meaning from Ctrl+O
in the OpenAgents TUI. Sources:
[prompt queue](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/queue.rs#L13-L105),
[queue controls](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/operator.rs#L381-L395),
[busy submission](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session.rs#L1212-L1218), and
[stop and mouse handling](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session_input.rs#L74-L122).
See [child task stopping](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session_called.rs#L147-L161).

**Execution and commands.** The active `Agent` tool defaults to native Coder,
with scoped child tools and task output/stop controls. Native Coder owns
its engine and supports local inference, so Claude Code is not required
for native tasks. Available foreign adapters include Claude Code through
the Claude Agent SDK and Codex through `codex app-server`, as well as other
local/cloud agents. A retained legacy `delegate` route instead prefers
available Claude Code, then native Coder; it is currently undeclared in
the normal foreground tool list. Sources:
[active delegation](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/delegation.rs#L281-L425),
[native worker](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/native_worker.rs#L19-L89),
[foreign adapters](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/delegate.rs#L139-L196), and
[legacy preference](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/agent_catalog.rs#L87-L131).

A shared command catalog drives help and completion, with session-bound
controls and standalone operations. It includes context/compaction,
permissions/plan, queue/history/resume, children, plugins/MCP, and repository
operations. `/autopilot` persists missions with proposal/confirmation,
pause/resume/stop, and fanout settings; it reports unlanded branches rather
than landing them automatically. A catalog entry alone does not establish
the depth of its implementation. Sources:
[catalog contract](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-contract/src/command.rs#L1-L21),
[visible command selection](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session_commands.rs#L29-L44), and
[autopilot lifecycle](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/autopilot_mission.rs#L170-L280).

**Limits relevant to a replacement spec.**

- Normal foreground requests advertise eleven ported Claude-style tools,
  plus `windows` when available. The tool-list builder ignores the supplied
  capabilities: retained plugin/MCP dispatch does not make those tools
  available to the foreground model. See
  [tool declaration](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/service.rs#L129-L145).
- Sessions start in `bypass`; other modes ask on selected effects or refuse
  writes/execution/delegation. Foreign adapters retain different permission
  contracts, so a parent mode is not proof of identical child behavior. See
  [parent modes](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/permission.rs#L1-L68)
  and [foreign harness setup](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/delegate.rs#L496-L563).
  The [session initializer](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session.rs#L627)
  sets the default mode.
- Plugins start disabled; none passes the default suite's no-regression
  criterion. Network plugins are refused. Gym is opt-in; chat sync remains
  a gated feature in preparation. See
  [plugin defaults](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/plugin_settings.rs#L1-L29),
  [network-plugin refusal](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/plugin_settings.rs#L970-L986),
  [Gym gate](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/features.rs#L3-L14), and
  [sync gate](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/sync.rs#L34-L47).
- `/agents`, `/review`, and `/verify` record a preferred role, but active
  `Agent` launches do not consume it: the translated call sets no role.
  `/rewind`
  restores recorded direct mutations with intervening-change checks, not
  arbitrary shell or foreign-agent changes. See
  [operator commands](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/operator.rs#L166-L238),
  [role setting](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/operator.rs#L92-L103), and
  [active child translation](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/delegation.rs#L324-L339).
- Some settings are shallower than their names suggest: `/vim` toggles a
  flag without changing the input handler; `/theme` stores a setting while
  the renderer selects its fixed palette; `/ssh` validates a target and
  describes it without opening a connection. See
  [Vim toggle](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/operator.rs#L206-L212),
  [input handler](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/session_input.rs#L131-L139),
  [theme storage](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-tools/src/command.rs#L659-L672),
  [palette selection](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-terminal/src/ladder.rs#L33-L76), and
  [SSH command](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-tools/src/remainder.rs#L157-L177).
- The editor understands graphemes, but the grid stores one `char` per cell;
  wide/combining-glyph rendering still needs demonstrated coverage. Pane
  integration currently implements tmux, and Windows child PTY methods
  return an unsupported-host error. See
  [cell storage](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-ui-core/grid.rs#L177-L182),
  [pane backend](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/panes.rs#L60-L76), and
  [Windows PTY methods](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/bins/coder-terminal/src/pty.rs#L220-L288).

Renderer golden tests, app regression fixtures, a PTY screen harness, and
multi-platform release scripts exist. They provide useful starting evidence,
not a new live acceptance result from this survey. See
[renderer goldens](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/crates/coder-terminal/tests/golden.rs#L1-L10)
and [PTY harness](https://github.com/OpenAgentsInc/coder/blob/0b9916b42f9b8a261ffaf911b6630c9ca72660ad/ops/tests/terminal-screen.py).

### OpenAgents Terminal: the chat TUI

**Architecture and routing.** Bare `openagents` on a TTY, or explicit
`openagents terminal`, opens the full-screen chat app. `openagents-terminal`
owns screen state; `openagents-cli` supplies pairing, host, settings, and
plugin operations; the shared chat client owns conversations and routing.
The client can use this computer's host, an in-process local store, or a
paired computer. Automatic host selection can fall back to local; an
explicit bad socket is an error. Sources:
[launch and options](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L1-L44),
[screen boundary](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/lib.rs#L1-L26), and
[client selection](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-chat/src/client.rs#L830-L946).

Repository work goes through Coder's shared task bridge. Settings determine
whether Coder starts immediately or offers a run; a detached controller
owns the task, with grants and durable storage. Local runs normally use a
detached Git worktree at repository HEAD, without copying uncommitted
changes. Provider preferences and fallback belong to Coder, rather than a
new TUI-specific model loop. Local results retain their actual check scope;
an executor's successful exit is not silently presented as independently
verified correctness. Sources:
[chat-to-task bridge](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/task/chat_client.rs#L150-L257),
[worktree and provider defaults](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/task/local.rs#L9-L80),
[detached controller](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/task/autostart.rs#L985-L1006), and
[result labeling](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-chat/src/client.rs#L1758-L1801).

Shared command effects decide whether a proposed command runs immediately,
needs Enter confirmation, or cannot run from chat. This policy is separate
from Coder's execution grants and provider-specific approval behavior. See
[command admission](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-chat/src/client.rs#L1979-L2025).

**Transcript and run controls.** Typed rows show turns, notes, cards,
commands, tool output, exits, thoughts, questions, provider switches, and
run changes. Ctrl+O expands details. A compact rail shows agent, activity,
and elapsed time; completed runs leave the rail after 30 seconds while
remaining in the log. Ctrl+R opens the current run; Alt+1–9 or `/open N`
opens a numbered run. The run composer sends steering or answers; how
steering works depends on the executor, including cancellation and restart
for agents that accept only initial instructions. Sources:
[row types](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/rows.rs#L12-L208),
[rail lifecycle](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/rail.rs#L23-L175),
[run submission](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L1085-L1146), and
[executor steering](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder/src/task/local.rs#L1521-L1591).

Esc is contextual: it closes views or dismisses their active interaction
before reaching task controls. In the thread picker it first leaves search
or clears a query. Other states reject a proposal or stop active work.
Closing a run view is not itself a stop. `/quit`, empty Ctrl+D, or double
empty Ctrl+C closes the TUI while a detached Coder task keeps working. Returning to its
thread reconnects to the run. Ordinary chat submission during streaming
is refused with the draft preserved; it is not queued. Sources:
[Esc handling](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L544-L595),
[busy submission](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L1109-L1112),
[quit handling](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/screen.rs#L947-L960), and
[CLI handoff](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L333-L342).
See also [thread hydration and task follow](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/screen.rs#L656-L685).

**Commands and retained state.** The screen has 17 slash commands:
`/new`, `/resume [ID or title]`, `/threads`, `/stop`, `/export`, `/settings`,
`/connect`, `/plugins`, `/background [words]`, `/worktrees`, `/import`,
`/efficiency`, `/expand`, `/run`, `/open N`, `/help`, and `/quit`. Tab completes
an unambiguous slash prefix. An unknown single lowercase `/word` errors;
other unmatched slash-leading text goes to the router. Consequently `/tmp`
errors, while `/tmp/file` is a message. Sources:
[command list and descriptions](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/slash.rs#L12-L122),
[parser](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/slash.rs#L143-L172), and
[Tab completion](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L607-L615).

The thread picker groups the current folder, general chats, and other
projects, with new/open, archive, expand/collapse, ID-copy, and search
controls. Search matches title, project, and ID prefixes rather than message
bodies. The picker requests at most 200 threads. Prompt history keeps up to 500 entries;
last-thread state remembers up to 256 folders. `--continue`, `--thread`,
`--resume`, `--scratch`, and read-only `--observe` expose different launch
semantics. Sources:
[picker grouping](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/picker.rs#L83-L131),
[picker controls](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/picker.rs#L225-L330),
[search](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/picker.rs#L665-L724),
[prompt history](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/prompts.rs#L1-L57),
[last-thread state](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/last.rs#L13-L45), and
[observe handling](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/screen.rs#L347-L356).

Settings edit Coder start behavior, permitted providers, and credentials;
plugins can be installed, toggled, and invoked. Background rules have
confirmation and watcher controls. Worktree controls inspect storage and
archive ended tasks. ATIF export retains thread/task evidence. Host-backed
import copies Claude Code/Codex messages and text replies; it does not
reconstruct their full tool state or foreign harness. Sources:
[plugin and import operations](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L586-L634),
[worktree operations](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L665-L681), and
[ATIF export](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/screen.rs#L718-L755).
The [importer contract](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/coder-host/src/sessions.rs#L1-L32)
also limits each imported turn and keeps the newest 200 turns.

**Input, files, and recovery.** Bracketed paste normalizes newlines without
submitting. Mouse drag selects text and copies it through OSC 52; clicked
paths open a read-only viewer, preferring the run worktree for relative
paths. The viewer rejects files over 2 MiB and probable binary files.
Ctrl+Y copies the latest completed OpenAgents reply and then its code blocks.
The chat send operation is text-only: this TUI does not currently provide
image/file attachment or `@`-mention input. Sources:
[paste](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L458-L478),
[reply copying](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L949-L977),
[path and mouse handling](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L1608-L1670),
[viewer](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/view.rs#L163-L237), and
[send operation](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L1125-L1132).

Reconnect uses stable send IDs, retains partial replies, and retries
interruptibly with bounded backoff; semantic refusals return immediately.
The screen reports reconnection and allows Esc to stop observation/retry.
Client tests and PTY fixtures cover these flows, but this survey does not
run them. Sources:
[reconnect policy](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-chat/src/client.rs#L71-L108),
[partial-reply retention](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-chat/src/client.rs#L1415-L1438),
[send retry](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-chat/src/client.rs#L2160-L2224),
[reconnect display](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/src/app.rs#L1328-L1337), and
[PTY fixture](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-terminal/tests/pty.rs#L623-L640).

### OpenAgents Terminal: the plain TTY shell

`openagents terminal shell [--root PATH] [--shell PATH]` opens a real shell
through the new `terminal-tty` crate. Ordinary editing, commands, and
full-screen program output remain with the shell. Explicit `# ` requests
use the shared typed chat bridge and return inline proposals; this path
does not use the native sheet's automatic command/question classifier.
The existing chat TUI remains a separate entry point. Sources:
[shell entry point](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/screen.rs#L1475-L1531),
[shell launch](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L151-L198),
[output forwarding](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L223-L238), and
[ordinary input](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L329-L343).

Ctrl+G starts approval of the displayed proposal; when the owner warns,
Ctrl+Y confirms that exact revision. `# /edit KEY COMMAND` creates another
revision; `# /reject KEY` rejects it. Approval requires a fresh idle prompt
outside bracketed paste; other input disables the displayed approval.
Completed commands return a typed result to the same thread. Unknown
acknowledgments do not cause execution or submission to replay. Sources:
[approval keys](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L257-L328),
[edit and reject](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L355-L394),
[completion match](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L515-L559), and
[shared result client](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/openagents-cli/src/chat_shell.rs#L102-L137).

This client owns its local shell for its own lifetime and starts a fresh
thread per launch. It offers no shell `--thread` or `--continue` option,
detached shell session, or additional coding harness. Supported hooks cover
zsh, bash 4.4 or later, and fish 3.3 or later; other shells retain ordinary
input with requests/proposals disabled. The session permits one request in
flight and at most 256 requests. Scratch real-shell fixtures exercise
edited proposals and result delivery through a fake helper; physical
SSH/tmux use and a signed-in provider remain unverified. Sources:
[request limits](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L395-L400),
[cleanup and shell support](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/src/runner.rs#L570-L601),
[fixture scope](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-tty/tests/plain.rs#L1-L2), and
[remaining physical checks](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/NEEDS_OWNER.md#L30-L32).

### Related native OpenAgents Terminal work

The native `openagents-terminal` app is useful reference material for shell
integration, even though it is a separate surface from the chat TUI.
`terminal-core` owns terminal state, blocks, smart input, and product views;
`terminal-gfx` supplies rendering, local PTYs, and the helper bridge;
`terminal-app` supplies the window. Verse embeds the shared terminal too.
The local adapter runs a real shell through `coder-pty` and supports zsh,
bash, and fish. Blocks retain command, directory, completion, elapsed time,
and bounded output. Sources:
[application boundary](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/application.rs#L51-L114),
[native adapter](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-gfx/src/native.rs#L55-L118),
[shell adapter](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-gfx/src/pty.rs#L165-L304), and
[block retention](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/blocks.rs#L10-L154).

Smart input classifies commands and questions locally, with explicit
overrides. Questions attach shell context to an OpenAgents thread through
the shared helper; the renderer does not own another model loop. Typed
shell proposals bind to the originating request, thread, directory, and
context. Enter confirms; commands that may change state require another
confirmation. Unknown outcomes do not cause automatic replay. Optional
workspace-scoped read-only autorun is off by default. F4 reads the same
thread; Studio and other workbench views project existing resources and
require their own action grants. Sources:
[proposal checks](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/smart.rs#L269-L362),
[question binding](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/smart.rs#L670-L730),
[helper bridge](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-gfx/src/helpers.rs#L14-L76),
[autorun setting](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/autorun.rs#L1-L14), and
[workbench resources](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/docs/terminal/workbench-resources.md#L124-L195).

Current work also adds a private-terminal viewer list and confirmed
watch/drive sharing controls, plus a browser projection of an admitted
terminal session. Browser rendering has retained scratch checks; physical
browser behavior remains unverified. Sources:
[sharing controls](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/sharing.rs#L1-L105) and
[browser receipt](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/docs/terminal/verification/2026-10-06-browser-workbench/receipt.json).

Its local shell sessions are process-bound: hiding a pane preserves it,
but shutting down the pane/application closes local sessions. This differs
from the durable chat threads and detached Coder tasks. The retained Mac
receipt covers a real helper/shell flow with simulated key events and
offscreen rendering, and says signing/publication and further Mac checks
are paused. The Studio receipt uses scratch scripted resources. Neither
establishes that the native app is a published, physically tested daily
replacement. Sources:
[session cleanup](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/crates/terminal-core/src/application.rs#L1378-L1393),
[Mac receipt and release status](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/docs/verse/verification/2026-10-05-smart-terminal/README.md#L3-L75), and
[Studio receipt scope](https://github.com/OpenAgentsInc/openagents/blob/636354ddc0a4aa044f971fafd39b47978fa62658/docs/verse/verification/2026-10-06-studio-workbench/README.md#L3-L25).

## What the history and current code contribute to the new TUI spec

These are evidence-backed questions for the specification, not a finished
design or a demand to reproduce every past feature:

- **Define the intended exit.** Does the TUI replace Claude Code's screen,
  its harness, or its provider dependency? Commander, Dashboard, Khala, and
  Desktop demonstrate how easy it is to achieve the first while claiming more.
- **Make difficult work and long context part of daily use.** Claude's
  subagent/context behavior in 243 and its reliability in 285 explain why
  an attractive replacement can still lose the tasks that matter.
- **Treat stop, queue, steer, and new chat as essential behavior.** The
  recurring failures in 246, 250, 254, and 255 directly obstruct the cutover.
- **Separate the screen's lifetime from the task's lifetime.** AutoDev's
  unmounted chat, interrupted OpenCode work, and orphaned Sarah delegations
  make continuity a repeated failure boundary.
- **Show the actual executor, model, account, and capacity.** Probe's account
  failover work and Desktop's mislabeled/unpinned model and exhaustion errors
  show why a single text box still needs trustworthy operating information.
- **Keep work inspectable.** Hidden commands, opaque subagents, missing
  histories, and unearned completion recur from Faerie through Gym.
- **Prove sustained replacement with representative work.** Partial usage,
  an installable client, a successful demo, green self-tests, and a tuned
  benchmark win establish different things. The next spec needs an explicit
  criterion for completing ordinary and difficult work without being forced
  back into the old product.
- **Specify which current foundations to reuse.** The public Coder toolkit
  already supplies a composer and typed transcript components; OpenAgents
  supplies durable threads and task integration; sibling Coder supplies a
  richer session owner, image input, attached clients, and child control.
  They have different ownership and persistence contracts.
- **Make queue and stop semantics explicit.** The small shell replaces its
  one queued draft; OpenAgents rejects an ordinary submission while a reply
  streams; sibling Coder holds prompts in FIFO order; closing an OpenAgents
  run view leaves work running. The next spec should
  name the desired behavior and acceptance cases for each state.
- **Check availability through the whole path.** A visible plugin catalog,
  retained dispatcher, slash-command name, or foreign-agent adapter is not
  proof that the active model can invoke it or that recovery and permission
  semantics match the native path.

The catalog groups interface revisions and supporting infrastructure with
their parent attempts. Payments, marketplaces, training, political statements,
gaming, and other archive topics are included only where they directly explain
an alternative coding workflow or its dependency. This avoids counting every
adjacent announcement as another failed replacement.
