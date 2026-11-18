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
- **New chat** (the icon in the window's top right corner, or Command-N) clears the window and makes Microcoder forget the conversation.
- **The working folder** shows in the window's top left corner. Commands start there. Press it (or File > Open Folder, Command-O) to choose another. Coder keeps the folder you chose; until you choose one, it is your home folder. You can't change it during a run.
- **Command-plus** and **Command-minus** make the text bigger and smaller, in eight sizes from 80% to 200%. Command-0 is the usual size. Coder keeps the size you chose.
- **Copy** on a code block puts its text on the clipboard.
- **The copy icon under a message** copies that message: yours as you wrote it, a reply as its Markdown.
- **The copy icon in the top right corner**, beside New chat, copies the whole chat: each message under `You:`, each command under `Ran:`, each reply under `Microcoder:`. Command output is not included.
- **Click a reply** to see who wrote it (Microcoder and its model), beside the reply's copy icon. Click again to hide it.
- **Sub-agents** take work Microcoder hands over. See [Sub-agents](#sub-agents).

Limits, all shown on screen when you reach them:

- A chat holds 24 messages. Start a new chat to send more.
- A reply longer than 60,000 bytes is cut.
- One run at a time.
- A message shows the first 12 commands and file reads. It says how many more ran.
- A command's output is shown up to 4,000 bytes.
- The working folder's name is cut with an ellipsis past 280 points wide.
- A message runs up to 12 sub-agents, 6 in one step. Microcoder is told when a task is not run for this reason.
- A sub-agent's task is cut at 1,500 bytes, its list of tool calls at 3,000 bytes and its reply at 8,000 bytes.
- A run has 15 minutes, with its sub-agents' time included. A command has 120 seconds.

Microcoder runs shell commands as you, starting in the working folder, with
the `danger-full-access` profile: nothing confines them, and a command can
reach outside the folder. The worker's other
profiles (`read-only`, `workspace-write`) need `bwrap`, which is Linux only,
so on macOS every command under them fails to start.

Each command shows as a row above the reply the moment it starts, with an
activity mark while it runs. The row takes the command's output when it ends.
A file read shows as a row when it ends. Press a row to open its input and
output. The reply streams in as the model writes it.

Each row shows on its right how long it took, such as `4s` or `1m 05s`.
While it runs, the time counts up.

Coder asks the worker to wait before each command (`"gated":true`) so the row
is on screen first, and then answers yes at once. Coder does not ask you.
Microcoder can still edit files directly (`"edits":true`).

## Sub-agents

Microcoder can hand parts of a message to sub-agents (#13655). It picks a
level for each task:

- **Simple** tasks go to a sub-agent on Gemini 4 Hydrogen.
- **Complex** tasks go to a sub-agent on Gemini 4 Carbon.
- **Claude** tasks go to Claude Code, on the model Microcoder picks: Haiku, Sonnet, Opus or Fable. See [Claude sub-agents](#claude-sub-agents).

Sub-agents handed over in one step run at the same time. Each is a row above
the reply, named for its model (Hydrogen, Carbon, or Claude Haiku, Claude
Sonnet, Claude Opus, Claude Fable), with its task beside the name and its
elapsed time on the right. While a sub-agent runs, the line under its row
says how many tool calls it has made and names the newest. Press the row to
see every tool call and, when it ends, its reply.

When all of them end, Microcoder gets their replies and carries on. A
sub-agent works in the same working folder, runs commands and edits files as
Microcoder does, and can't start sub-agents of its own. Stop ends them too.

To ask for them, say so: "Use two sub-agents at the same time: one to count
the tests, one to explain how login works."

### Claude sub-agents

When the `claude` command is installed, Microcoder can hand a task to Claude
Code (#13660). It is told the order of the models, weakest to strongest: Haiku, Sonnet,
Carbon, Opus, Fable. So it picks Haiku or Sonnet only when you ask for them,
and Opus or Fable when you ask or when a task needs more than Carbon gives. For example: "Have Claude Opus review the last commit."

A Claude sub-agent runs through the Coax Claude Agent SDK
(`coax/lib/agent/claude`), in the worker `coder/tui/claudeworker.coax`:

- It uses the Claude login already on this Mac. Coder passes no API key and never opens a login page. If Claude Code is not logged in, the row says so: run `claude` in a terminal and log in.
- It works in the working folder with Claude Code's own tools, your settings and your skills, and with no permission prompts.
- It saves no Claude Code session.
- Its usage counts against your Claude plan.
- It has 15 minutes. Each tool call shows in its row as it starts.

Coder looks for `claude` in `~/.local/bin`, `/opt/homebrew/bin`,
`/usr/local/bin` and the system `PATH` when it starts. Without it,
Microcoder is not offered the Claude models.

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
Each sub-agent is one more worker process, which the app starts when
Microcoder hands a task over and stops when the task ends.
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

Add `--folder PATH` to use a working folder for this launch only:

```sh
open -n apps/coder-desktop/build/Coder.app --args --folder ~/work/app --send "Run pwd."
```

Add `--then TEXT` to start a new chat once that reply ends and send `TEXT` in it:

```sh
open -n apps/coder-desktop/build/Coder.app --args --send "Say hi." --then "What is 2+2?"
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
| `build.sh` | Coax to C and Metal, the two workers' images, then clang, swiftc and codesign |
