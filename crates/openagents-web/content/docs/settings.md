# Settings

Coder's settings on a computer live in one file,
`~/.openagents/settings.json`. The Mac app, OpenAgents Terminal, the
`openagents` command, and the computer's host all read it. With no file,
every setting is its default.

## Change a setting

```sh
openagents settings show                          # every setting and the file
openagents settings get coder.providers
openagents settings disable devin                 # Coder won't use Devin
openagents settings enable devin                  # back on
openagents settings set coder.start ask_first
openagents settings unset coder.start             # back to the default
```

Lists are comma-separated. A bad value changes nothing and says why. In
OpenAgents Terminal, `/settings` shows them. In the Mac app, **Settings →
Coder** changes `coder.start` and turns agents off and on
(`coder.disabled`).

Every coding agent signed in on the computer is on: nothing needs enabling.
Settings only turn an agent off or change the order.

## The keys

| Key | Values | Default | What it does |
| --- | --- | --- | --- |
| `coder.disabled` | any of `codex`, `claude`, `grok`, `devin`, `opencode` | none | The coding agents you turned off; Coder never uses them. Every other agent is used whenever it's signed in on this computer and has room. At least one stays on. See [Coding agents](/docs/coding-agents). |
| `coder.providers` | up to 5 of `codex`, `claude`, `grok`, `devin`, `opencode`; each may add `:MODEL` | none: Codex, Claude Code, Grok Build, Devin, OpenCode | The order Coder tries agents in, first preferred, and a model for any of them. Agents you leave out still follow in the default order. |
| `coder.start` | `at_once`, `ask_first` | `at_once` | Whether a coding reply starts Coder at once, or only offers **Run Coder**. It decides for your phone's requests to this computer too. |
| `coder.usage_threshold_percent` | 1 to 100, or `off` | `90` | How full an agent's usage window can read before Coder moves on to the next agent. With `off`, Coder moves on only when an agent turns it away. |
| `coder.projects` | folders | none: any Git checkout | Which folders hold your projects. Coder runs only in a checkout inside one of them, and the Mac app tries them when a chat names no project. |
| `coder.access` | `full`, `toolchains`, `boundary` | `full` | What Coder's commands may reach. `full`: your own access; Coder runs every step without asking. `toolchains`: writes only in Coder's worktree, with this computer's developer programs and the network. `boundary`: the plain sandbox. |

Examples:

```sh
openagents settings set coder.providers claude,codex       # Claude Code first
openagents settings set coder.providers codex,opencode:openrouter/MODEL
openagents settings set coder.disabled grok,opencode       # turn two off
openagents settings set coder.projects ~/code,~/work
openagents settings set coder.access toolchains
```

OpenCode runs on the model its own configuration names (its `model`
setting), or the one you name as `opencode:PROVIDER/MODEL`. Devin bills its
own paid API for each run; turn it off if you don't want Coder to use it.

The Mac app reads the file when Coder first starts a run; restart the app
after editing the file by hand. A file that can't be read is never treated
as the defaults: Coder refuses to run and names the file, and a coding
reply only offers Coder until you fix or remove it.

## The Mac app's own settings

**Settings** (Cmd+,) has these pages:

| Page | What's there |
| --- | --- |
| Appearance | **Theme**: System (as the Mac is set), Light, or Dark, which is the default. **Reduce motion**, which keeps the Verse's camera still. |
| Text size | Smaller, default, larger, or largest (90, 100, 115, or 130 percent). |
| Keyboard shortcuts | Every shortcut. They can't be changed yet. |
| Notifications | **Notify me about Coder**: when Coder asks you something, finishes, or fails while OpenAgents isn't in front. |
| Coder | Each coding agent's sign-in and usage, **Start at once** or **Ask first**, and **Agents Coder may run**, each on until you turn it off. |
| Phones and computers | Connecting and removing phones, and Coder's project. |
| Archived chats | Chats you archived, with **Restore**. |

They're saved in the same file, under `app`.

## Environment variables

| Variable | What it does |
| --- | --- |
| `OPENAGENTS_SETTINGS` | Use another settings file. |
| `OPENAGENTS_CHAT_HOME` | Keep chats somewhere other than `~/.openagents/chat`. |
| `OPENAGENTS_TASKS` | Keep Coder's tasks somewhere other than `~/.openagents/tasks`. |
| `OPENAGENTS_CHANNEL`, `OPENAGENTS_VERSION` | For the installer: the `rc` or `stable` channel, or one version. |
| `OPENAGENTS_NO_LAUNCH=1` | For the installer: don't open the Terminal afterward. |
| `NO_COLOR` | OpenAgents Terminal draws with no color. |

Next: [Troubleshooting](/docs/troubleshooting).
