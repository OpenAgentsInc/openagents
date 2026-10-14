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
- [Built so far (2026-10-12)](#built-so-far-2026-10-12)
- [Setup: credentials and base URL](#setup-credentials-and-base-url)
- [Objects](#objects)
- [Projects](#projects)
- [Members and invitations](#members-and-invitations)
- [Threads and messages](#threads-and-messages)
- [Files, generated files, and artifacts](#files-generated-files-and-artifacts)
- [Project attachments and documents](#project-attachments-and-documents)
- [Sensitive files: encrypted before they leave your machine](#sensitive-files-encrypted-before-they-leave-your-machine)
- [Deleting](#deleting)
- [Scopes and privacy](#scopes-and-privacy)
- [The `openagents projects` command](#the-openagents-projects-command)
- [Coder](#coder)
- [Compared with Anthropic's content API](#compared-with-anthropics-content-api)
- [Build and deploy](#build-and-deploy)

## What exists today

| Piece | Where | State |
| --- | --- | --- |
| A project groups the owner's chats, with a name, instructions, and zero or more repositories (one primary) | `crates/openagents-web/src/projects/`, gateway `crates/gateway/src/github_repos.rs`, `oa_auth::repos` | Live on the website (sidebar: Projects); repositories as a list since 2026-10-13 |
| Create, change, and remove a project | gateway `POST /v1/projects`, `PATCH /v1/projects/{id}`, `DELETE /v1/projects/{id}` (older `/v1/account/projects`) | Built, first-party |
| List projects | gateway `GET /v1/account/github` ("connected GitHub access and projects") | Built, first-party |
| Drive sources on a project | web `/projects/{id}/sources` and `GET`/`POST /v1/projects/{id}/sources`, `DELETE /v1/projects/{id}/sources/{source}` (`crates/openagents-web/src/connections/sources.rs`, [connections.md](connections.md)) | Live for accounts allowed Connections (#11574) |
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

## Built so far (2026-10-12)

Steps 1 to 4 below are built (step 4, files, on 2026-10-12 as well). Where the proposal above and below differs
from the code, this is how it works today:

| Route (served by the website, `https://openagents.com/v1/...`) | Answer |
| --- | --- |
| `GET /v1/projects` | `{data: [Project], next}`, oldest first, `?limit` (default 100, at most 200) and `?after` |
| `POST /v1/projects` `{"name", "instructions"?, "repositories"?: ["owner/name", …], "protection"?}` | `201` Project. Always a new project, even when another has the same name or repository. A name is enough; the first repository is the primary. `{"repository": "owner/name"}` alone (the older form) makes a project named after it |
| `POST /v1/projects/{id}/repositories` `{"repository": "owner/name", "primary"?: true}` | The Project. Adds a connected repository; the first one a project gets is its primary |
| `DELETE /v1/projects/{id}/repositories/{owner}/{name}` | The Project. Removing the primary makes the first one left the primary; removing the last leaves a project with no repository. `404 not_found` when it isn't in the project |
| `GET /v1/projects/{id}` | One Project; `{id}` is its id or `name:NAME`. When several projects share the name: `409 ambiguous_name`, whose message names each (`2 projects are called money: prj_… (owner/repo), prj_… (no repository). Use its id.`) |
| `DELETE /v1/projects/{id}` | `{id, object: "project.deleted"}`; `409 project_has_threads` while chats are in it, unless `?detach_threads=true` |
| `GET /v1/threads?project=ID\|name:NAME\|none` | `{data: [Thread], next}`, by last activity, oldest first, `?limit`, `?after` |
| `GET /v1/threads/{id}/messages` | `{data: [Message], next}`, oldest first; each message's `content` is one `text` block |
| `POST /v1/threads` `{"request_id": UUID, "text", "project": ID\|"name:NAME"}` | `201 {id, answering: true}`: a new chat inside the project, answered by OpenAgents with the project's instructions and its primary repository (a project with no repository: a plain chat with its instructions; a project with Drive sources: answered from them, photos and videos included, see [connections.md](connections.md)), as starting a chat in a project on the website does ([#11542](https://github.com/OpenAgentsInc/openagents/issues/11542)). `404 not_found` when the project isn't yours. Read the reply with `GET /v1/threads/{id}` |
| `PATCH /v1/threads/{id}` `{"project": ID \| "name:NAME" \| null}` | Move the chat into one of your projects, or out of any (`null`): the Thread. Coder's sync files a synced chat under the project holding the chat's own GitHub repository (from its folder's git remote): the one project holding it, else the one where it is the primary; when several have it as primary (or none does), the chat stays unfiled. A person's move stays put (#11541) |
| `PATCH /v1/projects/{id}` `{"name"?, "instructions"?, "primary_repository"?, "protection"?}` | The Project. `instructions: null` clears them; `primary_repository` must already be in the project. `protection`: `standard` or `only_me` (`sealed` answers `400 not_available`). Repositories are added and removed with the routes above |
| `POST /v1/files?purpose=project&filename=NAME`, the file's bytes as the body | `201` File: `{id: "file_…", object: "file", filename, mime_type, size_bytes, sha256, created_at, stored_as: "original", project}`. 10 MB at most. Not multipart |
| `GET /v1/files/{id}`, `GET /v1/files/{id}/content`, `DELETE /v1/files/{id}` | The File; its bytes (`Content-Disposition`, `Digest: sha-256=…`); `{id, object: "file.deleted"}` |
| `GET /v1/projects/{id}/attachments` | `{data: [Attachment], next, protection}`, oldest first: `file`, `document`, `source` (the project's Drive sources), and `vault` |
| `POST /v1/projects/{id}/attachments` | `{"file": "file_…"}`, `{"document": {"title", "text"}}` (1 MB of text), or `{"vault": "<object id>"}` (an object already in the caller's vault): `201` Attachment. Drive sources attach with `POST /v1/projects/{id}/sources` ([connections.md](connections.md)) |
| `GET /v1/projects/{id}/documents/{doc}` | `{id: "pdoc_…", object: "document", title, text, size_bytes, created_at, project}` |
| `DELETE /v1/projects/{id}/attachments/{att}` | `{id, object: "attachment.deleted"}`; a file is deleted with it |

- **Credential.** The sign-in `openagents login` or `coder login` saves
  (`sess_`), or a sign-in key from Settings (`oa_sk_`). `oak_` API keys and
  their `projects:*` scopes come with step 5; until then these routes are
  on `openagents.com/v1`, not `api.openagents.com/v1` or
  `openagents.com/api/v1`.
- **Ids.** Projects are `prj_` and 16 hex characters; threads keep their
  chat ids (UUIDs); messages are `msg_` and their position in the thread.
- **Project object.** `{id, object: "project", name, instructions,
  repositories: [{repository, primary, default_branch, private}],
  repository, default_branch, private, created_at, updated_at, protection,
  counts: {threads, files, documents, sources}}`. `updated_at` is the
  latest activity in its chats. A project has zero or more repositories;
  when it has any, exactly one is `primary`. The same repository may be in
  any number of projects, and project names may repeat.
- **Deprecated (2026-10-13): `repository`, `default_branch`, `private`.**
  They are the primary repository's (`null`, `null`, `false` for a project
  without one), kept for apps built before projects held several
  repositories. Read `repositories` instead; the three fields go away with
  the app release after the next one. Projects kept before 2026-10-13 read
  with their one repository as the primary, without a migration step.
- **Files.** A project holds up to 200 files, documents, and vault files.
  A `standard` file or document is kept in the account's own folder of the
  chat store (the private chat bucket, encrypted at rest by Google);
  one whose text looks like it holds a password or key is refused
  (`400 looks_like_a_secret`). A file belongs to one project.
- **Only me.** In an `only_me` project the website takes no `standard` file
  or document (`409 only_me_project`). `openagents projects files add`
  locks each file on the computer with the person's vault (`openagents
  vault add --project`, NIP-VAULT: AES-256-GCM per file, its key in the
  sealed index) and then records `{"vault": object}` on the project. The
  website keeps only the object id, its size, and when it was added; the
  name and type stay in the index. A vault file removed from the vault
  itself leaves the listing. Removing a project forgets its files and its
  record of vault files; the vault files themselves stay in the vault.
- **Not yet:** `sealed`, multipart uploads
  through the API, `oak_` keys with `projects:*` scopes, and `GET
  /v1/threads` filters other than `project` (without `project` it still
  answers the apps' older shape).

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
| Project | `proj_` | A named home for chats, instructions, files, and documents, with zero or more repositories (one primary when any). |
| Thread | `thr_` | One chat: web, terminal (Coder), or phone. Belongs to at most one project. |
| Message | `msg_` | One turn in a thread, with content blocks (`text`, `tool_use`, `tool_result`, `image`) and any files, generated files, and artifacts. |
| File | `file_` | Bytes you uploaded: to a message or to a project. The same object the [Model calls](model-calls.md) and [Managed Agents](managed-agents.md) pages use. |
| Generated file | `genf_` | Bytes a tool produced during a turn (a PDF, a CSV, a slide deck). |
| Artifact | `art_`, version `artv_` | A versioned document a reply produced or revised. |
| Project document | `pdoc_` | Plain text kept on a project: a pasted note, or a document converted to text when added. |
| Project source | `psrc_` | A connected source on a project, such as a Drive folder or file. |
| Vault object | 64 hex | A file encrypted on your device under [NIP-VAULT](../../nips/openagents/NIP-VAULT.md). The service holds its ciphertext only; its name and type live in your encrypted key index. |

Times are RFC 3339 UTC. A deleted thread lists with `deleted_at` set, an
empty `title`, and messages without content until it is purged.

## Projects

| Route | Does |
| --- | --- |
| `GET /v1/projects` | List your projects, oldest first. `?limit`, `?after`. |
| `POST /v1/projects` | Create one: `{"name", "instructions"?, "repositories"?, "protection"?}`. A name is enough. |
| `GET /v1/projects/{id}` | One project, with counts of its threads, files, documents, and sources. |
| `PATCH /v1/projects/{id}` | Rename, set instructions, pick the primary repository, or change `protection` for new files. |
| `POST /v1/projects/{id}/repositories`, `DELETE /v1/projects/{id}/repositories/{owner}/{name}` | Add or remove a repository. |
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
      "repositories": [
        {"repository": "AtlantisPleb/finances", "primary": true, "default_branch": "main", "private": true}
      ],
      "repository": "AtlantisPleb/finances",
      "created_at": "2026-10-01T15:02:11Z",
      "updated_at": "2026-10-12T20:41:09Z",
      "counts": {"threads": 3, "files": 0, "documents": 0, "sources": 0}
    }
  ],
  "next": null
}
```

`protection` is the project's default for new files and documents
([below](#sensitive-files-encrypted-before-they-leave-your-machine)):
`standard` (the default), `only_me`, or `sealed`. The answer includes it.

A project may also be named in the path by its exact name when it is unique
for the caller (`/v1/projects/name:finances`), so the CLI and Coder can use
the name a person says. Names may repeat; when one is shared, the answer is
`409 ambiguous_name` naming each project, never a guess.

## Members and invitations

Built 2026-10-14 ([#11585](https://github.com/OpenAgentsInc/openagents/issues/11585),
part of the shared projects MVP [#11592](https://github.com/OpenAgentsInc/openagents/issues/11592)).
Anyone can share a project they own, and anyone can be invited.

| Role | Can |
| --- | --- |
| `owner` | Everything an editor can, plus invite, change roles, remove members, rename, delete, change `protection`, and give the project to another member. Exactly one per project. |
| `editor` | Read and add chats, files, and notes; edit instructions; add and remove repositories. |
| `viewer` | Read the project's chats, files, notes, and instructions. |

| Route | Who | Answer |
| --- | --- | --- |
| `GET /v1/projects/{id}/members` | Any member | `{data: [Member], next, role, invitations?}`; the owner also gets the open invitations `{id, role, expires_at}` |
| `PATCH /v1/projects/{id}/members/{account}` `{"role"}` | Owner | The Project. `editor` or `viewer`; `owner` gives the project to that member, and you become an editor |
| `DELETE /v1/projects/{id}/members/{account}` | Owner (or the member, which is leaving) | `{account, object: "project.member.removed", project}`. It takes effect on their next request |
| `POST /v1/projects/{id}/invitations` `{"role"?, "expires_in"?}` | Owner | `201 {object: "project.invitation", id, project, role, code, url, expires_at}`: a single-use link, shown once. `editor` and 7 days by default; `expires_in` is seconds, 10 minutes to 30 days. Only the code's SHA-256 digest is kept. An invitation can't make an owner. A project whose `protection` is `only_me` answers `409 only_me_project` ("Files in this project open only on your devices. Change it to standard to share it.") |
| `POST /v1/projects/invitations/preview` `{"code"}` | Anyone signed in | `{project: {id, name}, role, invited_by, expires_at, member}` without using it. `code` may be the whole link |
| `POST /v1/projects/invitations/accept` `{"code"}` | Anyone signed in | The Project, with your role. Someone already in keeps their role. A used or expired link: `404 invitation_not_found` |
| `POST /v1/projects/{id}/leave` | Any member but the owner | `{id, object: "project.left"}`; the owner gets `409 owner_stays` |

`Member` is `{account, object: "project.member", name, login, role,
joined_at, you}`. The Project object gains `role` (yours), `members` (how
many people are in it), and `shared`.

- A project you aren't in answers `404`, never `403`. An action your role
  doesn't allow answers `403 project_role`.
- Shared projects count toward the 200 projects an account can be in.
- A shared project's files and notes are kept with the project (in the
  storage of the account that made it), so every member reads the same
  ones. A member reads a file with `GET /v1/files/{id}/content?project=ID`.
  An editor's upload moves into the project when it is attached.
- A shared project can't switch to `only_me` (`409 shared_project`).
- Drive sources stay with the account that connected them.
- Chats synced from Coder file themselves only under projects you own
  alone.

Storage: `identity.projects` and `identity.project_members`
([schema](../data/schema.md)); migration 3 copied every project the
GitHub-access records held, each owned by its account with no other
members. `tenant-db projects-census` checks the copy.

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

## Sensitive files: encrypted before they leave your machine

For financial statements, tax records, and anything else you don't want us
to be able to read, a project keeps files in your **vault**. The vault is
[NIP-VAULT](../../nips/openagents/NIP-VAULT.md); the design, the threat model,
and its honest limits are in the [sensitive data vault](../security/sensitive-data-vault.md).

The command encrypts each file on your computer before anything is sent. The
service stores ciphertext only. No one at OpenAgents, the owner included, can
open it. What is already built: the `only_me` tier, `crates/oa-vault`, the
browser vault at `/settings/vault` and `/projects/{id}/vault`, and
`openagents vault`.

### How a file is protected

| Step | What happens | Primitive |
| --- | --- | --- |
| 1. Setup, once | `openagents vault setup` makes a 32-byte vault master key (`VMK`) on your computer, and a 24-word recovery code. | Random 256-bit key |
| 2. Who can open it | Each way in is a **key slot** that wraps the `VMK`: this computer's device key (in the OS keychain), your **Nostr key**, a passkey in the browser, and the recovery code. | Nostr slot: NIP-44 v2 to yourself, as NIP-51 private lists are. Recovery slot: scrypt, as NIP-49 does |
| 3. Each file | A fresh 32-byte data key (`DEK`) encrypts the file in 64 KiB chunks, each bound to the object's header, so truncation, reordering, and extension are refused. | AES-256-GCM, chunked (`aes-256-gcm-chunked-v1`) |
| 4. Its key and name | The `DEK`, the file's name, and its type go into your **key index**, which is sealed under a key derived from the `VMK` and stored as one blob. The stored object carries no key and no name. | HKDF-SHA256 key derivation, AES-256-GCM |
| 5. Upload | The ciphertext object goes to `PUT /vault/api/objects/{id}`, addressed by its id. The new index goes up with compare-and-swap. | Content-addressed blobs (any blob store, Blossom included) |
| 6. A new device | `openagents vault add-device` makes a 10-minute link. The new device gets in with it, with your Nostr key, or with the recovery code. Slots travel between your devices as self-addressed private artifacts (kind `3188`). | NIP-44 to self |
| 7. Delete | Removing the file drops its `DEK` from the index, and the service deletes the ciphertext. The bucket keeps no versions and no soft-deleted copies, so the file is gone for good. | Crypto-shredding |

At rest the ciphertext sits in `openagentsgemini-web-vault-prod`, a bucket
that is also encrypted by Google. That outer layer protects the disks. The
inner layer is the one that keeps us out.

### The three protection settings

| `protection` | Who can open a stored file | Who reads it when you ask a question | State |
| --- | --- | --- | --- |
| `standard` | The service (per-file keys under our Cloud KMS key; every use logged). Our team can read it. | The chat's model provider | Proposed for project files; the honest default for ordinary files |
| `only_me` | Only your devices: device key, Nostr key, passkey, recovery code | The route you pick each time: a model on your own computer (`openagents vault serve-local`, the default when one runs), or **Fast**, where Google's Gemini sees that turn's files and we store nothing | **Built** (`openagents vault`, the web vault) |
| `sealed` | Only an exact, publicly logged OpenAgents program in a hardware enclave (Intel TDX in Confidential Space, [NIP-ATT](../../nips/openagents/NIP-ATT.md)), and only while your client sends its share for that request | The same enclave (**Private**), or Gemini if you pick **Fast**. Agents and scheduled jobs can work while you're away, under a lease you grant | Designed; the attested endpoint runs today at [/att](https://openagents.com/att) |

Everything is configurable, per project and per file:

- the project default (`protection`), and `--protection` on each upload;
- which keys can open the vault: devices, Nostr key, passkey, recovery code
  (`openagents vault devices`, `add-device`, `add-nostr`, `remove-device`);
- where answers come from: on this computer or Fast, per question (`--route`),
  with Sealed Private once `sealed` ships;
- for `sealed`: whether a new enclave program needs your approval before it
  can open anything ("Ask me before any update"), whether your own devices
  keep a copy of each key ("device copy", on by default), and standing leases
  for agents: which files, for what purpose, until when;
- limits the service enforces today: 11 MiB a file and 250 files a vault.
  These rise with chunked uploads; they are not a protection setting.

### In the API and the CLI

An `only_me` or `sealed` file is a project attachment of `type` `vault`:

```json
{
  "id": "9f2c…e41a",
  "object": "attachment",
  "type": "vault",
  "protection": "only_me",
  "size_bytes": 482176,
  "created_at": "2026-10-12T21:30:00Z"
}
```

It has no `filename` or `mime_type`. The service never learns them. Your
client reads them from your key index after it opens the vault. The vault
routes themselves are NIP-VAULT's [service API](../../nips/openagents/NIP-VAULT.md#service-api-tier-user)
(`/vault/api/...`), with the same sign-in as the rest of this page.

```text
openagents vault setup                                  # once; write the recovery code down
openagents vault add-nostr                              # optional: your Nostr key opens it too
openagents projects add finances --protection only_me   # or: projects edit finances --protection only_me
openagents projects files add finances statements-2026-09.pdf
                                                        # encrypted here, then uploaded
openagents projects files list finances                 # names come from your index, not the server
openagents vault ask statements-2026-09.pdf --project finances \
  --question "What did I spend on travel?" --route device
openagents projects files remove finances statements-2026-09.pdf
                                                        # gone for good
```

`projects files add` follows the project's `protection`. An `only_me`
project runs `openagents vault add --project` underneath: same files, same
index. A `standard` file can't move into an `only_me` project, and an
`only_me` file can't move out. A copy is a new upload from your machine.

Coder uses the same path. When a person asks about a file in an `only_me`
project, the engine runs `openagents vault ask` on that computer. It reads
the file with the local model, or with Fast only when the person said so.
It never uploads plaintext anywhere else.

What the service can still see, said plainly: that you have a vault, how many
files are in it and how big they are, when they were added, and, for a Fast
answer, that answer's files and question while it runs. The browser vault
runs code we serve. A compromised page could read what you open there. The
command and Coder keep keys in your OS keychain.

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
openagents projects create NAME [--repo OWNER/REPO]... [--instructions TEXT]
openagents projects remove NAME|ID [--detach-threads]
openagents projects files list NAME|ID
openagents projects files add NAME|ID PATH...
openagents projects files get NAME|ID FILE --output PATH
openagents projects files remove NAME|ID FILE
openagents projects notes add NAME|ID --title T (--text TEXT | --file PATH)
```

Built today, each with `--json`; a project is found by its name, its
repository, or its id. When a name or repository matches more than one
project, the command names each and asks for the id instead of guessing:

```text
openagents projects list | show PROJECT | threads PROJECT
openagents projects create NAME [--repo OWNER/REPO]... [--instructions TEXT] [--protection standard|only_me]
openagents projects add OWNER/REPO            # older: a project named after the repository
openagents projects repo add PROJECT OWNER/REPO [--primary]
openagents projects repo remove PROJECT OWNER/REPO
openagents projects repo primary PROJECT OWNER/REPO
openagents projects edit PROJECT [--name NAME] [--instructions TEXT] [--protection standard|only_me]
openagents projects move CHAT PROJECT | move CHAT --none
openagents projects remove PROJECT [--detach-threads]
openagents projects files list PROJECT
openagents projects files add PROJECT PATH... [--protection standard|only_me]
openagents projects files get PROJECT FILE [--output PATH]
openagents projects files remove PROJECT FILE
openagents projects notes add PROJECT --title T (--text TEXT | --file PATH)
openagents projects sources list|add|remove PROJECT [LINK|ID]   # Drive sources (connections.md)
openagents chat send MESSAGE --project PROJECT
```

`chat send --project` starts a new chat inside the project on openagents.com
(working in its primary repository, or a plain chat with its instructions
when it has none) and streams OpenAgents's reply; it then shows in `projects threads PROJECT`,
on the website, and on the phone.

`files list` names only-me files from the vault's index on this computer
(and lists files added with `openagents vault add --project` too). `files
get` and `files remove` take a file's name, id, or the start of its id.

It uses the sign-in `openagents login` saved. The GitHub Project board
command is `openagents board`. `project`, its old name, was kept for one
release and is gone since #11538: `openagents project` now names both
commands (`openagents projects` for these projects, `openagents board` for
GitHub boards) instead of guessing one.

## Coder

Coder reaches projects through its `openagents_cli` tool on every engine,
and Claude Code gets it through Coder's tool server. The tool's description
says that "the person's projects" are openagents.com projects
(`openagents projects`), and that "make a new chat in my finances project"
is `openagents chat send MESSAGE --project finances`. It also says that a
project needs no repository (`openagents projects create NAME`), can hold
several with one primary, that a repository can be in several projects,
and that a shared name means asking which project. It is the description
that steers the model; no keyword routing. A Coder chat in a project's repository reads that project's
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
4. **Files and documents**: `/v1/files`, attachments, documents, private per-account storage, and `type: vault` attachments backed by NIP-VAULT, with the project `protection` setting and `projects files add` following it.
5. **Changes and deletes**, then scopes on the Settings key picker.

Deploying the website and gateway follows `scripts/deploy/web.sh`:
`stage` (staging smoke), `promote DIGEST` (a no-traffic revision tagged
`new`, smoked), `shift` (all traffic), and `rollback` if a check fails after
the shift. Check the serving revision right before shifting, because
another session may have deployed since staging.
