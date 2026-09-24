"""The early-stopping rule for targeted experiments.

The finished trials of an experiment often settle its question long before
the last planned attempt. After every graded trial, ``tbench experiment
run`` applies this rule and stops what needs no more trials. The rule is
code only; no model call decides a stop.

Every candidate arm is compared with the baseline, the first arm that
isn't a control (``nop`` or ``oracle``). A control arm never enters a
comparison and never stops by this rule. A candidate stops when one of
these holds, checked in this order:

1. **Dominated**: even if every open attempt of the arm passes, it ends
   with fewer passes than another arm has now, and its mean cost per
   graded attempt isn't lower than that arm's.
2. **Decided**: the exact McNemar test against the baseline, on attempts
   paired by task and attempt number, is below the significance level and
   stays below it with the same winner even if every open pair goes the
   other way.
3. **Undecidable**: even if every open pair went one way, the exact
   McNemar test couldn't get below the significance level. A design too
   small to get below it even if every planned pair went one way, such
   as one task with three attempts, is exploratory: this test never
   stops it.
4. **Below the acceptance bar**: even if every open attempt passes, the
   arm's pass rate stays under the bar the experiment set.

The experiment ends when every candidate has stopped. A stopped arm stays
stopped, and its open attempts never run. The baseline stops only when
the experiment ends.

This is the same rule as ``crates/gym/src/terminal_bench_stop.rs``, which
``gym experiment pulse`` and ``gym experiment replay`` use. Both are
tested against ``tests/fixtures/stop-rule/cases.json``.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Any

CONTROLS = frozenset({"nop", "oracle"})
DEFAULT_ALPHA = 0.05

PASS, FAIL, OPEN, DEAD = "pass", "fail", "open", "dead"
STOPPED = ("dominated", "decided", "undecidable", "below_bar")


def binomial_two_sided(k: int, n: int) -> float:
    """The two-sided exact binomial test of ``k`` successes in ``n`` at 1/2."""
    if n == 0:
        return 1.0
    low = min(k, n - k)
    return min(1.0, 2 * sum(math.comb(n, i) for i in range(low + 1)) / 2**n)


def cell_of(state: str, reward: float | None) -> str:
    """A trial's cell from its state and its verifier reward."""
    if state == "finished" and reward is not None:
        return PASS if reward >= 1.0 else FAIL
    if state in ("pending", "running"):
        return OPEN
    return DEAD


@dataclass
class Input:
    arms: list[str]
    tasks: list[str]
    attempts: int
    # (arm, task, attempt) -> cell; a missing one is open.
    cells: dict[tuple[str, str, int], str] = field(default_factory=dict)
    # arm -> mean cost per graded attempt, when known.
    mean_cost: dict[str, float] = field(default_factory=dict)
    # arm -> (state, reason) an earlier evaluation stopped it with.
    stopped: dict[str, tuple[str, str]] = field(default_factory=dict)

    def cell(self, arm: str, task: str, attempt: int) -> str:
        cell = self.cells.get((arm, task, attempt), OPEN)
        if cell == OPEN and arm in self.stopped:
            return DEAD
        return cell

    def slots(self):
        for task in self.tasks:
            for attempt in range(1, self.attempts + 1):
                yield task, attempt

    def counts(self, arm: str) -> tuple[int, int, int]:
        passes = graded = open_ = 0
        for task, attempt in self.slots():
            cell = self.cell(arm, task, attempt)
            if cell == PASS:
                passes += 1
                graded += 1
            elif cell == FAIL:
                graded += 1
            elif cell == OPEN:
                open_ += 1
        return passes, graded, open_

    @classmethod
    def from_json(cls, value: dict[str, Any]) -> tuple[Input, float, float | None]:
        data = cls(
            arms=list(value.get("arms") or []),
            tasks=list(value.get("tasks") or []),
            attempts=int(value.get("attempts") or 0),
        )
        for row in value.get("cells") or []:
            data.cells[(row["arm"], row["task"], int(row["attempt"]))] = row["cell"]
        data.mean_cost = {k: float(v) for k, v in (value.get("mean_cost") or {}).items()}
        data.stopped = {
            arm: (state, "stopped earlier") for arm, state in (value.get("stopped") or {}).items()
        }
        return data, float(value.get("alpha", DEFAULT_ALPHA)), value.get("accept_pass_rate")


def _discordant(data: Input, arm: str, baseline: str, fill: tuple[bool, bool] | None):
    arm_only = baseline_only = pairs = 0
    for task, attempt in data.slots():
        a = data.cell(arm, task, attempt)
        b = data.cell(baseline, task, attempt)
        if DEAD in (a, b):
            continue

        def resolve(cell: str, open_value: bool | None) -> bool | None:
            if cell == PASS:
                return True
            if cell == FAIL:
                return False
            return open_value

        ra = resolve(a, fill[0] if fill else None)
        rb = resolve(b, fill[1] if fill else None)
        if ra is None or rb is None:
            continue
        pairs += 1
        if ra and not rb:
            arm_only += 1
        elif rb and not ra:
            baseline_only += 1
    return arm_only, baseline_only, pairs


def compare(data: Input, arm: str, baseline: str, alpha: float) -> dict[str, Any]:
    """A candidate against the baseline, paired by task and attempt."""
    arm_only, baseline_only, pairs = _discordant(data, arm, baseline, None)
    a1, b1, _ = _discordant(data, arm, baseline, (True, False))
    a2, b2, _ = _discordant(data, arm, baseline, (False, True))
    p_arm_best = binomial_two_sided(b1, a1 + b1) if a1 > b1 else 1.0
    p_baseline_best = binomial_two_sided(a2, a2 + b2) if b2 > a2 else 1.0
    arm_locked = a2 > b2 and binomial_two_sided(b2, a2 + b2) < alpha
    baseline_locked = b1 > a1 and binomial_two_sided(a1, a1 + b1) < alpha
    slots = len(data.tasks) * data.attempts
    if binomial_two_sided(0, slots) >= alpha:
        # Even every planned pair going one way couldn't separate the arms:
        # an exploratory design, which this test never stops.
        state, winner = "exploratory", None
    elif arm_locked:
        state, winner = "separated", arm
    elif baseline_locked:
        state, winner = "separated", baseline
    elif p_arm_best >= alpha and p_baseline_best >= alpha:
        state, winner = "cannot separate", None
    else:
        state, winner = "open", None
    return {
        "arm": arm,
        "baseline": baseline,
        "pairs": pairs,
        "arm_only": arm_only,
        "baseline_only": baseline_only,
        "mcnemar_exact_p": binomial_two_sided(arm_only, arm_only + baseline_only),
        "p_if_open_pairs_go_to_arm": p_arm_best,
        "p_if_open_pairs_go_to_baseline": p_baseline_best,
        "state": state,
        "winner": winner,
    }


def _money(value: float | None) -> str:
    return "unknown" if value is None else f"${value:.2f}"


def evaluate(
    data: Input, *, alpha: float = DEFAULT_ALPHA, accept_pass_rate: float | None = None
) -> dict[str, Any]:
    """Apply the rule. Returns the verdict the scheduler records."""
    real = [arm for arm in data.arms if arm not in CONTROLS]
    baseline = real[0] if len(real) >= 2 else None
    candidates = [arm for arm in real if arm != baseline]
    counts = {arm: data.counts(arm) for arm in data.arms}
    arms: list[dict[str, Any]] = []
    comparisons: list[dict[str, Any]] = []
    for arm in data.arms:
        passes, graded, open_ = counts[arm]
        mean_cost = data.mean_cost.get(arm)

        def verdict(state: str, reason: str) -> dict[str, Any]:
            return {
                "arm": arm,
                "state": state,
                "reason": reason,
                "passes": passes,
                "graded": graded,
                "open": open_,
                "mean_cost_usd": mean_cost,
            }

        if arm in CONTROLS:
            arms.append(verdict("control", "a control arm: outside the rule"))
            continue
        if arm == baseline:
            arms.append(verdict("baseline", f"{passes} of {graded} graded, {open_} open"))
            continue
        comparison = compare(data, arm, baseline, alpha) if baseline else None
        if comparison:
            comparisons.append(comparison)
        if arm in data.stopped:
            state, reason = data.stopped[arm]
            arms.append(verdict(state, reason))
            continue
        best = passes + open_
        dominator = None
        for other in real:
            if other == arm:
                continue
            theirs = data.mean_cost.get(other)
            not_cheaper = mean_cost is not None and theirs is not None and mean_cost >= theirs
            if best < counts[other][0] and not_cheaper:
                dominator = other
                break
        if dominator:
            state = "dominated"
            reason = (
                f"at most {best} passes even if every open attempt passes, fewer than "
                f"{dominator}'s {counts[dominator][0]}, at {_money(mean_cost)} per graded "
                f"attempt against {_money(data.mean_cost.get(dominator))}"
            )
        elif comparison and comparison["state"] == "separated":
            state = "decided"
            reason = (
                f"{comparison['winner']} wins: only {arm} passed {comparison['arm_only']} "
                f"pairs, only {baseline} passed {comparison['baseline_only']}; exact McNemar "
                f"stays below {alpha} whichever way the open pairs go"
            )
        elif comparison and comparison["state"] == "cannot separate":
            state = "undecidable"
            best_p = min(
                comparison["p_if_open_pairs_go_to_arm"],
                comparison["p_if_open_pairs_go_to_baseline"],
            )
            reason = (
                f"even if every open pair went one way, exact McNemar against {baseline} "
                f"gets no lower than {best_p:.3f}, not below {alpha}"
            )
        elif (
            accept_pass_rate is not None
            and graded + open_ > 0
            and best < accept_pass_rate * (graded + open_)
        ):
            state = "below_bar"
            reason = (
                f"at most {best} of {graded + open_} "
                f"({100 * best / (graded + open_):.0f}%) even if every open attempt passes, "
                f"under the {100 * accept_pass_rate:.0f}% acceptance bar"
            )
        else:
            state = "active"
            reason = f"{passes} of {graded} graded, {open_} open"
        arms.append(verdict(state, reason))
    candidate_states = [a["state"] for a in arms if a["arm"] in candidates]
    ended = bool(candidate_states) and all(s in STOPPED for s in candidate_states)
    verdict_word = None
    if ended:
        verdict_word = next(
            (word for word in ("decided", "dominated", "undecidable", "below_bar")
             if word in candidate_states),
            "ended",
        )
    if not candidates:
        reason = "no candidate arm: every arm is a control"
    elif ended:
        reason = ", ".join(
            f"{a['arm']} {a['state'].replace('_', ' ')}" for a in arms if a["arm"] in candidates
        )
    else:
        reason = ", ".join(a["arm"] for a in arms if a["state"] == "active") + " still open"
    return {
        "alpha": alpha,
        "accept_pass_rate": accept_pass_rate,
        "baseline": baseline,
        "ended": ended,
        "verdict": verdict_word,
        "reason": reason,
        "arms": arms,
        "comparisons": comparisons,
    }


__all__ = [
    "CONTROLS",
    "DEFAULT_ALPHA",
    "STOPPED",
    "Input",
    "binomial_two_sided",
    "cell_of",
    "compare",
    "evaluate",
]
