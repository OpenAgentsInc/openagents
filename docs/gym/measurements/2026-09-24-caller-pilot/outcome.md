# Caller pilot: outcome

The verdict the frozen [`plan.md`](plan.md) was written to produce. The
numbers live in [`report.md`](report.md), the gym-rendered record; this
file says what they mean.

## What ran

`gym build` turned `records.jsonl` — 40 synthesized returns-desk records,
label source `acme`, licence "measurement use only" — into the suite
`caller-acme-returns-v1` and the question set of the same name. `gym
eval` asked two doors across the calibration and development partitions:
`kev-0.6b`, the local Kev adapter under `kev-serve`, and `constant`, the
deterministic first-option baseline in `constant_door.py`. `gym report`
rendered the record under the declared selection and wrote
`commitment.json`. `gym verify` walked the receipt chain and matched the
store to the commitment. The locked partition was never read.

## Against the declared criteria

1. `kev-0.6b` beat the constant baseline on development accuracy —
   0.69 against 0.25 — so the model reference is load-bearing on this
   workload, not decoration.
2. Per family, kev wins in both: 0.81 over 0.31 on `consumer-returns`,
   0.75 over 0.31 on `pro-returns`. `pro-returns` sits furthest under
   its 0.88 agreement ceiling and names where headroom sits.
3. Coverage is complete: 32 expected items per door, 32 recorded, none
   missing, none refused.
4. Calibration is reported, not judged. At 0.135 ECE on 16 development
   items the number is a limit statement, not a calibration claim — the
   record says so by construction.
5. Both lanes are local and unmetered; the record states that rather
   than quoting a cost.

Tamper evidence: flipping one row's `correct` flag in a copy of the
store makes `gym verify` exit 2 and name the edited row. The chain is a
check, not a courtesy.

## The outcome

**Useful measured-record offering, on the flow's mechanics.** The intake
→ suite → evaluation → receipt-chained store → report → commitment →
verification path ran end to end on workload-shaped data, and every step
is independently checkable by the caller. What this pilot cannot show,
by construction: a real caller's demand, a real label policy, or a
purchasing decision. The caller is fictional and the record says so.

## The smallest next serving requirement

Nothing in the serving path blocked the pilot. The friction was
operational: `gym eval`'s default ten-second call timeout is short for
an unmetered CPU door (kev's median call here was about 31 seconds), so
the run needed `--timeout 300`. The next requirement is caller-facing,
not code: a willing caller with real labelled data and a stated
permission, repeating exactly this flow.
