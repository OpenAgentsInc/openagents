# Issue Sense

Our hosted Clef judges issue work at the points where a typed answer is
cheaper than a Claude session or a guess
([#11484](https://github.com/OpenAgentsInc/openagents/issues/11484)). It
answers from the H100 and A100 pool behind `api.openagents.com/v1/decisions`
in about 0.2 s, with no key and no charge
([`docs/inference/clef-decisions-h100.md`](../../docs/inference/clef-decisions-h100.md)).

The solver asks these questions itself on every run
([`docs/coder/solver.md`](../../docs/coder/solver.md#issue-sense-clef-at-every-turning-point)).
This package adds the part that runs on its own: a daily sweep.

## The daily sweep

At 06:00, when no Coder task is running, the rule starts one Coder run in
this checkout. That run runs:

```sh
cargo run -q -p coder-new -- solver watch                        # did a new commit do an open issue's work?
cargo run -q -p coder-new -- solver queue --decisions gateway    # is each open issue landed, current, actionable?
```

and reports the issues judged likely landed (p ≥ 0.8) with their commit,
and those judged stale or blocked, each with its link. It changes nothing on
GitHub.

## Turn it on

```sh
openagents plugin install ./plugins/issue-sense   # installed, off
openagents plugin enable issue-sense              # on for this computer
openagents background list                        # its rule, with the others
```

The rule asks for `needs.coder` and nothing else: no deletions, no
notifications, no Git updates.
