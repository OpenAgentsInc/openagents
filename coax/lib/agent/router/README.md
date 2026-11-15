# The delegation router

This library decides which computer, coding agent, model and effort take a
coding task. The specification is [docs/router/README.md](../../../../docs/router/README.md).

So far: the types, the rules in code, the questions for the decision model,
a stand-in for Google's Decisions API, two labelled sets scored against
answers Google gave, and a command line to try decisions with. It starts no
agent. The iPhone app's account chat calls it through the API
(`coax/web/api/router.c`, `apps/openagents-ios/home/route.coax`) and shows
what would run.

## Try it

Run these from the repository's root. The first run builds the tool.

```sh
R=coax/lib/agent/router

# Which computers can take work, as the decision model reads them
coax/web/tool.sh $R/router.coax stations

# This computer, read now: its disk, memory, load, agents, logins and running agents
coax/web/tool.sh $R/router.coax stations --here
coax/web/tool.sh $R/router.coax decide "Fix the typo in the README title" --here --door google
coax/web/tool.sh $R/router.coax report --nothing-heavy > /tmp/this-computer.json

# A decision from the stand-in: no network, no key
coax/web/tool.sh $R/router.coax decide "Fix the typo in the README title"

# Say what the stand-in answers, and see what code does with it
coax/web/tool.sh $R/router.coax decide "Refactor the session store" \
  --mock "engine=claude,station_claude=coderos,difficulty=3,weight=1"
coax/web/tool.sh $R/router.coax decide "Use Codex on coderos: run the tests" \
  --mock "named_engine=codex,named_station=coderos"
coax/web/tool.sh $R/router.coax decide "Build the iOS app" --mock "needs=apple,weight=2"

# A follow-up in a conversation that is on the Mac with Claude Code
coax/web/tool.sh $R/router.coax decide "Now do the same for the help text" \
  --on mac --with claude --summary "We renamed Settings to Preferences." \
  --mock "continue=950,engine=codex"

# A decision from Google (Gemini Decision 1), about $0.0001
coax/web/tool.sh $R/router.coax decide "Refactor the session store" --door google

# The same, keeping Google's answer to replay later
coax/web/tool.sh $R/router.coax decide "Refactor the session store" \
  --door google --save /tmp/answer.json --show
coax/web/tool.sh $R/router.coax decide "Refactor the session store" \
  --door replay --answer /tmp/answer.json

# What happens when Google doesn't answer
coax/web/tool.sh $R/router.coax decide "Refactor the session store" --mock fail=rate

# 500 seeded stand-in answers: how often each computer and agent ran
coax/web/tool.sh $R/router.coax sweep "Refactor the session store" --seeds 500

# The decision as JSON
coax/web/tool.sh $R/router.coax decide "Fix the typo" --json

# The labelled set against the answers Google gave on 2026-11-14
coax/web/tool.sh $R/router.coax eval --door replay --answers $R/fixtures/google

# The labelled set against Google now (33 tasks, under one cent)
coax/web/tool.sh $R/router.coax eval --door google
```

The Google door reads `GEMINI_API_KEY`, or
`~/work/.secrets/openagents-gemini-provider.env` when that is unset. The key
goes to `curl`'s environment only.
It needs `curl` 8.3 or later, as every decision client here does. The
build hosts have 7.88, so a run there that calls Google (or a stand-in with
`--url URL`) needs a newer `curl` first on `PATH`.

`--here` reads this computer in place of a file, and `report` prints what it
read as a `RouteRequest` you can edit and pass to `--stations`. Your rules for
the computer are flags: `--keep-clear`, `--nothing-heavy`, `--max-agents N`.
The collector was run on a Linux build host. Its macOS path (`sysctl`,
`vm_stat`) has checks against output captured from this Mac, but the tool
itself has not been run on a Mac yet.

Otherwise the computers come from [`fixtures/stations.json`](fixtures/stations.json).
Copy it, change a computer's disk, logins or rules, and pass your copy with
`--stations FILE`. Its `now_ms` is fixed, so a report's age is the difference
between `now_ms` and `reported_at_ms` in the file, not the clock.

`--mock` takes a value for any question by its name: `engine=NAME`,
`station_claude=ID` or `station_codex=ID`, `difficulty=0..4`,
`effort=low|medium|high`, `weight=0..2`, `needs=none|apple|gpu|this_computer`,
`named_engine=NAME`, `named_station=ID`, `named_depth=faster|more_careful`
and `continue=0..1000`. It also takes `p=0..1000` (the named option's
probability, 900 by default) and
`fail=rate|server|safety|invalid|key|not_found|malformed|transport`.
`--seed N` picks whatever the script doesn't name. Seed 0, the default, gives
the plain answer: nothing named, nothing needed.

## How a decision is made

1. **Code decides what is possible** (`policy.coax`, `rtr_eligible`). A
   computer is ruled out by one fact: its report is over two minutes old, you
   keep it clear, its free disk is below its floor, or it runs as many agents
   as you allow. An agent on a computer is ruled out when it is not
   installed, not logged in, out of quota or resting.
2. **One request asks typed questions** (`ask.coax`), about 1,500 input
   tokens with six computers:

   | Question | Answers |
   | --- | --- |
   | `engine` | The agents that can run somewhere |
   | `station_claude`, `station_codex`, ... | For each agent, the computer if that agent takes the task. Only the computers that run it |
   | `difficulty`, `effort`, `weight` | Trivial to very hard; low to high; light, medium, heavy |
   | `needs` | Nothing, a Mac that builds for Apple platforms, a GPU, or the conversation's own computer |
   | `named_engine`, `named_station`, `named_depth` | What the person asked for in so many words, or none |
   | `continue` | Whether the task continues the conversation's work. Asked only in a conversation |

   A question with one option is not asked.
3. **A door answers.** Google (`coax/lib/systemone/gemini.coax`), the
   stand-in (`mock.coax`) or a recorded answer.
4. **Code picks what runs** (`policy.coax`, `rtr_pick`).
   - The agent, in this order: the one you named, the conversation's while
     the task continues its work, the model's answer when it is at least 0.5
     sure, else the most capable eligible agent.
   - The computer, in the same order: the one you named, the conversation's,
     the model's answer for the agent that runs, else the one with the most
     room.
   - Every candidate must be eligible, run that agent, take work of the
     task's weight and have what the task needs. One that can't is passed
     over, and the decision says why.
   - When no computer can run the agent for this task, the most capable
     agent that some computer can run takes it.
   - With no answer, a conversation stays where it is. Otherwise the default
     runs, and the work is taken to be heavy.

The decision lists the door's answers, what was ruled out and each reason
what runs differs from the answer.

## Files

| File | What |
| --- | --- |
| `wire.coax` | The one codec of `openagents.router.v1alpha1` ([schema](../../../../proto/openagents/router/v1alpha1/router.proto)): reads and writes a `RouteRequest`, writes a `RouteDecision`, through the generated JSON mapper. |
| `policy.coax` | Eligibility and the pick. Pure. |
| `station.coax` | This computer's report: what each fact is read from, pure parsers, and the collector. It reads no token, no Keychain item and no process arguments. A computer's resident service answers `DescribeStation` with it (`coder/tui/host-station.coax`); `wire.coax` writes and reads the `StationReport`. |
| `ask.coax` | The request for the decision model, and reading its answer. |
| `mock.coax` | The stand-in for Google's Decisions API, in process. It answers by script and seed, and refuses what Google refuses about a request's shape. |
| `decide.coax` | One decision end to end: `rtr_begin`, a door, `rtr_finish`. |
| `words.coax` | What a decision says to a person: the one line, each reason and why nothing runs, and the note the chat's model gets. Pure; the phone loads it. |
| `router.coax` | The command line. |
| `wire-goldens.coax` | The codec against `proto/coder/router-v1alpha1.json`, and 2,048 fuzzed decisions written and read back. |
| `eval.coax` | The labelled sets: reading `fixtures/labels.tsv`, scoring a decision want by want, and deciding a task from a recorded exchange. |
| `replay-check.coax` | Both labelled sets against the exchanges recorded with Google on 2026-11-14 (`fixtures/google/`, `fixtures/google-held/`): the request bytes, the answer check and the score. |

## Checks

Run them on a build host, not the Mac:

```sh
coax/remote/remote.sh run -- bash -c 'for f in eval station; do coax/bin/coax test coax/lib/agent/router/$f.coax | tail -1; done'
coax/remote/remote.sh run -- coax/web/tool.sh coax/lib/agent/router/wire-goldens.coax
coax/remote/remote.sh run -- coax/web/tool.sh coax/lib/agent/router/replay-check.coax
```

`eval.coax` loads the other files, so its run covers their checks too (203
checks). Three are exhaustive or fuzzed: 30,000 mixes of rules, logins,
conversations and answers through the pick, every probability and option
count through the stand-in's distribution, and 2,048 seeded answers through a
whole decision. Each holds that what runs is something code allows, and the
first also holds that what you named runs whenever it can.

Generated proto code can't run inside a compile-time `#test`, so the codec's
checks are in `wire-goldens.coax`.

## The labelled sets

`fixtures/labels.tsv` has 33 tasks and `fixtures/labels-held.tsv` has 20.
Each row says what the decision model should answer and, for some, what
should run. The computers are `fixtures/stations.json`.

| Set | Tasks | Wants right with Gemini Decision 1, 2026-11-14 |
| --- | --- | --- |
| Tuned: the wording of the questions was changed twice against it | 33 | 66 of 66 |
| Held out: written afterwards, scored once | 20 | 30 of 31 |

The held-out miss: "Fix the crash that happens when a laptop wakes from
sleep" was answered as needing a Mac.

What the tuning changed:

- **A task that mentions an agent is not a request for it.** "Fix the bug
  where Codex sessions show the wrong model name" was first read as asking
  for Codex. The question now asks whether the person tells you which agent
  must do the task.
- **The computer is asked once for each agent.** One question for the
  computer and one for the agent got answers that didn't go together: Claude
  Code, and a computer where it was logged out.
- **A named computer can sit anywhere in the sentence.** After the first
  fix, "Run the unit tests on coderos" stopped counting. The question now
  gives that form as an example.

These sets measure the questions against one door on one set of computers.
They don't measure which agent writes the better patch, and 53 tasks don't
establish calibration.

## Not built yet

In the specification's order: another account when one runs out; Fable for
very hard tasks where an account offers it; `#authority` and its mutation
pass on `policy.coax`; in a station report, how much of each account's quota
is used, whether someone is using the computer, and agent versions; the daily allowance;
`coder.route.v3`; starting the agent.
