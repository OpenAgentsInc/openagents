# TypeSafe opportunities

Status: design analysis and evaluation hypotheses. The source is
[thoughts on a typesafe coding agent](../coder/design/thoughts-on-a-typesafe-coding-agent.md),
including its [original exports and six images](../coder/thoughts-on-a-typesafe-coding-agent/).
The proposal identifies costs and possible improvements; its illustrative
percentages, cost examples, and asymptotic claims are not Coder measurements.

## Make extensions part of explicit state

The opportunity is larger than making tool output shorter. Coder can keep
versioned evidence, expose a small set of relevant operations, choose the
representation each recipient needs, and reuse observations across tasks.
Programs make the workflow inspectable; plugins supply bounded evidence
operations; TypeSafe functions judge relevance, sufficiency, and requirements.
Rust retains control of scheduling, authority, input construction, and effects.

This division follows the [System One design guidance](https://docs.typesafe.ai/concepts/how-to-build-with-system-one):
put narrow semantic judgments inside explicit software control flow. A
function's confidence is input to a measured consuming policy, not permission
to act. Use inference only where a mechanical rule cannot settle the question
well enough for the workload.

## Opportunities and concrete mechanisms

| Source opportunity | Specific system behavior | Evidence needed before claiming a gain |
| --- | --- | --- |
| Input-heavy coding turns and repeated reads | Store captures and source versions once. Plugins derive outlines, diagnostics, facts, and spans. Context manifests reference exactly which representations were supplied. | Actual input/output usage, repeated-read rate, missed evidence, task completion, and total cost against the existing turn. |
| Large tool catalogs and model-specific tool familiarity | Discover from small descriptors, mechanically filter, shortlist, then load only selected schemas. Keep host bindings stable and evaluate the target generator. | Retrieval recall, selection errors, schema tokens, invalid arguments, extra round trips, and full-task outcomes as catalog size grows. |
| Query-aware context preparation | Typed relevance and sufficiency functions select evidence under a budget. Mandatory instructions and acceptance evidence stay required. A failed sufficiency check expands retained sources. | Held-out recall, omitted-constraint failures, expansion rate, decision overhead, and resulting repair quality. |
| Switching models can waste a cached prefix | Keep context manifests and stable prefix identities. Route using actual provider usage and price configuration, expected future work, cache uncertainty, and switching cost. | Observed cached/uncached billing and complete-run latency/cost, including rebuilding context. A digest match alone is not a cache-hit measurement. |
| Subagents need preparation and their results must be assimilated | Pass a task frame and immutable evidence references through a native context binding. External adapters materialize only permitted evidence and return bounded artifacts with provenance. | Parent preparation/assimilation work, recipient disclosure, redundant reads, artifact quality, and adapter limitations. |
| Restart from relevant history instead of accumulated chat | Index tasks, attempts, and evidence with hierarchical retrieval plus source expansion. Restore constraints and unresolved work from the task frame. | Recovery recall and task success at increasing history sizes. Tree organization alone does not establish logarithmic useful retrieval. |
| Conditional instructions and structured skills | Apply mandatory scope rules deterministically. Select optional skills semantically, activate supported hooks for bounded lifetimes, and expire them with task/session scope. | Missed mandatory rules, irrelevant guidance tokens, stale activation, hook invocation counts, and task outcomes. |
| Explicit variables and reusable intermediate results | Typed program bindings refer to stored evidence/artifacts. A generator can request a bounded expansion without repeating a full transcript. | Reuse rate, freshness failures, total context and decision calls, and correctness under changed source versions. |
| Visualize attention to useful context | Terminal views show included spans, selected representations, reasons, omissions, and expansion history from context manifests. | Whether users can find unsupported conclusions and diagnose selection failures. A relevance score is not a model-attention measurement. |
| Parallel independent work with locks | Use project claims, effect scopes, dependency checks, immutable snapshots, and isolated writing delegates. A semantic decision can narrow uncertain work but cannot override known conflicts. | Correct parallelism, conflict/redo rates, elapsed time, and verification after integration. |
| Privacy constraints across local and hosted models | Filter by operator-declared data classification and permitted recipients before routing. Build a separate context per recipient and account for reviewer/fallback destinations. | Tests that disallowed evidence never reaches any destination, including fallback and logs. Geography is an explicit policy input, not an inferred nationality rule. |
| Shared observations and duplicate subgoals | Reuse exact compatible task/source/operation identities; coalesce in-flight reads. Semantic similarity proposes reuse but does not certify equivalence. | Avoided work, invalid reuse, freshness behavior, and how omitted work affects completion. |
| Background explanations, reviews, and test proposals | Schedule low-priority programs over shared snapshots within a separate allowance. Cancel stale work and publish results as attributable views. | Foreground interference, incremental cost, actionable findings, stale-result rate, and explicit acceptance of proposed changes. |

## Batteries without a permanent prompt cost

A large installed library can have a small active interface. Installation
populates the local catalog and verified content store; it does not append
every manual, schema, or skill to the generator's prompt. Cheap index lookup
and mechanical eligibility checks precede any semantic ranking. Shortlisting
still has a cost and may omit the right operation, so bounded expansion and
an explicit unavailable result are required.

The [TypeSafe reranking cookbook](https://docs.typesafe.ai/cookbooks/rerank_typesafe)
is a useful pattern: first retrieve candidates, then judge them in context.
Evaluate both stages. A perfect decision function cannot recover an operation
the retriever excluded. Record unselected and uninvoked eligible operations
instead of measuring only successful invocations.

The reference Coder suite suggests several useful package families:

| Family | Reference examples | OpenAgents adaptation |
| --- | --- | --- |
| Structural repository evidence | `rust-outline`, `repo-map`, `repo-tree`, `ast-grep-bounded` | Versioned symbol/structure evidence with links to exact source spans; use native parsers where they already suffice. |
| Bounded retrieval | `repo-search-bounded`, `code-search`, `repo-search` | A source-aware query interface with capture limits, ordered results, and explicit incomplete coverage. |
| Diagnostics and checks | `cargo-diagnostic-filter`, `test-report`, `patch-check` | Typed diagnostics and test evidence; verification authority stays with the protected host checker. |
| Context and facts | `repo-context`, `git-facts`, `git-diff-summary`, `env-facts` | Task-specific facts over pinned repository/configuration state, with disclosure filtering before use. |
| History and knowledge | `session-search`, `read-conversation`, `foreign-sessions`, `knowledge-base` | Scoped evidence retrieval; no automatic cross-session or cross-account access. |
| Output representation | `shell-digest`, `progress-filter`, file/directory statistics | Optional derivatives with validated format and retained original captures; no claim of success based on compression ratio. |

These are design references, not a list of implemented or promised ports.
The source proposal's headroom/rtk, ast-grep/ast-outline, fastcontext, and fff
examples identify compression, structural search, exploration, and indexing
opportunities. Evaluate equivalent native operations and maintained adapters
before choosing dependencies. Installing all reference plugins is not the
delivery objective.

## Worked workflow: diagnose and repair a Rust failure

Consider a future Rust investigation package with a repair program, diagnostic
parser plugin, symbol-outline plugin, optional Rust guidance, and admitted
evidence-selection functions. The package requires a native editing executor
and the host's protected verification service; it does not implement either
inside Wasm.

1. The task frame records the requested repair, repository base, constraints,
   and acceptance command or protected verification reference. A structured
   program request resolves directly; a natural request may use program
   selection among eligible workflows and `none`.
2. The host captures the failure under an execution permit and stores its
   bounded output. The diagnostic plugin produces typed errors and source
   references from that capture. Raw output remains expandable.
3. Mechanical source lookup finds named files and symbols. If there are too
   many plausible dependencies, an admitted function ranks their relevance.
   The context builder retains required constraints and verification evidence,
   selects representations, and records unresolved coverage.
4. The native executor receives the task-specific context and proposes a
   patch in its authorized workspace. A plugin cannot apply the patch. A
   writing delegate uses an isolated base and returns an artifact identity.
5. Protected checks run against the resulting artifact. Requirement review
   is separate from test exit status. A bounded repair round may expand
   evidence after an unresolved or failed result; it cannot retry indefinitely.
6. The result joins the patch, source versions, selected context, decision
   records, verification, and integration status. Optional background views
   reuse the same evidence for explanation or additional review.

The expected savings come from fewer repeated reads, smaller useful inputs,
less manual context preparation, and fewer avoidable repair rounds. Added
selection calls, parsing, storage, or mistaken omissions may erase those
savings. Compare complete runs against a deterministic host-only baseline
and the current agent, not just plugin output size.

## Cache economics and model routing

The source example shows why a cheaper uncached model can cost more after a
switch. Preserve that insight without freezing illustrative prices into a
policy. Estimate remaining generation, prefix reuse eligibility, expected
rebuild input, decision overhead, and provider-specific cache behavior using
observed usage and versioned price data. Record uncertainty and compare the
actual outcome with the estimate.

Prefer stable ordering for unchanged mandatory context, schema definitions,
and evidence references where provider semantics allow it. Do not preserve
stale or irrelevant context merely to keep a prefix stable. Decision-model,
generator, reviewer, and executor routing remain separate policies with the
same disclosure and budget boundaries.

## Background work and learning

The archived images propose progress pages, interactive explainers/quizzes,
test ideas, shadow evaluation, and hierarchical history. In this architecture
they are optional programs and views over shared evidence, not separate
agents that reread the repository from scratch. A view records its base,
sources, cost, and expiration; a changing base invalidates relevant claims.

Generating a test proposal is distinct from writing or running that test.
Displaying a local explainer is distinct from publishing a page. Traffic
shadowing, collecting training examples, and exporting histories each need
explicit data and resource policy. Never let a default background hook export
private work or spend an unbounded second task budget.

Promote repeatedly useful behavior into a versioned program or decision
function through reviewed evaluation. Do not let a successful session mutate
the active catalog, rewrite a question set, or train/deploy a model in place.
Keep source-proposal savings claims as hypotheses until the
[delivery evaluation](delivery.md#evaluation-and-default-admission) measures them.
