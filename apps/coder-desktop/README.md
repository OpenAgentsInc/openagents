# Coder for desktop

Coder is a Coax Native desktop app. It targets macOS first. It is a
conversation with Microcoder on Gemini 4 Hydrogen: your messages and the
replies scroll above the message bar.

Every part of the screen is the shared app set (`components/app/`): the
composer, your message, the reply as Markdown with code blocks and Copy,
the typing mark and notices. The shared AppKit editor
(`apps/shared/native/KitEditorMac.swift`) sits over the composer's text.

## What it does

- **Send** (or Return) starts one Microcoder run. Shift-Return makes a new line.
- **Stop** ends the run.
- **New chat** clears the window and makes Microcoder forget the conversation.
- **Copy** on a code block puts its text on the clipboard.

Limits, all shown on screen when you reach them:

- A chat holds 24 messages. Start a new chat to send more.
- A reply longer than 60,000 bytes is cut.
- One run at a time.
- A message shows the first 12 commands and file reads. It says how many more ran.
- A command's output is shown up to 4,000 bytes.

Microcoder runs shell commands in your home folder as you, with the
`danger-full-access` profile: nothing confines them. The worker's other
profiles (`read-only`, `workspace-write`) need `bwrap`, which is Linux only,
so on macOS every command under them fails to start.

Each command and file read shows as a row above the reply as it finishes.
Press a row to open its input and output. The reply streams in as the model
writes it.

## Skills and installed commands

Coder tells Microcoder which skills are installed for you: each folder under
`~/.claude/skills` with a `SKILL.md`, by name, file and description (the
first 16 skills, 300 characters of each description, 4,000 bytes in all).
Microcoder reads a skill's file before work the skill covers. The note goes
ahead of each message and is not shown.

The commands Microcoder runs also find what is installed for you: Coder puts
`~/.local/bin`, `/opt/homebrew/bin` and `/usr/local/bin` on the worker's
`PATH`.

So with the GameBuilder skill and command line installed
(`scripts/install-cli.sh` in the GameBuilder repo), a message such as
"List my GameBuilder levels" runs `gamebuilder list-levels`.

## How a message runs

The app bundles the Microcoder worker (`coder/tui/mcworker.coax`) and starts
it on the first message. `App.swift` carries bytes between the worker and the
page. The page (`page.coax`) writes the task line and reads every reply line
with the worker's own codec (`coax/lib/agent/engine/mcworker-wire.coax`).
The worker reads this machine's credentials as Coder's terminal app does
(`coder/tui/microcoder_keys.coax`): Vertex first, then the Gemini API, then
OpenRouter.

## Build and open

```sh
apps/coder-desktop/build.sh --open
```

The app is `apps/coder-desktop/build/Coder.app`, signed ad hoc. Its bundle
identifier is `com.openagents.coder`.

To send a message at launch, with no typing:

```sh
open -n apps/coder-desktop/build/Coder.app --args --send "What is 2+2?"
```

## Checks

```sh
coax/bin/coax test apps/coder-desktop/page.coax --library
```

## Files

| File | What |
| --- | --- |
| `page.coax` | The page: the turns, the draft, the scroll, layout, and the worker's lines |
| `app.coax` | The compiled unit: the page and the atlas tables |
| `host.c`, `host.h` | The page on the shared GPU carrier, as calls for Swift |
| `carrier.c` | The shared component carrier with this page |
| `App.swift` | The AppKit lifecycle, the window, the editor's placement and the worker process |
| `build.sh` | Coax to C and Metal, the worker's image, then clang, swiftc and codesign |
