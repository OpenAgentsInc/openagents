# DSPy in Effect: Git history audit

- Date: 2026-07-20
- Status: Historical evidence report
- Audience: Human
- Canonical branch: origin/main
- DSE history cutoff: 573c905410f9ce742450719f38890d001960a933
- Blueprint extension cutoff: 6122ad6feac24daaa1b24dc164a3c316e42dc023
- External research access date: 2026-07-20
- Evidence scope: All Git objects that are reachable from local refs at each cutoff

## Executive finding

OpenAgents made a substantial DSPy-in-Effect implementation in February 2026.
The first name was ds-effect.
The final package name was @openagentsinc/dse.
The first specification expanded DSE as Declarative Self-Improving Effect.
Later documents also used the short description “DSPy, but Effect TS.”

DSE was not only a proposal.
It had a typed Effect runtime, an evaluation system, a bounded compiler, immutable policy artifacts, receipts, budgets, and an RLM-lite strategy.
The web application added Convex storage, API routes, debug pages, a canary gate, rollback data, and an automated compile-to-promotion loop.
The terminal package tree had 53 files and 9,678 lines.
The web application had 19 DSE-named test files.

The implementation did not contain a real MIPROv2 or GEPA algorithm.
It contained deterministic grid search, greedy few-shot selection, a joint search, a bounded knob search, and rule-based refinement.
Its RLM-lite runtime was a bounded action interpreter.
It was not an arbitrary Python executor.

The evidence does not show that a DSE-specific technical failure caused removal.
The serving applications disappeared first in a whole-application deletion.
A repository-wide Rust-only mandate then deleted all TypeScript packages, including DSE.
The repository removed the Rust DSPy stack again five days later.
The removal sequence shows unstable architecture ownership.
It does not show a comparative rejection of DSE quality.

A June 2026 audit reached a different long-term design.
It recommended upstream Python DSPy and GEPA for offline candidate compilation.
It kept Effect as the online authority for selection, evidence, admission, and release gates.
That audit later received a historical banner.
The current repository still has Effect-based Blueprint contracts, mounted routes, and GEPA-related evidence projections.
Current design analysis treats the Blueprint kernel as deprecated prior art for new product work.
It does not have an active DSE package or a real in-process GEPA optimizer.

Blueprint does not replace DSE as an optimizer.
The February Blueprint was persistent agent profile, policy, and memory state.
DSE selected and optimized the typed tools that changed that state.
The current Blueprint is a separate Effect program kernel.
It controls program contracts, evidence, action proposals, and release decisions.
Its optimizer records describe candidate lineage, but they do not execute an optimizer.

Palantir Ontology is a useful comparison, but it is not an equivalent system.
Palantir documents a complete operational layer with objects, links, actions, functions, a transactional engine, dynamic security, generated SDKs, and cross-application branching.
Current Blueprint deliberately deferred the full business-object graph and object-set engine.
It is primarily a governance and provenance spine for agent programs.
Current OpenAgents terminology calls this system Blueprint, not “our ontology.”

## Direct answer

Yes, the project called the Effect attempt DSE.
It also called it ds-effect at the start.
The exact name history is:

| Date | Name | Evidence |
| --- | --- | --- |
| 2026-02-06 08:34 CST | DSE / ds-effect | beebeb672e72ebf56b750526757bf7f24382e1e2 |
| 2026-02-06 09:07 CST | DSE / dse and @openagentsinc/dse | 544aafa4b79dd6bc409146ab01ca2201305cbed3 |
| 2026-02-11 | “DSPy in Effect” | docs/transcripts/211.md in the retained transcript archive |

The January work under dspy-rs and dsrs was a Rust predecessor.
It supplied many concepts.
It was not DSPy in Effect.

## Main conclusions

| Question | Finding | Confidence |
| --- | --- | --- |
| Did an Effect implementation exist? | Yes. A package and production adapters existed. | Source code |
| Was it only a wrapper around Rust? | No. The final runtime and compiler were TypeScript and Effect native. | Source code and design records |
| Did it implement typed signatures? | Yes. It used Effect Schema for input and output contracts. | Source code |
| Did it implement prompt artifacts? | Yes. It used structured Prompt IR, canonical JSON, and stable hashes. | Source code |
| Did it implement evaluation? | Yes. It had datasets, metrics, rewards, caching, and reports. | Source code |
| Did it implement a compiler? | Yes. It compiled bounded parameter searches into immutable artifacts. | Source code |
| Did it implement MIPROv2? | No. The code did not contain that algorithm. | Source inventory |
| Did it implement GEPA? | No. The code did not contain that algorithm. | Source inventory |
| Did it implement RLM? | Partly. It implemented a bounded RLM-lite action kernel. | Source code |
| Did it reach production wiring? | Yes. The app had production routes, storage, and operator scripts. | Source code |
| Did a promotion complete? | A committed runbook records one successful production promotion. | Historical operational record |
| Why did it disappear? | Whole-app deletion and a Rust-only repository mandate removed it. | Deletion commits |
| Can the old package return unchanged? | No. Its Effect, runtime, storage, and deployment assumptions are obsolete. | Current contract comparison |

## Evidence method

### Search coverage

The audit used the following repository inventory at the cutoff:

- 52,743 reachable commits
- 5,232 refs
- 827 remote refs
- 385 commit subjects with direct DSPy or DSE terms
- 458 commits on canonical DSPy and DSE paths
- 392 direct path objects, with 353 blobs and 39 trees

The 458 path commits represent 229 logical commits.
The repository contains two disconnected replay histories for this work.
Each history has 229 related commits.
The origin/main history is canonical in this report.
The alternate copies survive mainly under old pull-request refs.

Of the 229 commit pairs, 225 have identical path and status lists.
Four large restore or prune pairs differ outside the DSE paths.
Their DSE changes agree.
This report does not count replay commits as separate design attempts.

The audit searched:

- Commit subjects and bodies for DSPy, DSE, ds-effect, dsrs, RLM, GEPA, MIPRO, optimizer, teleprompter, and prompt optimization
- Reachable object names for deleted and renamed paths
- Path history for docs/dspy, docs/dse, docs/autopilot/dse, packages/dse, Rust DSPy crates, and application integrations
- Exact-string changes with Git pickaxe searches
- Point-in-time source trees before each removal
- Current Blueprint and GEPA-related code
- Retained transcripts with explicit DSPy, DSE, RLM, or GEPA terms

Generic words produced many false positives.
For example, the reachable histories contain 438 subjects with signature and 1,376 subjects with module.
The audit accepted these entries only when a direct DSPy, DSE, RLM, GEPA, or MIPRO relation existed.

### Evidence grades

This report uses four evidence grades:

1. Source fact.
   The source tree, a diff, or a schema directly supports the statement.
2. Committed operational record.
   A contemporaneous runbook or receipt text records the event.
   This grade is not a new check of the external service.
3. Later decision record.
   A later audit describes the architecture and its trade-offs.
4. Inference.
   Multiple facts support the conclusion, but no single record states it.

### Limits

The audit covers all objects that were reachable from the local refs.
It does not cover unreachable and garbage-collected objects.
It does not inspect private backroom repositories.
It does not inspect external DSPy, dspy-rs, or dsrs repositories.
It does not validate old production services that no longer have repository authority.

## Attempt 1: Rust DSPy and RLM

### Initial bridge

The first explicit DSPy line started in January 2026.
It used Rust.

| Commit | Date | Result |
| --- | --- | --- |
| 5c6f6dec9abbdb593550931f1b6b98f061b3f0da | 2026-01-04 | Added the Rust RLM execution engine |
| 25a54f221dbc74a9e5b12e4866617a5b686f8b78 | 2026-01-06 | Added the first docs/dspy material |
| 85f407fc94364750632120e5d2ef13e5d1c1b274 | 2026-01-06 | Integrated dspy-rs with the RLM crate |
| ffa4e40e63504aa8d10435a48ff40405fd5f0135 | 2026-01-06 | Documented the bridge and planned optimizer work |
| ab069a36485ea82339c45eb6ac4e98e90960067f | 2026-01-08 | Imported dsrs as the Rust DSPy implementation |

The dspy-rs bridge described Route, Extract, Reduce, and Verify signatures.
The January 6 document said that COPRO and MIPROv2 optimization remained planned.
The January 8 import claimed a larger Rust implementation.
It included predictors, evaluation, caching, tracing, COPRO, MIPROv2, GEPA, and Pareto work.

### Rust expansion

The Rust line added:

- Compiler manifests and trace contracts
- Evaluation tasks and promotion gates
- A SwarmCompiler path
- Retrieval and privacy features
- Training extraction and outcome feedback
- Full Auto signatures and a dedicated runtime thread

Key commits include:

| Commit | Result |
| --- | --- |
| 205d2a4a95fefad0cd24e786631f9570c539c8dd | Compiler contract, callbacks, traces, and scorecards |
| 37ccf6faf707d1a9a4703f7ccd7b71ab5f2e6621 | Retrieval and swarm work |
| c339f2661b7566b056c8ec6cbd20b3418ca5eb3e | Evaluation, scoring, and promotion gates |
| 3f7d25e9b96207ecdd6632b810e3d88d486a2347 | SwarmCompiler |
| 2a89dfb0d1dd9fc1acfe056ea80099567291b93f | Privacy and OANIX work |
| 518da2eed2982e7f46f548e3f802ea78b9f02ec5 | Optimization storage and training extraction |
| c5785ffd8e907ebb735b643554d6f5437c7312a0 | Outcome feedback and automatic optimization |
| 60fc75efe142967d278405eabdaddaefa868c7fd | Full Auto DSPy loop |
| 9d4b82419c82c7c9df3231e9a9a3f767e562f737 | Dedicated DSPy runtime thread |

At its peak, crates/dsrs had 160 files and 42,408 lines.
The peak also had macro, Adjutant, and Autopilot integrations.
This work was much larger than a design note.

The Rust documents contained status tension.
They reported many completed waves and passing tests.
They also listed hard-coded prompts, incomplete routing, missing replay export, missing policy pinning, and future integration waves.
The broad completion language must not hide these named gaps.

### Relation to DSE

The Rust implementation supplied prior art for:

- Typed signatures
- Parameter trees
- Compiler manifests
- Evaluation and promotion
- GEPA and MIPRO concepts
- RLM execution
- Trace and callback contracts

It did not satisfy the web application requirement.
The web and Worker code could not use the Rust crates in process.
This constraint became the reason for an Effect-native design.

## Attempt 2: DSE conception

### The first specification

Commit beebeb672e72ebf56b750526757bf7f24382e1e2 added docs/autopilot/ds-effect.md.
Its title was “DSE / ds-effect: Declarative Self-Improving Effect.”
It mapped DSPy concepts to Effect concepts:

| DSPy concept | DSE concept |
| --- | --- |
| Signature | A stable value with Effect Schema input and output |
| Predict | Input to Effect with typed environment, error, and output |
| Module | Composable Effect programs with parameters and traces |
| Teleprompter | An optimizer that emits a versioned policy bundle |
| Global settings | Effect Layer and ManagedRuntime |

The first recommendation used a TypeScript runtime and a Rust compiler.
The Rust compiler would emit a portable policy bundle.
This recommendation changed quickly.

Commits 43f068f34418416df8fb12130abeb27581e9a5b2 and 30f40ebabe1a8e98d110c4277d099192d7032cbc expanded the specification.
They added Prompt IR, allowlisted transforms, canonical representation, and hashing.

Commit 544aafa4b79dd6bc409146ab01ca2201305cbed3 renamed the specification to docs/autopilot/dse.md.
It also created packages/dse.
The code used the description “DSPy, but Effect TS.”

### The Effect-only decision

The February 8 design records made Rust reference-only.
The final design required an Effect-native runtime and compiler for apps/web.

| Commit | Decision |
| --- | --- |
| 6e80c3b7bc6d492135a421d5e005be99e532cfb4 | Reviewed gaps for RLM, GEPA, and MIPRO |
| 01f9b6937e18292be6e3a056df956b931f9dd6d8 | Made Rust reference-only |
| 2ac8a75f8466736237e15375e16c4826891d7d70 | Added the Effect-only design |
| acba26de2ed2fe94bb6c14d96ac7f4dd45244c9b | Added an 11-step test plan |
| 329b76a6ea4e03a79f2cdc868108694add6d4d39 | Aligned the plan with the Convex-first application |

The documents used careful labels:

- MIPRO-like meant proposed instructions plus evaluation.
- GEPA-like meant a Pareto frontier plus reflect and propose steps.
- RLM meant a bounded structured action protocol.
- Full equivalence with upstream algorithms remained future work.

## DSE implementation

### Peak size

Immediately before application removal, the direct DSE surface had 106 named files.
The package itself had:

- 53 files
- 33 source TypeScript files
- 13 test files
- 3 design documents
- 9,678 total package lines
- 6,416 source lines
- 1,747 test lines

The package metadata was:

- Name: @openagentsinc/dse
- Version: 0.0.1
- Effect: 3.19.16
- TypeScript: 5.9.3
- Test runner: Bun

### Contract layer

DseSignature<I, O> contained:

- A stable signature ID
- An Effect Schema input contract
- An Effect Schema output contract
- Structured Prompt IR
- Default parameters
- Parameter constraints

SignatureContractExportV1 emitted a language-neutral contract with JSON Schema.
The contract allowed other systems to inspect a signature without the TypeScript type system.

Prompt IR used these blocks:

- System
- Instruction
- FewShot
- ToolPolicy
- OutputFormat
- Context

Canonical JSON and stable hashes covered:

- Schema contracts
- Prompt IR
- Parameters
- Datasets
- Compile jobs
- Artifacts
- Outputs

The design reduced the use of mutable prompt strings.
It also made candidate identity and compatibility checks explicit.

### Effect service layer

The runtime used Context.Tag services and Layers.
Its main services were:

- LmClient
- PolicyRegistry
- BlobStore
- VarSpace
- ReceiptRecorder
- ExecutionBudget
- ToolExecutor

Predict.make(signature) performed this flow:

1. Resolve the active compiled artifact or signature defaults.
2. Render Prompt IR in a deterministic form.
3. Call the selected model.
4. Decode the result with Effect Schema.
5. Use a bounded repair when the first decode fails.
6. Write a success or failure receipt.

The default package adapters were portable and in memory.
The applications supplied durable adapters.

### Parameters and budgets

DseParams included:

- Strategy
- Instruction text
- Few-shot example IDs
- Model and role selection
- Decode policy
- Tool policy
- RLM controller configuration
- Runtime budgets

The budget service supported:

- Maximum elapsed time
- Maximum LM calls
- Maximum tool calls
- Maximum RLM iterations
- Maximum sub-LM calls
- Maximum output characters

RLM-lite failed closed when the job did not set iteration and sub-LM limits.
This rule made the budget a runtime invariant.
It was not only a prompt request.

### Receipts

The receipt kind was openagents.dse.predict_receipt version 1.
A receipt included:

- Signature and compiled artifact IDs
- Strategy ID
- Schema, prompt, parameter, and output hashes
- Model, token use, and timing data
- Render statistics and context pressure
- Repair status
- Budget configuration and use
- RLM trace reference and event count
- Typed success or error data

The receipt design supported replay analysis and policy attribution.
It did not grant promotion authority.

### RLM-lite

RLM-lite became real code on February 9.
It used VarSpace and BlobRef values to keep large text outside the controller context.

The controller could emit one typed action for each iteration:

- Preview
- Search
- Load
- Chunk
- WriteVar
- SubLm
- ExtractOverChunks
- ToolCall
- Final

The runtime:

- Limited controller history
- Limited event count
- Required explicit iteration and sub-LM budgets
- Schema-decoded the Final action
- Stored a canonical openagents.dse.rlm_trace version 1 document
- Accepted and normalized the older trace form
- Rejected unsupported trace versions
- Recorded source spans
- Labeled origin and trust
- Removed raw snippets from carried state

The package also added distilled.search_line_extract.v1.
This strategy made a deterministic line extraction with no LM call.
It could fall back to RLM-lite or direct prediction.

RLM-lite was not upstream Python RLM.
It did not execute arbitrary code.
It used a bounded action language that fit a Worker runtime.

### Evaluation

The evaluation system included:

- Ordered and hashable datasets
- Named train, development, and holdout splits
- Tag filters
- Seeded sampling
- Deterministic metrics
- Pinned judge metrics
- Reward bundles
- Evaluation caching
- Evaluation reports

Reward bundles could combine:

- Format validity
- Task metric
- Tool failure
- Evidence quote quality
- Cost

The application stored labeled examples and reports in Convex.

One limitation is important.
When no split existed, the compiler could use the same data for train and holdout.
When no holdout existed, it could fall back to train.
This behavior weakens independent evaluation.

### Compiler

The compile job kind was openagents.dse.compile_job version 1.
The compiler produced openagents.dse.compiled_artifact version 1.

The artifact included:

- Signature ID
- Compiled ID
- Schema, prompt, and parameter hashes
- Final parameters
- Evaluation summary
- Optimizer metadata
- Provenance data
- Compatibility data

The registry keyed artifacts with signature ID and compiled ID.
The compiled ID was the parameter hash.
It was not a digest of every artifact field.
This choice is a second important limitation.

The terminal source exposed these optimizer IDs:

- instruction_grid.v1
- fewshot_greedy_forward.v1
- joint_instruction_grid_then_fewshot_greedy_forward.v1
- knobs_grid.v1
- knobs_grid_refine.v1

knobs_grid.v1 searched:

- Strategy
- Instruction variants
- Few-shot selections
- RLM controller instructions
- Chunk rules
- Main and sub-model roles
- Budget profiles

The default candidate cap was 128.
The configuration accepted 1 through 500 candidates.
The search included the base candidate.
It removed duplicate JSON candidates.
It used a stable hash tie-break.

knobs_grid_refine.v1 did not implement GEPA.
It applied rule-based changes for:

- Budget failures
- Decode failures
- Missing evidence
- Similar known failure classes

The compiler evaluated candidates in a deterministic bounded search.
It emitted a schema-validated immutable artifact.

### What did not land

The terminal package did not contain:

- MIPROv2
- GEPA
- COPRO
- BootstrapFewShot
- A Pareto optimizer
- A Bayesian search scheduler
- A generic Module graph executor
- Arbitrary Python execution
- Implicit online self-modification

The Module abstraction stayed conceptual.
Application modules were metadata with module ID, description, and signature IDs.
Normal Effect composition supplied the execution flow.

## Application integration

### First Worker integration

apps/autopilot-worker first supplied durable DSE services.
It used Durable Object SQLite for:

- Immutable artifacts
- Active artifact pointers
- Activation history
- Rollback
- Blobs
- Receipts

The Worker used SelectTool as a real hot-path signature.
It also pinned a default artifact.

### Convex web integration

The later apps/web integration became the main production path.
Its DSE schema contained data for:

- Active policies
- Artifacts
- Blobs
- Canaries
- Compile reports
- Evaluation reports
- Labeled examples
- Operator records
- Receipts
- VarSpace

The application provided API routes for:

- Evaluation
- Compilation
- Canary start, stop, and status
- Promotion
- Thread and predict exercises
- Example import
- Receipt and blob reads
- Trace export
- Operator run start, event, and finish

It also provided read-only pages for:

- DSE overview
- Operator runs
- Signatures
- Compile reports
- Evaluation reports

The model stack used OpenRouter as primary.
It used Kimi K2.5 for the main path.
It used Cloudflare Workers AI as fallback.
A later compile default used @cf/openai/gpt-oss-20b.

The policy registry selected a canary with a deterministic bucket.
The bucket used the salt, thread ID, and signature ID.
It fell back to the active artifact when no canary applied.

### Signature catalog

The terminal application catalog contained eight typed signature IDs:

1. @openagents/autopilot/bootstrap/ExtractUserHandle.v1
2. @openagents/autopilot/bootstrap/ExtractAgentName.v1
3. @openagents/autopilot/bootstrap/ExtractAgentVibe.v1
4. @openagents/autopilot/blueprint/SelectTool.v1
5. @openagents/autopilot/feedback/DetectUpgradeRequest.v1
6. @openagents/autopilot/rlm/SummarizeThread.v1
7. @openagents/autopilot/judge/ThreadSummaryQuality.v1
8. @openagents/autopilot/canary/RecapThread.v1

The catalog also exposed BootstrapFlow, BlueprintUpdate, and FeedbackIntake module metadata.

Not every signature was a proven production hot path.
SelectTool, long-context summary, upgrade classification, and canary recap had direct integrations.
Some bootstrap signatures remained catalog entries while the Worker still used state-forced tools.

### Tests

The web application had 19 DSE-named test files and 47 test or it cases.
Other shared tests also had DSE assertions.

The tests covered:

- Artifact activation and rollback
- Compile and evaluation reports
- Canary selection and promotion
- Receipt and blob routes
- VarSpace
- RLM routing and trace display
- Example import
- Operator records
- Production endpoint behavior

Some long-context and browser tests used deterministic or E2E-only stubs.
These tests prove plumbing and policy behavior.
They do not prove general model quality.

## Production compile and promotion record

### Headless loop

The overnight script ran this sequence:

1. Import a SelectTool fixture.
2. Compile a candidate.
3. Ensure that an exercise thread exists.
4. Select a canary salt that puts the thread in the canary bucket.
5. Start a 20 percent canary.
6. Require a holdout result and a minimum delta of zero.
7. Generate 20 requests.
8. Poll for at most three minutes.
9. Require an error rate no greater than 0.2.
10. Optionally run browser verification.
11. Promote the candidate.
12. Stop the canary.

The script tried to clean up a failed canary.
It also wrote operator events and status.

### First failed run

The committed runbook records an initial failure.
Run opsrun_9a1d8b92-a8ad-4af3-83f5-b2e57759eb5f reached a compile timeout after 600 seconds.
A direct request then returned no bytes after 900 seconds.

The fixes added:

- A 15-second OpenRouter timeout
- A request-scoped circuit breaker
- Workers AI fallback
- A dataset hash that included tags
- A seeded baseline control artifact
- Parallel traffic generation
- Evaluation concurrency of four

This failure is useful evidence.
It shows that the first production path had provider and gate-consistency defects.
It also shows the exact repair that unblocked the next run.

### Recorded successful run

Commit e6c650f4050d9774ce9abb549bb717f8018ed106 records an operator smoke run:

- opsrun_smoke_e9780d37b613406cbc113f92f6b7f2ee

Commit cbca11a03758c930d8aea4f4528d860bca088940 records:

- Compile request: dbg_compile_selecttool_prod_4
- Compiled ID: sha256:862f69e8a655c716e8eac0fe22fcfbdcf304702a8c729fa3a91e67cd2a9ee61a
- Dataset hash: sha256:57e3ad2d003681f24eda6cf416da26d3299ea57c6bc324505ad878ce99b5d286
- Full run: opsrun_4796ab2c-1544-4f17-9d88-500d171c454e
- Canary samples: 20
- Recorded error rate: 0
- Control artifact prefix: sha256:414e0b
- Promoted artifact prefix: sha256:862f69

The same record says these checks passed:

- packages/dse Bun tests
- packages/dse TypeScript check
- apps/web lint
- apps/web tests

This is a contemporaneous repository record.
It is not a new external receipt check.
It proves that the narrow SelectTool compile and release mechanism completed as recorded.
It does not prove a broad improvement in agent quality.

## Independent package check on 2026-07-20

This audit extracted the last package tree before deletion into a temporary directory.
It installed the historical frozen Bun dependencies.
It then ran:

    bun test
    bun run typecheck

The environment was:

- Bun 1.3.11
- Effect 3.19.16
- TypeScript 5.9.3

The result was:

- 24 tests passed
- 0 tests failed
- 92 expectations passed
- 13 test files
- TypeScript check passed

The package tests completed in 167 milliseconds.
They covered contract hashes, direct prediction, bounded RLM execution, trace compatibility, budgets, long-context evaluation, compile knobs, refinement, trace export, instruction grid, and few-shot selection.

This result proves that the terminal package remains reproducible in isolation.
It does not validate the deleted web application or the old production services.
The temporary package extraction was not added to the report change.

## Removal and architecture pivots

### First pivot toward DSE

Commit 8d375f2e55631f36ff3fe11f443e1fde4a90b2af removed the Rust codebase on February 11.
It deleted the January Rust DSPy and DSRS implementation.
Commit 26b36fcf5604d40cfa90a4c174972244d3bf4cf9 then archived the remaining Rust DSPy documents.

This sequence matters.
At that point, the repository had selected the Effect DSE line over the Rust line.

### Serving application removal

Commit 388473626c439a804568a461cbbbd21078f99492 deleted apps/web and apps/autopilot-worker on February 17.
This deletion removed:

- Durable DSE adapters
- Convex state
- API routes
- UI pages
- Operator scripts
- Application tests
- The live policy selection path

The package remained.
It no longer had a production consumer in the repository.

### Documentation drift

Commit 41788e00f92191315e28a23c3db4c0ac6d16b6c8 marked the DSE playbook and overnight loop as legacy on February 19.

Commit 3000a05d40563ba32869a4907807fe06c81cc186 then replaced Convex terms with Khala terms in old DSE documents.
This change introduced paths under an apps/web tree that no longer existed.
The retarget was a text rewrite.
It was not evidence of a new DSE runtime.

### Rust-only deletion

Commit 5afa49cdbc1520e753cfce5260328078a9098068 deleted packages/dse on February 20.
The commit title was “chore(rust): delete packages and enforce rust-only architecture mandate.”
Its architecture document required deletion of packages and rejected active TypeScript or Node runtime authority.

The DSE deletion removed:

- 53 files
- 9,678 lines
- The runtime
- The evaluation system
- The compiler
- The RLM-lite kernel
- All package tests

Commit 179355c61061b6d5b83ba48ca6f01016e03b70e4 removed the larger stale DSE and RLM plans on February 21.

Commit 4aff0f82390459a0e7bec7dc9ea1db72e9700bd7 removed the remaining docs/dse contracts on February 24.
Its archive manifest said that these contracts described an older Effect implementation.
It also said that the implementation was not maintained as canonical for the active Rust codebase.

### Second Rust removal

Commit 17aa21b5446239a71581a3bf031531f4b5330268 removed the restored Rust DSPy, RLM, and FRLM stack on February 25.
That prune deleted 222 files and 62,415 lines across the broad Rust surface.

### Causal finding

The following facts are direct:

- The first Rust line was removed while DSE remained.
- The production applications were removed while DSE remained.
- A Rust-only mandate then removed DSE.
- The repository later removed the Rust alternative too.
- No removal commit cites a DSE test failure or quality regression.

The best-supported inference is:

Architecture ownership changed faster than either optimizer line could become durable.
DSE lost its product host and then violated a new language mandate.
The history does not support the statement that DSE failed an optimizer comparison.

## Documents and source drift

DSE documents moved several times.
The path history can look like repeated deletion when a commit only moved files.

| Commit | Actual change |
| --- | --- |
| 974f6a8870262591c32ba542544be71ab2c19c95 | Moved DSE documents into organized autopilot folders |
| 6b83384affb669056e31bfbeae303c9bfcd0f9df | Added a concise docs/dse contract set |
| 41788e00f92191315e28a23c3db4c0ac6d16b6c8 | Marked old application plans as legacy |
| 3000a05d40563ba32869a4907807fe06c81cc186 | Applied an unreliable Convex-to-Khala text retarget |
| 179355c61061b6d5b83ba48ca6f01016e03b70e4 | Purged stale large plans |
| 4aff0f82390459a0e7bec7dc9ea1db72e9700bd7 | Archived the remaining docs/dse contracts |

The concise February 15 contract set still linked to packages/dse.
It specified on-disk policy bundles, manifests, receipts, replay attribution, tools, callbacks, privacy, metrics, and optimizers.
The package deletion made those contracts stale five days later.

## Actual implementation compared with promises

| Capability | Planned | Actual terminal state |
| --- | --- | --- |
| Effect Schema signatures | Yes | Implemented |
| Structured Prompt IR | Yes | Implemented |
| Stable contract and parameter hashes | Yes | Implemented |
| Immutable compiled artifacts | Yes | Implemented |
| Active pointer and rollback | Yes | Implemented in application adapters |
| Receipts and traces | Yes | Implemented |
| Runtime budgets | Yes | Implemented |
| Evaluation cache and reports | Yes | Implemented |
| Grid instruction search | Yes | Implemented |
| Greedy few-shot selection | Yes | Implemented |
| Strategy and budget knob search | Yes | Implemented |
| RLM-lite | Yes | Implemented |
| Distilled deterministic tactic | Yes | Implemented |
| Production canary and promotion | Yes | Implemented and recorded once |
| Generic Module graph runner | Yes | Not implemented |
| Full MIPROv2 | Future | Not implemented |
| Full GEPA | Future | Not implemented |
| Pareto optimizer | Future | Not implemented |
| Bayesian optimizer | Future | Not implemented |
| Arbitrary Python RLM | No | Intentionally not implemented |
| Implicit online self-promotion | No | Intentionally not implemented |

The most important naming rule follows from this table.
The old package was a DSPy-inspired Effect system.
It was not an Effect port of all DSPy algorithms.

## Blueprint lineage and relation to DSE

### One name for different systems

The Git history uses the name Blueprint for different systems.
These systems share some ideas, but they are not one continuous implementation.

| Period | Blueprint meaning | Direct source status | Relation to DSE |
| --- | --- | --- | --- |
| 2026-02-05 to 2026-02-17 | Typed Autopilot identity, user, policy, tools, heartbeat, and memory state | Reachable source | DSE selected and optimized its update tools |
| Before 2026-06-09 | Standalone Rust business and program system | Not present in reachable source | Later documents claim that it included DSPy, GEPA, and RLM concepts |
| From 2026-06-09 | Effect Program kernel for contracts, evidence, action proposals, and release gates | Current source | It can govern optimizer candidates, but it does not run a DSE optimizer |
| 2026-07-09 to 2026-07-10 | Sarah Blueprint fact map and short-lived company-brain UI | Deleted product surface | It modeled facts and provenance, not prompt optimization |
| Current strategy | Retained kernel code and adapters, but deprecated as a target for new product work | Current roadmap, design analysis, and source | New optimizer work needs separate product authority |

This distinction is necessary.
The February system was mainly durable context.
The current system is mainly program governance.
The missing Rust system is the claimed design bridge.
The repository does not contain its source.

### February Blueprint: durable agent state

Commit 239c79f8d6badccd0df5e31a25c89ed86fb594bb first gave the name Blueprint to the Autopilot single-file export.
The change renamed a typed bootstrap export.
It did not add an optimizer or a business ontology.

Commit 5654ee59632ba5b5d112501d0207a40089fe0e7e made that design executable.
It added an Effect Schema model with the format value openagents.autopilot.blueprint and version 1.
The model contained:

- Agent rules
- A bootstrap ritual
- Agent identity
- User details
- Agent character and boundaries
- Tool notes
- A heartbeat checklist
- Daily and long-term memory
- Bootstrap state
- Optional audit data

The Worker stored the state in Durable Object SQLite.
It decoded imports with Effect Schema.
It exported portable JSON.
It rendered a bounded form of the state into the model system prompt.
It also supplied typed tools for identity, user, character, tools, heartbeat, memory, bootstrap, and export changes.

Later commits added the visible product surface:

- e42ff1def8afc14406b3608678c143772eec81bf added the live Blueprint sidebar.
- e267ad6efc1b9dd677b9de43b6340f7dacd68c19 added the sidebar editor.
- fd1ffd7ac708a61682290e1f7195fcc76c19913f added form and raw views.
- 8a9e3e0daa35cc0fe53fe737548426e9982f5e35 stopped an incorrect polling loop.

This first Blueprint was a typed and portable agent profile.
It also acted as a context source for chat.
It did not contain Program Types, Optimizer Runs, Action Submissions, or a business object graph.

### DSE operated on the first Blueprint

DSE and Blueprint were directly connected in the February application.
They were not synonyms.

Commit 544aafa4b79dd6bc409146ab01ca2201305cbed3 added the DSE package.
Commit f54c4fe7031695444c0bff35a501c199e994e586 added the Autopilot DSE catalog.
Commit e2b9e13affd4388ae61492b7a8339fd5dbc5742b put the Blueprint tools behind the DSE router.

The catalog defined the signature:

    @openagents/autopilot/blueprint/SelectTool.v1

The signature classified one user message.
Its output selected no action or exactly one Blueprint tool.
The allowed tools covered identity, user, character, memory, tools, heartbeat, bootstrap completion, and export.

The catalog also defined:

- A BootstrapFlow module that extracted bootstrap data and then used Blueprint tools
- A BlueprintUpdate module that routed a message to one update tool
- Train and holdout examples for Blueprint tool selection
- A structured JSON output contract
- A default compiled artifact for the Blueprint selector

The live Worker used DSE Predict for the selection.
It forced the selected tool call when the DSE result was valid.
It used a fallback when selection failed.
The application tests proved that the selected tool changed durable Blueprint state.
The canary and promotion loop used this selector as its narrow production target.

The relation was:

| Layer | February owner | Function |
| --- | --- | --- |
| Durable context and policy | Blueprint | Stored identity, user, behavior, memory, and tool state |
| Typed decision contract | DSE Signature | Defined the input and output for tool selection |
| Runtime decision | DSE Predict | Selected one allowed Blueprint tool |
| Candidate improvement | DSE compiler | Searched prompt and example variants |
| Admission | DSE evaluation and promotion gate | Compared a candidate and changed the active artifact |
| External state change | Blueprint tool implementation | Validated and stored the selected change |

This is the strongest historical answer to how DSE relates to Blueprint.
DSE was the typed behavior and optimization layer.
Blueprint was the state and tool domain on which that behavior acted.

### February retirement

Commit db57c8176d4c12651ee94fe9d6de9bd8f205cfd2 migrated Convex Blueprint profile and personality data into Laravel Autopilot profile and policy rows.
This migration changed the storage and application boundary.

Commit 388473626c439a804568a461cbbbd21078f99492 then removed apps/web and apps/autopilot-worker.
That change deleted the live Blueprint and DSE integration.
Commit 5afa49cdbc1520e753cfce5260328078a9098068 later deleted the DSE package during the Rust-only pivot.

The first Blueprint did not evolve in place into the current kernel.
The application that owned it was removed.

### The missing standalone Rust Blueprint

Current documents describe a later standalone Rust Blueprint system.
They list:

- Business Object Types and Business Objects
- Object Sets and Views
- Source Authority
- Context Packs
- Access Explanation
- Program Types and Program Signatures
- Module Versions and Program Runs
- Optimizer Runs
- Eval suites and Release Gates
- Action Types and Action Submissions
- Trust and Failure Receipts
- Simulation Branches and Scenario Forks
- A manager, API, worker, and generated SDK

The current June inventory cites source paths under autopilot4-deprecated/blueprint.
The July Palantir analysis also cites docs/palantir-ontology-gap-analysis.md.

Those paths do not exist in any commit that is reachable from the local refs.
An object-name search also found no object for those paths.
The audit therefore cannot inspect the claimed Rust implementation or the claimed formal Palantir mapping.

This missing evidence changes the confidence grade:

- The existence and contents of the February Blueprint are source facts.
- The existence and contents of the current Effect kernel are source facts.
- The detailed Rust Blueprint design is a later committed description.
- The statement that Rust Blueprint was derived from Palantir Ontology is a retrospective claim.
- The claimed formal gap analysis is not independently recoverable from this repository history.

The missing source does not prove that the standalone project did not exist.
It means that this report must not present its detailed behavior as inspected code.

### Reachable Rust program bridge

The reachable history has a separate Rust program substrate outside the missing Blueprint archive.
This source provides a partial design bridge from DSE to the current kernel.
It does not prove the contents of the absent standalone Blueprint system.

- d120c687941f54c9e3459ac97ca3ad762402a4b4 added typed compiled-agent signatures, modules, graphs, evaluation, manifests, and promotion.
- b72d6b6cdfdec46d71f8ea5b91936587611fc0d6 added candidate and promoted hubs, shadow authority, confidence fallback, and traces.
- 5238387587dfd265d455bc1735a07277d0a5c61b added promoted artifact IDs, digests, rollback, and a desktop slice.
- ff9521f226eae98c5e73c945a9407f04cd5ed2e6 documented private Autopilot ownership of durable Program, Action, and Source Authority records.
- f5919c766930d5913d67484660ff670dd92776fd removed the Rust compiled-agent crate when it imported the Effect Blueprint kernel.

This Rust work kept the typed signature, artifact, evaluation, and promotion pattern.
It supports a conceptual DSE-to-Blueprint lineage.
It was not the February DSE package, and this report does not equate it with the absent standalone Blueprint tree.

### June Effect kernel import

Commit f5919c766930d5913d67484660ff670dd92776fd rebuilt the repository as an Effect workspace.
That commit imported 146 Blueprint-named files across the kernel, routes, documents, Pylon, and Probe.
The imported documents have dates from June 5 through June 7.
The Git graph first contains them on June 9.

The inventory made an explicit ownership decision.
The OpenAgents product surface owned the new Effect kernel.
The deprecated Rust system was reference material only.
Rust consumers were to use exported contracts and receipts.
They were not to own the customer-facing authority.

The inventory kept these concepts:

- Source Authority and Context Pack
- Program Type and Program Signature
- Module Version
- Program Run
- Optimizer Run
- Release Gate and eval fixtures
- Action Submission
- Trust and Failure Receipt kinds
- Simulation Branch and Scenario Fork

It deferred or discarded these concepts:

- A full Business Object graph
- Object Sets and Views
- A reusable Action Type registry
- A Blueprint Manager
- A separate Blueprint API and Worker service
- The old generated SDK
- The old Postgres migration shape

The inventory gave one reason for this scope.
It said to avoid a full ontology rebuild before Program Runs and Action Submissions.
It also said not to vendor DSPy.
The target was typed optimizer lineage and release gates, not a new DSPy implementation.

Commit df60c772b40fc7715366452584c80531015a4bb1 renamed the imported Omega inventory and package-boundary documents to OpenAgents.
Commit 059ba3f621abfd01870d06b54a343b3ca033380a applied the current package names.

### June and July kernel expansion

The main kernel then gained focused integrations:

| Commit | Change |
| --- | --- |
| df696eb281cedb606cd7bad7df1d30c6f81ef40a | Added a vertical Context Pack |
| 93b44472a8292e8823953d3d0e0254e4aec0a9c8 | Added delivery-pipeline Programs |
| cc60bff88e19ad86ee6e1675a0c6c14a6dabb333 | Registered delivery Programs |
| 68a8a53cfcabc511fa5723eb289f75f43661b2f3 | Added StudyBench contribution gates |
| 698abde52d2151a2b370d7fa869335e4ae20f2e1 | Added Tassadar module-step bindings |
| 5af4784f67a84b052a366e29094be29f72a31ed5 | Added the Tassadar module registry |
| d1d539583d96812cab5c7254fec93af2c237108b | Added the Blueprint chat Program runtime |
| 0a5d06784c2059526b70e4b0f0f18499415c65bc | Added a proof replay module signature |
| c3274648a647ffdb1eb0b0cfb8fd2f7d84bcb3e0 | Added the narrow shared contract and safety package |
| 8ded7efea744897e901fd67ee6a0bc38fddaca20 | Added Khala candidate wiring |
| 461e0a74a8f53e26648376ac065aa9d95a7ffb99 | Added evidence-only negative GEPA feedback |
| 7cffb9cdd4221e36bc6ecdec9f31d0ef7328c996 | Enforced operator grounding and command gates |
| ed8995301334d4b96d5a4b8d113b2456b8855793 | Enforced five Blueprint action signatures |
| e1fbd1c1858cd8a0ef9d86a18c72ced5ffdb6b4d | Pruned retired Tassadar and Psionic surfaces |
| dbf6ebea5a2f6bbe6df815c3c3bf68f4775323f7 | Updated the Pylon adapter for bounded workspace tools |

The July prune did not delete the Blueprint kernel.
Commit bbccd6ad47338fe919871e8848906438caa7d55c deleted an Autopilot Desktop consumer.
The contracts, routes, safety gates, and Pylon and Probe adapter trees remain.

July also used the name Blueprint for a separate Sarah knowledge surface.
Commit 84dd4f59560245378b00711aab4c7d4edf6fa19e added typed persona and knowledge facts with revision and provenance data.
Commit 9ee02315f53ad904c99f112e7af89faf1031c5ed added a Blueprint map graph.
Commit 13bc1e7443962d72765431d547a93290dd122b50 removed the Sarah surface at owner direction.
The Sarah fact graph is not evidence that the core Program kernel implements a Palantir-like object graph.

### Current implementation surface

At the Blueprint extension cutoff, the active tree contains:

- 75 files and 15,365 lines under apps/openagents.com/workers/api/src/blueprint
- 9 files and 1,061 lines under packages/blueprint-contracts
- 9 Pylon adapter files and 2,567 lines
- 9 Probe adapter files and 2,816 lines
- 26 focused Blueprint records under apps/openagents.com/docs/blueprint
- Six mounted API route families

The six route families are:

- /api/blueprint/program-registry
- /api/blueprint/program-runs
- /api/blueprint/action-submissions
- /api/blueprint/contributions
- /api/blueprint/contracts
- /api/blueprint/tassadar-modules

The shared package is intentionally narrow.
It owns the cross-consumer contract-export security predicate and selected operator gates.
It does not own the complete Blueprint data model.
The Pylon and Probe adapter trees contain broader local contract models.
The two adapter trees now differ, and the Pylon tree received a change on July 19.

This creates a status contradiction that the report must preserve:

- Current design analysis calls the Blueprint kernel deprecated and tells new memory work not to target it.
- The old standalone Blueprint service and broad plugin-ecology direction are historical.
- The Effect kernel and API routes remain mounted source.
- The shared safety package remains used source.
- The Pylon and Probe adapters remain used source.
- The Pylon tree received a current-line change on July 19.
- A broad Blueprint product or company-brain program is not active roadmap work.

“Deprecated” therefore describes product direction and new-work authority.
It does not mean that every Blueprint route, contract, gate, or adapter has been deleted.

### Implemented and contract-only parts

The current Blueprint system is not only documentation.
It also does not implement every named concept as a complete runtime.

| Capability | Current evidence | Status |
| --- | --- | --- |
| Program Type and Signature schemas | Effect Schema source and tests | Implemented contract |
| Module Version lifecycle | Schema, predicates, tests, and fixtures | Implemented contract |
| Program Run evidence boundary | Schema, repository, API, denial service, and tests | Implemented evidence path |
| Action Submission | Schema, proposal repository, API, and approval predicates | Implemented proposal boundary, not a general action engine |
| Source Authority and Context Pack | Schemas, projections, and vertical pack use | Implemented contract and bounded use |
| Release Gate | Schema, predicates, fixtures, and continuation-specific service | Implemented gate logic |
| Program Registry | Fixtures, safe projections, and API | Implemented bounded registry |
| Chat Program runtime | Signature selection, tool menu, Codex and Claude adapters, Program Run output, and tests | Implemented runtime |
| Contract export | Authenticated export route, safety predicate, and tests | Implemented seed export, not a generated OSDK equivalent |
| Optimizer Run | Schema, candidate lineage predicates, and tests | Evidence model only |
| GEPA feedback | Negative candidate feedback service and QA integration | Signal production only |
| Simulation Branch | Schema, isolation predicates, and tests | Contract and fixture layer |
| Full business object graph | Explicitly deferred | Not implemented |
| Object-set query engine | Explicitly deferred | Not implemented |
| General Blueprint Manager | Explicitly discarded for now | Not implemented |
| DSE Prompt IR compiler | No current source | Not implemented |
| GEPA or MIPRO search engine | No current source | Not implemented |

The chat Program runtime is the closest current equivalent to a DSE online runtime.
It selects registered Program Signatures and bounded tools.
It starts Codex or Claude session adapters.
It records a Program Run with an evidence-only authority boundary.
It denies direct deploy, email, spend, and source-mutation authority.

The current registry is a static seed.
Its fixture entries include draft and release-candidate states.
The selector permits draft and active Program Types for bounded dogfood.
There is no general Module Version activation API.
The chat runtime takes an injected session runtime and does not own provider prompt compilation.

The Khala adapter is implemented and tested, but this audit found no non-test production caller.
The Pylon Apple Foundation Models path is a real local tool loop, but it loads a static fixture.
The Pylon HTTP registry client exists, but this audit found no current non-test call path that loads the HTTP registry and posts the Program Run.

The Action Submission model is a separate proposal path.
It requires an approval state, approval evidence, optional dry-run evidence, and no direct Program Run execution.
The current API records proposals.
This does not make Blueprint a general external-action engine.

### Current Blueprint compared with DSE

The current kernel reuses several DSE design ideas.
It changes their purpose and their authority boundary.

| DSE concept | Current Blueprint relation | Important difference |
| --- | --- | --- |
| Signature | BlueprintProgramSignature | Blueprint adds evidence, receipt, tool, risk, and family policy |
| Module | BlueprintProgramType and BlueprintModuleVersion | Blueprint separates behavior contract from implementation artifact |
| Prompt artifact | ModuleVersion artifactRefs and implementationRef | No current structured Prompt IR or complete artifact hash contract |
| Predict runtime | Chat Program runtime | Current runtime governs an injected session adapter and evidence, not a generic typed predictor package |
| Trace and receipt | ProgramRun evidenceRefs and receiptRefs | Program Run is explicitly evidence-only |
| Dataset and metric | Eval fixtures and scorecards | No general current DSE dataset or metric engine |
| Compiler | BlueprintOptimizerRun | Blueprint records candidates but does not search for them |
| Active pointer | Module release state and Release Gate | Promotion needs explicit review, policy, rollback, scorecard, receipts, and decision |
| RLM-lite | Bounded tools and session adapters | No current DSE action interpreter or arbitrary RLM runtime |
| Tool execution | Action Submission boundary | Program output cannot execute a direct effect |
| Canary and rollback | Release Gate and rollback posture | Gate semantics exist, but there is no current DSE compile-to-canary loop |

The current optimizer record accepts these kind values:

- ablation
- gepa_style_reflection
- human_curated
- retained_failure_replay
- scorecard_search

These names describe provenance.
They do not prove that an algorithm ran.
The record stores candidate Module Version refs, failures, scorecards, release gate refs, and evidence.
Its predicate only verifies that candidate output cannot self-promote.

The chat-program-failure-gepa service has the same limit.
It emits a public-safe negative candidate signal.
It does not assemble a candidate.
It does not evaluate a candidate.
It does not promote a candidate.

The Probe GEPA standing loop is a projection and gate over evidence references.
It is not an optimizer algorithm.
The Mutalisk bridge models delegation, progress, and admission.
Its tests deny a Worker dependency on Python DSPy or GEPA.

The correct architectural statement is:

> Current Blueprint is a governance, provenance, evidence, and release spine around DSPy-like programs. It is not a successor implementation of the DSE optimizer.

### A possible composition, not a current product claim

Historical DSE evidence and current Blueprint contracts support one possible composition:

1. A bounded offline optimizer produces a candidate artifact.
2. The system records the artifact as an unpromoted Module Version.
3. Program Runs and eval fixtures produce evidence and scorecards.
4. A Release Gate checks fixtures, review, policy, rollback, receipts, and an explicit decision.
5. An authorized operator promotes the Module Version.
6. The online Effect runtime selects the released version.
7. A Program Run can propose an external effect only through an Action Submission.

This composition can use Python DSPy, a future Effect optimizer, MemoHarness, or another bounded worker.
Blueprint does not decide the optimizer implementation.
The current product specifications describe MemoHarness proof as planned.
They do not claim an implemented experience bank, optimizer, storage migration, or released policy bundle.

This composition is an architectural interpretation.
It is not implementation authority.

## Palantir Ontology comparison

### Official Palantir model

Palantir describes Ontology as an operational layer for enterprise decisions.
Its current architecture has three parts:

1. The Ontology Language defines objects, properties, links, actions, functions, and interfaces.
2. The Ontology Engine supplies queries, subscriptions, transactional writes, and change-data capture.
3. The Ontology Toolchain supplies development, deployment, generated SDK, and operational tools.

Palantir also documents:

- Objects and bidirectional links that represent real-world entities and relations
- Actions and Functions that change Ontology-backed state
- Dynamic security that applies to users and agents
- Generated TypeScript, Python, Java, and OpenAPI SDKs
- Agent templates that select allowed objects, actions, and queries
- Ontology MCP resources that become agent tools
- Scoped credentials that retain user permission limits
- Global Branching for coordinated cross-application changes, review, and merge

These are current product capabilities in Palantir documentation.
They are not claims about OpenAgents implementation.

Palantir AIP Evals is a closer external comparison for the DSE evaluation and candidate-search layer.
Palantir documents eval suites, metrics, comparisons, and experiments across prompt and model configurations.
It also documents simulated Ontology-edit evaluation so that a test does not change real objects.
This evaluation system is adjacent to Ontology.
It is not the Ontology core model itself.

### Three-system comparison

| Dimension | Historical DSE | Current Blueprint | Palantir Ontology |
| --- | --- | --- | --- |
| Primary purpose | Run and improve typed LLM programs | Govern typed agent programs and their evidence, actions, and release | Represent and operate an enterprise digital twin |
| Main modeled unit | Signature, prompt artifact, example, metric, trace | Program Type, Signature, Module Version, Program Run, gate, submission | Object, property, link, action, function, interface |
| Runtime | Effect Predict and bounded RLM-lite | Effect chat Program runtime and bounded adapters | Ontology Engine and application runtimes |
| Search or optimization | Deterministic candidate search | Candidate record only | Not a DSPy prompt optimizer |
| Durable business graph | No | Explicitly deferred | Core capability |
| Query engine | Dataset and application adapters | Bounded repositories and projections | Scalable object and link queries and subscriptions |
| Write model | Application tools outside the DSE core | Approval-gated Action Submission proposal boundary | Ontology Actions, Functions, and transactional edits |
| Promotion | Evaluation, canary, active artifact, rollback | Release Gate, review, policy, receipts, rollback posture, explicit decision | Deployment and branch workflows for Ontology resources and applications |
| Branching | Candidate and policy versions | SimulationBranch and ScenarioFork contracts | Cross-application Global Branching |
| Security | Budgets, trust labels, tool limits, receipts | Evidence-only runs, scoped tools, source and context policy, projections, approval gates | Mandatory and discretionary controls, row and column controls, markings, lineage, and scoped agent access |
| Developer interface | TypeScript package and application adapters | Effect schemas, HTTP routes, export seed, Pylon and Probe adapters | Generated OSDKs, OpenAPI, Ontology MCP, and platform tools |
| Current maturity in this repository | Removed historical implementation | Active bounded kernel with important contract-only areas | External commercial platform |

The similar names are not one-to-one mappings:

- BlueprintProgramType is a behavior policy.
  It is not a Palantir Object Type.
- BlueprintActionSubmission is an approval-gated proposal record.
  It is not a complete Palantir Action Type and transactional execution engine.
- BlueprintSimulationBranch is an isolation contract.
  It is not a complete Global Branching implementation.
- Blueprint contract export is a safe seed.
  It is not a generated OSDK or Ontology MCP system.
- SourceAuthority and ContextPack record source and access evidence.
  They are not a complete dynamic security and object-permission system.
- DSE is a prompt-program runtime.
  It is not an operational digital twin.

Palantir and Blueprint do share an important architectural idea.
Humans and agents should act through typed business or program semantics instead of raw model text.
Both systems also treat security and access as part of the operational model.

Current Blueprint adds a specific OpenAgents rule to its own program layer.
Model or optimizer output is evidence, not authority.
It cannot self-promote.
It cannot directly deploy, send email, spend, or mutate source-backed facts.
External effects remain behind a separate proposal and approval boundary.

This report does not claim that Palantir lacks comparable controls.
It only records the rules that current Blueprint source directly enforces.

### Internal Palantir claims audit

Commit 2d920c77c51586bf1556406eb9146209aa53b950 added the July 7 Palantir institutional-sovereignty analysis.
That document is an owner-directed historical strategy record.
It says that the standalone Blueprint was explicitly derived from Palantir Ontology.
It also says that a formal gap analysis mapped:

- Object Types
- Links
- Actions
- Object Sets
- Functions
- Interfaces
- OSDK
- Application scopes
- Ontology MCP
- Global Branching

The cited gap-analysis file is not in reachable Git history.
The cited standalone Blueprint tree is also absent.
The current June inventory supports part of the concept list, but it does not prove the complete derivation claim.

The July record gives one terminology rule that current product text still uses:

- Ontology is Palantir vocabulary.
- OpenAgents uses Blueprint, company brain, or typed business schema.
- OpenAgents does not call Blueprint “our ontology.”

The same July strategy proposed a small Blueprint company brain with typed objects, properties, links, provenance, and Action Submissions.
That was a recommendation.
It was not proof of a current object graph.

Palantir is not a declared source in the current FASTFOLLOW.md.
This comparison is user-directed external research evidence.
It is not a formal Fast Follow StudyPacket.
It does not grant adoption, implementation, product, release, or public-claim authority.

### Current roadmap boundary

The current master roadmap closes broad Blueprint-as-company-brain maturation unless a new bounded owner decision reopens it.
It also closes named-assistant standing responsibilities and broad role and template growth.
The July 20 MemoHarness analysis also calls the kernel deprecated and says not to use it as the new memory home.
It keeps the evidence-only and release-gate pattern as prior art.

The current boundary is therefore:

- Do not use the retained Blueprint kernel as a target for new product work without a bounded owner decision.
- Preserve or repair retained safety contracts only when an admitted product, compatibility, or security slice requires it.
- Use current Program, evidence, action-proposal, and release-gate paths where an admitted product slice requires them.
- Do not infer authority to build a full object graph, Ontology-like engine, manager, generated SDK, or company-brain product.
- Do not infer authority to restore DSE or to add GEPA from the existence of optimizer schemas.
- Treat historical Fable strategy as evidence, not current sequence or product authority.

### Official external sources

The Palantir comparison used current official product documentation.
The access date for each source was 2026-07-20.

- [Ontology overview](https://www.palantir.com/docs/foundry/ontology/overview)
- [Ontology system architecture](https://www.palantir.com/docs/foundry/architecture-center/ontology-system)
- [Why create an Ontology](https://www.palantir.com/docs/foundry/ontology/why-ontology)
- [Ontology SDK overview](https://www.palantir.com/docs/foundry/ontology-sdk/overview)
- [Object and link types](https://www.palantir.com/docs/foundry/object-link-types/link-types-overview)
- [Agents overview](https://www.palantir.com/docs/foundry/agents/overview)
- [Agent templates](https://www.palantir.com/docs/foundry/agents/agent-templates)
- [Global Branching overview](https://www.palantir.com/docs/foundry/global-branching/overview)
- [Branching Action Types](https://www.palantir.com/docs/foundry/action-types/branching-action-types)
- [Functions and Ontology edits](https://www.palantir.com/docs/foundry/functions/use-functions)
- [Object edits overview](https://www.palantir.com/docs/foundry/object-edits/overview)
- [Security overview](https://www.palantir.com/docs/foundry/security/overview)
- [AIP Evals overview](https://www.palantir.com/docs/foundry/aip-evals/overview)
- [AIP Evals experiments](https://www.palantir.com/docs/foundry/aip-evals/experiments)
- [Ontology edit evals](https://www.palantir.com/docs/foundry/aip-evals/ontology-edits)

### Blueprint reproduction commands

    git log origin/main --reverse --regexp-ignore-case --grep=blueprint
    git log origin/main --reverse -- apps/autopilot-worker/src/blueprint.ts
    git log origin/main --reverse -- apps/autopilot-worker/src/dseCatalog.ts
    git grep -n -i blueprint 8da5c649552419402c1e65cc6ee910815aa0abbf -- apps/autopilot-worker packages/dse
    git log origin/main --reverse -- apps/openagents.com/workers/api/src/blueprint
    git log --all -SBlueprintProgramType
    git rev-list --objects --all | rg -i 'palantir.*ontology|ontology.*palantir|autopilot4-deprecated/blueprint'
    find apps/openagents.com/workers/api/src/blueprint -type f
    rg -n 'api/blueprint' apps/openagents.com/workers/api/src
    rg -n 'Blueprint|ontology|Palantir' docs FASTFOLLOW.md

### June hybrid audit

Commit b76fa980233e5853f1de862ee191900b4556ad73 added the June 28 backend audit.
The audit recommended:

- Use upstream Python DSPy and GEPA for offline candidate generation.
- Treat Python output as untrusted.
- Use Python RLM only as a sandboxed leaf executor.
- Keep online selection and governance in Effect.
- Require evidence and release gates before admission.
- Do not reproduce GEPA, MIPRO, and SIMBA in Effect without a strong reason.

The audit also said that the then-current Effect GEPA runner was a status projection.
It did not perform optimization.

Commit 075ab137798c104b4d06d690e7628ddf5bf55d0b later marked the June audit as historical.
The exact deployment advice in that audit is also stale.
It referenced Cloudflare Workers.
Current repository authority permits Google Cloud production and retires Cloudflare runtime products.

The architectural principle still matches current code:

- Effect owns typed online control and gates.
- Candidate feedback can come from separate work.
- Model output cannot self-promote.
- Current code does not claim that a projection is GEPA execution.

### Current state at the Blueprint extension cutoff

The current tree has:

- No packages/dse
- No active ds-effect package
- No active dsrs package
- No real GEPA algorithm in an Effect runtime
- No real MIPROv2 algorithm in an Effect runtime
- Effect v4 Blueprint contracts and runtime control
- GEPA-related candidate feedback and gate projections
- Six mounted Blueprint API route families
- Active Pylon and Probe Blueprint adapters
- A narrow shared Blueprint contract-security package
- Historical reports and transcripts

The repository also uses a different supported toolchain:

- Node 24
- pnpm
- Vite Plus
- Effect v4
- Google Cloud production

The historical DSE stack used:

- Bun
- Effect 3.19.16
- Convex
- Cloudflare Workers
- Workers AI
- OpenRouter

The old source is useful design and test evidence.
It is not a safe direct cherry-pick into the current system.

## Lessons

### Decisions that worked

The strongest DSE decisions were:

- Stable and versioned signature IDs
- Effect Schema input and output
- Structured Prompt IR
- Deterministic hashes
- Immutable candidates
- Explicit active pointers
- Holdout checks before promotion
- Canary and rollback controls
- Runtime-enforced budgets
- Append-only receipts
- Bounded traces
- Provenance and trust labels
- No arbitrary code in the online RLM path
- No automatic self-promotion

These decisions survive in parts of the current Blueprint and assurance design.

### Decisions that need correction

A future system must correct these DSE limits:

- The compiled ID must cover the complete artifact.
- A missing holdout must fail.
- Train data must not silently become holdout data.
- Optimizer names must match the actual algorithm.
- The evaluator must have an explicit independence rule.
- Each candidate must bind to an immutable dataset revision.
- Cost and budget evidence must remain part of admission.
- Production adapters must follow current Google Cloud authority.
- The implementation must use the current Effect v4 and pnpm toolchain.

These points are recommendations.
They are not claims about current implementation authority.

### Recommended architecture interpretation

Do not restore packages/dse unchanged.
Use it as a design and regression-test source.

If OpenAgents returns to DSPy optimization, the evidence supports this split:

1. Effect owns signatures, Prompt IR, policy identity, runtime budgets, receipts, evidence, canaries, and release gates.
2. An offline optimizer owns expensive candidate search.
3. The optimizer output stays untrusted until an independent evaluation admits it.
4. A sandbox owns any Python or code-executing RLM step.
5. The current product contract decides whether the optimizer is Python DSPy, a new Effect implementation, or another worker.

This split preserves the best DSE work.
It avoids a false claim that Blueprint projections already execute GEPA.

## Canonical commit ledger

### DSE package commits

The following list contains every origin/main commit that touched packages/dse.
Some commits made broad repository changes and only changed package metadata or documents.

| Date | Commit | Subject |
| --- | --- | --- |
| 2026-02-06 | 544aafa4b79dd6bc409146ab01ca2201305cbed3 | Scaffold Effect-native signatures and predict |
| 2026-02-06 | 34a161d79aa0f7cdfa22766d95df0dcb234f5eb9 | Adopt DSE tool contracts |
| 2026-02-06 | f54c4fe7031695444c0bff35a501c199e994e586 | Add DSE catalog |
| 2026-02-06 | 3a96b9f1073126a4a2c4bae6f1b4ecb70d82f4cd | Add Phase 0 production spine |
| 2026-02-06 | 7bfeb8dc76f7aa1d56b1fb8ab0a883d1adf77dc9 | Add Phase 1 evaluation |
| 2026-02-06 | 4dfb8ee80189ab579fe1ce4bd2cf5aae41d1f638 | Add Phase 2 compiler |
| 2026-02-06 | 3b0adeed2af4cd650ec91fdc2e3917d6ffc52362 | Complete MVP slice |
| 2026-02-08 | 6e80c3b7bc6d492135a421d5e005be99e532cfb4 | Add gap review |
| 2026-02-08 | 01f9b6937e18292be6e3a056df956b931f9dd6d8 | Make Rust reference-only |
| 2026-02-08 | 2ac8a75f8466736237e15375e16c4826891d7d70 | Add Effect-only design |
| 2026-02-08 | acba26de2ed2fe94bb6c14d96ac7f4dd45244c9b | Add testable roadmap |
| 2026-02-08 | 5eba24be35e3c9941b5ada55b65e24703b7b0b04 | Add execution budgets |
| 2026-02-08 | 329b76a6ea4e03a79f2cdc868108694add6d4d39 | Align the plan with Convex |
| 2026-02-09 | fe484a4deba2bcfddc3e9d7a4359fd34b8f221d7 | Add context pressure and render stats |
| 2026-02-09 | 8a56bdfed8a8504cffdd954e5b8af1ac8ff03637 | Add strategy dispatch and RLM budgets |
| 2026-02-09 | 94fd87c0fbd7571d908990f2e9940993cf4de3b2 | Add RLM-lite, VarSpace, and trace |
| 2026-02-09 | f76c37ea210b24c1ea21b7e1ac7091dac4279606 | Stabilize the production E2E path |
| 2026-02-09 | f2a3a1c74b4f81a46ec981e7ff0bb3b8ee8b6089 | Integrate RLM-lite with the application |
| 2026-02-09 | 51463bd6d2f0ccd82a5816a476dd94e4c8c1c751 | Add long-context evaluation |
| 2026-02-09 | a0334cf15a62488a867d4c7bb74945be56550738 | Add trace export and distilled strategy |
| 2026-02-09 | 0e3a5aced6be93d2efd81a09b7b8eb900347d8e4 | Add compiler-visible RLM knobs |
| 2026-02-09 | 6ad8c9798cbd95a2838dc12b1e9a30bc93d839b2 | Add provenance and trust controls |
| 2026-02-10 | cbca11a03758c930d8aea4f4528d860bca088940 | Unblock the production overnight loop |
| 2026-02-11 | 5564548d0be1e9e1e7f307f0c9fecdde1050ff8c | Standardize Effect language support |
| 2026-02-11 | ce3f4e44c89b0e62ac1e8cfbb1a7e67ff67e992d | Type and normalize trace events |
| 2026-02-11 | 14d88b7fdcc70231a4c0b5171c2487c2b8ef78bd | Remove unsafe casts |
| 2026-02-11 | 488a124205aed99f261f21970e4a5cb70ef4bd3d | Move synthesis documents |
| 2026-02-11 | cd52cc1fb0d742354aa2b93acd9b5ef0aa5d8ca1 | Add Effect tracing spans |
| 2026-02-11 | 974f6a8870262591c32ba542544be71ab2c19c95 | Organize DSE documents |
| 2026-02-11 | 51b99aa40bad5160b2680c720059cbaaf8edc2b0 | Move log documents |
| 2026-02-19 | 3000a05d40563ba32869a4907807fe06c81cc186 | Retarget old Convex words to Khala |
| 2026-02-20 | 15ee5ae1c94af7db239c6e613bc274c5f7de1ec2 | Broad package and application move |
| 2026-02-20 | 5afa49cdbc1520e753cfce5260328078a9098068 | Delete DSE under the Rust-only mandate |

### Production and operator commits

These commits mostly changed apps/web and operator documents.
They do not all appear in the package-only path log.

| Commit | Result |
| --- | --- |
| 36dd538fe909a7370621da8f7a72dd5023c7916c | Convex artifacts and receipts |
| 1f53e7143ccdb083916b26c54ea871e75bedc4a5 | DSE chat parts |
| 77a966e1db758d4ad14d69ccef7617277c4ca405 | Signature hot path |
| 48dc077bf1596d639bd91e7bf68b0ea48669c29a | Labeled examples |
| 3d25da9c603bcf2578c10b6a2b4f64998d926dbd | Compile API and reports |
| e5cdf2d77b6e4ce8c444717bffd4a9ec36caff99 | Promotion and canary |
| 7abcdf30c596da79876784cd08de578ca07b554f | Operator playbook |
| 16bc4cf3f6b3ecfd4815d899b11aa6d876f673dd | Canary recap and debug UI |
| 84a17de7b854a2e8e58cab4f6b4dc5a486c0b4e6 | Operator authentication and records |
| e5e608a19f0fd4e30d2ecf23a7769ec71f5da8fe | Headless overnight runner |
| 3c68791f984071ba810ac06128a8e00b65fb1378 | SelectTool fixture and import |
| 7f58e0a244175d0bc4530e989be56bce43feac92 | Shared compile job specification |
| ac9575278d79dd5ebfcf07864b8e98f603c23045 | Automated canary monitoring |
| f3313b85d06e14ed5cc7ac766445d0a2126172af | Read-only operator pages |
| 868f494538be8256105a5dad4a2a71d197c4be4f | Judge rewards and evaluation reports |
| 84811f31256ac8bfcbe5c0c3df2ed412a493b5e1 | Trace mining |
| 37f8a987683dda8a2ecf084291386a431ff3f0c7 | RLM compile knobs |
| e6c650f4050d9774ce9abb549bb717f8018ed106 | Production operator smoke record |
| cbca11a03758c930d8aea4f4528d860bca088940 | Successful compile and promotion record |

## Reproduction commands

The following commands reproduce the main repository findings.
Run them from a clone that has all relevant refs.

### Inventory

    git rev-list --all --count
    git for-each-ref --format='%(refname)' | wc -l
    git log --all --regexp-ignore-case --grep='dspy\|dse\|ds-effect\|dsrs\|rlm\|gepa\|mipro'
    git rev-list --objects --all | rg '(^|/)(dspy|dse|ds-effect|dsrs)(/|\.|$)'

### Name history

    git show beebeb672e72ebf56b750526757bf7f24382e1e2:docs/autopilot/ds-effect.md
    git show 544aafa4b79dd6bc409146ab01ca2201305cbed3:docs/autopilot/dse.md
    git log --follow -- docs/autopilot/dse.md

### Terminal package

    git ls-tree -r --name-only 5afa49cdbc1520e753cfce5260328078a9098068^ packages/dse
    git grep -n -i -E 'mipro|gepa|copro|teleprompt|optimizerId' 5afa49cdbc1520e753cfce5260328078a9098068^ -- packages/dse
    git show 5afa49cdbc1520e753cfce5260328078a9098068 -- packages/dse

### Production wiring

    git grep -n '/api/dse/' cbca11a03758c930d8aea4f4528d860bca088940 -- apps/web
    git grep -n 'dse' cbca11a03758c930d8aea4f4528d860bca088940 -- apps/web/convex
    git show cbca11a03758c930d8aea4f4528d860bca088940:docs/autopilot/OVERNIGHT_SELF_IMPROVEMENT_PLAN.md

The last path can move in adjacent commits.
Use git ls-tree when the direct path is absent.

### Removal

    git show --stat 388473626c439a804568a461cbbbd21078f99492
    git show --stat 5afa49cdbc1520e753cfce5260328078a9098068
    git show 5afa49cdbc1520e753cfce5260328078a9098068:docs/ARCHITECTURE-RUST.md
    git show 4aff0f82390459a0e7bec7dc9ea1db72e9700bd7
    git show --stat 17aa21b5446239a71581a3bf031531f4b5330268

### Later decision

    git show b76fa980233e5853f1de862ee191900b4556ad73:docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md
    rg -n 'dspy|dse|gepa|mipro|rlm' apps packages docs

## Final assessment

DSE was the project’s real DSPy-in-Effect attempt.
It advanced from a three-commit specification to a production-wired Effect package in five days.
It had unusually strong runtime controls for a prompt optimization prototype.
The best parts were typed contracts, deterministic artifacts, enforced budgets, receipts, and explicit release gates.

The implementation stopped below full DSPy optimizer parity.
It never shipped MIPROv2 or GEPA.
Its compiler was a bounded search system.
Its RLM was a safe action interpreter.
Its recorded production win proved the release loop on a narrow task.
It did not prove broad agent-quality improvement.

The project removed DSE because the product host and language mandate changed.
The repository then removed the replacement Rust stack too.
This history is a warning against coupling optimizer architecture to a short-lived application or repository language mandate.

The first Blueprint and DSE did have a direct production relation.
Blueprint stored typed agent context and exposed mutation tools.
DSE selected, evaluated, compiled, canaried, and promoted the policy that chose those tools.
The current Blueprint kernel is a different system.
It retains typed Program, evidence, action-proposal, and release concepts, but it does not retain the DSE compiler or optimizer runtime.

Current Blueprint is also not a Palantir Ontology equivalent.
It deliberately deferred the business object graph, object-set query engine, generated domain SDK, and operational write engine.
Its main retained value is program governance and evidence discipline.
Current design analysis treats it as deprecated prior art for new product work even though mounted routes, safety gates, and adapters remain in source.

The old source should remain historical evidence.
A future implementation should preserve the contract and governance ideas.
It should use the current Effect v4, Node 24, pnpm, Vite Plus, and Google Cloud contract.
It should use honest optimizer names and an independent holdout.
It should keep candidate generation separate from admission and release.
