# The delegation router

This library decides which computer, coding agent, model and effort take a
coding task. The specification is [docs/router/README.md](../../../../docs/router/README.md).

This is the first piece: the types, the rules in code, the question for the
decision model, a stand-in for Google's Decisions API, and a command line to
try decisions with. It starts no agent, and no app calls it yet.

## Try it

Run these from the repository's root. The first run builds the tool.

```sh
R=coax/lib/agent/router

# Which computers can take work, as the decision model reads them
coax/web/tool.sh $R/router.coax stations

# A decision from the stand-in: no network, no key
coax/web/tool.sh $R/router.coax decide "Fix the typo in the README title"

# Say what the stand-in answers, and see what code does with it
coax/web/tool.sh $R/router.coax decide "Refactor the session store" \
  --mock "station=oa-build-2,engine=claude,difficulty=3,weight=1"

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
```

The Google door reads `GEMINI_API_KEY`, or
`~/work/.secrets/openagents-gemini-provider.env` when that is unset. The key
goes to `curl`'s environment only.
It needs `curl` 8.3 or later, as every decision client here does. The
build hosts have 7.88, so a run there that calls Google (or a stand-in with
`--url URL`) needs a newer `curl` first on `PATH`.

The computers come from [`fixtures/stations.json`](fixtures/stations.json).
Copy it, change a computer's disk, logins or rules, and pass your copy with
`--stations FILE`. Its `now_ms` is fixed, so a report's age is the difference
between `now_ms` and `reported_at_ms` in the file, not the clock.

`--mock` takes `station=ID`, `engine=NAME`, `difficulty=0..4`,
`effort=low|medium|high`, `weight=0..2`, `p=0..1000` (the named option's
probability, 900 by default) and `fail=rate|server|safety|invalid|key|not_found|malformed|transport`.
`--seed N` picks whatever the script doesn't name.

## How a decision is made

1. **Code decides what is possible** (`policy.coax`, `rtr_eligible`). A
   computer is ruled out by one fact: its report is over two minutes old, you
   keep it clear, its free disk is below its floor, or it runs as many agents
   as you allow. An agent on a computer is ruled out when it is not
   installed, not logged in, out of quota or resting.
2. **One request asks typed questions** (`ask.coax`): `station` and `engine`
   among the eligible ones only, then `difficulty`, `effort` and `weight`. A
   question with one option is not asked.
3. **A door answers.** Google (`coax/lib/systemone/gemini.coax`), the
   stand-in (`mock.coax`) or a recorded answer.
4. **Code picks what runs** (`policy.coax`, `rtr_pick`). An answer below 0.5,
   or one that names something code ruled out, gives way to the default: the
   most capable eligible agent on the computer with the most room. A computer
   marked "nothing heavy" takes no heavy work. With no answer, the default
   runs and the work is taken to be heavy.

The decision lists the door's answers, what was ruled out and each reason
what runs differs from the answer.

## Files

| File | What |
| --- | --- |
| `wire.coax` | The one codec of `openagents.router.v1alpha1` ([schema](../../../../proto/openagents/router/v1alpha1/router.proto)): reads a `RouteRequest`, writes a `RouteDecision`, through the generated JSON mapper. |
| `policy.coax` | Eligibility and the pick. Pure. |
| `ask.coax` | The request for the decision model, and reading its answer. |
| `mock.coax` | The stand-in for Google's Decisions API, in process. It answers by script and seed, and refuses what Google refuses about a request's shape. |
| `decide.coax` | One decision end to end: `rtr_begin`, a door, `rtr_finish`. |
| `router.coax` | The command line. |
| `wire-goldens.coax` | The codec against `proto/coder/router-v1alpha1.json`, and 2,048 fuzzed decisions written and read back. |
| `replay-check.coax` | Three exchanges recorded with Google on 2026-11-14 (`fixtures/google/`): the request bytes, the answer check and what runs. |

## Checks

Run them on a build host, not the Mac:

```sh
coax/remote/remote.sh run -- bash -c 'for f in policy mock decide; do coax/bin/coax test coax/lib/agent/router/$f.coax | tail -1; done'
coax/remote/remote.sh run -- coax/web/tool.sh coax/lib/agent/router/wire-goldens.coax
coax/remote/remote.sh run -- coax/web/tool.sh coax/lib/agent/router/replay-check.coax
```

`decide.coax` loads the other files, so its run covers their checks too.
Three of the checks are exhaustive: 4,096 mixes of rules and answers through
the pick, every probability and option count through the stand-in's
distribution, and 2,048 seeded answers through a whole decision. Each holds
that what runs is something code allows.

Generated proto code can't run inside a compile-time `#test`, so the codec's
checks are in `wire-goldens.coax`.

## What Google's answers showed

From the three recorded exchanges (about 740 input tokens and 0.3 to 0.5
seconds each):

- **The computer and the agent are answered apart.** For the refactor,
  Google chose Claude Code, and a computer where Claude Code is not logged
  in. Code moved the work. The request should ask about them together, or
  list only the computers that run the chosen agent.
- **The router must ask what a task needs.** Google sent an iOS build to a
  Linux computer. The `needs` question and its rule in code are the next
  step.
- **A probability under 0.5 is rare with two or three options.** The
  threshold seldom applies, so it needs measuring before it is trusted.

## Not built yet

In the specification's order: the `needs`, `named_station`, `named_engine`,
`named_depth` and `continue` questions; another account when one runs out;
`#authority` and its mutation pass on `policy.coax`; station reports from
real computers (`DescribeStation`); the daily allowance; `coder.route.v3`;
starting the agent.
