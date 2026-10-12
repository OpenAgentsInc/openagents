# Projects, threads, and files (proposed, 2026-10-12)

Your projects, the chats in them, and the files you keep there, readable and
writable by you through the API, the `openagents` command, and Coder. The
resource shapes follow Anthropic's content API for chats, files, and projects
(`/v1/compliance/apps/...`), without the compliance layer: there is no
organization-wide reviewer key and no eDiscovery scope. Every route answers
for **your own** data, or for a workspace you are a member of with the right
role, on the [API's shared contract](calling.md): `oak_` keys with scopes,
`sess_` sessions, cursor lists, one error object.

Tracking: [#11471](https://github.com/OpenAgentsInc/openagents/issues/11471).
The owner's ask: "our CLI needs to interact with that via api first class".

## Contents

- [What exists today](#what-exists-today)
- [Setup: credentials and base URL](#setup-credentials-and-base-url)
- [Objects](#objects)
- [Projects](#projects)
- [Threads and messages](#threads-and-messages)
- [Files, generated files, and artifacts](#files-generated-files-and-artifacts)
- [Project attachments and documents](#project-attachments-and-documents)
- [Deleting](#deleting)
- [Scopes and privacy](#scopes-and-privacy)
- [The `openagents projects` command](#the-openagents-projects-command)
- [Coder](#coder)
- [Compared with Anthropic's content API](#compared-with-anthropics-content-api)
- [Build and deploy](#build-and-deploy)

## What exists today

| Piece | Where | State |
| --- | --- | --- |
| A project is one repository that groups the owner's chats | `crates/openagents-web/src/projects/`, gateway `crates/gateway/src/github_repos.rs` | Live on the website (sidebar: Projects) |
| Add and remove a project | gateway `POST /v1/projects`, `DELETE /v1/projects/{id}` (older `/v1/account/projects`) | Built, first-party |
| List projects | gateway `GET /v1/account/github` ("connected GitHub access and projects") | Built, first-party |
| Drive sources on a project | web `/projects/{id}/sources` (`crates/openagents-web/src/connections/sources.rs`) | Website pages only |
| A chat's project | web `/chat/{id}/project`; `Conversation::project` | Website only |
| Threads over HTTP for the apps | `GET /v1/threads`, `GET /v1/threads/{id}`, `POST /v1/threads/{id}/messages` (`phone_api.rs`) | Built (staging), app token |
| Account export | `crates/openagents-web/src/account_export.rs` (projects and chats) | Live |

What is missing:

- **Credentials.** Coder's sign-in token (`~/.openagents/coder/account.json`) gets
  `404` on `https://openagents.com/v1/account/github` and
  `https://openagents.com/api/v1/projects`. No project read route takes an
  `oak_` key or the CLI's sign-in.
- **CLI.** `openagents project` is the **GitHub Project board** command. No
  command lists openagents.com projects. That is why a Coder chat asked
  about "my finances project" searched GitHub boards and Claude Docs.
- **Project data.** No route reads a project's chats, or adds a file or a
  text document to a project.

## Setup: credentials and base URL

Base URL `https://api.openagents.com/v1`. The same routes answer at
`https://openagents.com/api/v1`.

| Caller | Credential | How to get it |
| --- | --- | --- |
| The `openagents` command and Coder | The sign-in token `openagents login` saves (Coder's `account.json`) | `openagents login` (approve the code at openagents.com/device), or `openagents login --key` with a sign-in key from Settings |
| Scripts, agents, partner apps | `Authorization: Bearer oak_<id>.<secret>` with the scopes below | Settings, API keys (organization administration) |
| The website and apps | `sess_` session or app token | Signing in |

A personal account has one personal workspace, which the call uses when you
send no `X-Workspace-Id`. A team workspace's projects need the header and a
member role ([workspace rules](calling.md#workspace)).

## Objects

| Object | Id prefix | What it is |
| --- | --- | --- |
| Project | `proj_` | A named home for chats, instructions, files, and documents. Optionally bound to one repository (today every project is one). |
| Thread | `thr_` | One chat: web, terminal (Coder), or phone. Belongs to at most one project. |
| Message | `msg_` | One turn in a thread, with content blocks (`text`, `tool_use`, `tool_result`, `image`) and any files, generated files, and artifacts. |
| File | `file_` | Bytes you uploaded: to a message or to a project. The same object the [Model calls](model-calls.md) and [Managed Agents](managed-agents.md) pages use. |
| Generated file | `genf_` | Bytes a tool produced during a turn (a PDF, a CSV, a slide deck). |
| Artifact | `art_`, version `artv_` | A versioned document a reply produced or revised. |
| Project document | `pdoc_` | Plain text kept on a project: a pasted note, or a document converted to text when added. |
| Project source | `psrc_` | A connected source on a project, such as a Drive folder or file. |

Times are RFC 3339 UTC. A deleted thread lists with `deleted_at` set, an
empty `title`, and messages without content until it is purged.

## Projects

| Route | Does |
| --- | --- |
| `GET /v1/projects` | List your projects, oldest first. `?limit`, `?after`. |
| `POST /v1/projects` | Create one: `{"name", "instructions"?, "repository"?}` (the repository form replaces today's add-a-repository). |
| `GET /v1/projects/{id}` | One project, with counts of its threads, files, documents, and sources. |
| `PATCH /v1/projects/{id}` | Rename, set instructions, or bind or unbind a repository. |
| `DELETE /v1/projects/{id}` | Delete it. `409 project_has_threads` while threads are attached, unless `?detach_threads=true` (threads stay and lose their project). |

```bash
curl -sS https://api.openagents.com/v1/projects \
  -H "Authorization: Bearer $OPENAGENTS_KEY"
```

```json
{
  "data": [
    {
      "id": "proj_01JB7Q4XG0N2Y9R0C3V8P5W6E1",
      "object": "project",
      "name": "finances",
      "instructions": null,
      "repository": "AtlantisPleb/finances",
      "created_at": "2026-10-01T15:02:11Z",
      "updated_at": "2026-10-12T20:41:09Z",
      "counts": {"threads": 3, "files": 0, "documents": 0, "sources": 0}
    }
  ],
  "next": null
}
```

A project may also be named in the path by its exact name when it is unique
for the caller (`/v1/projects/name:finances`), so the CLI and Coder can use
the name a person says.

## Threads and messages

| Route | Does |
| --- | --- |
| `GET /v1/threads` | List your threads. Filters: `project` (an id, or `none`), `surface` (`web`, `terminal`, `phone`), `created_after`/`created_before` or `updated_after`/`updated_before`, `order` (`created_at` default, or `updated_at`). Ascending, ties broken by id. |
| `GET /v1/threads/{id}` | One thread's metadata. |
| `GET /v1/threads/{id}/messages` | Its messages, oldest first, whole by default. `?limit`, `?after`, `?order=desc` for long threads. |
| `PATCH /v1/threads/{id}` | Title, pinned, archived, or `project` (move into or out of a project). |
| `DELETE /v1/threads/{id}` | Delete the thread, its messages, and files attached to its messages. |

Keeping a copy current is one loop: list with `order=updated_at`, keep the
last `next` cursor, and resume from it next time. A thread comes back when it
gets a new message, moves into or out of a project, or is deleted. Handle
each thread by `id`, idempotently. A cursor is bound to its `order`. An
`updated_*` bound goes with `order=updated_at`.

A message:

```json
{
  "id": "msg_01JB7R1M7S0Q8KD2W6Z3Y4N5TA",
  "object": "message",
  "thread": "thr_01JB7QZ8C2P4N6R8T0V2X4Z6B8",
  "role": "assistant",
  "created_at": "2026-10-12T20:40:59Z",
  "model": "claude-haiku-5-5",
  "content": [{"type": "text", "text": "Here are this month's totals…"}],
  "files": null,
  "generated_files": [
    {"id": "genf_01JB7R2…", "filename": "october.csv", "mime_type": "text/csv", "size_bytes": 2048, "sha256": "…"}
  ],
  "artifacts": [
    {"id": "art_01JB7R3…", "version": "artv_01JB7R3…", "title": "October budget", "artifact_type": "text/markdown"}
  ]
}
```

A Coder or cloud run started from a thread shows its work in the same
messages, as `tool_use` and `tool_result` blocks.

## Files, generated files, and artifacts

| Route | Does |
| --- | --- |
| `POST /v1/files` | Upload (multipart, `purpose`: `message` or `project`). Returns a `file_`. |
| `GET /v1/files/{id}` | Metadata: `filename`, `mime_type`, `size_bytes`, `sha256`, `created_at`, `stored_as` (`original` or `extracted_text`). |
| `GET /v1/files/{id}/content` | The stored bytes, streamed, with `Content-Disposition: attachment; filename*=utf-8''…`, `Content-Type`, and a `Digest: sha-256=…` header for the served bytes. |
| `GET /v1/generated_files/{id}`, `.../content` | The same for a tool's output. |
| `GET /v1/artifacts/{version}` | One artifact version: `{"id", "version", "title", "artifact_type", "content"}`. |

What is stored can differ from what was uploaded. An image may be kept as a
processed copy, and some documents as their extracted text under the
original name (`stored_as: extracted_text`). Read the format from the bytes,
not the name. The file name is the uploader's, so a client never uses it as
a path without cleaning it.

## Project attachments and documents

A project holds three kinds of attachment, told apart by `type`:

| `type` | Id | What | Read with |
| --- | --- | --- | --- |
| `file` | `file_` | An uploaded file (PDF, image, spreadsheet) | `GET /v1/files/{id}/content` |
| `document` | `pdoc_` | Plain text: a note, or a document converted to text when added | `GET /v1/projects/{project}/documents/{id}` |
| `source` | `psrc_` | A connected source (a Drive folder or file) | `GET /v1/projects/{project}/sources/{id}` |

| Route | Does |
| --- | --- |
| `GET /v1/projects/{id}/attachments` | List all three kinds, oldest first, ties by id. `?limit`, `?after`. |
| `POST /v1/projects/{id}/attachments` | Attach `{"file": "file_…"}`, add `{"document": {"title", "text"}}`, or connect `{"source": {"kind": "drive", "ref": "…"}}`. |
| `GET /v1/projects/{id}/documents/{doc}` | A document's text and metadata. |
| `DELETE /v1/projects/{id}/attachments/{att}` | Remove an attachment. A file attached only here is deleted with it. |

Uploading private data to a project is two calls, or one for text:

```bash
id=$(curl -sS https://api.openagents.com/v1/files \
  -H "Authorization: Bearer $OPENAGENTS_KEY" \
  -F purpose=project -F file=@statements-2026-09.pdf | jq -r .id)
curl -sS https://api.openagents.com/v1/projects/name:finances/attachments \
  -H "Authorization: Bearer $OPENAGENTS_KEY" -H "Content-Type: application/json" \
  -d "{\"file\":\"$id\"}"
```

Files and documents live in the account's private storage (the per-account
bucket in [account storage](../deployment/account-storage.md)), encrypted at
rest, readable only by the account's own credentials. They are never used to
train a model, and they are sent to a model only when a turn in that project
reads them.

## Deleting

Every delete is immediate and permanent. There is no recovery window.

| Route | Deletes | Answer |
| --- | --- | --- |
| `DELETE /v1/threads/{id}` | The thread, its messages, and files attached to them | `{"id", "object": "thread.deleted"}` |
| `DELETE /v1/files/{id}` | A message file or a project file | `{"id", "object": "file.deleted"}` |
| `DELETE /v1/projects/{id}/attachments/{att}` | One attachment | `{"id", "object": "attachment.deleted"}` |
| `DELETE /v1/projects/{id}` | The project (`409 project_has_threads` unless `?detach_threads=true`) | `{"id", "object": "project.deleted"}` |

Check `object` before treating a delete as done.

## Scopes and privacy

New public scopes for `oak_` keys ([design 2.7](design.md#27-scopes-for-oak_-keys)):

| Scope | Allows |
| --- | --- |
| `projects:read` | List and read projects, attachments, documents |
| `projects` | Create, change, attach, delete |
| `threads:read` / `threads` | (existing names) Read threads and messages / change and delete them |
| `files:read` / `files` | Download / upload and delete files |

Sessions and the CLI's sign-in carry all of these for the person's own
account. Every route checks that the caller owns the object, or is a member
of its workspace with a role that allows the action. Another account's id
answers `404`, never `403`, so ids don't leak. Data class: Account
([design 2.11](design.md#211-privacy-classes)).

## The `openagents projects` command

```text
openagents projects list [--json]
openagents projects show NAME|ID [--json]
openagents projects threads NAME|ID [--json]
openagents projects add NAME [--repository OWNER/REPO] [--instructions TEXT]
openagents projects remove NAME|ID [--detach-threads]
openagents projects files list NAME|ID
openagents projects files add NAME|ID PATH...
openagents projects files get NAME|ID FILE --output PATH
openagents projects files remove NAME|ID FILE
openagents projects notes add NAME|ID --title T (--text TEXT | --file PATH)
```

It uses the sign-in `openagents login` saved. The GitHub Project board
command becomes `openagents board`, with `project` kept as a hidden alias
for one release.

## Coder

Coder reaches projects through its `openagents_cli` tool on every engine,
and Claude Code gets it through Coder's tool server. The tool's description
says that "the person's projects" are openagents.com projects
(`openagents projects`). It is the description that steers the model; no
keyword routing. A Coder chat in a project's repository reads that project's
instructions and documents as part of its context, once the context layer
lands ([#11451](https://github.com/OpenAgentsInc/openagents/issues/11451), phase 3).

## Compared with Anthropic's content API

| Anthropic (`/v1/compliance/apps/...`) | Here | Difference |
| --- | --- | --- |
| `GET chats` (org-wide, `user_ids[]`, `order_by`) | `GET /v1/threads` | Your own threads; `order` instead of `order_by`; no `user_ids[]` (a workspace admin view is a later, separate route) |
| `GET chats/{id}/messages` | `GET /v1/threads/{id}/messages` | Same shape: content blocks, `files`, `generated_files`, `artifacts` |
| `GET chats/files/{id}/content`, metadata | `GET /v1/files/{id}/content`, `GET /v1/files/{id}` | `sha256` instead of MD5; `stored_as` says when text was extracted |
| `GET chats/generated_files/{id}` | `GET /v1/generated_files/{id}` | Same |
| `GET artifacts/{version}` | `GET /v1/artifacts/{version}` | Same |
| `GET projects`, `GET projects/{id}` | `GET /v1/projects`, `GET /v1/projects/{id}` | Ours also create and change |
| `GET projects/{id}/attachments` (`project_file`, `project_doc`) | `GET /v1/projects/{id}/attachments` (`file`, `document`, `source`) | Adds connected sources; ours also attach |
| `GET projects/documents/{id}` | `GET /v1/projects/{id}/documents/{doc}` | Scoped under its project |
| Deletes, `delete:compliance_user_data` | Deletes, `threads` / `files` / `projects` scopes | Your own data, ordinary scopes |
| Cursors `first_id`/`last_id`/`has_more`; projects `next_page` | One shape everywhere: `{data, next}` with `?after` | [design 2.5](design.md#25-requests-and-responses) |
| Compliance Access Key | `oak_` key, session, or the CLI sign-in | Available to everyone |

## Build and deploy

1. **Read routes and credentials** (#11471): `GET /v1/projects`, `GET /v1/projects/{id}`, `GET /v1/threads?project=…`, reachable with an `oak_` key and the CLI sign-in; the gateway's OpenAPI regenerated (`OPENAGENTS_WRITE_OPENAPI=1 cargo test -p gateway audience`).
2. **CLI** `openagents projects list | show | threads` and the `board` rename.
3. **Coder** tool descriptions.
4. **Files and documents**: `/v1/files`, attachments, documents, private per-account storage.
5. **Changes and deletes**, then scopes on the Settings key picker.

Deploying the website and gateway follows `scripts/deploy/web.sh`:
`stage` (staging smoke), `promote DIGEST` (a no-traffic revision tagged
`new`, smoked), `shift` (all traffic), and `rollback` if a check fails after
the shift. Check the serving revision right before shifting, because
another session may have deployed since staging.
