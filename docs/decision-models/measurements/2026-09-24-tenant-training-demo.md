# The tenant-training rehearsal, 2026-09-24

`training/tenant-demo/run.sh` against a scratch registry, on
`tenant-train` built from this tree. Every number below is reproduced
by re-running the script — the artifacts are deterministic, so the
digests are identical run to run.

## What ran

| Step | Result |
| --- | --- |
| `check` on `corpus-eligible.json` | Registered: 8 training, 4 calibration, 6 development, 4 locked items; digest `sha256:6380eb95…c23cc71b6` |
| `check` on `corpus-leaked.json` | Refused: exact leakage across training and development, items `t-001`/`d-001` |
| `check` on `corpus-undocumented.json` | Refused: item `t-001` has no license |
| `headroom` on `baseline-scores.json` | 6 scored, 4 failed — `model_addressable` 3, `ambiguous_question` 1; verdict `train`; report `sha256:f3a29420…c43046601` |
| `freeze` on `recipe.json` | `sha256:9d329f44…0aabf805` |
| `make_artifacts.py --seed 11` | adapter `sha256:a8240ecd…02fa2a8`, head `sha256:40d25c02…653a6af0`, tokenizer `sha256:eace4fa1…8ada908` |
| `trial` on the kept run | Ledger position 1 |
| `trial` on seed 99 | Refused: outside the recipe's seed policy |
| `seal` on `candidate.json` | `atlas-ledger-v1`, signature `sha256:3bde6e18…a1d95a` |
| `delete` on the corpus | Tombstoned; 22 item digests retained, corpus digest still resolves |

## What it proves

The contract holds end to end: only the training partition is readable
for fitting, provenance and permission are registration requirements
rather than paperwork, a leaked pair is refused before any number is
computed on it, the baseline's failures are caused before a recipe
freezes, a trial outside the seed policy never reaches the ledger, the
seal binds exactly the artifacts the kept trial recorded, and deletion
removes content while keeping identity.

## What it does not prove

A model improved. `make_artifacts.py` emits deterministic placeholder
tensors honestly labeled as such; the metrics in the demo's trial record
are declared, not measured. Real adapter training runs through
`training/lev-adapter` or the kev pipeline, a sealed candidate wins
nothing until `tenancy::admission` replays locked evidence against it,
and the demo's locked partition was never read — that is the point.
