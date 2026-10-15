# Workspace and source-control gaps

Read [scope and evidence](scope-and-evidence.md) for status, priority, and source
keys. The baseline OpenAgents desktop has task-oriented change review and a
local shell. It does not yet have a general file explorer/editor or native Git
and pull-request workspace. File tools inside an agent do not close those UI
gaps.

## Files, editing, search, and project setup

Sources: Z06–Z08, Z10, Z12, Z15–Z16; OpenAgents O02–O06, O09–O10, O14–O15.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| WRK-01 | M / P0 | Workspace requests bind a device, chat, checkout, path, and advertised host capability. | Define the equivalent authorized file-service contract before adding explorer UI. Reading history is insufficient authority. Wrong-host/wrong-checkout requests must fail; an offline owner must never fall back to viewer-local access. |
| WRK-02 | M / P1 | A lazy, paged file tree browses the current workspace, with icons, Git status, expand/collapse, loading, and errors. | Add an explorer backed by the owner, not UI filesystem calls. Handle plain folders, empty folders, deleted entries, partial pages, and stale replies. Switching conversations invalidates the previous tree's requests. |
| WRK-03 | M / P1 | Tree keyboard controls, single/multi/range selection, context actions, and unfocused selection remain usable. | Implement arrow/Home/End/Enter/Escape navigation and modifier/range selection, including collapsed-parent behavior. Right-click preserves an existing multi-selection and actions operate on selection roots, not duplicate descendants. |
| WRK-04 | M / P1 | Tree and global-palette search use a warmed fuzzy filename index; content search returns line matches. | OpenAgents chat search and agent search tools are separate. Add host indexes, cancellation, bounded result pages, partial/indexing feedback, recent-file ranking, and filename/content modes. Results refresh after move/write/delete without reopening the app. |
| WRK-05 | M / P1 | Text files open as individual tabs with syntax highlighting, line numbers, caret navigation, selection, undo, and search. | Build a real editor surface. The composer editor is a reuse candidate, not a file editor with document lifecycle. Open multiple files, retain edits while switching tabs/chats, and support keyboard save without intercepting terminal/browser shortcuts. |
| WRK-06 | M / P0 | Saves carry expected content and checkout identities; concurrent disk changes become conflicts instead of overwrites. | Add optimistic concurrency and per-document revision tracking. A reply to an older save cannot mark newer edits saved. Detect file replacement, checkout changes, and lost replies; never overwrite external edits automatically. |
| WRK-07 | M / P1 | Optional debounced autosave, manual save, saved/saving/failure states, and retry retain the edited buffer. | Provide explicit settings and truthful dirty state. Navigation, panel collapse, and reconnect cannot discard edits. Test edit-during-save, autosave-after-rename, save refusal, and closing a dirty document. |
| WRK-08 | M / P1 | File reads report encoding, line endings, binary/large/read-only reasons; writes preserve supported encodings and endings. | Add a file-type contract before enabling writes. Keep unsupported/binary files read-only with useful actions. Saving a supported file must not silently change its BOM, line endings, or invalid content representation. |
| WRK-09 | M / P1 | File watchers update tree and clean buffers, while dirty buffers enter external-modification/conflict handling. | Reuse or add a bounded host watch service. Collapse/hidden views suspend expensive work; account/host changes cancel watches. External delete/rename and missed native events must converge through a repair read. |
| WRK-10 | M / P1 | File rename and move validate checkout, source revision, protected roots, symlinks, and destination collisions. | Add authorized structural operations distinct from text writes. A stale request must require refresh. Preserve dirty buffers/tab identities on success, deduplicate response/watch events, and refuse replacement or cross-volume copy/delete fallbacks. |
| WRK-11 | M / P1 | Multi-entry delete/move reports partial completion and stops on the first failure. | Add explicit confirmation for delete, selection-root normalization, and partial-result feedback. A timeout is not proof of failure; refresh before another explicit attempt. Never claim multi-file atomicity that the filesystem cannot provide. |
| WRK-12 | M / P2 | Drag/drop supports tree moves, hover folder expansion, and adding file references to chat without moving files. | Distinguish tree and conversation drop targets, internal references, and OS attachments. Escape cancels a drag. The source selection, target highlight, and operation scope must remain clear, including narrow windows and remote workspaces. |
| WRK-13 | M / P1 | Markdown files open in rendered preview with a source/editor toggle, workspace media, anchors, diagrams, and links. | Add preview as a view of the same document, not another untracked copy. Reflect edits, maintain safe owner-relative media resolution, bound parsing/decoding, and preserve dirty text when toggling. |
| WRK-14 | M / P1 | Image files open in the Files panel with zoom/pan and bounded owner-routed reads; the panel remains focused. | Input thumbnails do not satisfy workspace image inspection. Support common raster types and sanitized SVG according to an explicit policy, resource budgets, chunk consistency, cancellation, and modification/rename/delete updates. |
| WRK-15 | P / P1 | Repository creation, URL cloning, adding folders, and a remote-capable folder/drive browser are available in the app. | OpenAgents exposes a folder chooser and project configuration, but not the whole repository-management workflow. Add explicit create/clone/add operations with progress and failure recovery; remote paths and credentials stay with the owner. |
| WRK-16 | P / P1 | Branch/ref selection and worktree creation/reuse/deletion are visible session choices, with per-device settings. | Coder already creates isolated worktrees. Expose appropriate controls and cleanup with dirty/active-work protection. Display the actual selected branch and checkout; distinguish new worktree, reused worktree, and main checkout. |
| WRK-17 | M / P1 | Named Project Actions can be created, edited, deleted, selected, and run from the chat title bar. | General shell/agent tools are not a saved action UI. Store commands privately on the project owner and launch each in a fresh managed terminal. Keep the last successful action per project and show errors without altering chat execution. |
| WRK-18 | M / P0 | Repository-offered actions require explicit import; only one saved action can be selected as new-worktree setup. | Define trust and setup ordering before automation. Merely opening/cloning a repository must not authorize a command. Setup runs only for newly created worktrees, once per creation; setup failure leaves the checkout usable and is visible. |
| WRK-19 | M / P1 | Remote action/setup terminals attach to the originating chat even if selection changes while work starts. | Add command-scoped terminal handoff and idempotent request handling. A lost relay reply cannot strand the composer on Sending or open the terminal under a different chat. Creation acknowledgment and terminal attachment must tolerate different arrival order. |
| WRK-20 | P / P2 | Ignored/hidden visibility, autosave delay, wrapping, code font, and explorer widths have dedicated preferences. | Add file-specific settings after the surface exists. Defaults must protect large/ignored trees and malformed settings must not wedge the editor. Changing wrap/font preserves selection and scroll position. |

### File-service boundaries

Zeron's protocol has distinct directory, file-text, image, write, watch, move,
delete, and search operations. OpenAgents needs equivalent outcomes under its
own authorization model; it does not need matching method names. Important
request context includes the selected host, authorized workspace, checkout
identity, resource path, request identity, and expected content/metadata version.

Use separate mutable editor state and last-read disk state. A dirty buffer can
coexist with an external rename, a save in flight, and a later watcher event.
Those are normal cases, not reasons to reset the tab. Structural mutations must
coordinate with file writes and update tab/comment paths without losing text.
Cancellation after a filesystem call starts cannot promise undo.

Do not broaden file access by assuming a path printed by an agent is authorized.
Zeron distinguishes workspace-owned links from outside-root host-file reads.
OpenAgents must decide and enforce that boundary explicitly. Symlinks, protected
metadata, relative traversal, Windows device paths, and remote path syntax need
tests at the host boundary, not just validation in the explorer.

## Diffs and Git operations

Sources: Z08–Z10, Z12; OpenAgents O05–O07, O14.

| ID | Status / priority | Zeron behavior | OpenAgents gap and required acceptance |
| --- | --- | --- | --- |
| GIT-01 | P / P1 | Changes offers current working tree, branch-versus-base, and latest-turn scopes. | OpenAgents reviews a Coder task's exact base/head and can publish that reviewed result. Add the other scopes without weakening exact-review publishing. Scope labels, checkout, base ref, and stale state must match the rendered patch. |
| GIT-02 | M / P1 | A live checkout status watch tracks staged/unstaged/untracked paths, summary counts, and file-tree badges. | Task result summaries are not continuous repository status. Add owner-scoped status updates, watcher repair, and clean/error states, including changes made by the person or another tool outside Coder. |
| GIT-03 | P / P1 | Diffs support unified/split layout, wrapping, per-file collapse, syntax, counts, sticky structure, binary/rename/partial notices, and bounded virtualization. | OpenAgents already parses bounded unified diffs and highlights visible lines. Add missing layout/navigation controls and file-level context; verify rename/binary/truncated patches. Horizontal scroll must not hide comments or corrupt line anchors. |
| GIT-04 | P / P1 | Selecting a changed path opens its file; line notes from files/diffs stage into the next agent prompt. | Shared review comments exist in OpenAgents, but the desktop chat action set does not expose the full editor/diff note workflow. Mount add/edit/remove controls, old/new/renamed path anchors, staged-note summary, and stale-review refusal. |
| GIT-05 | M / P1 | Git history shows paged commits and a topological graph, refs, author/date/SHA columns, and commit details/diffs. | No matching desktop history surface was found. Add host-routed history reads, paging, commit selection, and persisted column preferences. Merge histories must retain graph relationships and large histories must not block the UI. |
| GIT-06 | M / P2 | History supports subject/SHA search, selectable comparison refs, optional author images, and explicit remote-ref fetch. | Add the secondary history workflow with bounded background requests. Fetch must not change HEAD or the index. Keep old results during refresh, reject late answers for another repository, and avoid treating avatar fetch as authorization to GitHub writes. |
| GIT-07 | M / P0 | Ref switching and discard are explicit owner-executed operations with current-checkout validation. | Shell access does not provide the equivalent controlled UI. Add dirty/active-work checks, destructive-action confirmation, expected-state validation, and stale/offline errors. Cancel must leave the checkout untouched; lost replies require status refresh before retry. |
| GIT-08 | P / P1 | Review surfaces retain prior content on read failure and distinguish preparing, clean, partial, stale, and failed states. | OpenAgents already refuses stale review publishing and has bounded completeness. Apply equivalent truthful state to new live/branch/turn diffs; a failed read must not become a falsely clean repository or remove the last useful patch. |
| GIT-09 | P / P1 | User-facing review and follow-up remain attached to the checkout they inspect. | OpenAgents Publish is an existing strength, not a missing feature. Preserve its exact reviewed revisions when adding general source-control tabs. Editing a file or receiving a newer head must invalidate applicable publish/comment actions until reviewed again. |
| GIT-10 | M / P1 | A Pull requests route lists repository/device-scoped requests with filters, status, checks, review state, and local bookmarks. | GitHub tools and issue operations elsewhere do not provide a native board. Add paged lists, repository selection, loading/stale/error states, and bookmarks that do not mutate GitHub. Scope changes must invalidate pending requests. |
| GIT-11 | M / P1 | Native pull-request detail shows description, conversation/review threads, commits/checks, changed files, and code diffs. | Add detail in the shared surface host and board side pane. Keep provider/repository/number identity, media limits, large-response fallback, and external-open action. A badge must not show cached metadata for another checkout. |
| GIT-12 | M / P1 | Pull-request links/badges open native detail by default, with an external-browser preference. | Existing external URL opening is partial coverage. Validate supported URLs, preserve focused chat/board context, and respect the selected destination. Unsupported links remain explicit external navigation, not guessed repository actions. |
| GIT-13 | M / P0 | Pull-request comments require explicit submission; drafts and permission/authentication/rate-limit failures are visible. | Add a distinct write authority and confirmation in the interaction itself. Rendering PR data never posts a comment. Preserve drafts and prevent duplicate posts after an uncertain result; fail clearly when the owning host lacks GitHub CLI/authentication. |
| GIT-14 | M / P1 | A pull request can seed a new project session with review/fix context staged in the composer. | Add a structured handoff with the selected host/project/repository and relevant request link. Do not send automatically, silently clone, or switch a checkout merely because someone opens detail. |
| GIT-15 | M / P1 | Review threads retain location, resolution state, and file context; the native code view can navigate those references. | Add typed review-thread projection and file/diff linking, separate from OpenAgents' local follow-up notes. Old/new lines and outdated threads must be identifiable; a missing diff cannot relocate a comment onto an arbitrary current line. |
| GIT-16 | V / P1 | Local and remote source-control requests use the same surface behavior and explicit unsupported-version errors. | Existing OpenAgents task review is host-backed, but new history/files/PR calls need capability checks. Validate old hosts, absent GitHub CLI, denied rights, deleted checkouts, offline operation, and stale responses without exposing raw command errors in product copy. |

### Source-control scope discipline

Zeron has `FetchAll`, `SwitchRef`, `DiscardWorkingTree`, multiple diff reads,
history reads, and pull-request reads/comment submission. This report does not
infer a complete native stage/commit/push/merge/rebase suite from those methods.
OpenAgents' own Publish workflow should remain available even if it differs from
Zeron's repository operations.

A useful release slice is read-only files plus current/branch diffs and history,
followed by guarded editing and explicit mutations. Read-only does not eliminate
host authorization requirements. It does reduce the number of simultaneous
write/conflict workflows needed for the first usable workspace.

## Required end-to-end checks

1. On an isolated local repository, edit a file, receive an external edit,
   resolve the conflict, rename it, and save while a background watch updates
   the tree. The dirty buffer and line comments must survive.
2. Repeat on a second temporary host with a different file at the same path on
   the viewer. Every read, save, search, and terminal action must use the owner.
3. Review an exact Coder change, then modify the checkout. A stale Publish or
   note action must refuse rather than applying to the newly changed head.
4. Open a large merge history, search a commit, select a branch diff, and keep
   chatting while it loads. Hide/reopen the pane without redoing expensive work
   on every streamed token.
5. Use fixtures for PR board/detail/threads/checks and submission failures. Any
   real comment test needs a dedicated test repository/account and explicit
   cleanup; the research task itself requires no GitHub writes beyond its docs.
