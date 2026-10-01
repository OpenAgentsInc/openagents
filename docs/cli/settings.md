# Local capability settings (`openagents settings`)

What Coder may use on this computer when the person at it asks for coding
from a chat ([#10036](https://github.com/OpenAgentsInc/openagents/issues/10036)).
One file, read by `openagents chat`, the desktop app, and a host on this
computer: `~/.openagents/settings.json` (`OPENAGENTS_SETTINGS` names another
file). The typed API is
[`coder::task::settings`](../../crates/coder/src/task/settings.rs); the local
run that honors it is [`coder::task::local`](../../crates/coder/src/task/local.rs).

With no file, or a field left out, every setting is its default, and the
defaults are exactly what a computer does with no file at all
([#10032](https://github.com/OpenAgentsInc/openagents/issues/10032),
[#10045](https://github.com/OpenAgentsInc/openagents/issues/10045)). Since
[#10091](https://github.com/OpenAgentsInc/openagents/issues/10091) the
default providers are Codex, then Claude Code, then Grok Build.

```json
{
  "schema": "openagents.settings.v1",
  "coder": {
    "providers": ["codex", "claude", "grok"],
    "start": "at_once",
    "usage_threshold_percent": 90,
    "projects": [],
    "access": "toolchains"
  }
}
```

| Key | Values | Default | What it changes |
| --- | --- | --- | --- |
| `coder.providers` | 1 to 5 of `codex`, `claude`, `grok`, `opencode:PROVIDER/MODEL`, `devin`, each optionally `NAME:MODEL` | `codex`, `claude`, `grok` | Which coding agents a run may use, first preferred. Each is still used only when it is signed in here and has capacity; one that is not is passed over with that reason ("Grok Build is not signed in here; using …"). A name alone runs `gpt-6-luna` (Codex), `claude-opus-5-5` (Claude Code), Grok Build's own default model, or Devin's default; OpenCode always names its model. |
| `coder.start` | `at_once`, `ask_first` | `at_once` | Whether a coding reply starts Coder at once, or only offers it: `openagents chat run-coder --thread ID` (or `send --run-coder`) or **Run Coder** in the app starts it. |
| `coder.usage_threshold_percent` | 1 to 100, or `null` (`off`) | 90 | The fresh usage reading at which a provider is passed over for a later one below it. Off: only a recorded refusal passes one over. A reading is only honored when the task store already holds one (a host's usage probe); the local run asks no provider. |
| `coder.projects` | absolute folders | none: any Git checkout | Which checkouts count as projects: a checkout inside one of these folders. Outside them nothing runs and the command says why. The desktop also tries these folders when a chat names no project. |
| `coder.access` | `toolchains`, `full`, `boundary` | `toolchains` | What a run's commands may reach: the filesystem boundary with this computer's developer tools (#10045), the person's full access (no sandbox, their login-shell environment and real `HOME`, credential variables still left out), or the plain boundary. |

OpenCode and Devin are never on by default. OpenCode has no default model: it
runs only as `opencode:PROVIDER/MODEL`, which the person names. Devin bills a
paid API for each run, so it runs only when the person adds `devin`
(`openagents settings set coder.providers codex,claude,grok,devin`, or its
toggle on the desktop's Settings → Coder page). A settings file written before
Grok Build became a default keeps the providers it names; `openagents settings
unset coder.providers` returns it to the default list.

A file that does not parse, has another schema, has an unknown `coder` key,
or a value outside these sets is never read as the defaults: a local run
refuses and names the file, a coding reply asks first, and
`openagents settings` refuses to overwrite it. Other top-level sections are
kept when the file is saved, so the desktop's own settings (#10021) live
beside these, in the `app` section:

```json
"app": { "text_size": "larger", "reduce_motion": true, "notifications": false }
```

`text_size` is `smaller`, `default`, `larger`, or `largest` (90, 100, 115, or
130 percent); `reduce_motion` keeps the Grid behind the window still besides
the system's own setting; `notifications` turns Coder's desktop notifications
off. A missing or unreadable `app` section is the defaults (default size,
reduce motion off, notifications on); nothing in it opens anything up
(`openagents_chat_app::preferences`).

## Commands

```sh
openagents settings show                          # every setting
openagents settings get coder.providers           # codex,claude,grok
openagents settings set coder.providers claude    # Claude Code only
openagents settings set coder.start ask_first
openagents settings set coder.usage_threshold_percent off
openagents settings set coder.projects ~/code,~/work
openagents settings set coder.access full
openagents settings unset coder.access            # back to toolchains
```

Lists are comma-separated; a relative project folder resolves against the
current directory and must exist. `--json` prints `{"key","value","path"}`
(`show`: `{"path","exists","settings"}`). A bad value exits 1 and changes
nothing; an unknown key or command exits 64.

## Where each program reads it

- `openagents chat`: before each run (`Local::here`), and for `coder.start`
  when a reply is judged coding work. `--no-run` still only offers;
  `--run-coder` still runs.
- The desktop: its Coder lane reads the file when it first starts a run
  (restart the app to pick up a change), and the chat panel reads
  `coder.start` each time a reply is judged coding work. Its Settings page
  (#10021) reads the `app` section at launch, applies each change at once,
  and saves it through `coder::task::settings`, which leaves a file it
  refuses untouched (the page then says the choice lasts until OpenAgents
  quits).
- A host on this computer: whether its chats tell the router this computer
  can run Coder (`coder::task::local::ready_here`) uses the allowed
  providers.

A device's auto-start policy (`coder host autostart on`) is separate: it is
the host owner's policy for work that phones send, and these settings do not
widen or narrow it.
