# Coder decision-function inventory

This inventory records the existing decision sites for
[#9503](https://github.com/OpenAgentsInc/openagents/issues/9503). It does not
introduce new questions or declare their policies calibrated. The common
function registry and workload-specific model/policy admission remain work in
that issue.

## Current functions

| Function or set | Definition | Input and output | Consumer and evidence boundary |
| --- | --- | --- | --- |
| `coder-turns-v2` / `action` | `classify::questions` in `crates/coder/src/classify.rs` | Bounded conversation state; Choice among respond, clarify, end conversation, and none | Routes a turn. The trace records the same `question_set`, `set_digest`, and `gate` provenance a file-defined set emits, through `classify::questions_provenance`. See the [question baseline](../../decision-models/measurements/2026-09-20-coder-question-baselines.md). |
| `coder-turns-v2` / `outcome` | `classify::shell_questions` in the same module | Shell-round context; Choice consumed by `ShellVerdict::route` | Decides how the loop continues; execution permission remains a separate host decision. Its trace record is `classify::shell_provenance`. The same baseline records the retained and retired questions. |
| `openagents.program.v1` | `questions/program.json` | Request plus resolved program options; Choice including none | Selects only among programs the host resolved. [Program-selection evidence](../../decision-models/measurements/2026-09-19-program-selection.md) is workload-specific. |
| `openagents.independence.v1` | `questions/independence.json` | Six-task wording; Noul independence, read-only, and tool-restriction judgments | Bound by no program since 2026-09-21; retained so the digests earlier runs recorded stay resolvable. |
| `openagents.independence.v2` | `questions/independence-v2.json` | Listed tasks of any length; the same three Noul roles | Referenced by `programs/delegate-fan-out.json` and `programs/burn-down.json`. Host write footprints, dependency checks, and resource reservations remain authoritative. |
| `openagents.completion.v1` | `questions/completion.json` | One requirement in recorded delegation state; Noul usability judgment | Used by delegation programs. A semantic answer cannot substitute for artifact verification, test evidence, or supervisor acceptance. |
| `openagents.review-finding.v1` | `questions/review-finding.json` | One finding and captured change evidence; Noul judgment | Used by `programs/review-changes.json` after mechanical verification. Pinned reviewer execution and evidence bounds do not establish semantic accuracy. Wording baseline: [2026-09-22-review-finding-baseline](../measurements/2026-09-22-review-finding-baseline.md) — 0.90 agreement on a twenty-finding labeled suite, both misses inside the ambiguous band. |

File-defined sets carry schema version `v: 1` and their own IDs. Their contents
are digested independently of the program that names them. Code-defined
functions share the contract: `coder-turns-v2` keeps its measured name, each
function's wording digests through `questions::wording_digest` on the same
`atif::digest` path a `Set` takes, and every decision call the turn records
carries the resulting `question_set`, `set_digest`, `gate`, and
`policy_version` fields beside the answer. A unified function identity
preserves the historical digests and binds the actual wording served —
renaming an entry cannot make different wording the same measured function.

## State and authority

Turn classification bounds serialized state through `classify::STATE_BUDGET`
and `Caps::PRODUCTION`; the [state-budget measurement](../../decision-models/measurements/2026-09-20-state-budget.md)
explains the workload and host assumptions. Program decisions build their own
state in `runtime`; they do not automatically inherit the turn-state evidence.
Each function needs an explicit input limit and retained evaluation covering
its actual state construction.

Model profiles and endpoint configuration live in `crates/coder/src/decision.rs`.
They do not yet form a per-function allowlist of admitted model artifacts.
The host still owns program approval, executor capabilities, write boundaries,
execution permits, and scheduler resource constraints. A model judgment cannot
add an executor, override an operator pin, or increase a granted write scope.

`requires_scorable_answer` asks for a usable typed score. It does not claim
calibration. A set's `policy.evidence` names the measurement references its
claims rest on — `openagents.program.v1`, `openagents.independence.v2`, and
`openagents.evidence-relevance.v1` declare theirs, and the decision's
provenance records them beside the wording digest. A set that names none is
unmeasured, and the record says so by saying nothing. Existing program
thresholds require their own development and held-out evidence before they
can support a quality claim.

## Remaining integration

1. ~~Give code-defined turn and shell functions the same versioned contract and
   digest path as file-defined sets, without changing measured wording.~~
   Done: `questions_provenance` and `shell_provenance` emit the
   `Set::provenance` record over the code-built wording, and the turn's
   decision calls carry it.
2. Bind each function to input/output schemas, state limits, allowed artifact
   profiles, evaluation/calibration evidence, and a versioned abstention policy.
3. Connect opt-in review/fallback to the original result, reviewer result,
   actual artifact identities, nullable scores, and every attempt's usage and
   cost. Honor local disclosure restrictions before a remote attempt.
4. Test absent candidates, changed artifacts, missing evidence, and confidently
   wrong judgments. Measure any new context, relevance, review, ranking, or
   executor-choice question before production adoption.

The retired v1 turn questions remain in
`crates/gym/questions/coder-turns-v1.json` for reproducibility. Do not restore
questions that no decision consumes merely to increase decision API usage.
