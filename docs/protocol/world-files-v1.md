# Attachments on a project and its worlds v1 (`world.files.v1`, `world.file.*`)

A maker attaches reference files to a project or to one of its worlds: concept
art, design documents, notes and links (GitHub #13468; the design is
[docs/editor/2026-11-13-world-attachments.md](../editor/2026-11-13-world-attachments.md)).
The editor's Files pane, the realm commands and the agent read them. They are
never game assets: no pack, no player and no page outside the editor reads
them, and only the project's members and the site's admins get their bytes.

One codec writes and reads every shape here: `metaverse-engine/editor/file-wire.coax`.
The pinned bytes are in [proto/world/files-v1.json](../../proto/world/files-v1.json).
Check them with:

```sh
coax/web/tool.sh metaverse-engine/tools/files-profile.coax proto/world/files-v1.json
```

## The routes

| Route | What it answers |
| --- | --- |
| `GET /api/worlds/ID/files` | The list: the world's attachments, then its project's |
| `POST /api/worlds/ID/files` | A request (below). The answer is the list with a `did` line, a part's answer, an upload's answer, or `422` with `{"error":WORDS}` |
| `POST /api/worlds/ID/files/done` | The API only: the upload's ask again, once the file is in storage. The answer is the list with a `did` line |
| `GET /api/worlds/ID/files/FID` | The attachment's bytes, as the content type its extension names, with `Cache-Control: no-store` and `X-Content-Type-Options: nosniff` |
| `GET /api/projects/SLUG/files` | The project page's list: every attachment of the project and its worlds |

Every route answers `401` to no one signed in and `403` to someone who is not a
member of the project or an admin. On the dev server everyone on the computer
is the admin.

## The limits

A file is 25 MB at most. A project holds 200 attachments and 1 GB at most,
counting its worlds' (a link counts as one of the 200 and holds no bytes). The
pane shows the limits before anything is sent, and a refusal names the limit
in words.

We take PNG, JPEG, WebP and GIF images, and Markdown, text and PDF documents,
by extension. The scan reads a file's first bytes, checks them against its
extension and refuses a program whatever its name. The content type a file is
served as is ours, by its extension, never the browser's.

## The list

```
world.files.v1 WORLD PROJECT COUNT BYTES FILE FILES TOTAL
f ID SCOPE KIND BYTES HASH AT BY NAME
u URL
n NOTE
t TAGS
did WORD ID
```

- The head: the world (`-` for a project's own list), its project (`-` for a
  world with none), what the project holds now with all its worlds (`COUNT`
  files, `BYTES` bytes), and the three limits.
- `f`: an attachment. `ID` is 16 hex digits. `SCOPE` is `world` or `project`.
  `KIND` is `image`, `document` or `link`. `BYTES` and `HASH` (SHA-256) are
  `0` and `-` for a link. `AT` is milliseconds since 1970. `BY` is the key that
  attached it, or `local`. `NAME` is the rest of the line, 1 to 100 bytes.
- `u`: a link's address, only after a link's `f`, and always there.
- `n`: its note, one line of 200 bytes at most. Left out for none.
- `t`: its tags, 1 to 8 of `[a-z0-9-]{1,24}` joined by commas. Left out for none.
- `did`: after a write, what it did (`attached`, `linked`, `changed` or
  `detached`) and to which attachment. Nothing follows it.

The world's attachments come first, then the project's, each newest first.

## The requests

Each request head is lines, every one ended by a newline.

```
world.file.attach.v1 SCOPE BYTES AT NAME
n NOTE            (optional)
t TAGS            (optional)
SHA256 or -
...the bytes from AT
```

A file attached whole, or a part of it from byte `AT`. A part needs the whole
file's SHA-256; `-` lets the server make it, only for the whole file in one
body. A part that is not the last is answered
`world.file.part.v1 SHA256 GOT` with the bytes held so far. The last part is
answered with the list. The realm command sends parts of 384 KB.

```
world.file.upload.v1 SCOPE BYTES NAME
n NOTE / t TAGS   (optional)
SHA256
```

On the API, a file named before it is sent by direct upload to the private
bucket. The answer is `world.file.upload.v1 put SHA256 BYTES URL` (send exactly
that many bytes there) or `world.file.upload.v1 kept SHA256 BYTES -` (storage
has the file). The same ask to `.../files/done` attaches it once the API has
read the object back and scanned it.

```
world.file.link.v1 SCOPE URL
name TITLE
n NOTE / t TAGS   (optional)
```

A link: an `http` or `https` address and its title.

```
world.file.edit.v1 ID SCOPE
name NAME
n NOTE / t TAGS   (optional)
```

An attachment's name, note, tags and scope, all of them: a field left out is
cleared. A rename keeps a file's extension.

```
world.file.detach.v1 ID
```

Removes the attachment. The stored file goes when no attachment names it.

## For the agent

Each turn's `world.turn.v1` lists the attachments by ID, kind, scope, name,
note and tags, never their bytes ([world-turn-v1.md](world-turn-v1.md)). A
hosted turn may open some of them; their bytes then join that one model call.
