# What's new in 1.0

OpenAgents 1.0 is out on the web and in the terminal. Install the terminal
version from the [download page](/download).

## On the website

- **Sign in with GitHub.** Click **Log in** at the top right, or go to
  [/login](/login). Your chats then follow you to any browser.
- **Pin, rename, archive, or delete a chat** from the **…** menu on its row.
  Search sits at the top of the sidebar. Find archived chats under
  [Archived chats](/chat/archived).
- **Delete all your chats** in [Settings](/settings).
- **Connect your GitHub repositories** at [/projects](/projects) with
  **Connect GitHub**. You choose which repositories. Chats are grouped by
  project in the sidebar.
- **See your signed-in computers** in [Settings](/settings), under
  **Computers**.

## In the terminal

Install or update Coder on macOS or Linux:

```sh
curl -fsSL https://openagents.com/cli/install.sh | bash
```

On Windows, in PowerShell:

```powershell
irm https://openagents.com/cli/install.ps1 | iex
```

- **Sign in to your account:** run `coder login`, then approve the code at
  [openagents.com/device](https://openagents.com/device). Inside Coder, type `/login`.
- **Sync your chats to the website and phone:** type `/sync on` in Coder;
  it saves your earlier chats too. Stop with `/sync off`. Remove this
  computer's chats from your account with `/sync delete`.
- **Reply from the website or your phone:** open a synced chat and type
  your reply while Coder is open on that computer.
- **Choose who answers:** the input says `Auto`. Press `Shift+Tab` to pick
  Claude Code, Codex, OpenCode, or another agent on your computer, and
  type `/models` to pick the model. See [Choose who answers](/docs/coder-agents).
- **Use Coder free each day:** when you're signed in, Coder's own loop on
  `auto` is free up to about $1 of model use a day. It starts again at
  midnight UTC. See [Pricing](/docs/pricing).
- **Undo a turn** (in the next release): `/rewind` puts back the files a turn changed.
- **Solve an issue:** ask Coder to solve `owner/repo#42`, or run
  `coder solver 42 --github owner/repo`. See [Solve an issue](/docs/coder-solver).
- **See what each model call cost:** Coder records the tokens, cost, and
  time of every call. Save them with the chat using `/export`.
- **Scroll a command's output** on the left edge of its box. Scroll
  anywhere else to move through the chat.

Sync is off until you turn it on. A message that looks like a key or a
password is never sent.

## Your privacy

We ask our model providers not to train on your chats and not to keep them.
Read the [privacy policy](/privacy) for the details.

Next: [Download](/docs/download).
