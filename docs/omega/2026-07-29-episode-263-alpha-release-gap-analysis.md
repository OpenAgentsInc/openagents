# Episode 263 Omega alpha release gap analysis

- **Date:** 2026-07-29
- **Decision:** no-go for the Episode 263 alpha promise today
- **Transcript authority:** the owner-locked Obsidian script at
  `/Users/christopherdavid/Documents/APEX REMOTE/EP263.md`
- **Application inspected:** `OpenAgentsInc/omega` at
  `1d94636b6f9fecad0aeeb825d3f74eaf23e83d9c`
- **OpenAgents source inspected:** `OpenAgentsInc/openagents` at
  `14d94935001b2237621d5f9aabac9631fe91568a`
- **Document type:** gap analysis and release work packet. This document does
  not admit a public claim, promote a build, or change runtime authority.

## 1. Executive verdict

Episode 263 promises three first-class ways to use Omega:

1. command Codex, Claude Code, Grok Build, and other ACP agents directly;
2. command Omega Agent and let it select the right executor; and
3. command Sarah by voice as a personal lieutenant.

The default installed product does not yet deliver any of those three paths
at the level the transcript describes.

- Direct external-agent selection is intentionally disabled. Codex, Claude,
  and Grok remain detected and attachable under the surface, but the public
  selector says direct selection is coming later. The separate external-agent
  menu still renders, but Zero Base clamps the resulting new thread back to
  Omega's native agent.
- Omega Agent has a real routing seam, route journal, disclosure model, and
  executor inventory. Its current public path nevertheless attaches no
  external executor and sends an unpinned new conversation to the native loop.
  That is not yet “smartly route your request to the right agent.”
- Sarah voice has substantial implemented client and gateway machinery, but
  the managed service is off by default, admission is limited, and the Sarah
  menu entry in Zero Base targets a panel that Zero Base does not load. The
  composer voice controls are narrower than the “personal lieutenant” claim.

The release should therefore stop at **no-go** until the work in section 6 is
complete and an installed candidate passes section 7. The good news is that
this is a restoration and integration program, not a greenfield agent build.
Most of the direct-agent, routing, Sarah, channel, and release machinery
already exists.

## 2. The product decision that changed

The mismatch is a consequence of a recorded product decision.

`crates/omega_zero_base/src/omega_zero_base.rs` makes Zero Base the no-argument
default, describes it as one thread and nothing else, and provides no runtime
way out. The full editor is available only through `--full-editor`. The
2026-07-29 single-experience plan then proposed removing the full editor around
Zero Base permanently.

The owner's current direction supersedes that release shape: Omega must again
let people choose and command agents directly, not only talk to Omega Agent.
That direction should be recorded in Omega's product delta ledger before the
implementation lands. Otherwise the code will continue to enforce two
contradictory owner decisions.

The recommended release shape is an **Agent Workroom**, not an unconditional
return to inherited Zed complexity:

- keep the focused conversation-first shell;
- make the three Episode 263 modes persistent, visible, and functional;
- make Files, Search, Git, Terminal, Review, and the editor reachable when the
  task needs them;
- retain Zero Base as an optional focus or diagnostic mode if it remains
  useful, but do not make its one-agent clamp the normal product contract; and
- never render an enabled-looking control whose action is refused, whose panel
  is absent, or whose result opens invisibly.

## 3. Claim-by-claim gap ledger

| Episode 263 claim | Current application truth | Release state | What must become true |
| --- | --- | --- | --- |
| “The first alpha build of Omega is ready.” | Omega has published prerelease candidates through `v0.2.0-rc26`, but no installed-candidate receipt proves the complete Episode 263 journey. The public site still calls Omega “In development” and says it is not available yet. | **Red** | Cut a new candidate only after this ledger is green. Record exact source, artifact digest, signature/notarization state, platform, installed journey, reviewer, and known limits. |
| “intended for experienced developers and coding agent power users” | The audience matches the current rough-edge state. | **Green as positioning** | Put the alpha/prerelease warning, supported platforms, prerequisites, support boundary, and data-risk warning beside every download. |
| “command all of the top coding agents simultaneously” | Omega can run several threads and has parallel delegation machinery. The default product does not let a person start independent direct conversations on the advertised harnesses. | **Red** | Start at least two different direct-agent threads, observe both streaming independently, switch between them, cancel either one without affecting the other, and preserve each thread's executor identity. Define “simultaneously” as concurrent threads, not one prompt broadcast to every agent, unless broadcast is deliberately implemented. |
| “Command agents directly … seamlessly switching between … Codex, Claude Code, Grok Build, and any agents that speak ACP” | The fork detects Codex, Claude, and Grok and has ACP attach paths. The public executor menu disables all three with `Coming soon — selectable in an upcoming version`. In Zero Base, `selected_agent()` converts a new non-native selection to `NativeAgent`. “Add More Agents” dispatches an ACP Registry action that Zero Base refuses and opens into a centre surface that can be invisible. | **Red** | Restore direct selection for installed agents, a working ACP registry, explicit unavailable/auth states, new-conversation switching, stable thread restore, and executor disclosure. Prove Codex, Claude Code, Grok, and one additional ACP agent from clean installed builds. |
| “Command our Omega Agent who will smartly route your request to the right agent” | The router type, journal, fallback reasons, and disclosure exist. The current server calls `attach_plan(None, ...)`; the plan attaches no external agents and the unpinned routing law selects the native loop. Older product documentation also warns that identity alone is not routing. | **Red** | Admit a deterministic routing policy that uses task requirements and live executor readiness, attaches the eligible executors, records the decision, shows the selected executor before or at dispatch, supports an explicit override, and fails visibly. Prove at least native, external ACP, and any claimed engine lane with positive and negative fixtures. |
| “Command me, Sarah, acting as your personal lieutenant” | Sarah has a managed realtime voice gateway and bounded editor commands. In Zero Base, the `Sarah` new-thread item dispatches `workroom::OpenPanel`, but that panel is not loaded and the action is refused. The composer admits voice controls, but version 1 cannot run shell, Git, network, delete, move, payment, or arbitrary agent/model selection. | **Red** | Give Sarah a working first-class entry, publish the exact capability boundary, support the intended owner/user population, and make every action either execute with a receipt, ask for visible confirmation, or refuse with a useful reason. Do not imply general executive authority from a bounded editor-command bridge. |
| “advanced mode requiring voice commands and expensive credits” | Voice is real and metered, with a credit hold and exact usage settlement. The managed service is off by default. The only unmetered exception is a bounded staging-owner record. The client still needs a complete user-facing admission, price, hold, remaining-credit, exhaustion, refund/release, and reconnect story. | **Red** | Enable the intended alpha cohort, show the exact credit hold and effective rate before starting, disclose the session limit, show remaining credit and final charge, and prove `402`, interruption, reconnect, expiry, settlement, and revocation paths. Replace the subjective word “expensive” with exact pricing in product UI and release notes even if the spoken line remains. |
| “tester channels you’ll find in the sidebar of Omega” | The sidebar has a collapsed section titled `Channels`. It adapts the one published Agent Chat manifest into one public channel. It is not a plural tester-channel registry, and the release-candidate support destination is not identified in the UI. | **Red** | Publish a signed/versioned channel registry with at least the exact alpha feedback destination, label it `Tester channels`, make it visible on first launch, prove read and send from the installed candidate, provide moderation/reporting rules, and preserve a support fallback when the relay is unavailable. |
| “Find the download links on our website, openagents.com.” | `openagents.com/download` currently serves **OpenAgents Desktop 0.1.0-rc.25**, not Omega. The homepage says Omega is in development and not yet available. GitHub has Omega prereleases, but that does not make the website statement true. | **Red** | Add Omega as an explicit product in the signed release resolver or provide an equally clear Omega download route. Never overwrite the supported Electron Desktop identity. The page must show Omega version, channel, platform, architecture, format, minimum OS, digest/signature truth, limitations, and unavailable targets. |

## 4. What already exists and should be reused

Do not rebuild these systems under new names.

| Existing capability | Reuse in the release path |
| --- | --- |
| External ACP registry, installed-agent detection, agent-specific attach commands, thread restore, and auth surfaces | Direct Agent mode |
| Codex, Claude, and Grok adapter identities plus background warmth management | Fast direct-agent startup and routing inventory |
| `OmegaAgentConnection`, deterministic route types, journal, fallback reasons, and disclosure | Omega Agent mode |
| Native loop plus delegated Codex and Claude execution | Router fallback and early multi-agent acceptance fixtures |
| Managed Sarah Realtime gateway, one-use tickets, credit holds, usage settlement, tool schema, and confirmation handshake | Sarah mode |
| Persistent sidebar, recent threads, channel controller, NIP-29 view, relay reconnect, and bounded media handling | Tester channels |
| Signed ReleaseSet resolver, `/download`, changelogs, updater, and rollback contracts in OpenAgents | Omega distribution, without creating a second unsigned download truth |

## 5. Required product behavior

### 5.1 One truthful front door

A clean launch with no flags must expose these primary actions without a
command palette or hidden settings:

1. **Direct Agent** — choose an installed harness or add an ACP agent.
2. **Omega Agent** — let Omega route, with the selected executor disclosed.
3. **Sarah** — start the voice-first advanced mode with admission and credit
   truth shown before the microphone opens.

The selected mode belongs to the conversation. Switching means creating or
opening a conversation on another executor, not replacing the executor under
an existing transcript.

Every row must have one of four honest states: ready, setup required,
temporarily unavailable, or not supported in this build. “Ready” cannot mean
that the binary was merely found; connection and session creation must pass.

### 5.2 The work surface must support the promise

The current shell renders controls whose dependencies are not loaded or whose
actions are refused. Before release:

- Files, Search, Git, Terminal, Review, Settings, Add More Agents, and Sarah
  must either work or be removed from the release candidate;
- a file, skill, outline artifact, or registry action must never open into an
  invisible centre pane;
- disabled controls need a visible reason and setup action where applicable;
- keyboard focus, accessible names, selection state, busy state, failure
  announcements, and reduced-motion behavior need an installed UI pass; and
- narrow-window behavior must keep the composer, Send/Stop, mode identity,
  and sidebar recovery controls reachable.

### 5.3 Identity and authority must not blur across modes

- Direct agents keep their own credentials, configuration, billing, memory,
  permission policy, and session history. Omega must not copy those into an
  OpenAgents account.
- Omega Agent owns routing, disclosure, and route receipts. The selected
  executor owns execution and must be named.
- Sarah's identity does not grant her action authority. Each durable or
  sensitive action still passes its specific confirmation and receipt gate.
- A missing service or adapter fails closed and never silently substitutes a
  different agent or model.

## 6. Release work packets

### P0 — must finish before the Episode 263 alpha release

#### EP263-01: Supersede the one-agent Zero Base contract

- Record the owner's new decision in `OMEGA_DELTAS.md` and the relevant Omega
  product documents.
- Choose the no-argument launch shape and remove contradictory prose, tests,
  and gates.
- Recommended result: Agent Workroom is the default; Zero Base survives only
  as an explicit focus/diagnostic launch if still needed.
- Acceptance: a code search and product-doc review find one answer to what a
  normal launch exposes.

#### EP263-02: Restore the three-mode new-conversation experience

- Add persistent Direct Agent, Omega Agent, and Sarah entry points.
- Remove the Zero Base new-thread clamp for an explicit direct-agent choice.
- Make the selected mode, executor, project, and readiness visible before send.
- Preserve the rule that an existing conversation does not change executors
  underneath its transcript.
- Acceptance: keyboard-only and pointer journeys create one conversation in
  each mode from a clean profile.

#### EP263-03: Restore direct ACP agents

- Enable ready Codex, Claude Code, and Grok rows.
- Make Add More Agents open a visible, functional ACP registry.
- Keep each harness's native auth/config flow and surface exact setup errors.
- Preserve executor-specific thread restore across relaunch.
- Prove a generic ACP agent in addition to the three named harnesses.
- Acceptance: the transcript title, composer label, disclosure, process, and
  resumed session all name the same selected agent; no path silently lands on
  Omega Agent.

#### EP263-04: Make Omega Agent actually choose an executor

- Define the first release routing inputs and deterministic priority law.
- Attach every executor the release claims can be selected by the router.
- Add visible route disclosure and a user override for a new conversation.
- Keep bounded fallback reasons and journal persistence.
- Acceptance: installed fixtures route different requests to at least two
  executor classes, reproduce the same decision from the same inputs, and fail
  visibly when the chosen executor disappears.

#### EP263-05: Admit Sarah voice for the alpha cohort

- Replace the broken Sarah panel action with the actual voice-mode entry.
- Define who can use it, how sign-in/device binding works, and what happens for
  a Nostr-only identity.
- Put price, hold, session limit, confirmations, transcript storage, and exact
  capability limits in the UI.
- Decide whether “personal lieutenant” is accurate for the bounded v1 command
  set. Expand capability or narrow the release copy; do not rely on persona to
  imply actions that cannot run.
- Acceptance: one installed, non-owner alpha account completes session start,
  voice turn, confirmed command, agent-thread start, reconnect, end, charge,
  and transcript recovery without operator intervention.

#### EP263-06: Ship tester channels in the sidebar

- Replace the one-channel compatibility adapter with a versioned channel
  registry or explicitly label the one supported tester room in singular.
- Make the alpha feedback destination expanded or otherwise unmistakable on
  first launch.
- Include posting, reconnect, moderation/reporting, privacy, and unavailable
  state tests.
- Acceptance: a clean installed candidate can read and send a test message,
  and a second account can receive it in the documented tester destination.

#### EP263-07: Publish Omega-specific download truth

- Create an Omega ReleaseSet/product identity rather than reusing the current
  OpenAgents Desktop page ambiguously.
- Keep the existing Electron application supported until an explicit cutover;
  the two products need distinct names, versions, packages, update feeds, data
  roots, and rollback instructions.
- Publish only platform cells with signed installed evidence.
- Acceptance: `openagents.com` leads an alpha tester to the exact verified
  Omega artifact and never labels the legacy Desktop artifact as Omega.

#### EP263-08: Canonicalize Episode 263

- **Transcript archive complete:** `docs/transcripts/263.md` contains the final
  spoken transcript and video-master digest. The displaced plugin-and-payments
  draft is unscheduled at `docs/transcripts/26X-omega-agent.md`.
- Reconcile the truth ledger, release notes, product UI, homepage, and download
  page with the canonical transcript before publication.
- Acceptance: the repository, video master, caption/transcript artifact,
  release notes, homepage, download page, and in-app wording make the same
  claims.

### P1 — required release hardening

#### EP263-09: Multi-agent supervision and worktree safety

- Show running, waiting, failed, completed, and cancelled state per thread.
- Prevent two write-capable agents from silently colliding in one worktree, or
  warn and require an explicit choice.
- Preserve queued input, cancellation, restart recovery, and independent
  history for concurrent agents.

#### EP263-10: Installed UX, accessibility, and failure pass

- Test first launch, no project, missing binary, expired auth, adapter crash,
  offline service, exhausted credit, relay outage, narrow window, keyboard
  navigation, screen-reader announcements, and reduced motion.
- Remove every enabled-looking no-op. A visible control must have an admitted
  action, loaded dependency, and visible result.

#### EP263-11: Alpha feedback operations

- Publish triage ownership, severity definitions, response expectations,
  privacy guidance, and a safe way to attach logs.
- Add a release-candidate identifier to feedback so reports bind to the exact
  build and mode.

## 7. Installed-candidate release gate

Source tests are necessary and insufficient. Use a packaged candidate from a
clean profile and preserve evidence for every row.

| Gate | Required observation |
| --- | --- |
| Launch | No flags; the user can identify and reach all three modes. No hidden `--full-editor` prerequisite for the advertised journey. |
| Direct Codex | Existing Codex account/config is used without copying secrets; send, stream, tool use, stop, resume, and relaunch pass. |
| Direct Claude Code | Same journey and identity checks as Codex. |
| Direct Grok Build | Native ACP server launches with the shipped command order; send, stream, stop, resume, and relaunch pass. |
| Generic ACP | Add one registry agent, authenticate if required, run a turn, and restore it. |
| Concurrency | Run two different direct agents concurrently; switch views and cancel one while the other continues. |
| Omega routing | Two materially different fixtures choose different eligible executors; decision, reason, override, fallback, and disclosure are visible and durable. |
| Sarah | Eligible alpha user sees price/hold/limit, starts voice, confirms a bounded command, starts an agent thread, survives a transient reconnect, ends, and sees the final charge. |
| Channels | Tester destination is visible; send, receive, reconnect, moderation/reporting, and outage fallback pass. |
| Work surfaces | Every visible Files/Search/Git/Terminal/Review/Settings/agent-registry/Sarah action either works or is visibly disabled with a reason. |
| Update safety | Install beside existing Zed and OpenAgents Desktop; upgrade, rollback, and uninstall touch only Omega paths. |
| Distribution | Download page identity, version, platform, artifact digest, signature/notarization state, release notes, and installed binary agree. |
| Independent review | A reviewer other than the candidate producer repeats the held-out journey and records the verdict. |

The release is green only when every P0 packet is complete, every retained
spoken claim has a passing installed observation, and no known release-blocking
failure remains. If schedule wins over implementation, cut or narrow the
corresponding spoken line; do not promote a candidate on the strength of
source shape or hidden capability.

## 8. Evidence map

### Locked transcript and conflicting archive

- `/Users/christopherdavid/Documents/APEX REMOTE/EP263.md`
- `docs/transcripts/263.md`
- `docs/transcripts/26X-omega-agent.md`

### Current Omega implementation

- `crates/omega_zero_base/src/omega_zero_base.rs`
- `crates/zed/src/main.rs`
- `crates/zed/src/zed.rs`
- `crates/agent_ui/src/agent_panel.rs`
- `crates/agent_ui/src/omega_executor_selector.rs`
- `crates/agent_ui/src/omega_router.rs`
- `crates/agent_ui/src/omega_sidebar.rs`
- `crates/agent_ui/src/omega_public_channels.rs`
- `docs/src/development/omega-native-agent.md`

### OpenAgents contracts and current public state

- [Zero Base mode audit](./2026-07-29-omega-zero-base-mode-audit.md)
- [Zero Base single-experience plan](./2026-07-29-omega-zero-base-single-experience-plan.md)
- [Omega Agent roadmap](./2026-07-25-omega-agent-roadmap.md)
- [Managed Sarah Realtime voice gateway](./2026-07-28-managed-sarah-realtime-voice-gateway.md)
- [Sarah voice entitlement plan](./2026-07-28-sarah-voice-unmetered-entitlement-plan.md)
- [Desktop release contract](../deploy/openagents-desktop-cross-platform-release.md)
- `https://openagents.com/`
- `https://openagents.com/download`
- `https://github.com/OpenAgentsInc/omega/releases`

## 9. Recommended order

1. Record the superseding product decision and canonical Episode 263 script.
2. Restore the three-mode front door and direct-agent selection.
3. Make the router choose among real attached executors.
4. Repair and admit Sarah for the intended alpha cohort.
5. Publish tester-channel and Omega-specific download truth.
6. Run the installed matrix, fix every red observation, and cut a new
   prerelease candidate.
7. Reconcile every spoken line against that exact candidate before publication.
