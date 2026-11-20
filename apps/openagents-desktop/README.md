# Coder for desktop

Coder is a Coax Native desktop app. It targets macOS first. It is a
conversation with Microcoder on Gemini 4 Neon: your messages and the
replies scroll above the message bar.

Every part of the screen is the shared app set (`components/app/`): the
composer, your message, the reply as Markdown with code blocks and Copy,
the typing mark and notices. The page draws and edits its own text fields
(`page/editor.coax` on `components/zeron/editor.coax`): the composer, settings'
search and a dialog's fields, with the Mac's input method through the shared host.

For the repeatable workflow used to match Claude Desktop's layout, spacing, and
typography in Coder's monochrome palette, see
[`docs/coder/claude-desktop-parity-process.md`](../../docs/coder/claude-desktop-parity-process.md).
For the tiered evaluation specification and phased rollout plan for Coder
desktop, see [`docs/coder/desktop-evals-spec.md`](../../docs/coder/desktop-evals-spec.md).

## What it does

- **Send** (or Return) starts one Microcoder run. Shift-Return makes a new line.
- **Stop** ends the run.
- **New chat** (the top row of the sidebar, the top-right icon when the sidebar is hidden, or Command-N) clears the window and makes Microcoder forget the conversation.
- **The composer** is one row while your message fits on a line and grows into a box when it doesn't, when it has a new line, or in a new chat, where it stands in the middle of the window. Return sends; Shift-Return makes a new line. The **+** at its start chooses the working folder.
- **Under the composer** are the working folder and its Git branch, the model (View > Choose Model, Command-/), a ring that shows how full the model's context was on its last call (hover it for the token counts), the chat's cost against its budget, and its goal. In a narrow window the goal goes first, then the cost, the ring and the branch.
- **The working folder** shows as a chip under the composer. Commands start there. Press it (or File > Open Folder, Command-O) to choose another. Coder keeps the folder you chose; until you choose one, it is your home folder. You can't change it during a run.
- **Command-plus** and **Command-minus** make the text bigger and smaller, in eight sizes from 80% to 200%. Command-0 is the usual size. Coder keeps the size you chose.
- **Copy** on a code block puts its text on the clipboard.
- **The copy icon under a message** copies that message: yours as you wrote it, a reply as its Markdown.
- **The copy icon in the top right corner**, beside New chat, copies the whole chat: each message under `You:`, each command under `Ran:`, each reply under `Microcoder:`. Command output and thoughts are not included.
- **Click a reply** to see who wrote it (Microcoder and its model), beside the reply's copy icon. Click again to hide it.
- **A message of yours** longer than five lines folds; **Show more** opens it and **Show less** folds it again.
- **Away from the end** of the chat, **Jump to latest** (or **New reply** while one is written) floats above the composer. It glides to the end, as sending does when you have scrolled up.
- **The sidebar** on the left lists your chats in chronological order, newest created at the top. Click one to open it, or press **Command-1** through **Command-9** to open the first nine chats from the top (and **Command-\`** to open Foreman when enabled). See [Saved chats](#saved-chats).
- **Sub-agents** take work Microcoder hands over. See [Sub-agents](#sub-agents).
- **A file a row or a reply names** opens in the pane on the window's right, as does a `.coax` file you open with Coder from the Finder (Coder registers the `.coax` type, `com.openagents.coax`, as plain-text source code). The pane shows the file's name, its path, and its lines with their numbers. Click in the text to edit it: the tab says **Edited** and **Save** (or File > Save, Command-S) writes it back. The arrow icon opens the file in the Mac's own app for it (for a `.coax` file, the Mac's text editor). Close asks first when there are changes, and so does quitting. A picture or a PDF opens in its own app as before. Files up to 16 KB and 512 lines are edited in the pane; longer ones up to 256 KB are shown read-only, and the pane says so. A file Coder can't read (missing, a folder, not text, or longer than 256 KB) says why. A press on a sub-agent's row or Review shows that pane instead; the file comes back when it closes.

Limits, all shown on screen when you reach them:

- A chat holds 1,000 messages. Start a new chat to send more. Only the messages near the view are laid out, so a long chat costs what shows; a message you have not scrolled to yet stands at an estimated height until it comes near, and what you are reading does not move when it is measured.
- The whole chat's copy (the top right copy icon) takes up to 2,000,000 bytes.
- A reply longer than 60,000 bytes is cut.
- One run at a time in each chat.
- A command's or a tool call's output is shown up to 2,000 bytes, and its text up to 400 bytes.
- A chat keeps 6,000 rows in all: commands, file reads, sub-agents' tool calls and thoughts (#13670, #13671). Past 6,000 it drops the oldest row in the chat, and the message or transcript it was in says at its top how many it dropped.
- The working folder's name is cut with an ellipsis past 280 points wide.
- The sidebar lists the 50 most recent chats and says how many older ones it does not show. It shows when the window is at least 900 points wide (at the usual text size).
- A chat's title in the sidebar is its first message, up to 120 bytes.
- When you open a chat, Microcoder is told its newest 6 messages and replies, up to 8,000 bytes of each; a longer one is cut on a character and says it was cut.
- A message runs up to 12 sub-agents, 6 in one step. Microcoder is told when a task is not run for this reason.
- A sub-agent's task is cut at 1,500 bytes. Its reply is never cut on its own: the app keeps, shows and hands back the whole reply, up to 32 MB of sub-agent replies in a chat. One step's results reach Microcoder in one 1 MB line; only past that does each reply get an equal share, with a line that says how much was left out and to resume the sub-agent for the rest.
- In Microcoder's memory, a sub-agent's reply is whole when it fits the step's 32 KB of results. Each reply is also saved whole to a file, and its last line names that file. A longer reply keeps its start and end, and the line between them names the file and the lines left out, so Microcoder can read the rest.
- A sub-agent's transcript follows its rows as they arrive. Rows out of view are not drawn, so a long transcript costs what shows.
- The model's thinking arrives as a summary of each step, up to 32,768 bytes a step. Thinking past that is cut, and the row says so.
- You can drag the edge of either sidebar to resize it: the chats' sidebar from 180 to 480 points, a sub-agent's transcript from 320 to 900 points. A drag stops where it would leave the conversation under 380 points. Coder keeps the widths you chose.
- A sub-agent's transcript is 440 points wide until you drag it. In a window where that leaves the conversation under 380 points, the chats' sidebar is hidden while the transcript is open. In a narrower window still, the transcript takes the whole window until you close it.
- A command has 120 seconds. A run has no limit on its time, its model calls, its commands or its file reads (#13667): it ends when it replies or you press Stop. The same goes for every sub-agent.

### Each chat in its own worktree

A new chat whose working folder is in a Git repository's main checkout gets a
worktree of its own before its first message runs (#13758):
`~/.openagents/worktrees/REPO/CHAT`, on the branch `coder/CHAT`, made from
`origin/main` (or `HEAD` when the repository has no `origin/main`). The chat,
its sub-agents and every later message work there, so two chats in one
repository never edit, stash or reset each other's files. Chats Foreman
creates get one too; the Foreman chat itself and chats kept before this work
where they were.

- The branch shows under the composer as `coder/` and the first 8 characters of the chat's id.
- Making a worktree takes as long as checking the repository out. The folders the repository lists in `.coder/worktree.json` (`{"clone": ["coax/bin", ...]}`) are then cloned in from the main checkout with APFS copy-on-write (`cp -c`), so the Coax compiler and built tools are there at once. Each worktree's copy is its own: a rebuild in one changes nothing in another, and a link back into the main checkout is removed.
- Every worker shares one Coax tool cache (`COAX_TOOL_CACHE`, `~/.cache/coax-tools`). Each repository has one Cargo target folder (`CARGO_TARGET_DIR`, `~/.cache/coder/cargo-target/REPO-HASH`, named from its real path, as its worktrees folder is, so `/tmp` and `/private/tmp` give one name), which its main checkout and all its chats' worktrees share. Outside a repository, Cargo builds where it always does. Coder's own environment wins over both. `apps/openagents-desktop/build.sh` builds into the worktree's own `build/`.
- Opening a chat goes back to its worktree. One removed by hand is made again from its branch.
- Deleting a chat removes its worktree when it has no changes, and its branch when everything on it is in another branch. A worktree with changed or new files is never removed: Coder says where it is and leaves it.
- `defaults write com.openagents.desktop chatWorktrees -bool NO` turns this off; `CODER_WORKTREES=DIR` puts worktrees somewhere else.

In a main checkout, Microcoder refuses the git commands that would take other
chats' work away before they start (`coax/lib/microcoder/git-guard.coax`):
`git stash` (all but `list` and `show`), `--autostash` on pull, rebase or
merge, `git reset --hard`, `git checkout .` or `git restore .` on the whole
tree, `checkout`/`switch` with `--force`, and `git clean -f`. The refusal says
to work in a worktree. A Claude sub-agent's Bash commands go through the same
check: Claude Code runs `gitguard` (`coder/tui/gitguard.coax`, shipped beside
`claudeworker`) as a PreToolUse hook before each one, and Claude is told why a
command was refused.

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
- **OpenRouter** tasks go to the OpenRouter model you set for sub-agents. Microcoder uses it when you ask for OpenRouter or name the model. See [OpenRouter models](#openrouter-models).

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
open -n apps/openagents-desktop/build/OpenAgents.app --env CODER_CLAUDE_KEY="$ANTHROPIC_API_KEY"
```

A key applies to the next Claude sub-agent; one that is running keeps what it
started with. Only the Claude worker gets the key, as `ANTHROPIC_API_KEY` or
`CLAUDE_CODE_OAUTH_TOKEN`: Microcoder and the commands it runs never see it.
Any other `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL`
or `CLAUDE_CODE_OAUTH_TOKEN` in Coder's environment is not passed on. If
Claude refuses the key, the sub-agent's row says so. You still need the
`claude` command installed.

## OpenRouter models

Microcoder and its sub-agents can run on any OpenRouter model that takes a
strict JSON schema (OpenRouter lists most models that way). Open
**Coder > OpenRouter Models…** and fill in:

- **Key:** your OpenRouter key (`sk-or-…`, from [openrouter.ai/settings/keys](https://openrouter.ai/settings/keys)). Coder keeps it in `~/Library/Application Support/Coder/openrouter-key`, readable by you only. Leave it empty to keep the key already set. Without a key here, Coder uses `~/work/.secrets/openrouter.env`.
- **Model for Microcoder:** an OpenRouter id such as `deepseek/deepseek-v4.1-flash-1017`. It joins the composer's model picker, under its name after the vendor.
- **Model for sub-agents:** an OpenRouter id. It becomes the sub-agent level `openrouter`: "use an OpenRouter sub-agent to count the tests" or "give it to DeepSeek" hands a task to it.

Press Save. Coder checks each id against OpenRouter's model list first and
refuses one that does not take a strict JSON schema. Leave a model empty to
remove it; a chat on the removed model goes back to Gemini 4 Neon.

An OpenRouter model runs on OpenRouter only: when OpenRouter can't answer,
the run fails and says so, and no other model answers in its place.
Requests ask OpenRouter for providers that keep no training data
(`data_collection: deny`) and that take every field sent
(`require_parameters`). The cost OpenRouter reports for each call counts
against the chat's budget.

To pick it for one launch, add `--model openrouter` (or a Gemini name such as `Carbon`):

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --model openrouter --send "What is 2+2?"
```

## Saved chats

Coder keeps every chat on this Mac (#13661). It saves a chat when you send a
message, when a reply ends and when you quit: the messages, the replies, the commands with
their output, the thoughts, and the sub-agents with their tool calls, the calls' output, their thoughts and their replies.

The sidebar lists the chats in chronological order by creation time, newest
created at the top. Clicking or updating a chat keeps it in its place. Click
a chat to open it:

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

To rename or delete a chat, right-click it in the sidebar and choose Rename
Chat or Delete Chat. For the chat that is shown, File > Rename Chat and
File > Delete Chat (Command-Delete) do the same. Deleting asks first; the
chat's file is deleted and can't be brought back, and deleting the chat that
is shown starts a new chat. You can't delete a chat while a reply is running
in it.

To open the Nth chat from the top at launch (1 is the newest):

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --chat 1
```

To open a sub-agent's transcript at launch, add `--agent N`. It opens the Nth
sub-agent of the newest message that has that many:

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --chat 1 --agent 1
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
it on the first message. `Worker.swift` carries bytes between the worker and the
page. The page (`page.coax` and its parts in `page/`) writes the task line and reads every reply line
with the worker's own codec (`coax/lib/agent/engine/mcworker-wire.coax`).
Each sub-agent is one more worker process, which the app starts (through the daemon, see [Runs outlive Coder](#runs-outlive-coder)) when
Microcoder hands a task over and stops when the task ends.
The worker reads this machine's credentials as Coder's terminal app does
(`coder/tui/microcoder_keys.coax`): Vertex first, then the Gemini API, then
OpenRouter. A model you set in OpenRouter Models runs on OpenRouter alone,
with the key Coder keeps (see [OpenRouter models](#openrouter-models)).

## Runs outlive Coder

Coder's background service runs the workers (#13694). It is the unified
daemon, `coder host serve` (`coder/tui/host.coax`), bundled as
`Contents/Resources/coder-host`. When Coder opens and no daemon answers on
`~/.openagents/coder/host/service.sock`, Coder starts it in a session of its
own, so it keeps running after Coder quits. Its output goes to
`~/.openagents/coder/host/daemon.log`.

- **Quit Coder** while a reply runs and the reply goes on. Coder quits without asking.
- **Open Coder** again and each chat whose reply ran while it was closed is back in the sidebar. A reply that is still running shows as running and fills in live; open the chat to watch it or press Stop. A reply that ended while Coder was closed shows complete and unread.
- **Several chats** run at once, and Foreman-managed chats run the same way.

How it works: for each worker the daemon starts, it keeps the worker's own
lines (`mcworker-wire`, unchanged) in `~/.openagents/coder/host/runs/ID.out`
and forwards what Coder appends to `ID.in` (`coder/tui/host-runs.coax`). Coder
keeps each running message in `~/Library/Application Support/Coder/turns/`
with the chat as it was when you sent it. When Coder opens, it loads that
chat, reads the run's lines from where the message started, and then follows
them. The control methods (StartRun, StopRun, ListRuns, ForgetRun) are on the
daemon's local socket only, never on the relay.

**On your phone.** OpenAgents Mobile, paired to this Mac, lists these chats
(those in the paired workspace) with the rest of the Mac's chats. It shows
each one's messages, thinking, tool calls and sub-agents, live. From the phone
you can send a message (it steers a running reply, or starts a follow-up
through the chat's queue) and press Stop. Coder shows what the phone sent. The
phone writes to `ID.in` through the daemon, which checks the pairing's grant
(`coder/tui/host-desktop.coax`).

Limits:

- The daemon runs up to 12 workers at once, sub-agents included. Past that a message or a sub-agent does not start, and the chat says so: "Coder runs up to 12 workers at once, sub-agents included."
- While Coder is closed, the daemon answers each command's check with yes, as Coder does. A sub-agent's pause after 100 tool calls in a row (see [Sub-agents](#sub-agents)) is answered the same way, so a sub-agent goes on past it.
- While Coder is closed, sub-agents that Microcoder hands work to don't start, and Microcoder waits for the results of sub-agents that finish. Both happen when you open Coder again.
- The Foreman chat's `foreman-ctl` calls need Coder open: the address they use is Coder's.
- Stopping the daemon, or restarting the Mac, ends its workers. Coder then shows those replies as stopped ("Microcoder stopped running").
- A daemon from an older Coder that doesn't run workers leaves them to Coder, as before: quitting Coder then stops them, and Coder asks first.
- A finished run's files are removed when Coder sees it end, and after a week otherwise.

## Build and open

```sh
apps/openagents-desktop/build.sh --open
```

The app is `apps/openagents-desktop/build/OpenAgents.app`, signed ad hoc. Its bundle
identifier is `com.openagents.desktop`.

To send a message at launch, with no typing:

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --send "What is 2+2?"
```

Add `--folder PATH` to use a working folder for this launch only:

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --folder ~/work/app --send "Run pwd."
```

Add `--then TEXT` to start a new chat once that reply ends and send `TEXT` in it:

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --send "Say hi." --then "What is 2+2?"
```

Add `--new TEXT` to start a new chat while that reply runs and send `TEXT` in
it. The first reply goes on out of view:

```sh
open -n apps/openagents-desktop/build/OpenAgents.app --args --send "Run sleep 20, then say hi." --new "What is 2+2?"
```

To build a second copy somewhere else, and leave a running Coder's files alone:

```sh
CODER_BUILD=/tmp/coder apps/openagents-desktop/build.sh
```

## Install in Applications

To keep `/Applications/OpenAgents.app` at the newest `main`, run this once:

```sh
apps/openagents-desktop/install.sh --agent
```

It builds `origin/main` in a checkout of its own under
`~/.cache/openagents/coder-release` (never in yours), installs it, and from then
on checks every 15 minutes. A check builds only when `main` changed something
Coder's build reads, which takes about 20 seconds. While Coder is open, a new
build waits; it goes in at the first check after you quit Coder. A check skips
its turn when the Mac is busy or has under 10 GB free.

| Command | What it does |
| --- | --- |
| `apps/openagents-desktop/install.sh` | Build and install now. A Coder open from Applications is quit and opened again |
| `apps/openagents-desktop/install.sh --status` | The commit of `main`, of the installed Coder and of a build that waits |
| `apps/openagents-desktop/install.sh --remove-agent` | Stop the checks. The installed Coder stays |

The installed app's `Info.plist` names its commit (`OACommit`) and its commit
count (`CFBundleVersion`). The log is `~/.cache/openagents/coder-release/log`.

## Checks

```sh
coax/bin/coax test apps/openagents-desktop/page.coax --library
```

That runs every page check. A check file under `page/checks/` also runs on its
own, for example
`coax/bin/coax test apps/openagents-desktop/page/checks/transcript-checks.coax --library`.

A check holds a page and a check-sized store (`CpCheckStore` in `page/state.coax`); the app's
page gets its full store (`CpStore`) from the host. So a check fits the compiler's value slots when
it runs in the evaluator, as a run of one check does (#13767):

```sh
apps/openagents-desktop/page-slots.sh
```

That runs every check file in the evaluator with the slot limit at 85%, a few at once, and fails
when a check needs more. `coax/remote/check.sh` runs it as the `coder-desktop-page-slots` step on
every change to Coder Desktop (`coder` and `coder-since`).

The Foreman control API (`docs/coder/foreman-control-api.md`), which lets a
script or another agent manage Foreman in a running Coder:

```sh
apps/openagents-desktop/foreman-control-test.sh          # credentials, route policy, control file, server
apps/openagents-desktop/foreman-control-test.sh --live   # also drives a built OpenAgents.app through foreman-ctl
```

## Files

| File | What |
| --- | --- |
| `page.coax` | The page's entry: it loads `page/parts.coax` and the checks |
| `page/parts.coax` | The page's parts, in one namespace; a check file loads it alone |
| `page/state.coax` | Capacities, ids and actions, the `CoderPage` struct, its store (`CpStore`, `CpCheckStore`), `cp_init` and the small helpers |
| `page/skin.coax`, `page/models.coax` | The page's skin and scale; the model picker's list and the OpenRouter models |
| `page/goal.coax` | The chat's cost and budget, the thread goal and `/goal` |
| `page/transcript.coax` | Turns and their rows: sending, the reply, commands, tool calls, thoughts, sub-agents and their workers |
| `page/chats.coax`, `page/persist.coax` | The kept chats' list, pins and the Foreman row; keeping and reading back a chat, the recall line, a new chat |
| `page/input.coax`, `page/host-api.coax` | Presses, the pointer, the wheel and the sidebars' edges; the rest of the host's `coder_page_*` calls |
| `page/layout/layout.coax` | `cp_layout`: the shell's regions, then the parts below in a fixed draw order |
| `page/layout/conversation.coax` | A message's activity group, the conversation, the Foreman HUD, the corner buttons and the composer |
| `page/layout/sidebar.coax`, `page/layout/pane.coax` | The chats' sidebar and its foot; a sub-agent's transcript and the shared tool row |
| `page/checks/*.coax` | The page's checks, one file per part they cover |
| `settings.coax` | The settings screen |
| `app.coax` | The compiled unit: the page and the atlas tables |
| `host.c`, `host.h` | The page on the shared GPU carrier, as calls for Swift |
| `carrier.c` | The shared component carrier with this page |
| `Worker.swift` | The worker (the daemon's run, or a child process), the Claude key, the OpenRouter models and the skills note |
| `Store.swift` | The kept chats' list and files |
| `Window.swift` | A chat's run and its kept turn, the window's state, the page and its keyboard, the clock, what the page asks for, and quitting |
| `Window+Runs.swift` | Workers wired to pages, the daemon's runs reattached after a relaunch, replies out of view, and syncing with the daemon |
| `Window+Chats.swift` | Opening, keeping, renaming and deleting chats, the working folder and text size |
| `Worktree.swift`, `Window+Worktrees.swift` | Each chat's own worktree: its path and branch, making and cloning it, and removing it only when clean |
| `worktree-test.sh`, `worktree-test.swift` | The checks of that, under a throwaway home folder |
| `page-slots.sh` | The page's checks in the evaluator with the value-slot limit at 85% (#13767) |
| `Window+Settings.swift`, `Window+Dialogs.swift` | The settings screen's wiring, and the budget, goal, Claude key and OpenRouter dialogs |
| `Window+Menus.swift` | Right-click menus |
| `Window+Update.swift` | Checking for updates, the update pill, install and relaunch |
| `Window+Foreman.swift` | The Foreman chat: its tier, history, audit log, and opening it |
| `Window+ForemanAPI.swift`, `Window+ForemanChats.swift` | The Foreman control API's routes |
| `AppDelegate.swift` | The app delegate, the status item, the main menu and `main` |
| `DaemonClient.swift` | The unified daemon's local socket: health, Foreman policy, session metadata, and the worker runs |
| `ForemanOperator.swift` | The Foreman control API's two credentials, route policy and control file, for programs outside the Foreman chat |
| `foreman-control-test.sh`, `foreman-control-test.swift` | The checks of that API |
| `build.sh` | Coax to C and Metal, the two workers' images and the daemon's, then clang, swiftc and codesign |
| `install.sh` | Builds `origin/main` and keeps `/Applications/OpenAgents.app` at it, every 15 minutes with `--agent` |
