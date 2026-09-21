# The talk, DSPy, and GEPA

## What the talk changes

Omar Khattab's [Engineering AI systems that endure the bitter lesson](https://www.youtube.com/watch?v=qdmxApz3EJI)
argues for separating the application's purpose from rapidly changing model
techniques. The supplied transcript describes three complementary engineering
materials: localized natural-language specifications, evaluations of what
matters, and code for essential composition and control. It challenges early
commitment to model-specific tricks without evidence that a higher-level
solution is insufficient.

That is our interpretation of the talk, not a claim that its author endorses
OpenAgents. The supplied machine transcription contains recognition errors;
this document paraphrases it. The video is dated June 2025 in the user's
introduction. The quoted DSPy social post has no supplied permalink or
independently verified publication date.

Hand-crafting hundreds of permanently fixed question calls can couple an
application to today's model architecture. An operation such as
“select sufficient evidence for this answer” should survive a change from
per-item judgments to learned retrieval, a joint selector, or a bounded
search strategy. Its evidence obligations and authority remain explicit.

The claim is not that engineering or typed judgments become unnecessary.
A task still needs a meaning and a reliable boundary. Neither a general agent
given a long instruction nor a low-level decision pipeline automatically
provides a durable software abstraction.

## Distinguish the concepts

| Concept | Role in our design |
| --- | --- |
| Semantic AI signature | Task meaning, semantic input/output fields, abstention, and invariants. It is distinct from a Nostr event signature and an operation discovery descriptor. |
| Module or inference strategy | A replaceable way to realize part of the task, possibly using multiple model/tool calls under host limits. |
| AI implementation | One complete, pinned realization of a signature, including its entry point, models, adapters, instructions, examples, and configuration. |
| Optimizer | Searches an explicitly allowed implementation space using an objective and authorized data. |
| Evaluation | Measures behavior against a frozen workload and policy. An optimizer's selection score is not independent confirmation. |
| Compilation | Produces a concrete implementation from task/program structure and selected parameters; it need not mean machine-code compilation or weight training. |
| Promotion | An operator-policy decision to admit an exact measured implementation for a scope. It is separate from compilation, package publication, and execution grants. |

DSPy signatures express semantic input/output behavior, including meaningful
field names and task instructions. They do more than demand JSON formatting.
For OpenAgents, their counterpart is a portable contract with explicit schema
validation and host enforcement; matching types alone cannot prove that the
task was performed correctly.
([DSPy signatures](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/programming/signatures.md))

DSPy modules compose model-backed behavior and abstract inference patterns.
This suggests separating our semantic operation from a particular chain of
calls. A DSPy module is not our Wasm plugin, and a DSPy program is not
automatically a NIP-PRG document. Export needs an explicit supported mapping.
([DSPy modules](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/programming/modules.md))

DSPy optimizers tune a program against examples and a metric. Different
optimizers search different things, including instructions, demonstrations,
or model parameters. Choosing an optimizer does not replace defining the
system objective or establishing independent evidence.
([DSPy optimizers](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/optimization/optimizers.md))

GEPA uses execution feedback and reflection to propose improvements and retain
useful candidates. Its DSPy integration is one application of the method.
Our study records identify the actual algorithm and configuration; we do not
label a random search, grid search, or manual edit as GEPA.
([DSPy GEPA overview](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/api/optimizers/GEPA/overview.md))

The current standalone GEPA API also exposes general task/evaluator adapters
and broader program evolution. This supports a design open to more than
question rewording. It does not justify permitting arbitrary candidate code
inside the product process.
([GEPA API](https://gepa-ai.github.io/gepa/api/))

## What we adopt and what we leave open

Adopt the separation of semantic intent, essential code, evaluation, and
replaceable implementation. Admit an optimizer as a bounded development tool.
Evaluate complete tasks alongside local module scores. Preserve negative
results, actual execution identities, and the cost of finding a candidate.

Do not make one optimizer, model family, or fixed decomposition foundational.
Do not claim that changing a model preserves quality, calibration, or privacy
just because it accepts the same schema. A provider alias can drift while its
name stays unchanged. Portability is a supported contract plus fresh evidence
for the target, not a promise of equal scores.

A broader search can be useful after a baseline exists. It can also be more
expensive than the improvement is worth. Include development spend and expected
deployment volume in the adoption decision; a small per-call saving need not
repay a large campaign. The first integration must demonstrate the full loop
before expanding into another optimization framework.
