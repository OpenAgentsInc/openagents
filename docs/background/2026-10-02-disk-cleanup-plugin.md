# Disk cleanup as a plugin: background plugins, off until turned on

Status: platform built 2026-10-02
([#10165](https://github.com/OpenAgentsInc/openagents/issues/10165));
the plugin itself is made through OpenAgents' own plugin flow, recorded in
"How the plugin was made" below. Umbrella
[#10155](https://github.com/OpenAgentsInc/openagents/issues/10155), phase 1
[#10156](https://github.com/OpenAgentsInc/openagents/issues/10156).

The owner asked for the disk cleanup monitor of the
[background processes spec](2026-10-02-background-processes.md) to become a
plugin that is optional to turn on and does the same work. This page says
what a plugin could do before, what changed so a plugin can run in the
background, and where the line between the plugin and the host sits.

## What plugins could do before

From [`docs/plugins`](../plugins/README.md), `crates/coder/src/package.rs`,
and `crates/openagents-cli` (`plugin list|run|test|defaults`):

- A plugin is a directory with a package record (`package.json`) that pins
  a workflow (a NIP-PRG program) by digest, plus skills, knowledge, Wasm
  guests, and tests (`evals/`). The record required a program.
- A plugin ran only inside a Coder turn or once from `openagents plugin run`
  (and the terminal's `/plugins`, #10151): its workflow with the `reads`
  effect, its Wasm reading a workspace snapshot under fuel and memory
  limits, never writing, spawning, or reaching the network.
- Nothing ran a plugin in the background or on a trigger, and no plugin
  could delete a file under any policy.
- "Installed" meant a folder under `~/.openagents/extensions/<key>/<slug>/<version>/`,
  which the terminal listed, but nothing wrote it and nothing turned a
  plugin on or off per computer. The only per-computer selection was
  `plugin defaults sync`, which writes the plugins Coder admits for
  everyone from a `coder-defaults` release.
- Tests (`plugin test init|run|publish|check`) run Coder with and without
  the plugin in a sandbox and publish the result to the Gym.

## Decisions

1. **A plugin can bring background rules.** The package record gains
   `"background": [{"name", "digest"}]`, each a rule document under the
   plugin's `background/` folder in the background crate's rule format
   (`openagents.background.rule.v1`): triggers, goal, ordered actions, class
   settings, safety lists, and cooldown. The record pins the rule's bytes;
   moved bytes refuse the rule. `program` is optional when a plugin brings
   background rules: a plugin that only runs in the background carries no
   workflow.
2. **A rule says what it needs.** `needs.delete` lists the candidate classes
   it may delete (the `fs.delete` capability, limited to the host's six
   classes); `needs.tasks` reads the task store (which tasks ended);
   `needs.notify` sends notifications. Nothing else can be asked for.
3. **The host admits; the plugin never executes.** `background::plugins::admit`
   takes a plugin's rule only within the host's grant, every time it loads
   it (on enable, on every check, after every edit):
   - its actions only over classes in `needs.delete`, and the task classes
     only with `needs.tasks`;
   - its allow roots, agent target patterns, and checkout patterns only at
     or under the host's own roots (`~/.openagents/targets`,
     `coder-one/target`, `worktrees`, `gate`, `background/trash`, `~/work`);
   - never a built-in rule's id; intervals of at least a minute; the start,
     stop, and emergency levels in order;
   - the origin is set by the host (`plugin: KEY:SLUG`, `version`), never
     read from the file.
   Every safety check stays in the host's code and applies to a plugin's
   rule exactly as to the built-in one: the built-in deny list (which now
   also holds `~/.openagents/extensions`), no links, no other volumes, live
   tasks, slot and Cargo locks taken and held, open files and working
   directories, unsaved or unpushed worktrees, dry runs, the audit log, and
   undo. A plugin cannot add a class, an action, or a path the host does
   not already know how to clean safely.
4. **Off until turned on, per computer.** `openagents plugin install DIR`
   copies a plugin into `~/.openagents/extensions/` and leaves it off.
   `openagents plugin enable NAME` admits each of its rules and then turns
   it on in `~/.openagents/extensions/enabled.json`; `disable` turns it off;
   `installed` lists them. In the terminal, `/plugins` shows on or off for
   each installed plugin and Space turns it on or off. The host's runner
   reads the rules at every check, so turning a plugin on or off takes
   effect at the next check (at most five minutes) without a restart.
5. **Same surfaces.** A plugin's rule is one more row in
   `openagents background list` (with its plugin), `/background`, and
   NIP-HOST `background.*`; `show`, `run [--dry-run]`, `pause`, `resume`,
   `edit`, `log`, and `undo` take its id. An edit on a computer is admitted
   against the plugin again, so it cannot widen what the plugin asked for.
   A plugin rule notifies only with `needs.notify`.
6. **The built-in rule steps aside.** Once the disk cleanup plugin exists
   and is on, the built-in `disk` rule defaults off, so a new install runs
   no cleanup until someone turns the plugin on. The executor (classes,
   checks, planner, log, undo) stays in the host.

## Phases 2 and 3

- **Rules from conversation (phase 2)** compile to the same rule document.
  A rule a person defines in chat is a plugin rule with `origin: plugin`
  once saved as a plugin, so publishing a rule as a plugin (phase 3) is
  packaging, not translation, and every admission check above applies.
- **Jev judgment (phase 3)** proposes a new candidate class; a confirmed
  class becomes host code, never something a plugin declares, so the host
  still owns what "safe to delete" means.
- **A plugin marketplace for background plugins** lists them like any
  plugin. Installing one never turns it on; what it may delete is shown
  from its `needs` before it is turned on.

## How the plugin was made

The disk cleanup plugin is made through OpenAgents, as a user would:
asking in chat for a plugin that keeps the disk from filling up, letting
OpenAgents build it, drafting and approving its tests, running them, and
publishing the result. The conversation, each place the flow fell short,
and the fix for each are recorded in
[#10165](https://github.com/OpenAgentsInc/openagents/issues/10165).
