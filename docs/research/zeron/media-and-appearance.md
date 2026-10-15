# Media, browser, voice, and appearance gaps

See [scope and evidence](scope-and-evidence.md) for the comparison rules and
source keys. Several features here require native platform integration, but
their state, authorization, and transport belong in Rust. The baseline includes
Zeron's implemented features even when its own native validation remains
incomplete; implementation and verified platform support are separate facts.

## Images and desktop capture

Sources: Z03–Z04, Z06–Z07, Z16; OpenAgents O03, O08, O10.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| MED-01 | P / P1 | Attachments use a shared owner-routed upload/read pipeline across new turns, follow-ups, queue rows, and remote sessions. | OpenAgents image input is enabled, but hosted routing sends only text and images leave with Coder start; several follow-up routes reject them. Extend supported turns/queues with capability checks, progress, acknowledgments, retry, and draft recovery. |
| MED-02 | P / P1 | Media can be inspected at full size through shared image views and original-file actions. | OpenAgents currently accepts PNG/JPEG with four-image, per-image, dimension, and draft-byte limits. Add a declared media-format matrix and richer viewing rather than removing bounds. Test unsupported formats, corrupt content, oversized decode, and an offline media owner. |
| MED-03 | M / P1 | Generated-image tool lifecycle produces an inline assistant image from a validated saved artifact. | Add an output-artifact path separate from arbitrary tool text. Verify owner, permitted generated-image root, signature/type, byte cap, atomic intake, metadata, and missing/offline states. Base64 tool output or a printed path must not automatically become trusted media. |
| MED-04 | M / P2 | Expanded tool chips lazily preview referenced images; collapsed/hidden previews release CPU/GPU resources. | Add owner-routed preview discovery, bounded image counts, decode scheduling, short reuse grace, and explicit retry. Fetch original bytes for Copy image, not the low-resolution thumbnail. Never read every tool path eagerly during streaming. |
| MED-05 | M / P2 | Appshots uses a global shortcut to capture another application's window, optionally include accessibility context, and stage it for review. | An image picker or manual screenshot is partial infrastructure, not this workflow. Add opt-in settings, shortcut conflict handling, native permission flow, capture identity/deadlines, preview and destination selection. Capture never sends automatically or runs on the remote agent host. |
| MED-06 | M / P2 | Appshot cards identify the source application/window and preserve context through queue editing and remote delivery. | Extend attachment metadata and presentation without turning observed application text into instructions. Permission denial or optional text failure must leave a usable screenshot when capture succeeded. Do not invent a source window identity on portal platforms that cannot verify it. |

### Media limits and product behavior

OpenAgents' image restriction is a routing restriction as well as a decoder
restriction. Adding WebP to the input decoder would not make hosted chat or
queued follow-ups capable of carrying it. The implementation must negotiate
transport/provider support and communicate where the image will go before
submission.

Zeron's workspace images and attachment images use different permitted roots
and read contracts. Keep that distinction: a general image viewer is a reusable
UI component, not general filesystem authority. Sanitize SVG, bound animated
inputs, reject inconsistent chunk metadata, and invalidate pending loads on
host, checkout, account, or selected-file changes.

## Browser and development previews

Sources: Z01, Z12, Z15; OpenAgents O02–O03, O10, O14–O15.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| WEB-01 | M / P1 | A conversation owns browser tabs with URL entry, navigation, loading/error state, reload, and explicit external opening. | OpenAgents opens links externally and has browser-oriented tooling elsewhere, but no equivalent native embedded website surface. Add browser tabs to NAV-01 with independent page ownership, focus, keyboard routing, and close/dispose behavior. |
| WEB-02 | M / P1 | Transcript web-link clicks create a new browser tab in the containing conversation; a preference and context menu allow external opening. | Add the route from both main and child transcripts and Markdown files. Discard a pending activation after conversation changes; inspect/copy/hover must not navigate. Retain URL validation and complete destinations even when labels are shortened. |
| WEB-03 | M / P0 | Persistent browser profiles isolate local/synced/development identities and retain cookies/site storage on supported systems. | Define profile identity and OS storage ownership before shipping browsing. Close old identity's pages on sign-out; preserve saved logins without mixing accounts. Test restart, last-tab close, profile switch, concurrent profile lock, corruption, and unavailable storage. |
| WEB-04 | M / P1 | Native browser startup happens off the UI thread; overlays, popovers, and focus coexist with native web views. | Add bounded asynchronous initialization, latest-navigation wins, close-during-start cancellation, native view bounds/clipping, and truthful missing-runtime errors. Settings/menus must remain clickable above a live browser, including on Linux. |
| WEB-05 | M / P1 | A browser's empty page lists HTTP development services for the selected project/checkout. | Add an authorized service catalog, stable names, disappearance/port-reuse handling, and an Open action. Do not probe arbitrary user listeners or one-shot login callbacks simply because the desktop runs on the same machine. |
| WEB-06 | M / P0 | Stable local preview names proxy HTTP and WebSocket/HMR streams to a live service identity. | Add host-owned discovery/proxy capability with bounded bodies, backpressure, cancellation, Host/Origin/redirect handling, and process/cwd revalidation. A URL or advertised port must not grant arbitrary TCP forwarding. |
| WEB-07 | M / P1 | Remote previews use the same project-oriented UI and authenticated service identities over a peer stream. | Existing host transport does not establish a complete application-preview proxy. Add authorized peer routing, account teardown, live ownership checks, error deadlines, concurrent streams, and WebSocket support. Preserve the selected remote checkout. |
| WEB-08 | V / P1 | Browser support differs by OS, macOS version, runtime installation, and direct network reachability. | Establish an explicit OpenAgents support matrix. Zeron uses embedded WebKit on macOS/Linux, external browsing on Windows; older macOS loses persistent isolated storage, and remote preview ICE has no TURN/cloud byte-relay fallback. Do not claim universal support from shared tab code. |

### Preview networking is separate from browser embedding

An embedded browser can display a local URL without providing remote preview
parity. Conversely, a forwarded URL is useful in an external browser but does
not provide conversation-owned tabs, profile persistence, or overlay/focus
integration. WEB-01–04 and WEB-05–07 can be developed independently, then verified
together.

Zeron's preview implementation now uses a dedicated WebRTC data channel for
application traffic, despite older architecture prose saying WebRTC is gone.
The transport is not the parity requirement. Discovery, stable identity,
authorization, HTTP/WebSocket behavior, and useful connection failure are the
requirements. OpenAgents may use an existing suitable host transport after
demonstrating those properties.

## Dictation and realtime voice

Sources: Z12, Z17–Z18; OpenAgents O03–O05, O10, O14.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| VOI-01 | M / P2 | Optional local Parakeet dictation has download/verify/remove, enablement, microphone selection, and local inference. | No equivalent desktop composer dictation was found. Add an optional Rust model/capture service and settings. Download size/runtime requirements must be explicit; disabled dictation must not unexpectedly load weights or request microphone permission. |
| VOI-02 | M / P2 | Hold-to-talk from a button or configurable shortcut transcribes into the selected draft range; release finalizes and Escape cancels. | Add capture/finalization state, accessible click alternative, timeout/recording bounds, and one grouped undo. IME, manual edit, selection/focus change, chat switch, or queued-edit takeover invalidates old results. Dictation never submits on its own. |
| VOI-03 | M / P0 | Audio stays on the viewing device; cancellation closes capture independently of slow model work. | Define separate capture and inference lifetimes. Send may wait for finalization once, then follow ordinary Send/Queue semantics. Failed or stale inference cannot send, attach audio, write recordings, or resume the microphone. |
| VOI-04 | M / P2 | A window-owned Codex realtime orchestrator has its own projectless conversation, voice selection, captions/activity, mute, and explicit end. | General chat, speech synthesis elsewhere, and dictation are not this feature. Add a voice-session state machine and UI stage that survives chat navigation, retains native provider context as intended, and handles missing subscription/helper/provider capability explicitly. |
| VOI-05 | M / P0 | Remote voice separates host-owned Codex from client-owned audio, with negotiated ownership and cancellation. | Add ephemeral call controls, local permission/media endpoint, bounded signaling, lease/generation checks, and teardown. No offline queued start or automatic microphone reconnect; local stop must work even while the remote host is unreachable. |
| VOI-06 | P / P2 | Voice-driven orchestration can inspect/create/message workspace chats through scoped tools. | OpenAgents has delegation/tool infrastructure, but no desktop voice-to-workspace flow. Integrate RUN-14 without granting broader rights than text interaction. Verify origin attribution, no self-message loops, and return of the correct chat's result. |
| VOI-07 | V / P1 | Physical audio, helper distribution, OS permissions, subscription compatibility, and remote call quality have separate evidence. | Zeron's remote-voice research explicitly leaves live G0 compatibility checks pending despite a current V2 implementation. Record local and split-host results independently; tests of fake helper pipes do not prove audible quality, echo control, or barge-in. |

The pinned dictation implementation downloads approximately 670 MB and supports
offline final transcription, not advertised live partial transcription. Parity
does not require that exact model if OpenAgents chooses another local model, but
it does require comparable supported languages, latency, privacy, cancellation,
and installation clarity. Those tradeoffs need measurements before selection.

Realtime voice is a different feature. It depends on Codex voice capabilities
and the local media helper/runtime, not the dictation model. Keep separate
settings, permission state, dependencies, and error messages so a failed optional
model download does not disable an otherwise supported voice call.

## Interactive agent output

Sources: Z18; OpenAgents O01, O07, O10, O12.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| VIS-01 | M / P2 | Agents publish a bounded visual artifact through `show_visual`; conversation content references that artifact. | OpenAgents has specialized slide/map/Gym surfaces, not a general interactive artifact contract. Define publication, versioning, retrieval, owner attribution, and plain-text failure fallback, including reconnect and remote viewing. |
| VIS-02 | M / P2 | Inline native visuals include controls, tables/charts, drawing canvases, simulations, 3D scenes, and shaders. | Add the supported component/event set and renderer under Rust ownership. Bound script/resource use, define deterministic state/error handling, and preserve transcript layout/selection. Zeron's Luau choice is not permission to add another OpenAgents product implementation language. |
| VIS-03 | M / P0 | Visual resources and animation are scoped to visibility and surface lifetime, with failures retaining usable content. | Suspend hidden animation/work, release GPU textures/scene state on disposal, cancel stale loads, and apply reduced motion. Test malformed artifacts and repeated open/close before claiming a general agent-visual platform. |

## Appearance and settings

Sources: Z19 and current UI settings schema; OpenAgents O04, O09–O10, O12.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| CUS-01 | P / P2 | Independent light and dark theme variants can be selected while appearance follows the OS. | OpenAgents already has system, Coder Light, and Coder Noir. Add a theme registry and independent variant selection; switching the OS mode chooses the stored variant without resetting user preferences. |
| CUS-02 | M / P2 | Local VS Code theme files/packages compile into validated theme families; imported and linked themes support reload and last-known-good fallback. | Add an explicit local import pipeline and durable library, not arbitrary theme execution. Invalid source/reload preserves the active usable theme and shows an actionable error. Keep source format details out of runtime components. |
| CUS-03 | M / P2 | Accent selection is separate from syntax/status/terminal palettes and preserves authored defaults. | OpenAgents has semantic tokens but no equivalent exposed accent overlay. Add contrast-checked interaction roles without recoloring diff meanings or code semantics. Every focus/selection/control state must remain readable. |
| CUS-04 | P / P2 | Theme-default/frosted/opaque treatment is independent of appearance and accent, with platform-aware glass composition. | The desktop uses OpenAgents' own surfaces. Add equivalent optional treatments if full visual parity is required, with contrast and native overlay checks. An imported theme must remain legible over adverse desktop backgrounds. |
| CUS-05 | P / P2 | UI prose, code/editor/diff, and terminal have independent font families and sizes, including installed-font choices. | OpenAgents offers shared text-size presets and fixed presentation values. Add independent validated families/sizes, fallback for missing fonts, fixed-width terminal restrictions, and consistent measurement/painting on DPI changes. |
| CUS-06 | M / P2 | New-thread artwork supports image framing, zoom/focal point, random folder selection/history, derived colors, and optional effects. | No equivalent desktop preference was found. Add device-local artwork management with bounded decode/cache, safe folder access, fallback, and independent theme/appearance behavior. This is not the preview-only Verse page. |
| CUS-07 | P / P2 | Conversation width, code-fence wrapping, compact transcript, composer submission, and compact model picker are saved preferences. | OpenAgents has a polished fixed chat presentation, but not the same set of controls. Add missing preferences with conservative defaults; changes must preserve selection, draft identity, scroll anchoring, and active session configuration. |
| CUS-08 | P / P1 | Shortcut recording supports per-action customization, conflict detection, per-row reset, and restore defaults. | OpenAgents explicitly exposes a read-only binding list. Add a validated keymap shared by menus and handlers, including focused editor/browser/terminal contexts. Invalid or colliding bindings need clear recovery without breaking default navigation. |
| CUS-09 | P / P2 | Completion/input/attention sounds can be controlled independently; banners can be background-only, and provider updates have their own toggle. | OpenAgents has notification and sound switches plus native delivery. Add the missing granularity and focus policy; suppress duplicate alerts across reconnect/replayed events while preserving actual attention changes. |
| CUS-10 | P / P2 | Reduced motion can follow the system or override it, and animations can pause while the app is unfocused. | OpenAgents already honors reduced motion. Verify the full expanded workspace and add any missing background pause policy; no hidden browser/visual/voice animation should drive unrelated chat frames. |
| CUS-11 | P / P1 | New settings deserialize with defaults, migrate renamed choices, debounce geometry, and persist durable choices reliably. | OpenAgents already has shared settings. Define ownership/migration for the larger schema; one malformed new preference must not reset unrelated credentials, engine choices, or drafts. Test restart during geometry and theme writes. |
| CUS-12 | P / P2 | Settings groups cover General, Appearance, Notifications, Voice, Shortcuts, Providers/accounts, Devices, Files, Appshots, and Archived sessions. | OpenAgents has nine different panes, including BYOK and Background. Add missing groups as their features land; keep existing BYOK/background capabilities. Zeron's old Accounts route is now an alias folded into Providers, not an extra required top-level page. |
| CUS-13 | V / P2 | Shell spacing, surface radii, titlebar alignment, inset/expanded layout, gutters, and visual density remain coherent across panel combinations. | Earlier OpenAgents Zeron-fidelity captures cover an older reference. Compare current matched states, including file/browser/terminal panels and narrow layouts. Record deliberate OpenAgents branding differences separately from clipping, alignment, or density defects. |
| CUS-14 | P / P2 | Menus/dialogs, sidebar resort, new-thread transitions, streaming entrances, and working indicators use coordinated motion. | OpenAgents has transitions and reduced motion, but no current full interaction comparison. Match feedback and interruption behavior: repeated toggles reverse from the current position, streaming never restarts completed entrances, and loaders stop when work ends. |
| CUS-15 | V / P2 | Hover intent, edge fades, wheel/pinch behavior, overscroll, anchored popovers, and optional haptic feedback support native-feeling interaction. | Audit mouse and trackpad behavior across actual surfaces. Verify scroll does not activate links, popovers stay within the window, pointer release outside a control is safe, and gestures remain usable when reduced motion is enabled. Do not require a haptic device for basic navigation. |
| CUS-16 | P / P2 | Current session/provider status, elapsed activity, sending bridge, and asynchronous loading maintain stable layout without false progress. | OpenAgents already has Coder progress and notices. Compare status placement and cadence across direct providers, remote adoption, queues, files, and media. Unknown progress is indeterminate; status changes must not jump the composer or imply execution before host adoption. |

### Visual comparison baseline

Current Zeron code uses a 1320 × 880-point fallback window with a 900 × 600
minimum. Its sidebar settings currently bound width to 242–400 points with a
256-point default; the older inventory's 208-point minimum is stale. Its file
explorer has a separate 220–440-point range and 286-point default. OpenAgents
requests 90% of usable display area with a 1200 × 840 fallback and 760 × 540
minimum, and its sidebar range is 224–400 points. Different minimum sizes can be
a product choice; losing controls at a supported size is a defect.

The current Zeron motion module includes 500 ms entrances, 150 ms quick fades,
140 ms menu entrances, 180 ms dialog/panel transitions, and a 420 ms new-thread
transition. These values are comparison references, not instructions to copy
obsolete CSS timings into the Rust adapter. Compare perceived continuity,
interruption, anchoring, and accessibility with current native captures before
changing OpenAgents' timings. Sources: Z01, Z19, and Z22.

Appearance parity is a user-outcome target, not a demand to discard the OpenAgents
visual identity. Any decision to retain only Coder Light/Noir, omit wallpaper, or
skip glass should be recorded as an explicit scope exception. Until then,
those remain P2 gaps under the requested full-parity target.
