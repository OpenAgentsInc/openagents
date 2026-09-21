# Coder decision-function inventory

This inventory records the existing decision sites for
[#9503](https://github.com/OpenAgentsInc/openagents/issues/9503). It does not
introduce new questions or declare their policies calibrated. The common
function registry and workload-specific model/policy admission remain work in
that issue.

## Current functions

| Function or set | Definition | Input and output | Consumer and evidence boundary |
| --- | --- | --- | --- |
| `coder-turns-v2` / `action` | `classify::questions` in `crates/coder/src/classify.rs` | Bounded conversation state; Choice among respond, clarify, end conversation, and none | Routes a turn; code-defined wording is not yet a file-registry function. See the [question baseline](../decision-models/2026-09-20-coder-question-baselines.md). |
| `coder-turns-v2` / `outcome` | `classify::shell_questions` in the same module | Shell-round context; Choice consumed by `ShellVerdict::route` | Decides how the loop continues; execution permission remains a separate host decision. The same baseline records the retained and retired questions. |
| `openagents.program.v1` | `questions/program.json` | Request plus resolved program options; Choice including none | Selects only among programs the host resolved. [Program-selection evidence](../decision-models/2026-09-19-program-selection.md) is workload-specific. |
| `openagents.independence.v1` | `questions/independence.json` | Six-task wording; Noul independence, read-only, and tool-restriction judgments | Referenced by `programs/delegate-fan-out.json`. Retained for the existing program and historical identity. |
| `openagents.independence.v2` | `questions/independence-v2.json` | Listed tasks of any length; the same three Noul roles | Referenced by `programs/burn-down.json`. Host write footprints, dependency checks, and resource reservations remain authoritative. |
| `openagents.completion.v1` | `questions/completion.json` | One requirement in recorded delegation state; Noul usability judgment | Used by delegation programs. A semantic answer cannot substitute for artifact verification, test evidence, or supervisor acceptance. |
| `openagents.review-finding.v1` | `questions/review-finding.json` | One finding and captured change evidence; Noul judgment | Used by `programs/review-changes.json` after mechanical verification. Pinned reviewer execution and evidence bounds do not establish semantic accuracy. |

File-defined sets carry schema version `v: 1` and their own IDs. Their contents
are digested independently of the program that names them. A future unified
function identity must preserve those historical digests and bind the actual
question/options served, including supplied program choices. Renaming an entry
cannot make different wording the same measured function.

## State and authority

Turn classification bounds serialized state through `classify::STATE_BUDGET`
and `Caps::PRODUCTION`; the [state-budget measurement](../decision-models/2026-09-20-state-budget.md)
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
calibration. Existing program thresholds require their own development and
held-out evidence before they can support a quality claim.

## Remaining integration

1. Give code-defined turn and shell functions the same versioned contract and
   digest path as file-defined sets, without changing measured wording.
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
