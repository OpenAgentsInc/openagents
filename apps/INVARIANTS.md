# Invariants — apps/INVARIANTS.md

> Scope: Desktop, iOS, web, CLI, Coder, Zeron, components, Metaverse OS (5 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## Coder reads foreign conversations as bounded inert data

`coder_session_read` and `coder session-read` share one typed request and
read-only Claude JSONL implementation (`coax/lib/agent/observe/claude-history.coax`,
`coder/tui/session-reader.coax`). A recovered name or instruction never
dispatches a tool or starts/resumes an engine. System/attachment payloads,
thinking and signatures are not emitted; UUID/parent edges of unfamiliar
records may bridge real messages without interpreting their payloads.
Compaction/snips and parallel results determine the recoverable branch.
Malformed input, missing parents, cycles, unavailable output and bounded
omissions remain observable. Exhausted graph budgets and changing file
snapshots cannot publish a silently partial branch. Pages have explicit
boundaries and a decreasing earlier-page cursor. This is a local data
reader, not a new permission grant or a sanitizer for arbitrary retained
text. Profile and limits: `coder/book/ch014-forks-subagents-and-jobs.md`.
Every new Coder Codex thread advertises this same reader directly through
app-server's dynamic tool contract. Calls pass existing hooks and ceiling
decisions; they do not inherit a new filesystem permission. CLI, MCP and
Codex dispatch share the typed implementation in the running binary.
Generated compaction summaries on the recovered branch are available only
in the explicit `context` view, labeled inert context, with UTF-8-safe
parts and bounded pages. They never replace the last human request or
become executable historical calls. Ambiguous/incomplete native ID-prefix
searches refuse selection.
Checks: `coder/tui/session-reader-checks.coax` (branch/compaction/snips,
parallel recovery, non-rendered attachment links, UTF-8 and bounded pages),
`session-reader-registration-checks.coax` (shared catalog and typed
requests), and `session-reader-oracle.py` (independent graph and native
MCP/CLI comparisons and a fresh actual Coder Codex chat; only aggregate
observations leave its private inputs).

## Voice: audio leaves the phone only inside a turn, and only to our voice host

The schema is `proto/openagents/voice/v1alpha1/voice.proto`; one `Frame` is one
WebSocket binary message, encoded and decoded only by the generated Coax codec
(`metaverse-engine/net/voice-wire.coax`), with no second hand-written codec.

- **Turns are Coax decisions.** `metaverse/voice.coax` decides when a turn starts
  and ends, barge-in and the media epoch. The phone sends uplink audio only for
  frames the engine marks as part of a turn (`OA_EV_FORWARD`) plus the turn's
  pre-roll. Silence between turns never leaves the phone.
- **Decisions are never awaited.** Audio flows under the last decision. A
  `FrameDecision` from any decider applies only within its stream and media
  epoch and before its deadline, and every numeric field is clamped. A slow or
  missing decider can fail to steer audio, but can't stall it.
- **Who may be heard and what speech may cost are `#authority`.**
  `metaverse/voice-floor.coax` (consent, input floor, routes by explicit binding
  only, never by reading the words) and `pay/voice-meter.coax` (free tier
  and price; no provider call for a turn the meter refuses) run with zero
  surviving mutants.
- **Only the voice host calls a speech provider.** The phone and web clients
  never hold Google credentials. The host never logs tokens or audio. Renders
  are cached by SHA-256 of profile, revision, text and rate.
- **The microphone is opt-in.** Phone/web conversation capture opens only after
  the person turns on the voice toggle and sees that Google processes speech.
  Coder for Mac has a separate, default-off **Local Dictation — Parakeet**
  setting and macOS permission. Its microphone opens only while both the app
  and its composer window are active. Audio stays in memory on this computer;
  no speech provider, audio file or telemetry receives it. Focus/consent loss
  stops capture and invalidates the request. Loaded model residency is separate
  from microphone admission. `openagents dictate` is a third, per-command
  opt-in: running it is the consent for one dictation. Its microphone opens
  only after the local model is ready and closes once the clip is captured;
  Enter, Escape, Ctrl-C, `--seconds` or the 60-second cap end it. It prints
  the final text (or one JSON result) by default and never pastes, types, sends or writes
  audio. A named input or recognition device never falls back to another.
  Coder's terminal composer is a fourth, default-off path (`--dictation`,
  `CODER_DICTATION=1` or `/dictation on`): its gesture (Ctrl+Space unless
  `--dictation-key`) runs `openagents dictate --events` as a helper, so the
  microphone and the model never live in Coder's process. A toggle, never a
  hold: terminals don't report key release. The final text enters the draft
  only through the `#authority` `coder/tui/dictation.coax` guard: the same
  draft, cursor, editor generation, send count, navigation, focus and consent
  as when the request began; valid UTF-8 whose only control byte is a line
  feed; one undoable insertion. Enter, Esc, Ctrl-C, Ctrl-D and a focus-out
  report cancel an open request and send nothing; any other key cancels it
  and then acts as usual. Nothing on this path calls the send path.

The Metaverse Engine for Mac preflights OS microphone authorization in visible foreground
setup, or at foreground launch for an existing setup receipt. Its
`apps/metaverse-engine-desktop/microphone.coax` authority requests only an undetermined
grant, never from automated background checks or while another request is pending.
Setup opens no microphone stream. After consent, `voice-ready.coax` separately
admits resident Parakeet and local transport preparation for a visible local
page, excluding ordinary diagnostic launches and duplicate preparations. Idle
preparation sends HELLO/WELCOME, never microphone audio or a voice turn, and its
worklet runs one silent preparation quantum, then stays suspended without a
microphone source. V/TALK checks the current OS grant
again, sharing any pending authorization request; capture still requires the
visible foreground world and a current native ticket. Denied/restricted permission
offers Settings. Permission is never inferred from saved setup. The permission
gate kills all 12 mutants; setup and native WebKit evidence is in the desktop README.
The voice-readiness gate kills all six mutants. Native warm checks observe the
real protocol and Parakeet with synthetic media, including repeat holds,
cancellation, connection recovery and the unprepared path.

Everglade's V/TALK explicitly selects hold-to-talk in the shared Coax engine.
Quiet onset and pauses belong to that held turn; ordinary VAD silence cannot
finalize a fragment and dispatch Coder while the gesture remains held. Release
stops microphone tracks immediately, drains already-posted worklet samples and
sends one TURN_END. Cancellation and the configured turn cap remain effective;
a capped hold cannot restart until another explicit press. Init preserves the
automatic conversation default for other clients. This is an additive C/Wasm
entry point; the voice wire contract is unchanged. Native diagnostics timestamp
capture phases/reasons without recording audio or transcript content, and keep
one previous voice-host log across launches.

Coder dictation's `apps/shared/dictation-policy.coax` is `#authority`: it gates
readiness/consent, 450 ms pre-roll, 300 ms minimum hold, 200 ms tail, a 60-second
capture cap, silence and current-ticket final publication. `dictation.coax`
reuses the Metaverse's resampling/VAD and plans complete nonoverlapping Parakeet slices:
at most three, each cut in a pause before its limit (the quietest-boundary
rule `metaverse/asr-cut.coax` shares with the voice host), with 300 ms of silence
the host adds before a later slice and after an earlier one inside the model's
20.47 s; the silence is never part of the clip (#12694). The Mac C bridge
forwards those Coax counts and the Swift worker adds exactly that padding;
it checks the current ticket between calls. Exact call-payload/cancellation,
native sanitizer, recorded Metal and rebuilt offscreen app evidence is in
`docs/verification/dictation-eval-2026-11-02/README.md` (2026-11-03 follow-up).
The desktop insertion authority holds the original composer text, selection
and send count. It admits one complete UTF-8 insertion only for the current
unchanged draft; edits, cancellation, IME and navigation invalidate it. Literal
newlines stay draft bytes. It never calls a send operation. Local device choice
is explicit and persisted by stable CoreAudio UID; a missing explicit input never
falls back to another microphone. Device/configuration changes stop the capture
generation, clear audio and cancel pending insertion. The shared
`apps/shared/dictation-device.coax` policy admits exact selected/default identity
and exact supported rates, channels and planar/interleaved strides; late callbacks
cannot enter a replacement generation. Checks and explicit
hardware limits: `docs/verification/coder-dictation-2026-11-02/README.md`.
Code-aware text and vocabulary (#12670) are opt-in per command (`--format
code`, `--terms FILE`). `apps/shared/dictation-text.coax` (`#authority`)
only rewrites the draft's words. It never changes a word with a digit, never
reads a word as a command, and its only possible control byte is the line
feed of "new line", inserted as text. A refusal keeps the raw transcript.
`psionic/lib/parakeet/vocab.coax` (`#authority`) replaces a span only when
Parakeet's own transducer scores the term within its bias. A span with a
digit, or one differing from the term only in case, is never a candidate.
The result keeps `raw_text`.
The CLI links the same controller unchanged; its own choices (backends,
devices, duration, keys, a stalled input, results and exit codes) are the
`#authority` `apps/openagents-cli/dictate-rules.coax`. Its checks inject
recorded audio through a test capture backend and never open a microphone:
`docs/verification/dictate-cli-2026-11-02/README.md`.

Model-boundary exception: Debug builds accept `-OAVoiceFixture` (a bundled
recording through the same uplink path as the microphone) and
`-OAVoiceObserve`; Release builds compile both out.

Check: `metaverse/test_voice.sh` (ASan/UBSan; every frame kind round-trips and
decodes with protoc; turns, barge-in, decisions, jitter buffer, fixtures);
`coax test metaverse/voice*.coax`; `coax mutate` on floor and meter (0
survivors); `metaverse/voice-host/test.sh` (offline, and live Google);
`apps/openagents-ios/tools/voice-sim-test.sh` (the simulator UI test, offline
and live).

## The Metaverse Engine pauses the world view after its first rendered frame

`apps/metaverse-engine-desktop/world-visibility.coax` admits the first real rendered
frame even with setup open, then pauses the world's loop while setup/status
covers it or its window is hidden, minimized or occluded. The browser host
cancels the animation callback and suspends realm audio; socket delivery and
the separate native room continue. Revealing the world schedules one loop;
Reload performs a real navigation and reacquires first-frame readiness.
The visibility authority kills all four mutants, including its 32-case fact
matrix. The visible `--setup-profile` measures process CPU, GPU submissions,
room delivery and idle panel draws through setup, status, reload, minimize,
hide and resume.

Setup and status paint through `components/zeron/gpui.coax` on the shared Mac
host (`apps/shared/host/apple/ui-host.m`, with the CoreText atlas in `ct-atlas.m`;
since 2026-11-11, GitHub #13156, the Metaverse Engine runs on that host as OpenAgents and
Everglade do). Coax owns labels, geometry, readiness and button rectangles, and
`fgu_hit` decides which button a press and its release land on; the host's
`UiApp.hit` hands every other point to the world view under the panel. The
`--shot` pictures of the setup and status panels and of the message feed match
the former Swift host's own drawing pixel for pixel. The setup buttons are no
longer AppKit buttons. Since GitHub #13180 the shared host gives VoiceOver every
row of an app's element table (`components/core/elements.coax`, read through
`UiApp.drive`'s `tree`): each row is an `NSAccessibilityElement` under the host's
view, with its stable id as the identifier, a role from the row's role, its label,
the value and enabled state the app's optional `about ID` gives, and AXPress as the
app's `tap ID`. Tab and Shift-Tab move focus through the rows that can be pressed or
typed into; Space and Enter press the focused one. The Metaverse Engine's panel fills that
table in Coax (`chrome.coax` `fui_elements`: `setup.open`, `setup.refresh`,
`setup.verify-voice`, `setup.folder`, `setup.microphone`, and the panel's status
lines). A disabled button refuses AXPress, as it refuses a click.
Since GitHub #13182 the host draws a focus ring (`apps/shared/host/focus-ring.h`,
one bordered quad in the components' vertex format, `UiApp.ring`'s colour or
Zeron's accent) around the focused row only after a key moved focus; a pointer
press or VoiceOver's own move hides it. A window no one sees (offscreen, `--hidden`,
or a drive socket listening) never opens a panel or a menu: a file chooser waits
for the driver's `choose PATH` or `cancel`, and a Metaverse Engine `--shot` press
says what it would open instead of opening Finder, Settings or the world.
`apps/shared/host/apple/a11y-check.sh` checks the window's tree, the ring and the
chooser headlessly; the desktop goldens `focus-button`, `focus-composer` and
`choose-folder` show them. The surfaces redraw on changes, with no idle animation loop. The game's own HUD is unchanged. CPU measurement converts
`proc_pid_rusage` Mach ticks to nanoseconds and is checked against the independent
POSIX process CPU clock before any numbers are recorded.

## Linux & Vulkan Host Contract (2026-11-05)

Linux and Metal admit action outboxes with the same `hap_span` authority
before publishing a nonempty view. Empty packets need no pointer/capacity
read; native missing capacity refuses. Linux copies before acknowledgement,
then executes the effect or reports 604 on a later poll. Actual normal and
ASan/UBSan/LSan Linux runs pass all 110 common hostile-span/acknowledgement/
continued-frame words. The Metal runtime gate is separate. See `docs/verification/host-contract-2026-11-05/linux-action-packets.md`.

The typed Vulkan loop dispatches real input through the realm gesture/audio
boundary and consumes the checked Wayland Unicode/modifier companion. Its
next-completed-frame HTTP32 stage retains concrete asynchronous ownership
before the JPEG capture and helper start. Coax owns JPEG/base64 and the
Unicode JSON/authorization composition. Actual normal and fully instrumented
producer/SDK physical requests preserve all 13 peer facts under the unchanged
five-second deadline; codec/observer root gates kill 100/44 mutants. Scaling,
ordinary window/input/room/audio composition and final default remain
separate exits. See `docs/verification/host-contract-2026-11-05/linux-frame-services.md`.

The ordinary Vulkan v1 adapter prepares the checked graph targets and actual
UNORM display before advancing its producer. Device replacement preserves
the producer, immutable original package identity, retained CPU buffers and
bounded decoded image cache; GPU resources retire before their physical
device closes. The replacement uploads complete retained buffer spans and
does not acknowledge a producer update merely to recover rendering. Actual
90-frame world recovery compares every FP16 graph and UNORM display byte
without an intervening producer call, retires a measured old handle and
closes all GPU/CPU owners. Its observer kills 30 root mutants. The complete
current producer and SDK also pass ASan/UBSan/LSan on a two-worker Lavapipe
device with Vulkan validation, within the existing 810-second qualifier
budget; earlier software submission/total-runtime failures remain retained.
The product's submission fence is unchanged. See
`docs/verification/host-contract-2026-11-05/linux-world-recovery.md`.

## OpenAgents Desktop worker mounts core tools and workspace awareness unconditionally on every normal turn

`coder/tui/oa-worker.coax` never strips the 8 core tools (`Read`, `Edit`, `Write`, `Bash`,
`Glob`, `Grep`, `TodoWrite`, `WebFetch`) or the `Working directory:` workspace context based
on heuristic keyword matching or message-text classification (`fww_turn_no_tools` returns
`true` strictly for explicit `force_wrapup_once` wrap-up turns). Optional heavy packs
(`Coax*` compiler-intel tools, `Wallet*`, `OpenAgentsCli`, `MetaverseRoute`, `GameBuilder`)
remain conditionally mounted by workspace/environment availability, while conversational
restraint is handled via system-prompt guidance rather than tool lobotomy. Auto-mode
System One routing (`fww_route_auto_plan_ex`) always receives the detected enclosing git
repository name (`fww_detect_repo_name`, resolving linked worktree `.git` pointer files) so
natural-language workspace prompts inside a repo are never misrouted as non-repo general
chat.
Checks: `coax/bin/coax test coder/tui/oa-worker.coax --library` (wired into
`evals/openagents-desktop/run.sh --tier 0`) and Tier 2 live regression task
`evals/openagents-desktop/tier2/tasks/f5_summarize_open_issues_no_jargon.json`.



