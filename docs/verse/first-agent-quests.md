# The Apprentice's Road: a quest line into Everglade

Status: the five-step native tracker and free scratch practice are implemented,
October 6, 2026. The acts, guide character, districts, rewards, and unlocks below
remain proposed.

## Current native practice

Run `verse --onboarding-practice /absolute/private/starter` or
`openagents-terminal --onboarding-practice /absolute/private/starter`. Choose a
new or empty directory. This creates a private starter repository with no remote,
an inert retained Studio task, scripted decision and local merge evidence, and
retained authored contribution metadata. It runs no model, publishes nothing,
spends $0, and grants no XP or execution authority. The panel labels this lane
**Simulated practice**; scripted responses never count as real completions.

Reopen the same directory with the same flag, or pass
`--onboarding-workbench /absolute/private/starter/config.json` to either app.
Reopening reads the retained evidence and does not submit another task, run an
engine, replay an answer, or create another commit. A partially initialized
starter refuses automatic replay.

The panel shows five objectives: open a terminal, submit a scratch Studio goal,
answer a decision, inspect and merge exact reviewed revisions, and inspect a
contribution or reproduction. It skips facts already true and shows the source
references and missing prerequisites. Task completion, activity, and disappearing
decisions cannot establish answered or reviewed objectives.

For real work, use an explicitly selected owner configuration and the existing
[Agent Studio](agent-studio.md) admission and review controls. The selected task
store and owner stream must match the scratch workspace. The host reader accepts
only retained tasks in the isolated repository, exact successful decision
acknowledgments, and matching reviewed revisions with a local commit. Native
confirmation retains these acknowledgments; the tracker itself sends nothing.
Real model cost is unavailable until the existing plan and operator limit admit
it. Review that cost and authority before sending any real operation. Terminal
and contribution inspection evidence must name their exact selected owner
source; an unavailable source remains incomplete. Onboarding awards no money,
levels, titles, decorations, or rights.


A new player arrives in [Everglade](everglade.md) knowing nothing about coding
agents, hosts, or the [Agent Studio](agent-studio.md). The Apprentice's Road is
a quest line that takes that player from a first walk through the glade to a
working team of agents on their own repository. Along the way they:

- connect a computer;
- build and name a first agent;
- answer its questions and review its work;
- raise a workshop of their own;
- run several agents at once;
- keep working from their phone.

Each quest teaches one real step. When the line ends, the player can do
day-to-day work in the studio without having read a manual.

## Principles

- **Quests are real steps.** Every objective is a fact the system already
  reports, such as a grant on the host, a seat in the studio snapshot, or a
  merge in the checkout. A quest never asks for a fake action that exists only
  to tick a box. The [game design document](gdd.md) says quests are real jobs;
  onboarding quests are real setup.
- **Show the place before the dashboard.** In Everglade, an agent's position
  shows what it is doing (see [Inspiration](everglade.md#inspiration)). The
  quests teach players to read the world first and open a panel only when they
  need detail.
- **Skip what's already true.** Each objective is a check against current
  state, so a player who already paired a computer or ran a studio completes
  those quests on arrival. Experienced players can skip a whole act.
- **A free path at every step.** Each step that costs model spend has a
  simulated version on the [simulated team](agent-studio.md#simulated-team),
  so a player can learn the flow before paying for it. The quest states the
  expected cost before a real run starts.
- **Safe by default.** Early quests work on a starter repository in a scratch
  checkout. Nothing pushes. `--full-access` and the player's own repository
  come later, behind an explicit choice.
- **No money and no farming.** Rewards are titles, workshop decorations, and
  unlocks. Only the final act's reproduction earns
  [NIP-XP](../../nips/openagents/NIP-XP.md), under the rules XP already has
  (see [Tutorial quests](tutorial-quests.md)).

## The guide

Bram, the glade's keeper, stands at the stepping-stone approach in front of
the workshop. He's a non-player character with a nameplate and a quest marker,
built from the Universal base characters. He hands out each quest, and he
reappears at the station that the current quest is about, so the player can
follow him through the glade.

Bram's lines are fixed text written for this quest line. They are not model
output, so the tutorial costs nothing and says the same thing every time.

The quest log opens with `J`. It also shows as a pinned tracker on the right
edge, in Everglade's colors (not black and white). Each objective shows a
checkmark when its fact becomes true, and a **Why this matters** line in plain
language.

## The road through the map

Each act opens a part of the [city map](everglade-map.svg). The glade
grows into the city as the player's practice grows. Until a district's act
begins, it shows as a signposted path under construction.

| Act | Theme | Where on the map |
| --- | --- | --- |
| 1. Arrival | Walk, look, and watch a team at work | The glade and workshop yard |
| 2. The Hearth | Connect a computer and an engine | Server Barn and Fab Yard |
| 3. The First Apprentice | Build one agent and finish one errand | The workshop hall |
| 4. The Workshop | Your own repository and a team | Makers' Hall, Creative District |
| 5. Beyond the Glade | Phone, standing orders, the Gym, other people | The Stacks, Old College, Lantern Quarter |

## Act 1: Arrival

Act 1 runs in the browser, on phones, and on the desktop. It needs no account
and no computer.

| Quest | Objectives | Teaches | Evidence |
| --- | --- | --- | --- |
| **A Clearing in the Woods** | Walk from the arch to Bram. Use the hotbar's Levitate to see the glade from above. | Movement, the camera, the hotbar | Player position and hotbar use, on the client |
| **The Team at Work** | Watch the simulated team for a minute. Find a seat at the Workbench, one at the Podium, and one in the Lounge. | That place shows activity: Workbench means running commands, Podium means waiting on you, Lounge means paused | The studio replay (`verse --studio-sim`); the player selects each seat |
| **Reading the Wall** | Open the Task Wall at the notice board. Open one seat's desk panel. Open a diff at the Merge station. | The three panels a player uses most | Panels opened in the simulated studio |
| **A Name in the Ledger** | Create an identity and pick a trainer name. | The Nostr key that later grants, XP, and the phone pairing all hang on | A trainer key on the device, shown on the trainer card |

Reward: the title **Wanderer**, and a lantern for the player's future desk.

## Act 2: The Hearth

Act 2 connects the player's computer, which hosts their agents and their work.
Bram meets the player at the Server Barn, a new building on the glade's edge
from the map's Server Barn and Fab Yard.

| Quest | Objectives | Teaches | Evidence |
| --- | --- | --- | --- |
| **Light the Hearth** | Install the OpenAgents desktop app, or run `openagents connect`, and start the Coder host. | What a host is: the computer that runs agents and keeps the work | Host presence ([NIP-REACH](../../nips/openagents/NIP-REACH.md)) for the trainer's owner key |
| **Two Keys, One House** | Pair this device (phone or browser) with the host. | Pairing, and that a device holds rights the host granted | A device grant from the host ([NIP-HOST](../../nips/openagents/NIP-HOST.md) enrollment) |
| **Tools of the Trade** | Sign in to at least one engine: the Codex login or Claude Code. Or choose the OpenAgents cloud fallback. | Engines, and that the player brings their own subscription | `coder doctor` reports a door with capacity in the capacity book |
| **The Barn Lights Up** | Walk into the Server Barn and see the computer drawn as a lit machine with its name. | That the world shows the player's real computers | The Computers projection (`coder-computers`) |

Reward: the title **Hearthkeeper**. The Server Barn shows one lit machine for
each connected computer from then on.

## Act 3: The First Apprentice

This is the heart of the line: the player builds their first agent and walks
one task through every station. It runs on the
**starter repository**, a small public template repository made for this
quest line. It holds a tiny command-line program with a failing test, an
errand list, and one change that is set up to need an approval. The quest
clones it into a scratch checkout on the player's host.

| Quest | Objectives | Teaches | Evidence |
| --- | --- | --- | --- |
| **An Apprentice's Name** | Name the first agent, and choose its engine and model, at the empty desk. | A seat is an agent with an engine; names make agents people you know | `studio up` with one seat; the seat in the studio snapshot |
| **A Small Errand** | Submit the starter repository's first goal at the notice board: "Make the failing greeting test pass." Watch the apprentice walk from desk to Workbench to Proving ground. | Goals, tasks, and reading progress from where the agent stands | A task that moves from planned to running to done |
| **A Question at the Podium** | When the beacon lights, answer the apprentice's approval at the Podium. | Decisions: the agent asks before risky steps, and the player answers | A decision answered (`studio.answer`) |
| **The Strongroom** | Review the change at the Merge station and merge it. | Review before merge; the host commits locally and never pushes | The task merged, and a commit in the scratch checkout |
| **Ask for Better** | On a second errand, use **Request changes** with one line comment, then merge the revision. | Steering through review instead of starting over | A review with changes requested, then a merge |

Each quest offers **Practice first**: the same steps on the simulated team at
no cost. Before the first real run, Bram states the expected cost. The
reference run on October 4 finished a two-task goal for $0.19.

Reward: the title **Apprentice Keeper**, a nameplate banner for the
apprentice, and its desk monitor in the player's color.

## Act 4: The Workshop

Act 4 moves the player from the starter repository to their own work, and
from one agent to a team. Bram meets them at Makers' Hall, the first building
of the map's Creative District, which becomes the player's own workshop. Its
desks, banners, and lanterns come from earlier rewards.

| Quest | Objectives | Teaches | Evidence |
| --- | --- | --- | --- |
| **Raise the Workshop** | Point the studio at a repository of your own (`--repo`) and name the workspace. The quest explains `--full-access` and asks before turning it on. | Workspaces, the clean-checkout rule, and what full access means | A studio workspace on a repository outside the starter |
| **Many Hands** | Add a lead and a second worker, with at least two different engines, and submit one goal that the lead splits into tasks. | The lead plans, workers build, and tasks wait on their dependencies | Three seats; a goal with two or more tasks on the Task Wall |
| **When Hands Collide** | Resolve a merge conflict that the host sends back to a seat. The starter repository has a branch that causes one on purpose. | Conflicts come back with the merge already started, and the seat finishes it | A task returned for a conflict, then merged |
| **The Lounge** | Pause a seat, then resume it from the Lounge. | Controlling a busy team without stopping the goal | Seat activity: paused, then resumed |

Reward: the title **Workshop Keeper**. Makers' Hall carries the workspace's
name over its door.

## Act 5: Beyond the Glade

Act 5 is about daily habits and other people. Its quests can be done in any
order.

| Quest | Objectives | Teaches | Evidence |
| --- | --- | --- | --- |
| **From Afar** | Answer a decision from the phone after a push wakes it. | Working away from the desk; the phone acts through the paired computer | A decision answered from a phone device grant |
| **Standing Orders** | Make one answer a standing rule (`--always`). | Fewer interruptions for trusted actions, and how to revoke a rule | A standing rule in the host's approvals |
| **The Old College** | Complete one [tutorial quest](tutorial-quests.md): reproduce a published Terminal-Bench pass. | The Gym, evidence, and earning real XP | A NIP-XP award from the referee |
| **Open Doors** | Invite a friend with a watch-only grant, and walk the workshop together. | Sharing a studio safely: watching is not acting | A second device with an `observe` grant ([shared Everglade](networking.md#hosted-social-profile-implementation)) |

Reward: the title **Steward of the Glade**, and the first level shown over
the player's head. That level comes from the XP earned in *The Old College*.

## How quests work

**Quest definitions.** Each quest is a data file: an ID, a version, its
text, its objectives, and its rewards. Each objective names a typed check,
such as `host.present`, `grant.device`, `doctor.door`, `studio.seats >= 3`,
`task.state = merged`, `decision.answered`, or `xp.award`. Rust code
evaluates the checks against what the client already reads: the studio
snapshot, the Computers projection, the host's answers, and the NIP-XP
ledger. Adding a quest means adding data, not code, unless it needs a new
check.

**Progress.** Progress is local to the trainer and recomputed from facts, so
it can't drift and can't be forged by editing a file. A quest is complete
when all its checks hold, or held once while the quest was active, for
transient facts such as a decision answered. The client records each
completion with the evidence it saw: a task ID, a decision ID, or a commit.

**Where it runs.**
- **Browser:** Act 1 only.
- **Phone:** Act 1, then Act 2's pairing, and Act 5's *From Afar*. Everything
  else on the phone shows progress and points to the computer.
- **Desktop:** the whole line.

**Hints.** When the player is stuck, Bram's marker moves to the station that
needs attention. The objective's **Show me** button highlights the station or
key, and the panel's control when the panel is open.

## What exists and what's missing

| Needed | Status |
| --- | --- |
| Everglade, its stations, panels, and the simulated team | Exists |
| Studio up, goals, decisions, review, merge, conflicts, pause and resume | Exists (`openagents studio`, Everglade panels) |
| Hosts, pairing, device grants, the Computers screens | Exists |
| `coder doctor` and the capacity book | Exists |
| Tutorial quests and NIP-XP awards | Exists |
| Shared Everglade with watch-only grants | Mostly built (#10553); watch-only studio reads are not built yet |
| Quest data format, check evaluator, quest log, and tracker | New |
| Bram and quest markers | New |
| The starter repository with its planned approval and conflict | New |
| The Server Barn, Makers' Hall, and the district signposts | New layout, from existing kits |
| Studio setup from inside Everglade (name a seat, pick an engine) instead of the CLI | New: a panel over `studio up` |
| Push for a decision on the phone | Specified in Agent Studio; check delivery |

## Suggested order of work

1. **Quest engine:** the data format, typed checks, the quest log, and the
   tracker. Act 1 runs entirely on it, in all three clients.
2. **Starter repository and Act 3:** the starter repository, the setup panel,
   and Act 3 on the desktop, with the simulated practice runs.
3. **Act 2:** the Server Barn, and the computer and engine checks.
4. **Act 4:** Makers' Hall and the team quests.
5. **Act 5:** after shared Everglade and phone push are done end to end.

## Open questions

- Whether the guide is one character, or one per district (a smith at the
  Server Barn, a librarian at the Old College).
- Whether quest completions should be published as Nostr records, so a
  trainer card can show "Workshop Keeper" to others, or stay local.
- Whether the starter repository should come in several languages (Rust,
  Python, TypeScript) so the first errand feels familiar.
- How much of Act 4 should be required. A player with one repository and one
  engine may never need a three-seat team.
