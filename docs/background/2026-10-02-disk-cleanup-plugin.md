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
- **Jev judgment (phase 3)** proposes a folder; a confirmed folder becomes
  an entry of the rule on this computer (class 7, trashed for a day
  first), never something a package brings, so the host still owns what
  "safe to delete" means. A plugin may only propose folders in its
  record's `classes`, which the person confirms like Jev's (built in
  #10158; see the main spec's "Phase 3 as built").
- **A plugin marketplace for background plugins** lists them like any
  plugin. Installing one never turns it on; what it may delete is shown
  from its `needs` before it is turned on.

## How the plugin was made

`plugins/disk-cleanup` came out of OpenAgents, driven as a user would, on
2026-10-02 (#10165):

1. **Asked in chat** (`openagents chat`, thread
   `06904082b508c7130af68f9a00386d37`): "Help me make a plugin that keeps
   my disk from filling up. It should run in the background on this
   computer and be off unless I turn it on. Same behaviour as the built-in
   disk monitor: …" with the thresholds, the six kinds of folders, what it
   must never touch, dry run, log, and undo. The router chose
   `work.dispatch` and offered Coder; Codex built the package (record,
   pinned rule, README) and its tests in the `background` crate. It chose
   to start the rule paused, so the first real run waits for a dry run, as
   the spec's safety rule 5 asks, and turned the built-in rule off for new
   hosts.
2. **Asked for one change** in the same thread: plan worktrees exactly as
   the built-in does (it had refused worktrees with no task record).
   Coder's second turn did, with a test that the plugin plans exactly what
   the built-in plans.
3. **Wrote its tests with the interview** (`openagents plugin test init
   plugins/disk-cleanup`): approved the description; said what a good and
   a failed run look like and to grade the reply; approved the six tests
   (four where it should help, two where it should stay out of the way),
   the checks, and the size.
4. **Ran them** (`openagents plugin test run plugins/disk-cleanup`):
   **No clear change**, 6 of 6 with the plugin and 6 of 6 without (one
   test 2 of 3 with). The tests ask Coder about cleanup policy, which it
   answers well either way; what the plugin adds is the work it does in
   the background, which a Coder turn does not exercise.
5. **Published the result** to the Gym: suite release
   `d3c07460b9dc685a690f783991e39f03482fe6551f2aceb9dc105868f96efd25`,
   result `7b7bda59b41a3a0d7295ff2dd5f5c8532ddad5d07509768aff26278f49627893`
   on `wss://relay.openagents.com`.
6. **Published the plugin** (#10182) with `openagents plugin publish
   plugins/disk-cleanup` from the same key: id
   `0b010805ac08dd95c8344799cc1bf0fc99c22631345a6ffe44d05067ed4610f3:disk-cleanup`,
   release `c25978cd4dfd3ac73e60a5c337e5ebbcfeeb2467ed5fa70a751253b6b02beb3a`,
   listing `0ea516da6bccb10719009e146affec4f50aac66ee7d1aa246d3eee842c1dc4a9`
   on `wss://relay.openagents.com`, its files in the eval blob bucket
   (the relay still refused uploads, #10181). `openagents plugin install
   disk-cleanup` fetches and checks it, off.
7. **Turned it on** on each computer with `openagents plugin install
   plugins/disk-cleanup`, `openagents plugin enable disk-cleanup`, a dry
   run compared with the built-in rule's, and `openagents background resume
   disk-cleanup`.

### Where the flow fell short

| Friction | Fix |
| --- | --- |
| A plugin could not run in the background, ask for a delete capability, or be turned on per computer. | `background::plugins`, `package.json` `background`, `plugin install|enable|disable|installed`, `/plugins` Space (72cf012985). |
| "Help me make a plugin …" with no details got a plan reply and a Coder offer, not the interview; with details it went straight to Coder. The interview only writes tests for a plugin that exists. | On a computer the request is the plugin-creation flow: what it should and shouldn't do, Coder's draft with its tests, the tests for approval, a run, and publish and turn on (#10177). |
| `plugin test init` sent the model door an empty conversation, which the gateway refuses (`input: Too small`). | The person's request is the first message. |
| A background plugin's tests admitted nothing in the subject arm (no program, no skills). | The subject arm reads each pinned background rule as guidance. |
| The interview cannot try or run the tests itself from a terminal; it prints the command. | None yet. |
| Tests that grade files a run makes fail: a `coder -p` turn answered in chat. | Asked the interview to grade the reply. |
| `plugin test publish` uploads suite files to the relay, which refuses uploads (405); there is no public store for a person's suite. | Released with `--blobs-dir`, copied the files to the eval blob bucket, and published with `--blossom` naming it. |
| Nothing publishes a plugin itself to a registry (a NIP-EXT listing); the Gym holds its test result. | `openagents plugin publish`, `search`, and `install NAME` (#10182). |
