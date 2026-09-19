# Training a Lev adapter

An adapter is the only way to teach Apple's on-device model a task rather
than describe one. It is also the only route to the two things Lev cannot
reach by engineering: the accuracy gap against hosted Jev, and a certainty
band that carries any signal at all.

This directory holds everything except the training itself, which belongs to
Apple's toolkit.

## Status

| Step | State |
| --- | --- |
| Suite to Apple's JSON Lines format | works, `convert.py` |
| Python and Rust renderers agree | works, `check_parity.py`, 52 of 52 items |
| Toolkit located and version-checked | works, `toolkit.py` |
| Device base signature read | works, `9799725` on this machine |
| Package format written and read | works, `crates/lev/src/adapter.rs`, 9 tests |
| Runtime loads a package we wrote | **works** — verified against live hardware |
| Serving through an adapter | works, `lev-serve --adapter` |
| Training | toolkit 26.0.0 installed; `BASE_SIGNATURE` matches the device |

The toolkit's own `export/constants.py` carries
`BASE_SIGNATURE = "9799725ff8e851184037110b422d891ad3b92ec1"`, which is
exactly what this device reports through
`compatibleAdapterIdentifiers(name:)`. The pairing is confirmed on both
sides rather than assumed.

Its `assets/base-model.pt` is 12.7 GB — Apple's real 3B base weights, the
thing no other route provides. That is why the toolkit is the only way to
train an adapter the runtime finds *useful* rather than merely loadable.

## Why the toolkit is not in this repository

Apple distributes the adapter training toolkit to Developer Program members
who accept its terms. Those terms do not permit redistribution, and this
repository is open source, so vendoring it would be a license violation
rather than a convenience. The operator fetches it into `toolkit/`, which is
ignored by git.

`toolkit.py` searches `training/lev-adapter/toolkit`, then
`~/code/adapter_training_toolkit_v26_0_0`, then `~/Downloads`, and honors
`LEV_TOOLKIT_ROOT`.

Take version **26.0.0**. The two betas are withdrawn, and 26.0.0 is the last
release for OS 26 — Apple states it is not compatible with OS 27 and later,
which is the base-signature treadmill in concrete form.

Its own requirements: Apple silicon with at least 32 GB, or a Linux GPU
machine, and Python 3.11 or later. It brings its own pinned dependencies:

```sh
cd training/lev-adapter/toolkit
python3.11 -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt
```

Deploying an adapter inside a shipped app additionally needs the Foundation
Models Framework Adapter Entitlement, which the Account Holder requests.
Local training and local serving through `Adapter(fileURL:)` do not, which is
everything this lane does today.

## The base model signature, and why it governs everything

An adapter is pinned to one base model. The base ships with the operating
system. Get the pairing wrong and the device refuses the package.

The signature is readable from the device, which is the only way to learn it
from outside the framework:

```sh
./scripts/build-lev-bridge.sh
python3 training/lev-adapter/toolkit.py
```

```text
device base signature prefix: 9799725
an adapter for this device is identified fmadapter-<name>-9799725
```

That prefix comes from `SystemLanguageModel.Adapter.compatibleAdapterIdentifiers(name:)`.
Download the toolkit version whose base assets match it. On this machine the
full signature is `9799725ff8e851184037110b422d891ad3b92ec1`, unchanged since
the format was first documented in March 2026 — but an operating system
update can change it, and when it does, every adapter and every calibration
map fitted against the old base is invalid. `lev-serve` refuses to start on a
mismatch rather than serving a bad pairing.

## The full process

```sh
# 0. Build the bridge helper and confirm the device.
./scripts/build-lev-bridge.sh
python3 training/lev-adapter/toolkit.py

# 1. Convert the suite. Training reads the calibration split; the evaluation
#    split becomes validation and is never trained on, which is what makes
#    the improvement number afterwards mean anything.
#
#    --permutations shuffles Choice option order so an item appears under
#    several orders. The first adapter gave back four of its thirteen points
#    when options were reversed, because every record had presented them one
#    way. Only the training split is augmented.
#
#    --band trains the certainty field, labelled from the base model's own
#    measured outcomes. Produce those first with `lev-eval --dump`.
cd training/lev-adapter
python3 convert.py --out data/ --permutations 3 \
    --band --base-rates runs/base-rates.json

# 2. Prove the renderers agree. Training data rendered differently from
#    serving text is the worst kind of bug: everything runs and the numbers
#    are quietly worse.
CARGO="cargo +1.97.1" python3 check_parity.py

# 3. Train.
python3 train.py --data data/ --out runs/lev-v1

# 4. Export and check. Our reader names which rule failed; the device decides.
python3 export.py --run runs/lev-v1 --out runs/lev-v1/lev.fmadapter

# 5. Serve through it and score it against the base on the same suite.
cargo run -p lev --features serve --bin lev-serve -- --port 11437 \
    --adapter training/lev-adapter/runs/lev-v1/lev.fmadapter
cargo run -p lev --features serve --bin lev-eval -- \
    --door lev-base=http://127.0.0.1:11436 \
    --door lev-adapted=http://127.0.0.1:11437 --fit
```

## What to train

Two objectives. The second is the one worth having.

**Pick the right option.** Ordinary supervised fine-tuning on
`(state, question) -> correct option`. `convert.py` emits this by default.
It moves accuracy, which is 0.85 against hosted Jev's 0.96.

**Report a truthful certainty band.** `convert.py --band` adds an ordered
`certainty` field to the response format and labels it from *outcomes*
rather than from an opinion: pass `--base-rates` a map of item id to the base
model's measured accuracy, and an item the base model reliably gets right is
labelled `almost certain` while one it gets wrong is labelled `even odds` or
`unlikely`.

This matters more than the accuracy point. On the base model the band is
constant — every item in the behavior record came back `likely`, including
the wrong ones — so L3 has nothing to calibrate and the door has no honest
probability to serve. A model that says `even odds` before it is wrong is
worth more to a workflow than two points of accuracy.

## What counts as success

Not "it trained." The same gate the calibration map has to pass: beat the
base on items it was not trained on.

1. **Accuracy** on the evaluation split, base against adapted, same suite and
   same estimator. Base is 0.85.
2. **The band varies**, and its values separate correct from incorrect better
   than chance. Base does not vary at all.
3. **A calibration map fitted on the adapted model passes `calibrate::admit`**
   — the gate that requires a map to beat the raw signal on held-out items.
   No base-model map has ever passed it.
4. **Order sensitivity re-measured.** Base flips one greedy answer in eight;
   fine-tuning often reduces this.

Record all four whether they improve or not, the way kev's previews ship with
their release screen marked failed.

## The recipe

Defaults in `train.py`, and they are guesses until the first run says
otherwise:

| Setting | Value | Why |
| --- | --- | --- |
| LoRA rank | 32 | the toolkit's own default; no evidence yet to move it |
| Epochs | 4 | 98 training records is very little; more epochs on less data overfits |
| Learning rate | 1e-4 | kev's research log found that too high a rate erodes the base knowledge the task depends on, which is the single largest quality effect it recorded |
| Seed | 0 | recorded so a run reproduces |

**The corpus is the weak point, not the recipe.** 98 training records is
still below what a fine-tune wants. Growing `support-v1` to 150–200 items per
family is the same work that unblocks calibration, and it should happen
before anyone tunes a hyperparameter.
