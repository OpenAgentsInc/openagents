# Historical attempts to replace Claude Code

Research snapshot: October 6, 2026. This catalog is background for a new TUI
specification. It covers the retained `docs/transcripts/001.md`–`289.md`
archive and the removed, unreleased Gym preparation session linked by the
[archive index](../../docs/transcripts/README.md).

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
establishes that Claude Code dependence has permanently ended. Later source
code, measurements, and terminal work are outside this transcript catalog.

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

## What this history contributes to the new TUI spec

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

The catalog groups interface revisions and supporting infrastructure with
their parent attempts. Payments, marketplaces, training, political statements,
gaming, and other archive topics are included only where they directly explain
an alternative coding workflow or its dependency. This avoids counting every
adjacent announcement as another failed replacement.
