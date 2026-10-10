# The codebase as the first customer

The first customer is an unusually demanding Rust monorepo with several Cargo workspaces, large retained experiments, native clients, paid services, generated artifacts, and many agents working concurrently. This makes it a useful proving ground. It also makes it a poor sole source of evidence for a general repository product. A system can learn this repository's issue wording, commit conventions, file layout, and test shortcuts without learning to solve unfamiliar customers' issues.

This chapter connects the whole-codebase health audit to the product. It does not restate 709 historical findings as 709 present defects. The [complete inherited register](evidence/prior-health-findings.csv) preserves their identities; the [earlier remediation record](../2026-10-10-codebase-health-audit/remediation.md) documents fixes since the health snapshot.

## Three kinds of improvement

Keep these outcomes separate in records and public claims:

1. **Improving the repository:** the agent fixes a bug, removes an expensive dependency, improves tests, or shortens a build. Subsequent work may get cheaper because the repository changed.
2. **Improving the coding system:** a human changes the briefing, tool set, runtime, model, or resource policy. This is engineering improvement, even when an agent implements it.
3. **Learning from the coding system's outputs:** verified outcomes change a ranker, decision head, or calibration map; that candidate beats its predecessor on independent future work. This is the product spec's recursive learning claim.

All three are valuable. Only the third establishes the promised recursion. A smaller repository or warmer Cargo cache cannot be credited to a learned ranker without a comparison that holds those factors constant. Conversely, requiring a model update before counting a useful build fix would obscure real customer value.

## What to reuse

| Existing owner | Contribution to the product | Boundary to preserve |
| --- | --- | --- |
| `coder`, `coder-new`, `briefed-agent`, Claude Agent SDK | Task execution, issue briefing, tools, histories and agent loops | Choose and document the supported execution path; behavior in another runner is not inherited automatically |
| `coder-environment`, `coder-cloud`, `coder-lease`, `supervise` | Workspaces, images, placement, resource sharing and bounded child processes | A lease coordinates cooperative processes; it is not a tenant sandbox or permission grant |
| `atif`, replay tooling, `receipts` | Structured attempts, tool results, replay evidence and attempt identity | A receipt proves what its producer recorded, not that an independent observer witnessed it |
| `gym`, `ext-eval`, `knowledge` | Frozen studies, paired comparisons, reviewed transfer, activation and withdrawal concepts | Reuse the strict evidence rules; do not interpret a synthetic fixture as a measured benefit |
| `tenancy::training`, `jev`, Clef/Psionic, Pylon | Corpus records, recipes, decision contracts, training/serving components | Join them with explicit dataset, candidate, permission, calibration and release identities |
| `coder-access`, `coder-connect`, `coder-reach`, Nostr contracts | Paired devices, bounded remote authority, transport and host identity | Observation rights, execution rights and production deployment authority remain distinct |
| `oa-auth`, tenancy, gateway, account/compute/sales records | Accounts, repository access, budgets, usage, accepted delivery and claims | Account membership or payment does not itself grant training permission or repository write permission |
| Shared Rust native presentation and clients | Terminal, web, desktop and phone access to the same work | A new UI must consume the same outcome and authority records rather than create another agent runner |

These are owners to inspect and integrate, not a proposal for a new orchestration framework. The [runtime chapter](02-execution-and-verification.md), [learning chapter](03-learning-and-evaluation.md), and [data chapter](06-customer-data-and-authority.md) identify where those joins are presently incomplete.

## CODE-01: Version the whole decision and execution bundle

**Assessment: missing joined proof, high product importance.** The product story names a ranker or head version, but the result depends on more than its weights. At minimum, bind an experiment or accepted task to:

| Part | Required identity |
| --- | --- |
| Repository | Host, owner, repository ID, base commit, exact input patch and admitted untracked files |
| Issue | Immutable issue text and acceptance snapshot; subsequent edits recorded separately |
| Finder | Index build revision, history cutoff, feature code, embedding model, ranker, candidate budget and feedback set |
| Brief | Generator revision, source selections, repo instructions and template |
| Agent | Provider/model, version where available, effort, system instructions, tools and stopping limits |
| Decisions | Door/model, recipe, head, calibration, threshold/cost policy and fallback used |
| Verification | Trusted checker revision, test selection, expected tests, command exits and environment |
| Environment | Image/toolchain/lockfiles, architecture, resource limits and cache condition |
| Outcome | Attempt tree, reviewed patch, merge commit, deployment evidence and later rejection or revert |
| Accounting | Model calls, setup, replay, retries, integration, storage and operator time where measured |

Existing owners already record portions of this. The proposed work is a reference manifest that joins them without copying all raw content. Unknown fields remain unknown. A model alias is not an immutable model version; changing an upstream alias should begin a new analysis stratum.

**Acceptance:** one sampled task can be reconstructed from input to deployed or rejected result; a change in any result-affecting component either changes the bundle identity or is explicitly excluded with a reason. A bundle with an absent required artifact cannot qualify for promotion. An exact reproduction is still not a causal comparison without a control.

## CODE-02: Keep the acceptance authority outside the candidate's control

**Assessment: architectural requirement requiring end-to-end enforcement.** A deterministic verifier is not necessarily independent. If an agent can edit tests, its verifier script, a fixture, a build script, the expected output, or the acceptance specification before the independent run, clean-worktree replay can faithfully reproduce a weakened check.

The [product spec](../../product/self-improving-codebases.md) says checks and ground truth stay outside the model's reach. Implement this as a write and execution boundary, not a prompt instruction. A source-controlled test may legitimately need to change with a feature. Keep that patch reviewable, while binding the authoritative acceptance suite and expected coverage outside the writable candidate tree. Treat changes to the reference evaluator as a separately reviewed release, with its own evidence and never as proof of its own correctness.

Tests execute candidate code and build scripts. Replay therefore needs an execution sandbox, bounded output/time/resource use, and restricted credentials even when the runner itself is trusted. A digest protects identity, not the semantics or safety of the identified program. A malicious test can print an expected success line; an independent process result and trusted expected-test list are stronger than log parsing alone.

**Acceptance:** negative fixtures that delete or ignore a required test, rename the required target, edit the verifier, emit forged pass text, modify a build script, or change the issue's acceptance conditions cannot convert failure into accepted evidence. Permitted test changes remain possible through explicit review. Production approval and customer acceptance remain separate from test success.

## CODE-03: Use the health backlog as a task portfolio, not as an automatic repair order

**Assessment: immediate opportunity with selection bias risks.** The health audit is rich task discovery material: 709 rows across 34 overlapping sections, with locations and proposed fixes. The rows overlap, have different ages and severities, and include work already remediated. Counting them as 709 independent bugs or feeding them straight into an autonomous queue would inflate apparent throughput and duplicate work.

A proposed portfolio should include:

| Task family | Value to the first customer | Independent acceptance |
| --- | --- | --- |
| Verifier and process failure handling | Prevents false successes and polluted learning labels | Missing executable, nonzero exit, timeout, zero-test and stale-cache fixtures |
| Finder and trace provenance | Makes training examples attributable | Exact issue/base/patch/check binding; tampering and cross-repo rejection |
| Small deterministic runtime defects | Exercises ordinary repository maintenance | Reproduction fails before patch and passes afterward; relevant regression checks |
| Build scope and workspace routing | Shortens every subsequent iteration | Printed scope fixtures for nested workspaces, lockfiles, embedded documents and direct consumers |
| Resource cleanup/recovery | Avoids stalled workers and unexpected spend | Scratch restart, interrupted job and idempotent cleanup fixtures |
| Data deletion and tenant isolation | Establishes the customer boundary | Seeded content traced through allowed stores, then absent or explicitly retained under policy |
| Documentation or generated-output maintenance | Measures low-risk end-to-end delivery | Link/generated-file check, accurate copy and review; report separately from coding |

Sample issue families prospectively. Include ordinary work, difficult tasks, failures and blocked tasks. Do not fill S1's quota with twenty nearly identical document edits, repeated historical fixes, or twenty attempts at one issue. The system can suggest candidates and explain expected benefit; the owner sets priorities and the accepted issue set. Do not reward issue creation, closure count, lines changed, or model agreement as substitutes for accepted useful work.

## CODE-04: Reduce iteration cost where it changes measured outcomes

The prior audit's large-file and dependency findings are relevant because retrieval, compilation, worktree creation, review and integration all consume the product's budget. File size alone does not prove a defect. Its published sizes describe its earlier snapshot and are not fresh measurements from this audit.

Prioritize by observed repeated cost:

1. Keep long-lived target directories outside worktrees and honor the build/quiet leases. Cache misses, queue time and compilation time belong in task accounting.
2. Finish and validate affected-package scope before considering broader test expansion. The default remains the edited crates' tests and formatting, with relevant consumers. Full release verification remains a release operation.
3. Decouple serving paths from research-only dependencies where profiling and the crate graph confirm they dominate builds. Preserve exported contracts and benchmark reproduction when moving code.
4. Separate retained large artifacts from checkout-critical sources only after proving retrieval and digest validation from their new home. Preserve `docs/transcripts/`.
5. Consolidate proven duplicate custody/process/store behavior incrementally, beginning with active high-risk callers. Do not rewrite all implementations under one broad cleanup issue.

The health audit's SHA-2 consolidation suggestions point in different directions in different sections. Likewise, sorted JSON and an RFC-style canonical format need not produce the same bytes. Before unifying a dependency or digest helper, preserve golden vectors and identify consumers; changing historic receipt identities to simplify code could destroy evidence continuity.

**Acceptance:** each cost change carries a before/after measurement with the same workload and cache conditions, plus scoped correctness checks. Report cold and warm behavior where both affect customer experience. Attribute infrastructure savings separately from model savings.

## CODE-05: Source-reference checks are useful but do not validate claims

**Source-confirmed boundary.** The test `every_working_promise_names_evidence_that_exists` in [promises.rs](../../../crates/openagents-web/src/promises.rs) verifies that a test name, smoke name, golden ID or document exists. It is a valuable broken-reference guard. It does not verify that the cited test recently passed, that it covers the current deployment, or that a promised benefit was measured in a customer cohort.

Closed [#11122](https://github.com/OpenAgentsInc/openagents/issues/11122) created the promises registry. That implementation closure cannot establish every listed product outcome. Reuse the more specific sales claims/price records and the experiment manifest for numerical claims. Bind a displayed improvement statement to the actual study, population, period, bundle and result. Retire or relabel a statement when its evidence no longer applies.

**Acceptance:** a current claim can be traced to current evidence of the appropriate class. An old passing test, a synthetic fixture, a source file and a measured customer outcome are displayed distinctly. This does not require running the entire repository for every content edit.

## CODE-06: Preserve evidence through repository history changes

**Upcoming dependency:** [#11116](https://github.com/OpenAgentsInc/openagents/issues/11116) concerns repository history rewriting. Training rows, historical replay, issue-to-fix mining and evaluation artifacts depend on old commits. A rewrite can invalidate those references even if current source is unchanged.

Before an authorized rewrite, inventory all referenced commits and external artifacts. Preserve an old-to-new mapping, content hashes, the minimum authorized replay inputs, and explicit unavailable outcomes. A commit mapping does not prove that an index rebuilt from rewritten history has the same split membership or learned features. Rebuild and compare manifests before training resumes. Do not rehydrate deleted secrets or content that a customer withdrew merely to preserve reproducibility; mark those examples unavailable and exclude them from future builds.

**Acceptance:** representative old experiments can either replay through the mapping or fail explicitly as unavailable. No lookup silently substitutes today's file for an old version. Rebuilt datasets retain group boundaries and holdout status. This audit does not authorize or perform a history rewrite.

## Which parts of the repository gate the first product?

| Surface | Relationship to S1–S3 | Work to defer unless an actual blocker appears |
| --- | --- | --- |
| Coding runner, finder, verifier, replay, integration | Direct critical path | Another parallel runner or wholesale orchestrator replacement |
| Gym, corpus, ranker, calibration, model promotion | Direct learning proof | General foundation-model pretraining or universal recursive-intelligence claims |
| Cloud/host placement and credentials | Needed for production dogfood and cost truth | Supporting every provider before one reliable owned route |
| Payments, tenancy, sales fulfillment | Needed for an honestly bounded paid pilot | Every payment protocol and autonomous sales acquisition |
| Terminal, web, desktop and phone | Access and supervision; shared records matter | Full parity on every client before the first measured loop |
| Verse, Studio, game agents and memory | Possible users of execution/learning primitives | Game success as evidence of accepted coding improvement |
| Psionic/Tassadar/Psion research | Useful bounded experimental lessons and model components | Treating authored reports or missing-fixture experiments as deployable product capability |
| General health cleanup | Remove measured bottlenecks and dangerous defects | Completing all 709 historical findings as a launch prerequisite |

This ordering reduces scope without discarding existing work. The product succeeds when one well-defined loop is reliable, affordable and demonstrably improves, then travels to another repository. Breadth of implemented components is an asset only when their boundaries are actually connected and checked.
