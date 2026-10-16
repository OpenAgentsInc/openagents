# Create a project, add files, and invite a person

A project holds chats, files, notes, and repositories, and can be shared
with other people. These routes act for one account, so they take that
account's login, not an API key.

| | |
| --- | --- |
| Base URL | `https://api.openagents.com/v1` |
| Credential | `Authorization: Bearer oa_sk_...` (a login key) or a session |
| An `oak_` API key | Refused: `401 signed_out` |

Get a login key at [Settings](https://openagents.com/settings) under
Login keys for agents, or make one from the command:

```sh
openagents keys create --name my-agent --kind signin
export OPENAGENTS_TOKEN=oa_sk_...
```

## Step 1: create a project

```sh
curl -s https://api.openagents.com/v1/projects \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "Launch notes", "instructions": "Answer in short lists.", "repositories": ["OpenAgentsInc/openagents"]}'
```

`201 Created`, with the project:

```json
{
  "id": "prj_3f9a1c0d2b7e4a51",
  "object": "project",
  "name": "Launch notes",
  "instructions": "Answer in short lists.",
  "repositories": [{"repository": "OpenAgentsInc/openagents", "primary": true, "default_branch": "main", "private": false}],
  "protection": "standard",
  "counts": {"threads": 0, "files": 0, "documents": 0, "sources": 0},
  "role": "owner",
  "created_at": "...",
  "updated_at": "..."
}
```

`name` or a repository is required. `protection` is `standard` or
`only_me`. With the command:

```sh
openagents projects create "Launch notes" --repo OpenAgentsInc/openagents --instructions "Answer in short lists." --json
```

List your projects with `GET /v1/projects` (`?limit=` up to 200, and
`?after=` with the `next` value for the next page):

```sh
curl -s https://api.openagents.com/v1/projects -H "Authorization: Bearer $OPENAGENTS_TOKEN"
```

## Step 2: add a file

Two calls: upload the file's bytes, then attach the file to the project.

```sh
curl -s "https://api.openagents.com/v1/files?purpose=project&filename=notes.pdf" \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  --data-binary @notes.pdf
```

The body is the raw file, not a form. The answer is `201`:

```json
{"id": "file_...", "object": "file", "filename": "notes.pdf", "mime_type": "application/pdf",
 "size_bytes": 48211, "sha256": "...", "created_at": "...", "stored_as": "original", "project": null}
```

Then attach it:

```sh
curl -s https://api.openagents.com/v1/projects/prj_3f9a1c0d2b7e4a51/attachments \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"file": "file_..."}'
```

To add text without a file, send `{"document": {"title": "...", "text": "..."}}`
to the same route (up to 1 MiB of text).

| Limit | Value |
| --- | --- |
| One upload | 10 MiB on most accounts, never over 16 MiB |
| Attachments per project | 200 |
| A file that looks like a password or key | Refused: `400 looks_like_a_secret` |

The command does both calls:

```sh
openagents projects files add "Launch notes" notes.pdf --json
```

## Step 3: invite a person by GitHub username

```sh
curl -s https://api.openagents.com/v1/projects/prj_3f9a1c0d2b7e4a51/invitations \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"github": "octocat", "role": "editor"}'
```

`201 Created`:

```json
{"object": "project.invitation", "invitee": {"github": "octocat"}, "id": "...",
 "project": "prj_3f9a1c0d2b7e4a51", "role": "editor", "code": "prjinv_...",
 "url": "https://openagents.com/projects/join/prjinv_...", "expires_at": "..."}
```

The invitation waits on that person's Projects page on openagents.com,
with Join and Decline. `role` is `editor` (the default) or `viewer`, and
`expires_in` sets how long it lasts in seconds (7 days by default). Leave
out `github` to get a link anyone can open.

```sh
openagents projects invite "Launch notes" --github octocat --role editor
```

| Status | `code` | Why |
| --- | --- | --- |
| 400 | `unknown_github_user` | No GitHub account has that name |
| 403 | `project_role` | Only the owner can invite |
| 409 | `only_me_project` | The project is set to only me |

## Step 4: check who is in it

```sh
curl -s https://api.openagents.com/v1/projects/prj_3f9a1c0d2b7e4a51/members \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN"
```

```json
{"data": [{"account": "...", "object": "project.member", "name": "...", "login": "octocat",
           "role": "editor", "joined_at": "...", "you": false}],
 "next": null, "role": "owner", "invitations": [{"id": "...", "role": "editor", "expires_at": "..."}]}
```

## See also

- [Start a chat and stream the answer](/docs/agents/chat-and-stream), also inside a project
- [The `openagents` command for agents](/docs/agents/cli)
