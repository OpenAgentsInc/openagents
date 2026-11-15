# Private Projects API, v1alpha1

This is the first working consumer of the [Projects plan](../anthropic/implementation-plan.md),
`private-text-v1alpha1`. It supplies private Projects, instructions, reference knowledge
and personal Project chats to the mobile app. It is an owned OpenAgents API.

The schema is [project.proto](../../proto/openagents/project/v1alpha1/project.proto),
profile `projects`, descriptor `proto/descriptors/projects.pb`. The single JSON codec
is `coax/lib/project/api-wire.coax`; the descriptor oracle is `api-profile.coax`.
JSON uses the explicit snake case `json_name`s, integer millisecond timestamps and
null for absent nullable fields. Responses use `schema: "openagents.projects.api.v1alpha1"`
and `data` (or `error`). Mutation bodies are closed, route typed objects. They refuse
unknown or duplicate fields, invalid UTF-8, NULs and fractional revisions.

## Routes

`P` is `/api/projects/v1/prj_<32 lowercase hex digits>`. Every request uses the
existing verified person account session or NIP-98 key linked to that account.
An unlinked signer cannot access this API. Reads resolve that account's rows only;
a different owner receives 404. Writes also retain same-site, limited-account and
frozen-account restrictions. No creator grant is needed to create a private Project
or paste knowledge. `Project.url` points at its owned API resource in this native-first consumer.
No Project operation grants a repository, computer, World or
public publication capability.

| Method and path | Request / response |
| --- | --- |
| `GET /api/projects/v1?cursor=ID` | `ProjectList`, at most 32 summaries; `next_cursor` advances by stable ID |
| `POST /api/projects/v1` | `CreateRequest` → `ProjectReply`, 201 |
| `GET P` | Complete `ProjectReply` |
| `POST P/update` | `UpdateRequest` → `ProjectReply` |
| `GET P/instructions` | `InstructionsReply` |
| `POST P/instructions` | `InstructionsRequest` → `InstructionsReply` |
| `GET P/knowledge` | `KnowledgeList`, metadata for at most 200 sources |
| `POST P/knowledge` | `KnowledgeRequest` → `KnowledgeReply`, 201 |
| `GET P/knowledge/K/content` | `KnowledgeContentReply`, the complete extracted text |
| `POST P/knowledge/K/remove` | `RemoveRequest` → `ActionReply` |
| `GET P/chats` | Existing `openagents.chat.history.v1` response; `folder` is the Project ID |
| `POST P/chats` | `ChatLinkRequest` → `ProjectReply`; links an owned saved account thread |
| `GET P/chats/THREAD` | Existing `openagents.chat.thread.v1`, scoped to this Project and account |
| `POST P/chats/new` | Additive `openagents.projects.chat.v1alpha1.WriteRequest` → saved thread, 201; atomically creates and links a user turn |
| `POST P/chats/THREAD/messages` | Same additive request → saved thread; appends a user turn after an assistant reply |
| `POST P/complete` | `ChatCompleteRequest` → existing non-streaming Chat Completions response |
| `GET P/context?operation_id=OP` | `ContextReply`, the context admitted for that attempt |

```json
{"operation_id":"a-client-generated-identity","value":{"name":"Research","instructions":"Cite the Project notes."}}
```

```json
{"operation_id":"another-identity","expected_revision":1,"value":{"title":"Launch notes","source":{"kind":"text","text":"The code is violet."},"selected":true}}
```

Encoded mutation bodies are at most 131,072 bytes. `operation_id` is 1–80 UTF-8 bytes. The client retains it with the exact request
until its result is known. All existing-Project writes require `expected_revision`.
Every metadata, knowledge or chat-association change advances the Project revision; instructions retain their
own revision, timestamp and SHA-256. A stale write returns 409. A retry of an
already committed operation returns its stored status/body; reusing its ID with a
different path or body returns 409. Lists and errors are private and non-cacheable.

## Knowledge and context

Names are 1–160 bytes; descriptions at most 4,096; instructions at most 65,536.
Knowledge titles are 1–256 bytes. Text sources are 1–65,536 bytes of UTF-8.
File sources reference an existing owned, scanned `drops.files` ID. This consumer
accepts `.txt` and `.md` files up to 64 KiB and stores the complete verified UTF-8
text. It reuses the installed document picker, credential-free direct PUT and scan
workflow. Starting a file upload retains that service's existing upload grants
and quotas. It does not create another object store or bypass a scan. Removing
knowledge removes its context selection; it does not delete the original Drop.

Metadata reports extraction version `utf8-text/1`, byte length and SHA-256. Token
counts are null: this consumer does not pretend a byte estimate is a tokenizer.
Every selected source is included in the next turn, following the explicit
instructions as delimited reference material. The whole instructions/reference
payload must fit 32,768 UTF-8 bytes; an oversized context is refused before asking
the model. Nothing is silently truncated or fetched from an unowned file ID.

An admitted attempt keeps the Project/instructions/knowledge revisions and hashes,
model ID and existing conversation ID. The private context row also retains the
exact model input JSON, so later instruction edits or knowledge removal cannot
change the record of what was sent. The public response exposes only the snapshot,
never the internal model request or a signing identity.

## Saved chats and mobile behavior

The existing account transcript protocol remains unchanged: 32 text messages,
48,000 aggregate text bytes and its existing per-message bounds. History lists
retain the existing 60-row window. A user turn is
saved first, its thread is linked to one Project, then completion names its saved
revision. Neither the old slug API nor this API can reassign that thread to a
different Project. Project conversations are personal to their account.

Completion reads the saved transcript, instructions and selected knowledge from
the server; it accepts no caller-supplied context or model override. It uses the
existing funded Hydrogen model adapter and account quotas. The current team beta
funding requirement still applies. The assistant reply and operation receipt are
committed before a successful response is sent. The mobile **Stop waiting** action
ends the client wait; an admitted server attempt may still finish and save its reply. Replaying a completed request
returns that response without another model call. An in-progress or failed attempt
is never automatically dispatched again; the client reopens its saved chat to
check the result. A saved thread revision admits only one model attempt, even with different operation
identities. A fresh attempt requires another saved user turn and a new operation identity.

The CLI uses the additive chat-write schema in
`proto/openagents/project/chat/v1alpha1/chat.proto` and its one codec
`coax/web/api/projects-chat-wire.coax`. A request carries `schema`, `operation_id`,
`expected_revision` and `value.content` (1–16,384 UTF-8 bytes). Append also requires
`value.thread_revision`; create refuses that field. New-chat creation advances the
Project revision once; appending a message advances only the thread revision.
Both save the transcript and operation receipt in one transaction. A lost create
response can be retried without making another thread. The existing published
Project and account-thread schemas keep their identities.

## Command line

`openagents projects` exposes every operation above using the same account and
codecs as the phone. [CLI commands and examples](../../apps/openagents-cli/README.md#account-projects)
cover creation, metadata, instructions, knowledge, chat creation/linking,
continuation, completion, scoped reopening and context inspection. The singular
`project` namespace retains the older public Nostr work queue.

Mutating commands print an operation ID on stderr before sending. To reconcile
an uncertain response, rerun the same command, revision, file bytes and options
with `--operation-id ID`. Chat new/send also derive a stable completion operation
from that ID, so repeating the whole command replays both stages. `--save-only`
saves the user turn without starting inference; `chat complete` exposes the
completion stage explicitly. Successful output is the full API JSON, without
the ordinary CLI report's 64 KiB truncation boundary.

The native Projects list exposes **New Project** for this private flow and a
separate **Publish a public project** for existing drafts. Native screens create
Projects, edit instructions, add/read/remove knowledge, attach supported files,
and start/list/reopen Project chats. The shared account transcript, Markdown,
composer and native document effects are reused. Private Project chats also reopen
from account history with their Project association intact. Logout clears private
state. Unknown write results retain their original operation for reconciliation.
The phone editor accepts 20,000 bytes and refuses an oversized encoded request;
it never replaces a larger saved instruction with an empty editor.

## Subsequent consumers

The full plan remains the field catalog and rollout direction. Sharing, Workspaces,
Claude import/export, rich conversation blocks, configurable models, retrieval,
PDF/Office extraction, knowledge updates, archive/delete, browser screens and World,
repository, work, agent and environment connections are subsequent work. Fields
that anticipate these features are fixed or null here: active/private status,
Hydrogen/low model, selected knowledge mode and zero connection counts. Unsupported
write fields are rejected. This profile makes no Anthropic endpoint-parity claim.
Existing published realm wire identities keep their names; new product terminology
and new fields use **Worlds**.
