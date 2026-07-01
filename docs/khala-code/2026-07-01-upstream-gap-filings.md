# Khala Code Upstream Codex App-server Gap Filings

Date: 2026-07-01
Source matrix: `docs/khala-code/2026-07-01-codex-app-server-gap-matrix.md`
Tracking context: OpenAgents issue #7903 / T16.2

Status: draft upstream writeups for Codex app-server gaps. These are not public
product copy and do not change Khala Code promise state. They are written so an
operator can file narrow upstream Codex issues or PR notes without copying
private OpenAgents runtime details.

## Filing 1: `codex.app_server.gap.tui_preferences`

Title: Expose app-server preference metadata for TUI-owned appearance controls

Current Khala wrapper state:
Khala Code can already persist preference values through Codex app-server
`config/read`, `config/value/write`, and `config/batchWrite` for keymap, Vim
default mode, statusline, theme, pets, pet anchor, and personality.

Upstream gap:
Codex TUI still owns richer picker metadata and validation affordances for
`/keymap`, `/vim`, `/statusline`, `/theme`, `/pets`, and `/personality`. A
desktop wrapper can write known config keys, but it cannot present the same
option lists, labels, defaults, and validation semantics without duplicating TUI
logic.

Requested app-server shape:
Add a read method that returns preference descriptors for these controls:
stable key, current value, allowed values, display labels, default value,
validation errors, and whether a restart or new turn is required.

Acceptance:
- Desktop clients can render the same available options as Codex TUI without
  importing TUI picker code.
- Writes still go through existing config mutation methods unless Codex wants a
  dedicated preference mutation endpoint.
- Invalid values produce typed app-server errors.

## Filing 2: `codex.app_server.gap.memory_and_import_management`

Title: Add app-server methods for memories and AGENTS.md init semantics

Current Khala wrapper state:
Khala Code wraps existing app-server methods for `skills/list`,
`skills/config/write`, `skills/extraRoots/set`, `hooks/list`,
`externalAgentConfig/detect`, `externalAgentConfig/import`,
`externalAgentConfig/import/readHistories`, `fs/readFile`, `fs/writeFile`, and
`fs/getMetadata`.

Upstream gap:
`/memories`, `/init`, `/debug-m-drop`, and `/debug-m-update` still depend on
TUI-owned semantics. A desktop wrapper should not independently define memory
list, memory mutation, debug-memory behavior, or AGENTS.md initialization rules.

Requested app-server shape:
Expose memory list/read/write/delete operations and a narrow AGENTS.md init
operation that returns the intended file path, proposed contents or patch,
conflict state, and confirmation requirements. Debug memory operations can be
explicitly marked development-only.

Acceptance:
- Desktop clients can show memories and stage memory edits through app-server.
- AGENTS.md initialization behavior matches Codex TUI behavior.
- Debug memory commands are either available as typed development methods or
  explicitly unavailable with a stable reason code.

## Filing 3: `codex.app_server.gap.side_agent_plan_controls`

Title: Expose app-server controls for plan editing, side conversations, and subagents

Current Khala wrapper state:
Khala Code sends `/btw` active-turn side notes through `turn/steer`. Other
commands in this family return typed unavailable state rather than cloning TUI
state.

Upstream gap:
`/approve`, `/plan`, `/agent`, `/subagents`, and `/side` combine TUI command
parsing, popup state, turn state, and server-request state. Khala needs narrower
app-server methods or metadata before presenting equivalent desktop controls.

Requested app-server shape:
Add methods for reading and updating the active plan, listing subagents or side
threads, starting side conversations, injecting side-thread messages, and
approving guardian-denied or auto-review actions with typed status.

Acceptance:
- Desktop clients can render plan state without parsing TUI transcript text.
- Subagent and side-conversation controls round-trip through Codex-owned thread
  or turn state.
- Approval actions are explicit and typed; unavailable controls return stable
  reason codes.

## Filing 4: `codex.app_server.gap.ide_mentions_diff`

Title: Expand app-server IDE, mention, and diff metadata beyond the current wrapper slice

Current Khala wrapper state:
Khala Code already uses `fuzzyFileSearch`, `gitDiffToRemote`,
`fs/readDirectory`, `fs/readFile`, and `config/read` to project bounded mention
candidates, remote diff content, and IDE status.

Upstream gap:
The current methods cover the basic wrapper path, but richer IDE mutation and
metadata remain TUI or host-specific. Khala should not infer ignored-file
semantics, IDE attachment state, or mention insertion behavior outside Codex's
own workspace interpretation.

Requested app-server shape:
Expose richer IDE status and mention metadata: source, active workspace roots,
ignored or hidden reasons, candidate ranking details, insertion payloads, and
diff metadata suitable for desktop rendering.

Acceptance:
- Desktop clients can render mention and diff UI from app-server responses
  without recreating Codex workspace rules.
- IDE unavailable states are typed and distinguish no IDE, disabled IDE, and
  unsupported host.
- Diff metadata remains bounded and safe for UI display.

## Filing 5: `codex.app_server.gap.windows_sandbox_read_roots`

Title: Add app-server readable-root mutation for Windows sandbox setup

Current Khala wrapper state:
Khala Code can call `windowsSandbox/setupStart`, `windowsSandbox/readiness`,
`config/read`, and `config/value/write` where those methods cover setup and
readiness.

Upstream gap:
The `/sandbox-add-read-dir` behavior needs a precise app-server contract for
readable-root mutation. Khala should not implement Windows sandbox root mutation
by writing config heuristically or cloning TUI command behavior.

Requested app-server shape:
Add a method to validate and add a readable root, returning normalized path,
deduplication status, resulting readiness, and any required restart or setup
step.

Acceptance:
- Desktop clients can add readable roots through Codex-owned validation.
- Duplicate, invalid, unsupported-platform, and setup-required cases return
  typed errors or statuses.
- Existing setup/readiness methods remain the source of truth for sandbox state.

## Filing 6: `codex.app_server.gap.background_terminals`

Title: Stabilize background terminal list, clean, and terminate app-server methods

Current Khala wrapper state:
Khala Code wraps Codex's experimental
`thread/backgroundTerminals/list`, `thread/backgroundTerminals/clean`, and
`thread/backgroundTerminals/terminate` methods for `/ps`, `/stop`, `/clean`, and
explicit process termination. The adapter is bounded and tested, but product
copy treats the methods as unstable.

Upstream gap:
Background terminal state is Codex-owned, but the relevant app-server methods
are still experimental. Desktop wrappers need stable request/response shapes and
termination semantics before presenting them as durable product behavior.

Requested app-server shape:
Stabilize list, clean, and terminate methods with stable process identifiers,
thread/session association, command preview bounds, elapsed/runtime state,
exit/termination status, pagination, and typed not-found/already-exited errors.

Acceptance:
- Desktop clients can list and terminate background terminals without scraping
  transcript text or process tables.
- Termination is idempotent and returns typed final state.
- Experimental method names either become stable or return a migration path.
