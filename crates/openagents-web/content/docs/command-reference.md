# Command reference

The commands you run in a terminal, for people rather than scripts. This
list comes from the commands' own help: add `--help` to `coder` or to
`openagents GROUP` (such as `openagents projects --help`) for the same
text on your computer. Commands for scripts and other agents are in
[Coder from scripts](/docs/coder-scripts).

## `coder`

| Command | Does |
| --- | --- |
| `coder` | Open Coder in this folder. |
| `coder login` | Log in to openagents.com: approve the code at [openagents.com/device](https://openagents.com/device). |
| `coder login --key` | Log in with a login key from Settings, pasted or piped in. Or set `OPENAGENTS_TOKEN`. |
| `coder logout` | Sign this computer out. |
| `coder sync` | Send every chat here to your account now (sync must be on). |
| `coder trace upload` | Upload a chat to your account as a trace: `--last` for your latest, `--share` for a public link. |
| `coder trace list` | List the traces on your account. |
| `coder export --account` | Save everything on your account to one file. |
| `coder solver N` | Solve issue `N` to a pull request. See [Solve an issue](/docs/coder-solver). |

Options for `coder` itself:

| Option | Does |
| --- | --- |
| `--in DIR` | Work in `DIR` instead of this folder. |
| `--models` | Open the model picker first. |
| `--plugins` | Open plugins first. |
| `--follow ID` | Watch a chat another program is running; press any key to take it over. |
| `--state DIR` | Keep Coder's data in `DIR` instead of `~/.openagents/coder`. |
| `--version` | Print the version. |
| `--help` | Print the help. |

## `coder update`

| Command | Does |
| --- | --- |
| `coder update` | Install the newest Coder on your channel now. |
| `coder update --check` | Say whether a newer Coder is published. |
| `coder update --rollback` | Go back to the Coder the last update replaced. |
| `coder update --mode auto` | Download and install updates on their own (the default). |
| `coder update --mode notify` | Only say when an update is ready. |
| `coder update --mode off` | Never check. |
| `coder update --channel stable` | Follow stable releases (the default). `--channel rc` follows release candidates. |

## `openagents coder`: models, plugins, and saved chats

| Command | Does |
| --- | --- |
| `openagents coder status` | Who answers, the model, plugins, and the folder. |
| `openagents coder models list` | The models you can pick, and the agents turned on here, with the one selected. |
| `openagents coder models set SLUG` | Choose the model. `claude-code`, `codex`, `opencode`, or `opencode/PROVIDER/MODEL` starts new chats on that agent, as `/models` does; `auto` goes back to Auto choosing. Add `--reasoning EFFORT` for the reasoning level. |
| `openagents coder plugins list` | Your plugins and whether each is on. |
| `openagents coder plugins enable ID` | Turn a plugin on. |
| `openagents coder plugins disable ID` | Turn a plugin off. |
| `openagents coder plugins check ID` | Check a plugin's connection with its key. |
| `openagents coder agents list` | The other coding agents found on this computer. |
| `openagents coder agents refresh` | Look for installed agents again. |
| `openagents coder sessions list` | Your saved chats and their IDs. |
| `openagents coder sessions delete ID` | Remove one saved chat. |
| `openagents coder export ID --output FILE` | Save a chat to a file. |
| `openagents coder import FILE` | Open a chat file to read or continue it. |
| `openagents coder memory list` | What Coder remembers here. `memory forget NAME` deletes one. |

The plugin IDs include `claude-code`, `codex`, `opencode`, and
`microcoder`; `openagents coder plugins list` shows them all.

## `openagents projects`: your projects on openagents.com

| Command | Does |
| --- | --- |
| `openagents projects list` | Your projects: name, repositories, chats, and last activity. Projects shared with you are listed too. |
| `openagents projects show PROJECT` | One project, with its repositories and chats. |
| `openagents projects create NAME --repo OWNER/NAME` | Make a project. It needs only a name; each `--repo` adds a GitHub repository. |
| `openagents projects move CHAT PROJECT` | Move a chat into a project. `--none` in place of `PROJECT` takes it out. |
| `openagents projects members PROJECT` | Who is in the project and what each can do. |
| `openagents projects invite PROJECT --github USER` | Invite someone by GitHub username: it waits on their Projects page with Join and Decline. Add `--role viewer` for read-only. |
| `openagents projects join LINK` | Join a project with an invitation link someone sent you. |
| `openagents projects role PROJECT ACCOUNT ROLE` | Make a member an `editor`, `viewer`, or `owner`. |
| `openagents projects remove PROJECT ACCOUNT` | Take a member out of the project. |
| `openagents projects leave PROJECT` | Leave a project someone shared with you. |
| `openagents projects files add PROJECT PATH` | Add files to the project. |
| `openagents projects delete PROJECT` | Delete a project for good, after you type its name. |

See [Share a project with a teammate](/docs/share-a-project).

## `openagents chat`: chats from the shell

| Command | Does |
| --- | --- |
| `openagents chat send "MESSAGE"` | Send one message to OpenAgents and print the reply. `--project PROJECT` starts the chat inside one of your projects. |
| `openagents chat threads --account` | Your chats on openagents.com, newest first. |
| `openagents chat read --thread ID` | Print a chat. |
| `openagents chat follow --thread ID` | Follow a chat's Coder run until it ends. `Ctrl+C` stops following, not the run. |
| `openagents chat stop --thread ID` | Stop a chat's Coder run. |
| `openagents chat export --thread ID` | Print a chat as an ATIF file. |

## `openagents settings`: what Coder may use here

| Command | Does |
| --- | --- |
| `openagents settings show` | Every setting, with the defaults. |
| `openagents settings set KEY VALUE` | Change one setting. `unset KEY` returns it to the default. |
| `openagents settings disable AGENT` | Turn a coding agent off: Coder never uses it. `enable AGENT` turns it back on. |
| `openagents settings provider-key set PROVIDER` | Add your own OpenRouter, Vercel AI Gateway, or TypeSafe key, from a hidden prompt. |

See the [settings reference](/docs/settings).

## `openagents connect`: your phone

| Command | Does |
| --- | --- |
| `openagents connect invite` | Show a QR code for the OpenAgents app on a phone. `--text` prints a code to paste instead. |
| `openagents connect devices` | The phones that can reach this computer. |
| `openagents connect remove DEVICE` | Cut a phone off now. |
| `openagents connect status` | This computer's connection at a glance. |

See [Connect a computer](/docs/connect-a-computer).

Next: [Pricing](/docs/pricing).
