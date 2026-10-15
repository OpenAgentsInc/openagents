# Navigation and chat gaps

Status and priority definitions, plus source reference keys, are in
[scope and evidence](scope-and-evidence.md). Rows describe the complete missing
behavior, not just a proposed control. The acceptance column is required in
addition to preserving existing chat behavior.

## Shell, projects, and navigation

Sources: Z01–Z02, Z12, Z18–Z19; OpenAgents O02–O04, O11, O14.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| NAV-01 | M / P0 | One conversation owns a right-panel set of file, browser, diff, terminal, subagent, side-chat, and pull-request surfaces. | The desktop has specialized change and terminal panes, without the common surface model. Add typed tabs, stable identities, close/select behavior, ownership, and disposal. Opening one kind must not discard another kind's unsaved state. |
| NAV-02 | P / P1 | Explorer and surface panel have independent widths, per-chat open state, and an expanded takeover mode. | OpenAgents can resize/collapse its sidebar and show auxiliary panes. It lacks the equivalent multi-column fit policy. At narrow widths, retain usable conversation/editor controls; switching chats restores that chat's panels without inheriting another chat's takeover. |
| NAV-03 | M / P1 | A project is a device/folder pair, including ordinary folders; the project chooser filters sessions and supports management. | A Coder project default and row label do not provide a desktop project workspace. Add local/remote project selection, add/rename/remove, plain-folder handling, and clear ownership. Selecting a filter must not silently move a session or change its execution directory. |
| NAV-04 | P / P1 | Sidebar view options select organization and sorting, compactness, project filtering, metadata visibility, and archived visibility. | OpenAgents has pinned/recent/archived ordering and project labels. Add the missing user choices, persist them, and use the displayed order for keyboard cycling. Each metadata toggle must affect only presentation. |
| NAV-05 | P / P2 | Named sidebar sections and manually arranged pins support drag/drop, with account synchronization and profile isolation. | OpenAgents pins conversations but has no comparable exposed section editor and order management. Create/rename/delete sections, assign/reorder chats and pins, and handle concurrent changes. Moving a row must not duplicate or delete its conversation. |
| NAV-06 | M / P1 | Session navigation supports back/forward and context-sensitive next/previous: session list when chat has focus, surface tabs when the right pane has focus. | OpenAgents cycles chats and has navigation state, but no matching panel-focus routing. Preserve previous focus, block navigation while a modal owns keys, and route identical shortcuts according to the active surface. |
| NAV-07 | M / P2 | Stable conversation links carry a workspace locator; supported provider links can open the native provider conversation, currently Codex. | Desktop notification targeting and internal IDs are not equivalent copyable, cold-launch links. Add validated OpenAgents conversation URLs, account/workspace mismatch handling, existing-window routing, and verified provider-link actions. Do not imply support for every provider scheme. |
| NAV-08 | P / P1 | Rows expose project, branch, provider, pull request, running/awaiting/finished status, and seen state. | OpenAgents has titles, project labels, and task attention. Add branch/provider/PR details and inspect status freshness across local/remote sessions. A disconnected old “working” record must not indefinitely appear live. |
| NAV-09 | P / P1 | Global search combines commands, conversations, workspace filenames, and content, with keyboard tabs and owner-routed asynchronous results. | The command registry and chat search exist; file/content results do not. Search the active workspace, reject stale replies after target/query changes, preserve selection by result identity, and open the chosen file at its match. |
| NAV-10 | P / P2 | Conversation menus include copy links, project/context actions, archive/delete, and provider-specific actions where supported. | Rename, pin, archive, restore, and saved-session actions exist. Inventory and add missing applicable actions with distinct archive/delete semantics. Cancellation and failures preserve the current conversation and draft. |
| NAV-11 | M / P1 | Side chats can start empty or fork the current conversation through its latest completed reply, remain children, and have their own composer. | The desktop has no comparable user-driven child-chat surface. Support both creation modes, sibling/parent relationships, independent drafts and task identity, and error reporting when there is no resumable completed context. |
| NAV-12 | P / P1 | Subagents have running and finished sections, completed/failed groups, pagination, and read-only transcript tabs. | OpenAgents has a shared subagent model and a terminal Agents board. Those do not provide the same parent-chat explorer and transcript experience. Mount the shared data in desktop panels; preserve parent context and never mix a child's output into the parent's reply. |
| NAV-13 | M / P2 | A session information card exposes its project, device, child conversations, and actions; it can be hidden. | OpenAgents displays task start/result details, but lacks this persistent workspace summary. Provide a compact, actionable summary whose displayed device and checkout match the actual execution target, including while remote/offline. |
| NAV-14 | P / P2 | Settings remembers its section, resolves old aliases, and opens an explicitly requested section without losing the session. | OpenAgents has settings panes and navigation. Verify restoration across close/reopen, restart, and deep navigation; add remembered sections where missing. Returning restores conversation focus, selection, and drafts. |

### Shell design work

The first implementation decision is a typed desktop surface host. Keep
application state in shared Rust where phones and other clients need it, but
keep desktop pane geometry in the desktop adapter. A surface key needs at least
its kind, conversation, host, checkout where applicable, and resource identity.
An editor tab for `src/main.rs` on one host must not collide with the same path
on another host.

Zeron's current `tabs.rs` explicitly says that the main session tab strip was
removed and the sidebar is the session navigation spine. Its surviving right
surface tabs remain relevant. Do not implement obsolete top-level session tabs
merely because older settings still deserialize `open_tabs`.

## Composer and input

Sources: Z03–Z04, Z11–Z12, Z19; OpenAgents O03–O05, O08, O10.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| COM-01 | D / P1 | The composer directly selects a coding harness before the first turn and keeps the chat's harness identity. | OpenAgents starts with hosted chat and routes work to Coder; the user can request an engine in words. Add an explicit coding-session mode or equivalent controls without removing general OpenAgents chat. A new session must visibly identify what will run and where. |
| COM-02 | M / P1 | Model selection shows the selected provider's catalog, reasoning levels, and advertised options; defaults are remembered per provider. | Desktop settings enable Coder agents, and run cards explain selection afterward. Add pre-send model/effort/option selection, preserve it on retry, and handle catalog refresh or an unavailable model without silently changing the request. |
| COM-03 | M / P1 | Repository/folder, branch, and isolated-worktree choices are available during session creation. | OpenAgents uses project defaults and automatic Coder worktrees. Add discoverable per-session checkout controls, display the resulting checkout, and keep failed creation from binding a chat to the wrong directory. |
| COM-04 | P / P2 | Composer supports compact/expanded layout, measured growth, Markdown-aware editing, and explicit Enter versus modified-Enter preference. | OpenAgents has a real Rust text editor, multiline input, and per-chat drafts. Add missing input preferences and formatting behavior; verify IME, selection, undo, Unicode, and caret position across layout changes and attachment insertion. |
| COM-05 | M / P1 | `@` references select files and conversations; selected chips retain identities separately from visible text. | Dropped non-image files currently insert a path. Add owner-aware completion, keyboard selection, chip editing/removal, and stable dispatch expansion. Literal `@` text or an unselected path must remain ordinary text. |
| COM-06 | M / P1 | `/` completion combines local workspace actions with provider commands; conflicts get a namespace prefix. | The command palette exists, but the composer lacks the equivalent catalog. Expose supported actions and provider commands at valid word boundaries. Selecting a local action consumes only its trigger and does not send a model turn. |
| COM-07 | M / P2 | Skill completion has per-provider `$` and `/` preferences and a structured selected skill identity. | Skills elsewhere in the workspace are not composer completion. Add provider/project-scoped discovery and insertion, preserve surrounding Markdown and queued edits, and keep currency, code, URLs, and literal dollar signs unchanged. |
| COM-08 | P / P1 | Pickers and submissions retain the exact target and selections across asynchronous updates. | OpenAgents protects draft stamps and uncertain request identities. Extend that protection to new provider/file/branch pickers: a late response or catalog refresh must not overwrite newer picks or send on another selected chat. |
| COM-09 | P / P1 | Awaiting-input mode supports multiple questions, single/multiple choices, free text, paging, number keys, and back navigation. | OpenAgents has questions, approvals, and answer modes. Verify each provider's typed questions reaches a usable desktop control, with answers retained across reconnects. Add missing choice/paging controls; a stale request must not accept an answer. |
| COM-10 | P / P1 | Busy-send behavior distinguishes native steering, queued follow-up, turn-end hold, and explicit interrupt-and-send. | OpenAgents has Queue, Steer now, and Stop and send, with engine-specific constraints. Make the selected behavior visible and capability-driven; preserve the existing safety distinction between steering before a turn and interrupting an active turn. |
| COM-11 | P / P1 | Queued messages keep identity, order, editable text, attachments, author, and a delivery policy shared across devices. | OpenAgents exposes task queues and protected editing, but image follow-ups remain restricted and local runs have their own queue path. Add attachment-bearing queue rows and consistent local/host behavior, with no optimistic transcript bubble before actual delivery. |
| COM-12 | P / P0 | An edit lease prevents delivery; expiry requires review rather than silently sending. Stale edits cannot overwrite a newer row. | Host-backed OpenAgents task edits already use leases. Verify equivalent behavior for every new desktop direct-provider/remote path, including app crash, offline expiry, edit renewal, lost acknowledgments, and another client's reorder/remove. Reuse the existing authority. |
| COM-13 | P / P1 | Queue rows can be reordered, removed, explicitly steered or sent, and edited while restoring the previous composer draft afterward. | OpenAgents has move/send/remove and queue editing. Close the remaining UI consistency gaps across local and host-backed sessions, including blocked heads and keyboard activation; never skip a gated row to send a later row implicitly. |
| COM-14 | M / P2 | Empty modified-submit can act on an eligible queued item without stopping the agent; submit behavior is configurable. | Add explicit, tested shortcut semantics after queue policy is unified. Saving an edit, sending a draft, and acting on a queue row must be distinguishable. Empty Enter or a held modifier must never accidentally interrupt work. |
| COM-15 | P / P0 | Draft text, attachments, pending send state, and explicit failure feedback stay associated with their conversation through uncertain delivery. | OpenAgents already preserves text and request IDs on several paths. Extend recovery to new attachments, references, and direct-provider controls. Relaunch after a lost reply must reveal whether the host adopted the send and must not duplicate its effect. |
| COM-16 | P / P2 | Escape is consumed by the active context; stopping the agent with bare Escape is an opt-in setting. | OpenAgents has dismiss and stop commands, but new browser, editor, voice, completion, and surface contexts need one precedence rule. Cancel the innermost interaction first and keep focused text edits intact. |

### Input integration detail

Do not build separate drafts for each new picker. Extend the existing draft
generation protection to include attachments, selected references, pending
dictation, review comments, and send configuration. A pending mutation should
record the submitted generation; its acknowledgment clears that generation,
not text or media entered while the request was in flight.

Question and approval UI need separate authority. Answering a question continues
the existing request. Approving an operation follows its existing approval
contract. Neither action grants broad file access, changes a provider account,
or upgrades a device grant.

## Transcript and conversation history

Sources: Z05, Z15–Z16, Z18–Z19, Z21; OpenAgents O03, O05–O08, O10–O11.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| TRN-01 | P / P2 | A user-prompt rail shows the current location, prompt/reply previews, and direct jumps; it hides when the column is narrow. | OpenAgents has scrolling and earlier/latest controls, without this navigational rail. Add stable message anchors, hover and keyboard activation, and width-sensitive layout without forcing the scroll position during streaming. |
| TRN-02 | P / P1 | Working details can fold into a compact turn accordion; tool groups preserve errors and user-pinned expansion. | OpenAgents groups tools and shows reasoning/output. Add a conversation-level compact preference and verify grouping retains chronological order, active state, failures, and full details on demand. This is not a missing basic tool renderer. |
| TRN-03 | V / P1 | Block-level virtualization, tail-follow springs, scroll runway, and anchor correction keep long streaming replies readable. | OpenAgents has a bounded virtual transcript and tail controls. Compare the same long fixture, resize/scroll/revisit behavior, and per-delta work before declaring equivalence. Preserve user scroll intent, code horizontal scroll, and selection. |
| TRN-04 | P / P2 | A context-capacity ring reports current occupancy, unknown versus zero, and provider-specific measurement. | Run statistics and fleet token counts are not context occupancy. Add a current-session context model and desktop ring; exclude cumulative billing and child-agent counts, preserve partial measurements, and show unavailable when the provider cannot report it. |
| TRN-05 | P / P1 | A live checklist stays available above the composer, folds long lists around the current item, and compacts/dismisses when done. | OpenAgents already has `plan_panel`, used by local Coder run presentation. Verify composer-adjacent placement and host/provider coverage; close those differences rather than rewriting the plan model. Child plans must not replace the parent's plan. |
| TRN-06 | P / P1 | Subagent tool events open separately scoped, live, read-only child transcripts with clear finished/failed state. | The shared OpenAgents subagent types and terminal fleet board cover some data. Add the desktop transcript-tab route, lazy loading, child identity mapping, and return-to-parent behavior for supported engines. A provider with no child stream must say so. |
| TRN-07 | M / P1 | Workspace links and file mentions open owner-routed file surfaces at a line/column, including read-only outside-root links where allowed. | OpenAgents has shared link handling but no matching file surface. Add path classification, line navigation, correct parent/child checkout resolution, and explicit read-only handling. Never resolve a remote path on the viewer's filesystem. |
| TRN-08 | P / P2 | Web links have destination disclosure, copy/external/embedded actions, keyboard focus, validation, and selection that preserves the full URL. | Existing external links do not cover embedded browser routing or long-URL behavior. Implement the common link policy with WEB-02; test misleading labels, unsafe URLs, table cells, streaming offset changes, and focus restoration. |
| TRN-09 | M / P2 | Mermaid code fences render as bounded native diagrams with source/error fallback, caching, and lightbox viewing. | Markdown/code support alone is insufficient. Add a bounded background diagram renderer and shared media view; unsupported or invalid diagrams leave readable source. No render task may block the UI thread. |
| TRN-10 | P / P1 | Assistant-generated images and tool-referenced images appear inline with lifecycle/error states and owner-routed original-image actions. | OpenAgents input thumbnails are not equivalent output media. Add trusted artifact intake, decode limits, lazy previews, retry, lightbox, and copy-path/copy-image behavior. Distinguish provider-generated media from arbitrary paths in tool text. |
| TRN-11 | M / P2 | Agent-authored interactive visuals render native controls, charts, canvases, scenes, and shaders inline. | OpenAgents has specialized decks, maps, and Verse, but no equivalent general chat-visual runtime. Define a Rust-owned bounded content contract and event lifecycle; support failure fallback, hidden-state suspension, and remote artifact retrieval. |
| TRN-12 | P / P1 | Warm conversation revisits restore content synchronously while a fresh authoritative update arrives. | OpenAgents retains selected state and bounds loaded history. Verify long-chat revisit without a blank flash, account-switch cache clearing, true-empty resets, and deleted-chat eviction. The cache must have byte and item limits. |
| TRN-13 | P / P1 | Existing provider sessions continue with their native session identity where supported. | OpenAgents' saved Codex/Claude viewer is read-only; Continue with Coder starts a new Coder task using bounded context. Add an explicitly distinct native-resume workflow for parity, or obtain a product decision accepting the difference. Do not label summarized-context transfer as native resume. |
| TRN-14 | V / P1 | Selection, copying, link activation, tool expansion, and scroll gestures coexist across streamed Markdown and tables. | OpenAgents has rich text and selection. Verify cross-block Unicode selection, code copy, pointer-release outside bounds, focus loss, font/width changes, and completed-tool folding against representative Zeron fixtures; record native-platform results. |

## User journeys that expose combined gaps

**Inspect a remote agent's result.** Select its project, open the conversation,
jump to the initiating prompt, inspect the child's transcript, open a changed
file, add a line comment, and send a follow-up without losing an unsent draft.
This crosses NAV-01/03/12, COM-05/15, TRN-01/06/07, and the file/review work.
Passing each isolated widget test does not establish this journey.

**Change provider settings while work continues.** Open a catalog, inspect
available reasoning/options, preserve the active run's configuration, and choose
the next turn's settings. A stale catalog or a provider at its limit must not
change an in-flight request. COM-01/02/08 and RUN-04/05 define the required behavior.

**Edit a queued follow-up from another device.** While a reply streams, acquire
the row's edit lease, change text and images, navigate away and back, and finish
or recover after expiry. Preserve the user's previous draft and transcript
anchor. COM-11–15 define the queue and recovery requirements.
