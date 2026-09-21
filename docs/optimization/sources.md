# Conceptual references

These sources inform the architecture. They do not specify OpenAgents wire
contracts or establish performance for an OpenAgents workload.

| Source | Concept used in the design |
| --- | --- |
| [Engineering AI systems that endure the bitter lesson](https://www.youtube.com/watch?v=qdmxApz3EJI), Omar Khattab | Separate task meaning, evaluations, and essential code from replaceable model and inference techniques. |
| [DSPy signatures](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/programming/signatures.md) | Semantic input/output contracts for model-backed behavior. |
| [DSPy modules](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/programming/modules.md) | Composable implementations and inference strategies. |
| [DSPy optimizers](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/learn/optimization/optimizers.md) | Metric-guided optimization of composed AI software. |
| [DSPy GEPA overview](https://github.com/stanfordnlp/dspy/blob/main/docs/docs/api/optimizers/GEPA/overview.md) | Reflective candidate improvement through execution feedback. |
| [GEPA API](https://gepa-ai.github.io/gepa/api/) | Task/evaluator interfaces and adapters for a broader optimization space. |

The [concepts document](concepts.md) explains the interpretation. The talk is
paraphrased; machine transcription is not a source of verified quotations.
The user-supplied DSPy social excerpt has no verified permalink here and is
not used to establish a publication date or a performance claim.

These upstream pages are conceptual references. A concrete integration pins
its dependencies, algorithm configuration, exporter, and execution environment
and verifies their compatibility. No upstream library version is mandated by
NIP-OPT, and changing a version creates a new implementation or study identity
where it changes behavior.
