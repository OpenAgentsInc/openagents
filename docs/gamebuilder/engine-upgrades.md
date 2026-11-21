# Engine upgrades from a request

Status: design (issue 20). The engine capability list (issue 21) and
`triage` (issue 22) are built; "Triage" says how the built command differs
from this design. `engine-ready`, `engine-mine` and the fork wording
(issue 23) are built; "The pull request" says how. Nothing else in this document is built unless it says
"today".
The work is listed in [Build steps](#build-steps).

## The goal

Anyone who uses the `gamebuilder` command line can ask for a change to their
game in plain words. So can Coder, which loads the GameBuilder skill. One of
four things happens:

1. **The commands can do it.** "Move the tower next to the gate" runs as
   commands: `read-level`, then `move` or an `apply-edits` batch. This is the
   common case and it needs no model to decide.
2. **It needs GameBuilder's own code.** A new kind of thing, a template or a
   game rule.
3. **The engine can do it, and GameBuilder does not use it yet.** The work is
   in this repository, not the engine.
4. **The engine can't do it.** The person is asked:

   > The engine can't do this yet. Would you like to code up this feature and
   > potentially earn rewards for anyone who uses it?

   A yes ends in a pull request at
   [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents).
   The person does not need push access to that repository. Their agent works
   in their own fork.

The command line says which of the four a request is, and why. It does not
start an engine change for something a command already does.

## What exists today

- **The skill** (`gamebuilder skill`, installed by `scripts/install-cli.sh`
  at `~/.claude/skills/gamebuilder/SKILL.md`) has a section "When the world
  needs more than the commands". It names the three owners (the commands,
  GameBuilder's code, the engine) and prints the seven steps.
- **`gamebuilder engine`** answers the openagents commit GameBuilder is built
  on (`openagents.lock`), a link to the source at that commit, six engine
  parts in a sentence each, and the seven steps (`GB_ENGINE_STEP` in
  `coax/commands.coax`). Step 3 says "fork it if you can't push". Step 6 is
  `gh pr create --repo OpenAgentsInc/openagents`.
- **`gamebuilder code-brief openagents`** answers `gamebuilder.code.v1`: what
  a Code agent on the engine is given and may do. It works in a fresh
  worktree from `origin/main` under `~/gamebuilder-code`, ends in a pull
  request, never pushes to `main`, and has 80 turns and 1,000 cents. The
  person confirms before it starts.
- **Coder** reads the first 16 installed skills, so it knows the skill
  exists and reads it before GameBuilder work.

What is missing:

- Nothing decides which of the four a request is. The skill tells the agent
  to "be sure" and to read the engine's source. An agent with no openagents
  checkout can't do that well, and a wrong guess costs a pull request nobody
  wants.
- The list of engine parts is six sentences. It does not say what the engine
  can do.
- The offer is not worded anywhere, and no reward exists.
- The fork steps are one clause. Nothing checks that `gh` is installed and
  logged in, makes the fork, or points the worktree at it.
- openagents has no guide for an outside pull request. See
  [What openagents needs](#what-openagents-needs).

## Triage

A command answers where a request belongs.

**As built (issue 22).** `gamebuilder triage "REQUEST"` differs from the
design below in these ways:

- It takes no `--level`.
- It has five fits, not four: `engine-partly` is for a chapter that is
  partly implemented, which is 57 of the 103.
- It has no embeddings and no rules ahead of the model: OpenAgents has no
  embeddings route the command line can call. One question set to the
  decision door picks the owner, the command and the chapter, by meaning.
  Everything after the picks is a rule: the chapter's status makes the fit.
- `sure` is `false` when the model was under 75% sure of the owner, or of
  the chapter for an engine fit. The answer then carries the chapter it
  picked, whatever the fit.
- It names one command and one chapter, not 6 and 3.
- A request is up to 1,000 bytes.
- With no network it refuses as `unreachable` and points to `engine-list`.
- It signs each call with a key it makes on first use and keeps in
  `~/.openagents/gamebuilder/decide-key-PROFILE` (mode 0600). The key is no
  login and names no account. The door limits each key by the minute and
  the day.
- It is on the command line only, not on MCP or HTTP.

The same request gives the same answer each time: two requests, run four
times each from four different keys on 2026-11-17, answered the same every
time. A request worded another way can get another answer, and so can the
same request after the command table or the engine's book changes, since
both are in the question set. In seven trial requests the owner pick was
right in six; the seventh was wrong and said it was not sure. Treat
`"sure":false` as a prompt to read `engine-list`, not as an answer.

The design:

```sh
gamebuilder triage "water that reflects the sky" --level 3
```

It reads only. It answers `gamebuilder.triage.v1`:

| Field | What |
| --- | --- |
| `fit` | `commands`, `gamebuilder`, `engine-has` or `engine-needs` |
| `sure` | `true` when a rule decided it, `false` when a model did |
| `why` | One sentence the agent can say to the person |
| `commands` | For `commands`: the verbs to use, in order |
| `chapters` | For the engine answers: up to 3 engine book chapters, each with its number, title, status and source files |
| `part` | For `engine-needs`: the engine folder the change belongs in |
| `offer` | For `engine-needs`: the offer's words, to say as they are |
| `next` | The next command: an edit command, `code-brief gamebuilder` or `code-brief openagents` |

### How it decides

Rules first, a model only when the rules are unsure.

1. **The command table.** `coax/commands.coax` has one row for each thing the
   command line does, with its arguments and bounds. The registry has one
   row for each property a thing can have. If the request is an edit these
   rows cover (place, move, turn, resize, remove, set a property, copy,
   make a level or a game), the answer is `commands`.
2. **The engine capability list** (next section). If a chapter that is
   implemented covers the request, the answer is `engine-has`. If only a
   design draft covers it, or none does, the answer is `engine-needs`.
3. **GameBuilder's own lists**: the kinds (`shared/kinds.coax`), the
   templates and the game types. A request for a new one of these is
   `gamebuilder`.
4. **A model, when the steps above disagree or find nothing.** One question
   to the OpenAgents decision door, the one `coax/decide.coax` already
   calls. The question carries the request, the nearest command rows and the
   nearest chapters, and the model picks one of the four. The answer says
   `"sure": false`.

Matching a request to rows and chapters is by meaning, not by words in the
request. Each row and chapter has an embedding made when the command line is
built. The request is embedded once and compared by cosine similarity. This
follows the workspace rule: no keyword routing for what a person asks.

So triage makes at most two network calls: one embedding and, when unsure,
one decision. With no network it answers from the rules it can run without
an embedding, says `"sure": false`, and says the network was not reached.

Limits, stated in the answer:

- A request is up to 2,000 bytes.
- Up to 3 chapters and 6 commands are named.

### What the agent does with it

The skill's section changes to: run `triage` first, then act on `fit`.

| `fit` | The agent |
| --- | --- |
| `commands` | Runs the commands. No question to the person |
| `engine-has` | Says the engine can do it and GameBuilder does not use it yet. Offers a change to GameBuilder (`code-brief gamebuilder`) |
| `gamebuilder` | The same offer, for GameBuilder's own code |
| `engine-needs` | Says `why`, then says `offer` as it is. On a yes, follows [The pull request](#the-pull-request) |

When `sure` is `false`, the agent says so before it offers anything: "I
think the engine can't do this, but I'm not certain."

## The engine capability list

Triage needs to know what the engine can do, with no openagents checkout and
no network. The list ships inside the command line.

**Source.** At the commit in `openagents.lock`:

- `metaverse-engine/book/chapters.json`: 103 chapters. Each has a title, a
  status (14 implemented, 57 partly implemented, 32 design draft at the
  pinned commit), its source files, the chapters it depends on, and the
  files that use it (`adopted_by`).
- `docs/metaverse/engine-core.md`: one section for each primitive.
- `docs/reuse.md`: the "you need, use, where" table.

**Shape, today.** A generated Coax table, `coax/engine-list.coax`: one row
for each chapter with its number, title, status, the book's part, engine
folder, up to 4 source files and a summary of up to 300 bytes. 103 rows. It
is made from `chapters.json`, the book's `SUMMARY.md` and each chapter's
file. It does not yet read `engine-core.md` or `reuse.md`, and it has no
embeddings.

**How it is made, today.** `tools/engine-list.coax` reads the book in the
pinned tree and writes the table. `scripts/pin.sh bump` runs it. The file is
generated and tracked, like the pin: a bump that changes the engine changes
the list in the same commit. `scripts/check.sh` fails when the list is not
what the book at the pinned commit makes.

**Who reads it, today.** `gamebuilder engine-list` answers every row in a
line. `gamebuilder engine-read N` answers one chapter in full, with what its
status means for a request. The skill tells an agent to read them before it
says the engine can't do something.

The list is as good as the engine's book. A capability with no chapter is
not in it. Triage's `why` names the chapter it relied on, so a wrong answer
can be traced to a wrong or missing chapter, and the fix is a book change in
openagents.

## The pull request

One path, the one that exists: the seven steps and the Code agent brief.
This section fills in the fork.

**As built (issue 23).** The commands are `gamebuilder engine-ready` and
`gamebuilder engine-mine`. `engine-ready` also checks git, and it answers
`push_to`: `origin` when the person may push to the repository, `fork`
otherwise. Nothing forks ahead of time. A Code agent on the engine is told to
ask GitHub whether the person may push, and when they may not, to run
`gh repo fork OpenAgentsInc/openagents --remote --remote-name fork` in its
folder and push its branch there. The words the person approves before a
Code agent starts say that it makes the fork in their name the first time.
`engine-mine` reads the newest 20 pull requests and asks GitHub, for each
merged one, whether the pinned commit holds it. The fork path itself has not
been run: nobody without push access has tried it.

`gamebuilder engine --ready` checks what the person needs and answers what
is missing, each with the one command that fixes it:

| Check | Fix |
| --- | --- |
| `gh` is installed | `brew install gh` |
| `gh` is logged in | `gh auth login` |
| A fork exists | `gh repo fork OpenAgentsInc/openagents --clone=false` |
| Claude Code is installed, for a Code agent | The install line from its site |

An agent runs the fork command itself once the person has said yes to the
offer. It never logs in for them.

Then the steps:

1. **Agree.** The agent says what the engine can't do, what it would add and
   where, and that the pull request is public and in the person's GitHub
   name. The person says yes.
2. **Worktree.** Under `~/gamebuilder-code`, from `origin/main` of
   OpenAgentsInc/openagents, on a branch named for the feature. The push
   remote is the person's fork.
3. **Build it as an engine primitive**: its own module, its own `#test`
   checks, and a chapter or section in the engine's book. Any world can use
   it, never only GameBuilder. The agent reads openagents' `AGENTS.md` and
   `CONTRIBUTING.md`.
4. **Check.** The one command `CONTRIBUTING.md` names.
5. **Push to the fork and open the pull request**:
   `gh pr create --repo OpenAgentsInc/openagents`. The description says what
   a GameBuilder world needs the feature for, which chapter it adds or
   changes, the check's output, and the contributor's OpenAgents account for
   credit.
6. **Wait.** The agent gives the person the link. `gamebuilder engine
   --mine` lists their open and merged engine pull requests and whether
   GameBuilder's pin has reached each.
7. **Use.** After the merge, GameBuilder's pin moves (`scripts/pin.sh
   bump`), the capability list changes with it, and the next release of the
   command line and the app can use the feature.

Limits, said before the agent starts:

- A Code agent has 80 turns and 1,000 cents (`gamebuilder.code.v1`).
- The pull request is reviewed by a person at OpenAgents. It may be changed
  or declined.
- The feature reaches GameBuilder only at the next pin bump and release.

Step 7 is not in the contributor's hands. Until a release carries the new
pin, their world can't use their own feature. `engine --mine` says this in
words, with the pin's commit, so nobody waits on a merge that already
happened.

## What openagents needs

Checked on 2026-11-17 at `origin/main`:

- The repository is public, Apache-2.0, with 63 forks.
- Outside pull requests were opened and merged between July and October
  2026. None has come since the repository's history was rewritten on
  2026-11-01.
- It has no `CONTRIBUTING.md` and no `.github` folder. So it has no pull
  request template, and no check runs on a pull request.
- Its `AGENTS.md`, "Git", is written for agents that push to `main`: a
  pre-push check, an `Agent:` line in each commit, hooks installed from
  `coax/remote/hooks/`. It does not say what an outside contributor does.

Each of these is a change in openagents, by issue there:

1. **`CONTRIBUTING.md`.** How to fork and branch, the one check command,
   what an engine primitive must have (a module, checks, a book chapter),
   that commits need no `Agent:` line from an outside contributor, and the
   licence the contribution is under.
2. **A pull request template** with the fields step 5 names.
3. **One check command** a contributor runs with no secrets and no private
   hosts: the checks the pre-push hook runs, less the ones that need the
   owner's machines. `AGENTS.md` points to it from "Git".
4. **Review.** Who looks at an engine pull request and how soon. A proposal:
   an OpenAgents agent does a first review within a day and asks for
   changes; a person approves the merge.
5. **Safety.** Code from a fork is never built or run on a machine that
   holds secrets, a production login or a wallet. A review agent reads the
   diff first and runs it only in a sandbox with no credentials. No check on
   a pull request gets repository secrets.
6. **After the merge.** The merge commit keeps the contributor as its
   author. Someone tells GameBuilder: an issue here, "Bump the pin for
   FEATURE", opened by the merge.

Until items 1, 3 and 5 exist, the offer should not be shown to people
outside the team: a contributor would have no check to run and their code
would have no safe place to be reviewed.

## Rewards

**Nothing is defined today.** `docs/business/README.md` names tips, the
market, bounties and a daily pool of sats for makers. None of them pays
someone for an engine feature. That is why the offer says "potentially".

A proposal, for the owner to decide:

- **Credit.** Each merged engine primitive records its contributor: the
  OpenAgents account named in the pull request, in the chapter's row of
  `chapters.json` (a new `contributed_by` field).
- **Use.** A primitive is used by a world when the world's level or game
  loads the module. The engine's book already keeps `adopted_by` for source
  files; the count that matters for pay is worlds that are played, which the
  server would count the way it counts play for the makers' pool.
- **Pay.** A share of the daily makers' pool goes to contributors, split by
  play in worlds that use their primitive.
- **Rails.** Bitcoin only: Lightning, to the contributor's Lightning
  Address. No other rails.

### At launch: credit only

Decided for launch (issue 25), until the owner sets a reward:

1. **Credit only.** Nothing pays an engine contributor. The offer keeps the
   word "potentially".
2. **No pool share.** No part of the makers' pool goes to engine
   contributors, so there is no share and no earning time to set.
3. **No bounties.** Nobody can post a bounty for a missing feature yet.
4. **A GitHub name is enough for credit.** `contributed_by` holds the
   contributor's GitHub name. An OpenAgents account is not needed. It is
   needed only if a payment is added later, because a payment goes to the
   account's Lightning Address.

Still the owner's to decide, and listed in the workspace's `NEEDS_OWNER.md`:
whether to pay at all, the share of the pool and how long a primitive earns,
and whether bounties exist. Each is a change to money, so no agent decides
it.

The offer's words change from "potentially earn rewards" only when a reward
is decided and the payment exists.

## Coder

Coder needs no code of its own for this. It reads the skill and runs the
command line. Two things make it work well:

- **The skill says to run `triage` first** and how to act on each answer.
  This is the change in [What the agent does with it](#what-the-agent-does-with-it).
- **Coder's sub-agents fit the work.** Triage and edits are Microcoder's.
  An engine primitive is a task for a stronger model: the skill tells the
  agent to hand step 3 to the strongest sub-agent it has, or to start a Code
  agent with `gamebuilder code-agent`.

When Coder's working folder is an openagents checkout, it can answer "where
is this in the engine" with the compiler's own lookups (`coax vocab`,
`coax query`, and Microcoder's `intel` option). That is an openagents
change, not a GameBuilder one. It does not replace the capability list: a
person with no checkout still needs triage to work.

## Build steps

Each is an issue of its own. Steps 1 to 4 are this repository's. Steps 5 to
7 are openagents'. Step 8 is the owner's.

1. Done (issue 21). The capability list: the generator,
   `coax/engine-list.coax`, the `pin.sh` hook, the check, `engine-list` and
   `engine-read`. Finding the nearest chapters to a sentence is triage's
   (step 2).
2. Done (issue 22), as "Triage" says: the row, the decision question, the
   rules on its answer, its checks, `docs/cli.md`. Not built: embeddings,
   rules ahead of the model, MCP and HTTP.
3. Done (issue 23), as "The pull request" says: `engine-ready`,
   `engine-mine`, and the fork in a Code agent's brief and approval words.
4. Done (issue 24). The skill's section starts with triage, carries the
   offer word for word with what "potentially" means today, and names the
   fork steps.
5. openagents: `CONTRIBUTING.md`, the pull request template, the one check
   command.
6. openagents: the review and safety rules for fork pull requests.
7. openagents: `contributed_by` in `chapters.json`, and the pin-bump issue
   opened on merge.
8. Done for launch (issue 25): credit only, as [Rewards](#rewards) says.
   Whether to pay is still the owner's decision.

Steps 1, 2 and 4 make the guidance work for the team now. Steps 5 and 6 are
what must exist before the offer goes to anyone else.
