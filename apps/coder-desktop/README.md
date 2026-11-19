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
- **The copy icon in the top right corner**, beside New chat, copies the whole chat: each message under `You:`, each command under `Ran:`, each reply under `Microcoder:`. Command output and thoughts are not included.
- **Click a reply** to see who wrote it (Microcoder and its model), beside the reply's copy icon. Click again to hide it.
- **The sidebar** on the left lists your chats, most recent at the top. Click one to open it. See [Saved chats](#saved-chats).
- **Sub-agents** take work Microcoder hands over. See [Sub-agents](#sub-agents).

Limits, all shown on screen when you reach them:

- A chat holds 24 messages. Start a new chat to send more.
- A reply longer than 60,000 bytes is cut.
- One run at a time in each chat.
- A command's or a tool call's output is shown up to 2,000 bytes, and its text up to 400 bytes.
- A chat keeps 6,000 rows in all: commands, file reads, sub-agents' tool calls and thoughts (#13670, #13671). Past 6,000 it drops the oldest row in the chat, and the message or transcript it was in says at its top how many it dropped.
- The working folder's name is cut with an ellipsis past 280 points wide.
- The sidebar lists the 50 most recent chats and says how many older ones it does not show. It shows when the window is at least 900 points wide (at the usual text size).
- A chat's title in the sidebar is its first message, up to 120 bytes.
- When you open a chat, Microcoder is told its newest 6 messages and replies, up to 1,200 bytes of each.
- A message runs up to 12 sub-agents, 6 in one step. Microcoder is told when a task is not run for this reason.
- A sub-agent's task is cut at 1,500 bytes and its reply at 8,000 bytes.
- A sub-agent's transcript follows its rows as they arrive. Rows out of view are not drawn, so a long transcript costs what shows.
- The model's thinking arrives as a summary of each step, up to 32,768 bytes a step. Thinking past that is cut, and the row says so.
- You can drag the edge of either sidebar to resize it: the chats' sidebar from 180 to 480 points, a sub-agent's transcript from 320 to 900 points. A drag stops where it would leave the conversation under 380 points. Coder keeps the widths you chose.
- A sub-agent's transcript is 440 points wide until you drag it. In a window where that leaves the conversation under 380 points, the chats' sidebar is hidden while the transcript is open. In a narrower window still, the transcript takes the whole window until you close it.
- A command has 120 seconds. A run has no limit on its time, its model calls, its commands or its file reads (#13667): it ends when it replies or you press Stop. The same goes for every sub-agent.

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

## Thinking

Microcoder and its sub-agents think before they answer or run a command
(#13671). Each step's thinking is a **Thought** row, in its place among the
commands, named by its first line. Press it to read it, formatted as a reply is.
A thought longer than 2,000 bytes takes more rows, so none of it is cut.

- Hydrogen, Carbon and Neon (Gemini) send a summary of their thinking, which is what Gemini gives; the full trace is not available.
- A Claude sub-agent's thinking shows as Claude Code summarizes it: Coder asks for the summaries (`--thinking-display summarized`), which Claude Code otherwise leaves empty.
- Thoughts are saved with the chat. Copying the whole chat leaves them out.

Coder asks the worker to wait before each command (`"gated":true`) so the row
is on screen first, and then answers yes at once. Coder does not ask you.
Microcoder can still edit files directly (`"edits":true`).

## Sub-agents

Microcoder can hand parts of a message to sub-agents (#13655). It picks a
level for each task:

- **Hydrogen** (Gemini 4 Hydrogen) takes lookups, searches, commands and small changes.
- **Carbon** (Gemini 4 Carbon) takes work that needs many steps or careful reasoning.
- **Neon** (Gemini 4 Neon) takes complex work that should finish fast: frontier-level software, research and analysis tasks done end to end.

Microcoder knows all three by name, so "task it to Carbon" or "give it to Neon" works.
- **Claude** tasks go to Claude Code, on the model Microcoder picks: Haiku, Sonnet, Opus or Fable. See [Claude sub-agents](#claude-sub-agents).

Sub-agents handed over in one step run at the same time. Each is a row above
the reply, named for its model (Hydrogen, Carbon, Neon, or Claude Haiku, Claude
Sonnet, Claude Opus, Claude Fable), with its task beside the name and its
elapsed time on the right. While a sub-agent runs, the line under its row
says how many tool calls it has made and names the newest.

Click a sub-agent's row to open its transcript in a sidebar on the right
(#13666). It reads as the conversation does: the task as a message, each tool
call as a row you can press to see its input and output, what the sub-agent
says between its tool calls as text in its place, and the reply. A Claude
sub-agent's read of an image shows "An image", since Claude gets the picture
and not words. A
running sub-agent's transcript fills in as it works. The X in the sidebar's
top right corner closes it. So does a second click on the row.

A sub-agent that makes 100 tool calls in a row without changing a file pauses
(#13676). Its row says so, and Microcoder is told what it has been doing.
Microcoder then lets it go on, with a note that tells it what to do next, or
leaves it stopped. This is a check-in, not a limit: a sub-agent that is let go
on runs until the next 100. Two cases never pause: a Claude sub-agent, and a
sub-agent that only reads files and runs no commands.

When all of them end or pause, Microcoder gets their replies and carries on. A
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

- It uses the Claude login already on this Mac, unless you give Coder a key (see [Claude key](#claude-key)). Coder never opens a login page. If Claude Code is not logged in and there is no key, the row says so.
- It works in the working folder with Claude Code's own tools, your settings and your skills, and with no permission prompts.
- It saves no Claude Code session.
- Its usage counts against your Claude plan.
- It has no time limit. Each tool call shows in its row as it starts.

Coder looks for `claude` in `~/.local/bin`, `/opt/homebrew/bin`,
`/usr/local/bin` and the system `PATH` when it starts. Without it,
Microcoder is not offered the Claude models.

#### Claude key

Claude sub-agents can use a key instead of the Claude Code login: an
Anthropic API key (`sk-ant-api…`, from the Claude Console), or a long-lived
Claude Code token (`sk-ant-oat…`, from `claude setup-token`). An API key is
billed to its Console account; a token counts against the plan it came from.

- **Coder > Claude Key…** sets it. Paste the key and press Save. The dialog shows the last 4 characters of the key that is set, and Remove Key goes back to the login. Coder keeps the key in `~/Library/Application Support/Coder/claude-key`, readable by you only.
- **For one launch**, pass it in the environment. It wins over a key set in the menu:

```sh
open -n apps/coder-desktop/build/Coder.app --env CODER_CLAUDE_KEY="$ANTHROPIC_API_KEY"
```

A key applies to the next Claude sub-agent; one that is running keeps what it
started with. Only the Claude worker gets the key, as `ANTHROPIC_API_KEY` or
`CLAUDE_CODE_OAUTH_TOKEN`: Microcoder and the commands it runs never see it.
Any other `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`
or `CLAUDE_CODE_OAUTH_TOKEN` in Coder's environment is not passed on. If
Claude refuses the key, the sub-agent's row says so. You still need the
`claude` command installed.

## Saved chats

Coder keeps every chat on this Mac (#13661). It saves a chat when you send a
message, when a reply ends and when you quit: the messages, the replies, the commands with
their output, the thoughts, and the sub-agents with their tool calls, the calls' output, their thoughts and their replies.

The sidebar lists the chats, most recent at the top. A chat moves to the top
when you send a message in it. Click a chat to open it:

- It shows as it was. A message that was still running when Coder quit shows as stopped.
- The working folder becomes the one the chat used, if that folder still exists.
- Microcoder is told the chat's newest messages and replies ahead of your next message, so a follow-up has its context.

You can open another chat, or start a new one, while a reply is running. The
reply goes on out of view, and its chat shows a spinner in the sidebar. Its
chat is saved as it changes and when the reply ends. Click the chat to see it
again, still running or finished. New chat starts an empty one; it joins the
list when you send its first message. Coder opens on a new chat.

The chats are files in `~/Library/Application Support/Coder/chats`: one
`ID.json` for each chat and `index.json`, the list. Nothing is sent
anywhere.

To delete a chat, right-click it in the sidebar and choose Delete Chat. For
the chat that is shown, File > Delete Chat (Command-Delete) does the same.
Coder asks first. The chat's file is deleted and can't be brought back.
Deleting the chat that is shown starts a new chat. You can't delete a chat
while a reply is running in it.

To open the Nth most recent chat at launch (1 is the newest):

```sh
open -n apps/coder-desktop/build/Coder.app --args --chat 1
```

To open a sub-agent's transcript at launch, add `--agent N`. It opens the Nth
sub-agent of the newest message that has that many:

```sh
open -n apps/coder-desktop/build/Coder.app --args --chat 1 --agent 1
```

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

Add `--new TEXT` to start a new chat while that reply runs and send `TEXT` in
it. The first reply goes on out of view:

```sh
open -n apps/coder-desktop/build/Coder.app --args --send "Run sleep 20, then say hi." --new "What is 2+2?"
```

To build a second copy somewhere else, and leave a running Coder's files alone:

```sh
CODER_BUILD=/tmp/coder apps/coder-desktop/build.sh
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
| `App.swift` | The AppKit lifecycle, the window, the editor's placement, the worker processes and the kept chats' files |
| `build.sh` | Coax to C and Metal, the two workers' images, then clang, swiftc and codesign |
