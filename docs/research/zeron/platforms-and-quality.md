# Platforms and quality gaps

Sources are keyed in [scope and evidence](scope-and-evidence.md). Packaging,
native interaction, accessibility, and performance are part of desktop parity.
Source inspection cannot substitute for installed-app or physical-device checks.
No such checks were run for this documentation task.

## Platform comparison

The table distinguishes implemented paths from remaining validation. It does
not certify every current release artifact or provider/platform combination.

| Surface | Zeron baseline | OpenAgents baseline | Parity consequence |
| --- | --- | --- | --- |
| Desktop shell/chat | macOS, Linux, Windows paths | macOS, Linux, Windows paths | Compare installed apps and the expanded surface set per OS. |
| Files, diff/history, PRs | Shared native UI and owner-routed engine operations; native-specific mutation behavior | Task diff/review/Publish; no equivalent file/editor/history/PR workspace | New functionality must work on every claimed desktop OS, including remote owners with different path syntax. |
| Terminal | Alacritty-backed, owner-hosted, tabbed | `coder-vt`, window-local pane; reusable host PTY protocol | Match practical terminal behavior and expose the remote/tabbed lifecycle. Different emulators alone are not a gap. |
| Embedded browser | macOS WebKit and Linux WebKitGTK helper; Windows uses the external browser | External links and separate browser tools, no equivalent embedded surface | Windows embedded browsing is not a Zeron parity requirement. Any stronger OpenAgents implementation is additional scope. |
| Browser login persistence | Isolated persistent data on macOS 14+ and Linux; isolated temporary data on macOS 12/13 | No equivalent embedded profile | Preserve isolation and state truthfully; do not claim persistent logins on unsupported native versions. |
| Preview discovery | macOS/Linux process and listener discovery | No equivalent project-service browser | Define Windows behavior explicitly and test supported owner/viewer pairs. |
| Appshots | macOS capture with optional AX; Linux X11/portal with constrained text enrichment; Windows absent | Manual image input, no equivalent global app-capture workflow | Match supported platforms; explain unsupported capture rather than leaving an inert control. |
| Local dictation | Rust capture/model implementation targets macOS arm64, Linux x86_64/aarch64, Windows x86_64; physical inputs need separate verification | No equivalent desktop dictation | Record runtime/bundle dependencies and native microphone evidence. Intel macOS is not automatically covered by the selected runtime. |
| Realtime voice | Codex/helper-dependent local and V2 remote implementation; live remote evidence remains qualified | No equivalent desktop call UI | Separate implementation completion, packaged-runtime availability, and proven audio compatibility. |
| Notifications | macOS native/click path, Linux notifier; Windows implementation is a no-op | Linux portal/server, macOS Notification Center, Windows registered toasts | Preserve stronger OpenAgents Windows support. Match missing preference granularity, not Zeron's limitation. |
| Accessibility | GPUI controls include labels/focus/key handlers; no complete independent screen-reader certification established here | AccessKit plus documented headless/macOS AX checks; manual reader pass remains qualified | Preserve semantic access and validate all new custom surfaces. Do not claim one app is globally more accessible from source alone. |
| Updates | Managed installs, macOS bundle, Windows portable self-update; unmanaged report-only | macOS release updater, Linux AppImage self-update or package-manager download, Windows MSI handoff or ZIP download | Some differences are install-policy choices, not absent updating. Decide portable/managed parity explicitly. |
| Extra OpenAgents surfaces | No equivalent requirement identified for OpenAgents-specific worlds/routes | Verse/Map/Gym/feedback code exists, default desktop entry points are preview-gated | These are additional product capabilities, not substitutes for missing editor/browser/voice workflows. |

Sources: Z01, Z06–Z07, Z13, Z15–Z17, Z20; O01, O08–O15.

## Installation, lifecycle, and operating-system integration

| ID | Status / priority | Zeron behavior or comparison requirement | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| OPS-01 | V / P0 | Engine data-directory locking prevents concurrent writers, and UI startup connects to or embeds the intended engine. | OpenAgents has resident host/service management and a local broker. Verify the expanded desktop uses one host owner per state root, handles simultaneous launches, and never accidentally starts competing writers during update/reconnect. |
| OPS-02 | P / P1 | Startup/loading/login/organization/local-workspace phases give usable retry and account-switch paths. | OpenAgents uses different service identity and pairing semantics. Match recoverability rather than WorkOS screens. Test signed-out start, no network, expired account, unavailable local host, partial migration, and local-only coding entry. |
| OPS-03 | V / P1 | Closing/reopening windows, macOS dock reopen, and OS conversation URLs preserve engine and viewport lifecycle. | OpenAgents has native event handling and a host independent of the window. Verify close versus quit, resource ownership on reopen, cold/warm URL delivery, and pending editor/terminal/voice cleanup across the full new surface set. |
| OPS-04 | P / P1 | Supported install types have clear check/download/stage/restart behavior, including managed and portable updates. | OpenAgents already updates app packages; ZIP and `.deb` use download-only flows. Document the accepted policy difference or implement equivalent portable/managed updates. Test host/app version pairing, failed staging, interrupted replacement, and reopening the intended binary. |
| OPS-05 | P / P2 | Desktop updates can install at next quit after staging; provider updates are independently managed. | Verify and fill the next-quit behavior where desired, separate from existing Restart to update. Never interrupt active work or lose unsaved editor state. Headless-host and provider updates must obey their own idleness/ownership policies. |
| OPS-06 | V / P1 | Native menus, tray/titlebar controls, fullscreen, DPI, display changes, resize, and keyboard dispatch remain correct with auxiliary panes. | OpenAgents already has shell/menu/fullscreen infrastructure. Compare 1×/2× and narrow/wide layouts after adding native web views, editors, and terminals; inactive surfaces must not steal clicks or keyboard focus. |
| OPS-07 | P / P1 | Linux and Windows installers/startup checks cover their shipped runtimes and registration. | Browser and voice add WebKit/ALSA/inference/helper dependencies not covered by the existing app package. Add startup/runtime diagnostics and package verification for supported targets. Follow this repository's manual/non-GitHub release policy. |
| OPS-08 | V / P1 | Development, test, and release builds use isolated stores, native identities, and optional-feature switches. | OpenAgents already isolates development world keys and hides preview features. Extend that discipline to browser cookies, voice settings, action imports, editor caches, provider installs, and update endpoints. Fixtures must never open real home/account state. |
| OPS-09 | P / P1 | Settings and error surfaces explain recovery in ordinary language without exposing internal implementation. | OpenAgents has `oa-copy` and an explicit product voice contract. Apply them to every new feature/error, including remote denial, stale saves, model installation, and missing runtimes. Comment out unavailable controls instead of displaying placeholders. |
| OPS-10 | V / P1 | Every new surface supports keyboard navigation, accessible names/state, focus restoration, and discoverable errors. | AccessKit provides a foundation, not automatic semantics for browser/editor/terminal/custom visuals. Verify VoiceOver, Orca, and Narrator on supported targets; include asynchronous errors and native overlay focus, not just initial tree snapshots. |
| OPS-11 | V / P1 | Permission requests belong to the action/device that needs them and denial leaves the rest of the application usable. | Extend existing permission patterns to microphone, capture, optional accessibility, files, and previews. Request on explicit use; cancellation, revocation, or missing runtime must not repeatedly prompt, silently broaden access, or block chat. |
| OPS-12 | V / P1 | Account teardown and app shutdown finish or cancel owned work without losing accepted input or leaking media/processes. | Existing tests cover parts of host/task lifecycle. Add resource accounting for new PTYs, native web views/helpers, downloads, file watchers, model workers, call leases, and visuals. Close capture locally first; retain durable command/file draft recovery. |

## Performance and robustness

| ID | Status / priority | Zeron behavior or comparison requirement | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| PERF-01 | V / P1 | Long transcripts have responsive first paint, streaming, scroll, selection, search, and warm revisit. | Build matched deterministic fixtures with small/large/very long histories and identical output cadence. Record p50/p95 latency and frame stalls; history truncation must remain visible and must not be mistaken for faster full-history loading. |
| PERF-02 | V / P1 | Idle, background, minimized, and completed sessions stop unnecessary UI/engine work. | OpenAgents already retains pixels and bounds polling; Zeron has its own idle regressions. Measure the same lifecycle phases with the same open surfaces. Detect hidden animation, repeated scans, constant reparsing, and focus polling independently. |
| PERF-03 | V / P1 | Memory remains bounded through repeated navigation, streaming, theme changes, media viewing, and pane close/reopen. | Measure CPU RSS and GPU/texture retention separately with cache hit/eviction counts. Set budgets for transcripts, file documents, image variants, browser views, diagrams, and visual shaders; confirm resources settle after closure. |
| PERF-04 | V / P1 | File/content search and Git/PR operations stay off the UI thread and remain responsive on large workspaces. | Add indexed/search/history fixtures at meaningful scale and measure typing-to-results, first page, cancellation, and repair after mutations. Do not count truncated/partial results as completed search without labeling them. |
| PERF-05 | V / P1 | Slow or disconnected networks preserve text interaction while attachments, diffs, previews, or terminal replay are active. | Compare first reply, command adoption, media readiness, control latency, retry count, and peak queued bytes under the same link shaping. Do not compare Zeron's complete remote session with OpenAgents local text-only rendering. |
| PERF-06 | V / P1 | Renderer and parser work scales with changed visible content, not the whole transcript/workspace per token. | Profile the new panels alongside streaming chat; search/tree/history surfaces should not repaint/rebuild on every text delta. Time-slice syntax and diagram/media work, preserve row identity, and honor reduced motion and visibility. |

### Measurement protocol

Use release builds on the same machine, display scale, window size, renderer,
network conditions, and fixture. Separate UI and host costs when one application
embeds an engine and the other uses a resident host. Report CPU as a fraction of
one core, memory units, workload size, cold/warm state, and the exact revision.
Use repeated trials for comparisons and retain raw results in an approved
verification directory when implementation work reaches that stage.

Zeron's resource report contains historical Linux software-Vulkan measurements.
OpenAgents' desktop verification contains measurements from other machines and
earlier layouts. Those numbers are not a fair head-to-head benchmark. No current
speed, battery, or memory superiority claim follows from this analysis.

## What this documentation change verifies

This task requires documentation checks: internal links and source paths,
source-revision references, unique gap IDs, valid cross-references, coverage of
the inspected surface inventory, whitespace, and the final committed diff.
Rust tests, Clippy, release gates, live providers, and production smoke runs are
unnecessary for the report and were not used as evidence of product parity.

Implementation work should follow the repository's targeted-check rule:
formatting and tests for changed crates and relevant consumers. Add focused
regressions for the failure modes a feature introduces. Run full release gates
only for a release, and run native/real-provider checks when that work actually
depends on them. No GitHub workflows or GitHub-billed automation are proposed.
