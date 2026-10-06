# Workbench resource references and surface intents

Status: implemented contract, 2026-10-05
([#10645](https://github.com/OpenAgentsInc/openagents/issues/10645)). The
code is [`crates/workbench`](../../crates/workbench/src/lib.rs); its
fixtures are
[`crates/workbench/fixtures/workbench.json`](../../crates/workbench/fixtures/workbench.json).

Every workbench surface refers to the resources it shows in one way: the
standalone window, Verse's `T` overlay, and later the phone, the browser,
and headless clients. A reference names a resource that another owner
keeps. It is not a copy of the record and grants nothing; the owner checks
the caller's rights on every intent. The [workbench roadmap](workbench-roadmap.md)
describes the panes this enables.

## Resource references

A reference, `openagents.workbench-resource.v1`, has `kind`, `host`, `id`,
and, as its kind requires, `generation`, `workspace`, `part`, and
`revision`. `host` is a paired host (`{kind: "paired", key}`, its NIP-HOST
public key) or a surface's own process (`{kind: "local", instance}`, a
random ID minted at start). A revision is `{"counter": n}`,
`{"sha256": digest}`, or `{"version": text}`.

| Kind | `id` | Generation | Workspace | Revision |
| --- | --- | --- | --- | --- |
| `terminal` | The NIP-TERM terminal ID, or a local pane number | Required | Optional | None |
| `thread` | The thread ID | None | None | Optional counter |
| `run` | The run or task ID | None | Optional | Optional counter |
| `studio` | The record's ID, with `part`: `goal`, `seat`, `task`, `decision`, or `review` | None | Optional | Optional counter; a review requires the SHA-256 of the exact revision it reviews |
| `artifact` | Its SHA-256 content digest | None | None | None |
| `file` | A path relative to the workspace, without `.` or `..` components | None | Required | Required SHA-256 of the content |
| `tool` | `<publisher>:<package>/<component>` | None | None | Required version |
| `evidence` | The Gym or check evidence ID | None | None | Optional SHA-256 |

A field its kind does not take, a missing required field, or a revision of
the wrong form refuses as `malformed`. Two references name the same
resource when everything but `revision` matches.

## Directories and intents

An owner publishes a directory, `openagents.workbench-directory.v1`: its
host, its current generation, and one capability per kind it serves. A
capability lists the operations it serves (`open`, `attach`, `detach`), the
owner intents a surface may route by name (`answer-decision`), and an
optional fallback: an `https` link with `{id}` in it, or a per-resource
plain-text summary the owner writes.

A surface sends an intent, `openagents.workbench-intent.v1`: a request ID,
the target reference, and an action (`open`, `attach` with a mode, `detach`
with an attachment, or `owner` with an intent name and an object input of
at most 4,096 bytes). The surface never performs an owner intent itself.

`workbench::dispatch` is the one path every surface uses:

1. It refuses a malformed intent, and a target on another host as
   `identity_mismatch`. A surface never asks one owner about another's
   resource.
2. When the directory has no capability for the kind, the outcome is
   `unsupported`. When the operation or owner intent is not served, the
   outcome is the directory's link fallback for an open, or `unsupported`.
   The owner is not asked.
3. When the target's generation differs from the directory's, the outcome
   is `lost`. The owner is not asked, and nothing is created in its place.
4. Otherwise it asks the owner and checks the outcome: the request and the
   target must be exactly the intent's, including the revision, the state
   must answer the action, and a fallback must be the one the directory
   permits. Anything else refuses as `identity_mismatch` or `malformed`.

An outcome, `openagents.workbench-outcome.v1`, has one state:

| State | Meaning |
| --- | --- |
| `opened`, `attached`, `detached`, `routed` | Supported: the surface draws the resource, or the owner accepted the owner intent and names its own receipt. |
| `fallback` | Permitted fallback: the surface shows the owner's link or summary instead of a native pane. |
| `unavailable` | The owner cannot answer now. Nothing changed. |
| `stale` | The reference's revision is not current. The owner names the current revision; the surface asks again only when the person chooses. |
| `closed` | The resource ended in this generation. |
| `lost` | The resource belonged to an earlier owner generation. |
| `unsupported` | The owner does not serve the kind or operation. |
| `refused` | The owner refused, with a shared refusal code. |

## Product panes

Added 2026-10-05 ([#10646](https://github.com/OpenAgentsInc/openagents/issues/10646)).
A product pane shows a resource beside the shells: `workbench::pane`
defines its descriptor, `openagents.workbench-pane.v1`, and the adapter
each domain writes for its own kind. The kinds are closed: `thread`, `run`,
`studio`, `file`, `diff`, `artifact`, `preview`, `knowledge`, `evaluation`,
`background`, `account`, and `receipt`. A kind that version-1 references
name takes one as its subject (a diff and a file take a `file`, an
artifact and a preview an `artifact`, an evaluation `evidence`); the
others name the owner's own record by host, ID, and optional revision. A new
kind is a new schema version.

A descriptor carries the subject exactly as asked, a plain title of at most
128 bytes, a plain detail of at most 2,048 bytes, the owner intents the pane
offers (at most eight), and a state:

| State | Meaning |
| --- | --- |
| `ready` | The adapter describes the resource; its actions are offered, and an empty list means read-only. |
| `missing` | The owner keeps no such record. Nothing is created in its place. |
| `stale` | The reference names an older revision; the owner names the current one. |
| `revoked` | This device no longer holds the right to it. |
| `unavailable` | The owner cannot answer now, or its adapter answered with something a pane may not show. |
| `fallback` | No adapter here: the kind's declared view, a TTY command (`{id}` replaced) or an `https` link, or the label alone, under "No *kind* viewer here". |

`Panes::resolve` is the one path: it checks that the subject fits the kind,
asks only that kind's adapter, keeps the asked subject rather than any the
adapter might name, and drops every action from a pane that is not ready.
An adapter only reads; the trait gives it no way to create, start, or
change anything, so navigation, focus, layout, and what a pane shows stay
in the workbench, and stores, execution, and the admission of each action
stay with the owner. `openagents_chat::pane::ThreadPanes` is the thread
adapter, over the chat service's read operations. `terminal-core` keeps a
mount's open product panes (`Application::products`, the `pane` control
request, and the `products` entry of its status), with TTY fallbacks for a
thread (`openagents chat read --thread {id}`) and a run (`coder task show
{id}`), so the Grid's overlay and the standalone window resolve the same
resources to the same pane state. Account and receipt panes take the retail
cloud track's adapters when those land.

The sheet draws a thread natively
([#10659](https://github.com/OpenAgentsInc/openagents/issues/10659)): F4
shows the thread its questions go to in place of the transcript
(`terminal_core::thread`). The mount's transport reads it through the shared
chat client's `openagents --json chat read --thread ID`, the same thread ID
the TTY fallback reads, so there is one client and one thread store. A read
answers only for the thread asked; a thread the client does not keep is
`missing`, and one it cannot read is `unavailable`. Opening, reopening, and
refreshing only read, and ENTER on the page sends the line once, as the
sheet's next question to that thread.

## Consumers

`terminal-core` is the owner of a mount's local panes. The standalone
window and Verse's overlay both mount its `Application`, so both use this
contract the same way: each pane's `status` entry carries its `resource`,
the status carries the mount's `directory`, and the control request
`{"op": "resolve", "intent": ...}` answers an intent through `dispatch`. A
mount's instance ID is also its generation, so a pane reference kept from an
earlier run is `lost`, and opening it never starts a shell. The headless
fixture in `crates/workbench/tests/contract.rs` drives a fake owner through
a supported open, a link and a summary fallback, a stale file, a lost
terminal, an unavailable owner, unsupported kinds and intents, a cross-host
target, and a substituting owner, and checks that the owner created nothing.

## Compatibility and migration

- **Strict version 1.** Each body's `v` names its schema. A reader refuses
  an unknown field, kind, state, or enum value as `malformed`, and another
  `openagents.workbench-*` version as `unsupported_version`, rather than
  guessing.
- **Additions need a new version.** A new kind, field, or state is a new
  schema version, or a separately named feature, so a version-1 reader
  never acts on what it cannot check.
- **Migration.** There is no earlier schema. When version 2 arrives, a
  reader of both converts version 1 references by rule; a version-1 reader
  refuses version-2 bodies, and a surface shows the resource as unsupported
  rather than as a different resource.
- **Persisted references** (session members in [NIP-TERM](../../nips/openagents/NIP-TERM.md#sessions),
  saved layouts) store the reference whole. A host stores a session's
  resource members without resolving them, so a reference survives an owner
  restart and resolves as `lost`, `stale`, or current when it is next used.
