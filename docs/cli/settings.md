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
[#10184](https://github.com/OpenAgentsInc/openagents/issues/10184) coding
agents are opt-out: every agent signed in here (Codex, Claude Code, Grok
Build, Devin, OpenCode, in that default order) is used with nothing to
enable. The settings only record what the person turns off
(`coder.disabled`) and an optional order (`coder.providers`). No one edits
this file to use an agent they have signed in to.

```json
{
  "schema": "openagents.settings.v1",
  "coder": {
    "providers": ["claude"],
    "disabled": ["devin"],
    "start": "at_once",
    "usage_threshold_percent": 90,
    "projects": [],
    "access": "full"
  }
}
```

| Key | Values | Default | What it changes |
| --- | --- | --- | --- |
| `coder.disabled` | any of `codex`, `claude`, `grok`, `devin`, `opencode` | none | The agents the person turned off: the only ones a run never uses. Turning every agent off is refused. `openagents settings disable AGENT` / `enable AGENT`, the terminal's `/settings`, and the desktop's Settings → Coder toggles edit only this. |
| `coder.providers` | up to 5 of `codex`, `claude`, `grok`, `devin`, `opencode`, each optionally `NAME:MODEL` | none (the default order) | The order a run tries agents in, first preferred, and a model for any of them; never a list of what may run. Agents it leaves out follow in the default order. Each is still used only when it is signed in here and has capacity; one that is not is passed over with that reason ("Grok Build is not signed in here; using …"). A name alone runs `gpt-6.1-sol` at medium reasoning (Codex), `claude-opus-5-5` (Claude Code), Grok Build's own default model, Devin's default, or the model OpenCode's own configuration names (its `model`; OpenCode with none is passed over). |
| `coder.start` | `at_once`, `ask_first` | `at_once` | Whether a coding reply starts Coder at once, or only offers it: `openagents chat run-coder --thread ID` (or `send --run-coder`) or **Run Coder** in the app starts it. A host on this computer also tells its paired phones (presence capability `coder-start-at-once`, while its auto-start policy is on), so a coding reply on the phone starts Coder here at once too, or offers **Run Coder** under `ask_first` (#10101). |
| `coder.usage_threshold_percent` | 1 to 100, or `null` (`off`) | 90 | The fresh usage reading at which a provider is passed over for a later one below it. Off: only a recorded refusal passes one over. A reading is only honored when the task store already holds one (a host's usage probe); the local run asks no provider. |
| `coder.projects` | absolute folders | none: any Git checkout | Which checkouts count as projects: a checkout inside one of these folders. Outside them nothing runs and the command says why. The desktop also tries these folders when a chat names no project. |
| `coder.access` | `toolchains`, `full`, `boundary` | `full` | What a run's commands may reach: the filesystem boundary with this computer's developer tools (#10045), the person's full access (no sandbox, their login-shell environment and real `HOME`, credential variables still left out), or the plain boundary. |
| `coder.shadow` | 1 to 100 (percent of runs), or `null` (`off`) | off | The shadow baseline ([#10209](https://github.com/OpenAgentsInc/openagents/issues/10209)): that share of this computer's finished Coder runs (a first turn on Codex or Claude Code, not an issue flow) also runs once through the raw engine on its own defaults (`claude -p`, or `codex exec` when Codex ran the task) with the person's request, in a scratch clone of the same commit with no remote. Its changes are never applied: the clone is deleted once `openagents shadow report` writes the record (cost, wall time, turns, and the recipe's kept checks on both sides) to `~/.openagents/shadow/records.jsonl`. Sampled by the task ID, one at a time. [`coder::task::shadow`](../../crates/coder/src/task/shadow.rs). |
| `coder.shadow_budget_usd` | dollars (`5`, `2.50`), or `null` (`off`) | off (no cap) | The most the shadow baselines may cost in all, as the person sets it: once the recorded baselines reach it, no new one starts. |
| `coder.claude` | `session` or `loop` | `session` | How Claude Code takes a turn ([#10246](https://github.com/OpenAgentsInc/openagents/issues/10246)). `session`: one Claude Code session, briefed by Jev (probes, survey, knowledge, frozen checks), with the cost audit's lean settings: six tools (`Bash, Read, Edit, Write, Glob, Grep`), the trimmed system prompt, the five-minute prompt cache, and medium effort (low for a question). `loop`: Microcoder's step loop, one `claude -p` call per step. `session` is the default because it measured 0.61× raw Claude Code's cost (95% CI 0.57–0.65) at 21 of 21 passes, where the loop cost 1.68× ([measurement](../cost/2026-10-02-shadow-baseline-measurement.md#the-lean-session-arm-10246)). `session` applies under `coder.access` `full`; under `toolchains` or `boundary` Claude Code runs the loop, whose commands the host bounds. [`microcoder::repository::claude_session`](../../crates/microcoder/src/repository/claude_session.rs). |

Migration: a file written before #10184 whose `coder.providers` left an agent
out (the old default was `codex,claude,grok`) reads as that order, and the
agents it left out now follow it; nothing is excluded unless it is in
`coder.disabled`. An empty `coder.providers` is the default order. Devin
bills a paid API for each run; a person who does not want that turns it off
(`openagents settings disable devin`).

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
openagents settings disable devin                 # Coder never uses Devin
openagents settings enable devin                  # Devin back on
openagents settings get coder.disabled            # (empty: every agent on)
openagents settings set coder.providers claude    # Claude Code first, the rest after
openagents settings set coder.start ask_first
openagents settings set coder.usage_threshold_percent off
openagents settings set coder.projects ~/code,~/work
openagents settings set coder.access full
# Unsetting coder.access restores the default value, full.
openagents settings unset coder.access
```

Lists are comma-separated; a relative project folder resolves against the
current directory and must exist. `--json` prints `{"key","value","path"}`
(`show`: `{"path","exists","settings"}`; `disable`/`enable`:
`{"agent","on","disabled","path"}`). A bad value exits 1 and changes
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
  can run Coder (`coder::task::local::ready_here`) and the agents it lists
  (`engines_here`) use every agent not turned off.

A device's auto-start policy (`coder host autostart on`) is separate: it is
the host owner's policy for work that phones send, and these settings do not
widen or narrow it. `coder.start` only decides whether a phone sends that work
for a coding reply without a tap (#10101).
