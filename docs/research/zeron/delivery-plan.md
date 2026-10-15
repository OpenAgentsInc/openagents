# Desktop parity delivery plan

This is a proposed implementation sequence for the gaps in this directory.
It does not authorize shipping placeholder controls or excluding later stages
from the requested full-parity target. No new implementation issues or product
changes are part of this documentation commit.

## Architectural decisions to make first

1. **Keep general chat and expose coding choices.** Preserve OpenAgents chat,
   while adding an explicit coding-session/workspace mode that can select a
   provider, model, and execution target before sending. Specify how a routed
   OpenAgents conversation becomes such a session and how native resume differs
   from starting Coder with summarized context.
2. **Use one host authority.** Extend existing paired-host task/file/terminal
   boundaries. Do not create a second desktop credential store or give history
   observers write authority. Define rights/capabilities for file mutations,
   provider accounts, source-control writes, previews, and ephemeral voice.
3. **Add a typed surface host.** Conversation-owned resource identities and
   lifecycles precede file/browser/subagent/side-chat tabs. Keep geometry in the
   desktop adapter and domain state in Rust. Read the Rust Native contracts
   before extending the platform boundary.
4. **Separate durable input from ephemeral interaction.** Text/queue sends need
   stable retry identities; browser focus, audio start, native capture, and
   preview streams need cancellation and explicit lifetime, not offline replay.
5. **Agree on supported platform outcomes.** Record OS/version/runtime limits
   for browsing, capture, voice, terminal behavior, and install types. Different
   platform behavior needs usable UI and a supported alternative, not a hidden
   failure.

These decisions can be recorded in the implementation issues and existing
architecture documents when necessary; the task does not need a new invariant
table or a second long design specification.

## Work packages and dependencies

Each package is a planning boundary and should be split into small changes.
The existing repository issue-claim/board process applies when implementation
issues are created or taken. Durations are intentionally absent: this source
review did not prototype the editor/browser adapters or measure provider work.

| Package | Scope and gap families | Depends on | Exit condition |
| --- | --- | --- | --- |
| A. Session and surface foundation | NAV-01–03, NAV-06, NAV-13; COM-01–03/08/15; REM-01/03/10; OPS-01/02/08 | Architectural decisions | Local/remote target, session configuration, typed surface identity, capability checks, and late-reply/account isolation are implemented and testable with fixtures. |
| B. Provider and session controls | RUN-01–10/14–16; COM-02/06/07/09/10; TRN-04/06/13 | A | Every claimed provider has install/readiness, catalog, input, continuation, cancellation, child-event, and account behavior documented and tested; unsupported features are unavailable. |
| C. Read-only workspace | WRK-01–04/09/13/14; TRN-07–09; NAV-09 | A | Browse/search/open local and remote text/Markdown/images with correct ownership, bounded work, reliable links, and no viewer-local path fallback. |
| D. Editing and workspace mutation | WRK-05–12/20; GIT-04; CUS-05/08/11 | C | Manual save/autosave, conflicts, dirty navigation, rename/move/delete, keyboard selection, and line-note drafts work through the owning host, including stale and uncertain outcomes. |
| E. Checkout and action workflow | WRK-15–19; COM-03; TERM-01/04/05/08 | A and terminal host path | Create/clone/add project, select branch/worktree, explicitly import/run actions, and attach setup output to the originating chat without duplicating setup. |
| F. Source control and review | GIT-01–09/16; TRN-07 | C; D for editing/notes/mutations | Live/branch/turn diffs, history/search/ref operations, notes, and exact-review Publish coexist without stale writes or lost buffers. |
| G. Pull requests | GIT-10–16; NAV-08 | A, C, F | Board/detail/code/review threads/checks, external preference, explicit comment submission, and staged agent handoff work with device-scoped failures and retained drafts. |
| H. Remote desktop and queue completion | REM-01–12; COM-10–15; RUN-11/12; TERM-01–05 | A; corresponding feature service | All enabled features behave as local/remote peers with idempotent adopted input, revocation, older-host fallback, sender suspension, and complete recovered transcript/media. |
| I. Terminal workspace | TERM-01–08; NAV-01/02/06; CUS-05 | A and existing PTY authority | Multiple chat-owned local/remote terminals, dock/surface layouts, detach/replay, practical VT behavior, and action handoff work across supported OSes. |
| J. Conversation organization | NAV-04–14; COM-04–09/14/16; TRN-01–06/12–14 | A; B for native-provider features | Sections/pins/filtering, links, child chats/subagents, compact transcript, checklist, completion, and keyboard navigation preserve drafts and current focus. |
| K. Media and capture | MED-01–06; TRN-10; COM-11/15 | A, H, bounded artifact/file services | Images work on supported turns and queues; generated/tool media and Appshots have owner identity, review-before-send, full-size viewing, cancellation, and bounded resource use. |
| L. Browser and previews | WEB-01–08; TRN-08; NAV-01/06 | A; authorized preview host service | Persistent isolated browser profiles and conversation links work on supported OSes; local/remote project previews support HTTP/WebSocket with truthful failure and teardown. |
| M. Dictation | VOI-01–03/07; COM-04/15/16 | Stable draft generation and optional native runtime packaging | Local hold-to-talk inserts into the intended draft, with install/device/permission controls, cancel/stale-result protection, no recording persistence, and physical microphone evidence. |
| N. Realtime voice | VOI-04–07; RUN-14; REM-10 | B, H, native media capability | Explicit local/split-host call ownership, mute/end, captions/context, navigation survival, and failure cleanup work; live compatibility is separately recorded. |
| O. Interactive output | VIS-01–03; TRN-11 | A, bounded artifact service and Rust visual contract | A versioned artifact can render supported interactive visuals locally/remotely, fail safely to readable output, and release work/resources when hidden or disposed. |
| P. Appearance and settings | CUS-01–16; RUN-13; NAV-14 | A; owning features for per-surface controls | Themes/imports/accents/materials, fonts, wallpaper, keymaps, sound policy, layout/motion, and input preferences persist and remain readable across the supported surfaces. |
| Q. Platform and release quality | OPS-01–12; PERF-01–06; WEB-08; VOI-07; GIT-16 | Continuous; final pass after enabled features | Installed-package, native input/accessibility, resource, reconnect, and cleanup evidence exists for the actual supported release matrix. |

Packages C, B, and the terminal host path can proceed independently after A.
Browser profile work can start before file editing once surface and identity
boundaries exist. Voice does not need the file editor, but realtime orchestration
does need provider/remote lifecycle work. Do not make all parity work wait on a
single renderer rewrite or a full workspace release gate.

## Acceptance journeys

Use isolated accounts, state roots, homes, repositories, and hosts. Native input
checks supplement deterministic fixtures; live provider checks run only when
they establish behavior a fixture cannot. Production account checks must follow
the repository's dedicated-test-account and cleanup contract.

| Journey | Required observable result | Failure cases to include |
| --- | --- | --- |
| Local project from scratch | Create/add/clone a project, choose provider/model/branch/worktree, send, answer input, inspect results, edit a file, run a saved action, review and publish. | Missing CLI/model, login cancel, bad repository URL, setup error, external file edit, stale publish. |
| Plain folder | Start a session in a non-Git folder, browse/edit/search files, open a terminal, and later detect `git init` appropriately. | Missing `.git`, protected paths, deleted root, no branch picker, stale search results. |
| Remote coding workspace | Select another host, open a project/chat, read/edit/search its files, inspect Git/PRs, run a terminal/action, and receive complete results. | Viewer has same path with different bytes; owner offline; rights revoked; wrong checkout; old host; host restart. |
| Queue from two devices | Queue text and images, reorder, lease/edit a row, interrupt/send explicitly, then observe one delivery and the same order everywhere. | Lease expiry, sender exit, held acknowledgment, blocked head, concurrent remove, failure during dispatch, user types a newer draft. |
| Existing provider conversation | Open native history, distinguish read-only import from native resume, continue the selected supported session, and inspect its children/context usage. | Deleted history, unsupported resume, stale catalog, child traffic, unknown context, source cursor change. |
| Review and PR handoff | Inspect working-tree/branch/turn diffs, leave line notes, open PR code and threads, stage a fix prompt, and explicitly submit a comment when intended. | Outdated line, renamed path, missing CLI/auth, rate limit, duplicate result, canceled comment, large response. |
| Browser and service preview | Open transcript link in a new tab, keep login across supported restarts, switch profile, open a project service locally/remotely, and use HMR. | Missing WebKit, storage lock/error, macOS version limit, denied ICE path, recycled port, wrong project, account teardown mid-stream. |
| Capture and media | Capture another app, inspect source context, queue/edit the image, send to the owner, then view generated/tool images at full size. | Permission denial, portal cancel, unavailable owner, corrupt/oversized input, stale capture result, hidden-preview resource release. |
| Dictation | Download/enable model, choose mic, hold/release, edit transcription, undo, and explicitly submit through the ordinary queue/send path. | Focus/IME/chat changes, permission delay, no speech, mic disconnect, model failure, late inference, Escape, close during finalization. |
| Realtime voice | Start/mute/end locally and with a remote execution host, inspect captions/activity, navigate chats, and route a scoped workspace request. | Provider/helper mismatch, lost ownership, remote disconnect, local close during negotiation, stale events, no automatic mic restart. |
| Daily desktop use | Keyboard-only navigation, two display scales, narrow/wide layout, screen readers, background alerts, update, close/reopen, and sign out/in. | Dirty editor, pending send, live terminal/browser/voice, missing runtime, update failure, old-account late reply. |

## Completion record

Maintain this register as implementation lands. For each ID, record the closing
issue/commit, supported platforms/providers, targeted check, and retained native
evidence where needed. A shared-library change closes a desktop gap only when
the desktop path consumes it. A control closes a workflow gap only when the
action, errors, recovery, and authorization work.

Use these outcome states for follow-up tracking: implemented and checked;
implemented with named native verification remaining; explicitly accepted
exception; or still open. Do not convert a verification gap into “done” by
linking to an old screenshot or a unit test of only the view model.

For implementation issues, follow the repository's existing close rule: close
when merged to `main` with the issue's checks and required host deploy complete.
Put owner-only device, credential, money, or store-release steps in
`NEEDS_OWNER.md`; do not hold an otherwise complete issue open. The report's
verification register can still describe that evidence honestly.

An unqualified full-parity claim requires all missing/partial rows to be closed
or have an explicit approved exception, plus verification of the supported
end-to-end journeys. Counts of rows, code volume, passing unrelated tests, or
visual resemblance of the empty chat screen cannot establish it.
