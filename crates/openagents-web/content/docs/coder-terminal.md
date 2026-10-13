# Use Coder in the terminal

Run `coder` in a project folder to open it there. To open it on another
folder from where you are:

```sh
coder --in ~/code/my-project
```

It works in any terminal, over SSH, and in tmux. Install it from [Download and install](/docs/download).

## The screen

- **The chat** fills the screen: your messages, Coder's replies, and
  each step it takes (files read, searches, edits with a diff, and
  commands with their output).
- **The input** sits at the bottom, between two lines, starting with `❯`.
- **Top right of the input:** who answers this chat (`Auto`, or the
  agent you picked) and, once it has answered, the model, such as
  `Auto → Claude (Haiku 5.5)` ("Auto picked Claude"; the chat can still
  move). An agent you picked shows alone, such as `Claude (Haiku 5.5)`.
  Whenever a turn uses your Claude or Codex plan, on any agent, it also
  shows how much of that plan's usage windows you've used, such as
  `5h 12% · week 15%`, or `Auto → Coder (Haiku 5.5) · Claude 5h 12% ·
  week 15%` when Coder's own loop ran on your Claude plan. Otherwise
  Coder's own loop shows the chat's cost so far. Once the model has
  answered, it shows how much of the model's room for this chat is used.
  See [Choose who answers](/docs/coder-agents).
- **Bottom right of the input:** the folder Coder works in and its Git
  branch.
- **Under the input:** one row for each agent this chat handed work to,
  with its task, time, and tokens. See
  [Choose who answers](/docs/coder-agents).

## Keys

| Key | Does |
| --- | --- |
| Enter | Send. |
| Shift+Enter, Alt+Enter | A new line. |
| Esc | Stop the reply. Closes a list or screen. Twice on a draft clears it. |
| Shift+Tab | Choose who answers: Auto, Claude Code, Codex, OpenCode, or another agent here. |
| Tab | Complete a `/` command. |
| Up, Down | Your earlier messages; Down from the input selects the agent rows under it. |
| Ctrl+N | Start a new chat in the same folder. The last one stays in `/resume`. |
| Ctrl+O | Show each step in full, or fold the steps back to one line each. |
| Ctrl+P (Cmd+P on macOS), F2 | Open plugins. |
| PageUp, PageDown, mouse wheel | Scroll the chat. Ctrl+End jumps to the bottom. |
| Ctrl+A, Ctrl+E | Start or end of the line. |
| Cmd+A | Select the whole input, where the terminal passes Cmd keys on. Terminal, iTerm2, and Ghostty keep Cmd+A for themselves: use Ctrl+Shift+A there. |
| Drag | Copy text from the chat. Double-click copies a word, triple-click a line. |
| Ctrl+C | Quit. The chat is saved. |

## Commands

Type `/` to see them above the input; keep typing to filter, and press
`Enter` to run one.

| Command | Does |
| --- | --- |
| `/help` | Every command and key. |
| `/models` | Choose the model and reasoning level. |
| `/plugins` | Turn plugins on and off, and add keys. See [Coder's plugins](/docs/coder-plugins). |
| `/preferences` | How much to favor cost, speed, or intelligence, and your saved choices of agent. |
| `/resume` | Open an earlier chat. `/resume 3` picks from the last list; `/resume ID` opens one by its ID. |
| `/rewind` | Undo a turn's file changes and go back to its message. |
| `/export` | Save this chat to a file. `/export PATH` chooses where. |
| `/compact` | Summarize the older part of the chat so the model reads less. |
| `/memory` | What Coder remembers. `/memory forget NAME` deletes one note. |
| `/agents` | Background agents: open, stop, message, or resume one. |
| `/agent` | Start a background agent: `/agent codex fix the flaky login test`. |
| `/loop` | Repeat a prompt while Coder is open: `/loop 5m check the deploy`. `/loop stop 1` or `/loop stop all` ends them. |
| `/schedule` | Run a prompt on this computer on a schedule, even while Coder is closed: `/schedule weekdays 09:00 triage new issues`. `/schedule remove ID` deletes one. |
| `/shells` | Commands running in the background. `/shells stop ID` stops one. |
| `/login`, `/logout` | Sign in to openagents.com, or sign out. |
| `/sync` | Save chats to your account: `/sync on`, `/sync off`, or `/sync delete`. See [Sync, share, and export](/docs/coder-sync). |
| `/appearance` | Use your terminal's own background color instead of Coder's. |
| `/brainstorm` | Public profile lookups, when the Brainstorm plugin is on. |

## Chats

Every chat saves itself as you go, on this computer, in
`~/.openagents/coder/sessions/`.

- **New chat:** `Ctrl+N`. It starts in place, in the same folder.
- **Earlier chats:** `/resume` lists your 100 most recent. Up and Down
  (or `j` and `k`) choose, `Enter` opens, `Esc` goes back. A resumed chat
  comes back with everything it showed, without running anything again.
- **Undo a turn:** `/rewind` lists your last 20 messages, each with the
  files its turn changed. Pick one: Coder puts back the files that turn
  and every later turn changed (and deletes files they created), cuts the
  chat to just before that message, and puts the message back in the
  input for you to edit and send again. A file someone changed outside
  Coder since then is left as it is and named. Files over 1 MB and
  binary files can't be put back; the list says so.
- **Stop:** `Esc` stops the reply. What was done so far stays.

Stop a running reply before you resume or rewind.

## Send more than text

- **Screenshots and files:** paste a screenshot, drop or paste the path
  of an image or a PDF, or write `@path/to/file` in your message. Up to
  8 per message; an image up to 5 MB, a PDF up to 20 MB.
- **A shell command:** type `!` on an empty input, then the command, such
  as `!git status`. It runs in your folder and its output joins the chat.
  `Backspace` on the empty input goes back to a normal message.
- **While Coder works:** on Coder's own loop, a message you send waits
  and goes next.

## Instructions and memory

Every turn, Coder reads your project's `AGENTS.md` and `CLAUDE.md`
files, from your folder up to your home folder, plus your own
`~/.openagents/AGENTS.md`.

Coder also keeps short notes across chats: about you (for every project)
and about this project. Say "remember that we use pnpm" to add one and
"forget the pnpm note" to remove it. `/memory` lists them. Notes stay on
this computer, under `~/.openagents/memory`, and anything that looks like
a password or key is refused. Set `OPENAGENTS_MEMORY=off` to turn
instructions and memory off.

## Where Coder keeps things

| What | Where |
| --- | --- |
| Chats | `~/.openagents/coder/sessions/` |
| Settings, plugins, and keys | `~/.openagents/coder/` (only you can read the files) |
| Exports | `~/.openagents/exports/` unless you name a path |
| Memory | `~/.openagents/memory/` |

To keep all of Coder's data in another folder instead:

```sh
coder --state ~/coder-data
```

Next: [Choose who answers](/docs/coder-agents).
