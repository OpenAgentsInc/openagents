# Attachments: reference files on a project and its worlds

Status: spec, 2026-11-13. Issue #13468.

## What we want

A maker attaches reference material to a project or to one of its worlds
(scenes): game design documents, concept art, mood boards, screenshots,
notes. The files show in the editor, and the agent in the editor can read
them, so "make the garage look like the concept" works without pasting
anything.

Attachments are not game assets. Asset import (E11, #13420) puts a mesh,
texture or sound into the world's pack, with tight limits, and players
download it. An attachment is never in a pack and never reaches a player.

## Decisions

- **Two scopes.** A file is attached to a project or to one world. A world
  shows its own files and its project's. A world with no project still has
  its own.
- **Private by default.** Attachments are stored privately. Only the
  project's members and the site's admins can list or open them. Publishing
  a world doesn't publish its attachments. Nothing about a file is logged:
  no names and no bytes.
- **Never in git.** The repository holds no attachment and no thumbnail.
- **One store.** Files go to the private bucket that project drops already
  use, by direct upload, through the same admission and scan
  (`coax/web/api/drops.c`, `drops.scan`). On a dev server they are files
  under the app's build folder. No new cloud resource.
- **What you can attach:**
  - Images: PNG, JPEG, WebP, GIF.
  - Documents: Markdown, plain text, PDF.
  - Links: a URL with a title, for a Google Doc or a board that lives
    elsewhere. We store the link, not a copy.
- **Limits, shown in the pane:** 25 MB a file, 200 files and 1 GB a
  project, counting its worlds. A file over a limit is refused with the
  limit named.
- **A file has:** a name, a kind, a size, a hash, who attached it and when,
  an optional note, and tags. The note and tags are what the agent reads
  first.

## In the editor

A **Files** pane, opened from the bar beside Assets, built from the kit:

- A drop target, and a button to pick files or add a link.
- The files as a list with small previews, grouped as "This world" and
  "The project". Search by name, note and tag.
- Opening an image shows it large beside the scene, so you can build
  against it; it can stay pinned while you work. Opening a document shows
  it rendered (Markdown) or paged (PDF).
- Rename, edit the note and tags, move between world and project, remove.
  Remove asks once more, and removes the stored file.

`/projects/SLUG` lists the project's attachments too, for members.

## For agents

- **Each turn names what is attached.** The world's turn context
  (`world.turn.v1`) lists attachments: name, kind, note and tags, never the
  bytes.
- **Commands,** as rows of `REALM_COMMANDS`, so the CLI and the MCP tools
  come from the table:

  | Command | Effect |
  |---|---|
  | `files` | Read: list a world's or project's attachments |
  | `file-read` | Read: a document's text, or an image for a model that takes images |
  | `attach` | Write: add a local file or a link, with a note and tags |
  | `file-note` | Write: change a note or tags |
  | `detach` | Destructive, two-step: remove a file |

- **Reading.** A hosted Gemini turn can open an attachment as part of its
  typed plan, and the image or text is added to that one model call. A
  local agent reads through the commands. A PDF is read as text first, and
  as page images when it has no text.
- **Budget.** What a turn reads counts against its stated size limit; the
  page says when a file was too large to read whole.

## Not in this pass

- Editing a document in place. Attach a new version instead.
- Comments on a file.
- Syncing a Google Drive folder. A link is stored as a link; files are
  attached by hand or by a command.
- Generating assets from an attachment. That is asset generation's job
  (E11), which may take an attachment as its reference later.

## The test

Attach files from the owner's "Concept art" Drive folder to Splice on a dev
server, by command and by drop:

- "Raiders", "RaiderGarage", "Raider garage inside", "Boss reference",
  "Digger reference", "Main character shape and art style and outfits",
  "RoverShapeandStyle", "City".
- A design note in Markdown about the raiders' garage.

Then, in the editor: the pane lists them with previews; an image pins
beside the scene; and the agent, asked "what does the raiders' garage look
like in the concept art, and what in this world should change to match?",
opens the right file and answers from it.
