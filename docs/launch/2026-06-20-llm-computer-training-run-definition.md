# Definition — "LLM-Computer Training Run" (claim term spec)

- Date: 2026-06-20
- Promise: `claims.world_first_public_llm_computer_training_run.v1` (state: **red**)
- Clears blocker: `blocker.product_promises.llm_computer_training_run_definition_missing`
- Scope: this document fixes the **precise meaning** of the term
  "LLM-computer training run" as used in the Episode 238 world-first claim, so
  the claim cannot drift into an overclaim against what the live
  `compute.tassadar_executor_poc.v1` lane actually does.
- This is a definition spec only. It does **not** flip any promise state, it is
  **not** an owner-signed claim upgrade, and it does **not** assert the
  world-first as proven. The claim stays red.

---

## Why this document exists

The world-firsts prior-art review
(`docs/launch/2026-06-18-world-firsts-verification.md`) found the claim
defensible only as a narrowed wording — "first **public**, open-contributor
LLM-computer training run," with Percepta credited as the paradigm originator.
But "training run" is a loaded phrase: to most readers it means
**gradient-descent model training** (a model gets better by learning from
data). The Tassadar LLM-computer core does **not** do that. Without a fixed
definition, on-camera or marketing copy can silently slide from the true,
narrow claim into the false, broad one.

This document is the term sheet that the eventual evidence pack and
owner-signed upgrade must conform to.

---

## The definition (load-bearing)

> An **LLM-computer training run** (in the OpenAgents/Percepta-class sense) is a
> public, open-contributor run in which **programs are compiled directly into
> transformer weights** and then **executed exactly** by contributors, with the
> execution **independently replay-verified** and **paid**. The "training" is
> the **construction of the executor artifact** (compiling a program into
> weights), **not** gradient-descent learning. There is **no loss function, no
> backpropagation, and no weight update from data** in the core.

Reading guide for the term, word by word:

| Term | What it MEANS here | What it does NOT mean |
| --- | --- | --- |
| **LLM-computer** | A transformer whose weights encode a program/interpreter, so a forward pass *executes* that program (Percepta `transformer-vm` paradigm). | A trained chat/instruct LLM, a served model, or a model with learned general capability. |
| **training run** | The **executor-construction** sense: compiling a program into weights and exercising/replaying that artifact across a public network. | Gradient-descent training, pretraining, fine-tuning, RLHF, or any data-driven weight learning. |
| **public / open-contributor** | Anyone can join, claim work, run the executor on their own consumer device, and be paid for replay-verified work. | A single-org closed research artifact (this is the axis on which the claim is "first" vs. Percepta). |
| **run** | A bounded, receipt-backed live window (`run.tassadar.executor.20260615`). | An open-ended production "we train models" capability. |

---

## What the live lane actually does (ground truth)

Source of truth: `packages/tassadar-executor/` and
`compute.tassadar_executor_poc.v1`.

- Programs are compiled into a portable, digest-pinned numeric artifact
  (`TassadarAlmNumericModel` v1) by the psionic executor-compiler.
- A Pylon **executes** the digest-pinned workload; a validator **replays** it on
  a separate device; the verdict is a **byte-for-byte digest comparison**.
- Claim boundary from the package README: "faithful re-execution of
  digest-pinned compiled workloads only — **no softmax, no learning, no
  serving, no performance claim** against conventional CPUs."

So the live evidence supports the **executor-construction** reading of
"training run" and nothing beyond it.

---

## What this term does NOT authorize (guardrails)

Copy using "LLM-computer training run" MUST NOT imply any of:

1. **Gradient-descent training.** No model learned anything from data. Do not
   say "we trained a model," "the model improved," "loss went down," or imply
   pretraining/fine-tuning.
2. **General LLM-computer capability.** The PoC executes specific
   digest-pinned compiled programs. It is not a general programmable computer
   with arbitrary capability, and not a served product.
3. **Paradigm invention.** Percepta defined and demonstrated the
   compiled-program-in-weights paradigm (March 2026). The OpenAgents "first" is
   the **public, paid, open-contributor run**, never "we invented the
   LLM-computer."
4. **Performance parity.** No claim of speed/efficiency vs. conventional CPUs
   or GPUs.
5. **Network scale.** A bounded run with a small number of settled
   contributors is not a network-scale training operation.

The promise's `unsafeCopy` field remains the binding authority over all public
copy; this document is the definitional backing for it.

---

## On-camera / copy phrasing that conforms to this definition

Safe (matches the definition + the prior-art review):

> "The first **public, open-contributor LLM-computer training run** — the
> compiled-program-in-weights paradigm defined by Percepta, run for the first
> time as a public network anyone can join and get paid for. The 'training' is
> compiling programs into transformer weights and verifying their exact
> execution — not gradient-descent model training."

Unsafe (defeats the definition): "first LLM-computer," "we trained an LLM,"
"first public LLM training run," any bare "world first."

---

## What this clears vs. what remains

**Cleared by this document:**
`blocker.product_promises.llm_computer_training_run_definition_missing` — the
term "LLM-computer training run" now has a precise, written definition that
binds the claim to the executor-construction sense and is grounded in the live
`packages/tassadar-executor` lane.

**Still open (promise stays RED):**
- `blocker.product_promises.world_first_evidence_pack_missing` — a
  dereferenceable evidence pack tying the qualified world-first to the live run
  receipts.
- `blocker.product_promises.world_first_owner_signed_upgrade_missing` — an
  owner-signed receipt-first upgrade per `proof.claim_upgrade_receipts.v1`.

This definition is necessary but not sufficient for green; it removes the
ambiguity, it does not establish the first-in-the-world fact.

---

## References

- `docs/launch/2026-06-18-world-firsts-verification.md` (prior-art review)
- `docs/transcripts/238.md` (the on-camera claim)
- `packages/tassadar-executor/README.md` (live executor lane ground truth)
- Promise `compute.tassadar_executor_poc.v1`
- Promise `models.tassadar_percepta_executor.v1`
- Percepta: https://www.percepta.ai/blog/constructing-llm-computer ,
  https://github.com/Percepta-Core/transformer-vm
