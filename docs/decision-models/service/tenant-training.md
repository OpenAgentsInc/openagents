# Tenant training

`tenancy::training` turns authorized caller data into a sealed candidate:
a LoRA adapter plus a pointer head with reproducible artifact identity.
Candidate production authorizes nothing — a sealed candidate is
inspectable through `tenant-train inspect` and servable only after
[`tenancy::admission`](gateway.md) replays locked-partition evidence and
the registry activates it.

The store lives in `training/` beside the registry. There are no HTTP
routes: corpus registration, recipe freezes, and candidate seals are
operator acts through the `tenant-train` binary.

## The corpus

A corpus is `openagents.tenant_training.corpus.v1` — a labelled dataset
in four partitions. A Gym suite's three exist to measure; this adds
`training`, the only partition a trainer reads:

| Partition | Role |
| --- | --- |
| `training` | What the trainer fits on. |
| `calibration` | Fits maps and thresholds. |
| `development` | Chooses between trials; read freely, never trained on. |
| `locked` | Spent once by the admission flow; no path here reads it. |

Every item carries `provenance` — source, license, and the permission
that authorizes training on it — plus a per-item `label_rule` inside the
corpus digest. `check` refuses a corpus with no training partition, an
undocumented item, a `group` that spans partitions (a session's
near-duplicates take one role), an unconfirmed `model`-sourced label —
hosted output is a draft, not ground truth — or an exact or
near-duplicate across a partition boundary, where near means a
normalized token-set overlap of 0.8 or more.

`tenant-train check --registry DIR --corpus FILE` validates and
registers; re-registering the identical digest is a no-op and a
different document under the same name is refused.

## Headroom

Before a recipe may freeze, the baseline's failures on the development
partition are caused, not just counted. `tenant-train headroom --corpus
NAME --scores FILE` reads an `openagents.tenant_training.scores.v1`
file — one row per development item — and buckets every failure:

- `state_shape` — missing or oversized state; a pipeline bug.
- `ambiguous_question` — more than one defensible answer.
- `label_uncertainty` — the label itself is uncertain.
- `exact_logic` — derivable by a rule; the fix belongs in Rust.
- `model_addressable` — everything else: the headroom.

A corpus needs at least three model-addressable failures for a `train`
verdict. Fewer produces a sealed `no_train` report — a documented
decision not to train is a result, not a gap. Scores that skip a
development item are refused; a subset is not the baseline.

## The recipe

`tenant-train freeze --recipe FILE` seals
`openagents.tenant_training.recipe.v1`: the base-model identity, the
adapter and pointer-head shape, the seed policy, the trial cap, the
compute budget, the winning metric and its margin over the baseline,
rejection rules, and transfer controls. The digest binds every field;
an edit after freezing is a new recipe and a re-verification is a
tamper refusal.

## The trials ledger

`tenant-train trial --record FILE` appends one
`openagents.tenant_training.trial.v1` to `trials.jsonl` — kept,
rejected, and failed runs alike, each pinned to the recipe's digest. A
record is refused when its seed is outside the recipe's policy, when
the recipe is at its trial cap, or when a kept trial carries no
artifacts.

## The candidate

`tenant-train seal --candidate FILE` binds a winning trial into
`openagents.tenant_training.candidate.v1`: code, recipe digest, corpus
digest, base-model identity, and the adapter, head, and tokenizer
digests — which must be the artifacts that trial recorded, not just
digests that parse. The seal's signature is the `artifact_signature` an
admission record binds; `inspect` renders the card. Production routing
is unchanged: the registry's admission path is still the only way a
binding moves.

## Retention

The corpus declares `retention` — how long raw content may live, who
may read it, and what survives it. `tenant-train delete --corpus NAME
--reason TEXT` tombstones: item content leaves the store, and per-item
content digests stay, so a candidate's `corpus_digest` still resolves
to what it was trained on without retaining the text. Tenant data and
secrets never enter public artifacts — the candidate binds digests.

## The demonstration

[`training/tenant-demo/`](../../../training/tenant-demo/README.md)
rehearses the whole contract on a synthetic corpus: an eligible corpus
registers, a leaked and an undocumented corpus are refused, the
baseline's failures are caused, a frozen recipe's digest pins every
trial, a foreign seed is refused, and the sealed candidate inspects —
with `make_artifacts.py` standing in for a real trainer so the flow is
reproducible without paid compute.
