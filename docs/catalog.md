# Document catalog

A complete path inventory of Markdown documents under `docs/`, excluding
`docs/transcripts/`.
Roles identify how to read a document; they are not implementation or deployment
status. Dated reports and designs can describe work that is partial, superseded,
negative, or deferred. Use their scope statements and linked current guides.

Start with the [documentation index](README.md), [master roadmap](roadmap.md),
[glossary](glossary.md), or [maintenance rules](documentation.md). The separate
[transcript index](transcripts/README.md) owns the video archive.

This catalog lists 450 documents plus itself as of September 26, 2026.
It retains historical and evidence paths rather than copying their content.

Crate-owned documentation sits outside this inventory and its count. The
[framework index](../crates/rust-native/README.md) owns the generic UI contract.
The [Coder adoption index](coder/rust-native/README.md) owns the application
architecture, source reviews, styling direction, and migration plan; those
files are included below.

## Top-level navigation and reference

| Document | Role | Topic |
| --- | --- | --- |
| [README.md](README.md) | Index | Documentation |
| [coder-earn.md](coder-earn.md) | Design / assessment | Earn in the coder repository |
| [coderbench.md](coderbench.md) | Reference | CoderBench, and the first recorded episode |
| [delegation-brief.md](delegation-brief.md) | Design / assessment | Delegation brief |
| [dependencies.md](dependencies.md) | Reference | Dependency review |
| [documentation.md](documentation.md) | Reference | Documentation maintenance |
| [glossary.md](glossary.md) | Reference | Glossary |
| [gym.md](gym.md) | Reference | Gym documentation |
| [programs.md](programs.md) | Reference | Capabilities, executors, and programs |
| [psionic-and-pylon.md](psionic-and-pylon.md) | Design / assessment | Psionic and Pylon |
| [roadmap.md](roadmap.md) | Design / assessment | OpenAgents master roadmap |
| [text-optimization.md](text-optimization.md) | Design / assessment | Text optimization here: nine programs, one experiment, no wins |
| [verification.md](verification.md) | Reference | Rust verification |

## agents

| Document | Role | Topic |
| --- | --- | --- |
| [agents/README.md](agents/README.md) | Index | General agent infrastructure |
| [agents/agents.md](agents/agents.md) | Reference | The decision API for agents |
| [agents/auth.md](agents/auth.md) | Reference | Authenticate to the decision API |
| [agents/market-infrastructure.md](agents/market-infrastructure.md) | Reference | Agent labor and market infrastructure |
| [agents/roadmap.md](agents/roadmap.md) | Design / assessment | General agent architecture: remaining work |
| [agents/skills.md](agents/skills.md) | Reference | Call the decision API from an agent |

## audits

| Document | Role | Topic |
| --- | --- | --- |
| [audits/2026-09-19-codebase-audit/README.md](audits/2026-09-19-codebase-audit/README.md) | Evidence index | Workspace audit: September 19, 2026 |
| [audits/2026-09-19-codebase-audit/issues.md](audits/2026-09-19-codebase-audit/issues.md) | Retained evidence / audit | Open issue map |
| [audits/2026-09-19-codebase-audit/remediation.md](audits/2026-09-19-codebase-audit/remediation.md) | Retained evidence / audit | Remediation register |
| [audits/2026-09-19-codebase-audit/verification.md](audits/2026-09-19-codebase-audit/verification.md) | Retained evidence / audit | Verification record |
| [audits/2026-09-20-delegation-after-action-report/README.md](audits/2026-09-20-delegation-after-action-report/README.md) | Evidence index | After-action report: stalled issue burndown and missing task history |
| [audits/2026-09-21-project-completion-failure/README.md](audits/2026-09-21-project-completion-failure/README.md) | Evidence index | Audit of project completion failure |
| [audits/2026-09-22-jev-opportunities/README.md](audits/2026-09-22-jev-opportunities/README.md) | Evidence index | What cheaper semantic decisions could change in OpenAgents |
| [audits/README.md](audits/README.md) | Evidence index | Engineering audits |

## coder

| Document | Role | Topic |
| --- | --- | --- |
| [coder/README.md](coder/README.md) | Index | Coder documentation |
| [coder/beat-fable-showcase.md](coder/beat-fable-showcase.md) | Reference | Where Microcoder beats Fable 5.1 low, and what that doesn't show yet |
| [coder/beat-fable-together.md](coder/beat-fable-together.md) | Reference | Beating Fable together: the plan after Episode 288 |
| [coder/design/2026-09-21-project-roadmap-snapshot.md](coder/design/2026-09-21-project-roadmap-snapshot.md) | Design / assessment | Decision Router and Coder project snapshot |
| [coder/design/2026-09-24-assessment.md](coder/design/2026-09-24-assessment.md) | Design / assessment | Where Coder stands: assessment, 2026-09-24 |
| [coder/design/2026-09-25-assessment.md](coder/design/2026-09-25-assessment.md) | Design / assessment | Where Coder stands: assessment, 2026-09-25 |
| [coder/design/2026-09-25-morning-assessment.md](coder/design/2026-09-25-morning-assessment.md) | Design / assessment | Where Coder stands: morning, 2026-09-25 |
| [coder/design/README.md](coder/design/README.md) | Index | Coder design |
| [coder/design/coder-as-decision-router-consumer.md](coder/design/coder-as-decision-router-consumer.md) | Design / assessment | Coder as a decision service consumer |
| [coder/design/coder-suite-migration.md](coder/design/coder-suite-migration.md) | Design / assessment | Coder suite migration: gaps and delivery roadmap |
| [coder/design/coder-terminal-v05-algorithm-and-goldens.md](coder/design/coder-terminal-v05-algorithm-and-goldens.md) | Design / assessment | Coder Terminal v0.5: algorithm, benchmark tasks, and golden traces |
| [coder/design/decision-function-inventory.md](coder/design/decision-function-inventory.md) | Design / assessment | Coder decision-function inventory |
| [coder/design/knowledge-base.md](coder/design/knowledge-base.md) | Design / assessment | Shared knowledge base |
| [coder/design/luna-pivot.md](coder/design/luna-pivot.md) | Design / assessment | The Luna pivot: Jev structure around a cheap executor |
| [coder/design/microluna-parallel.md](coder/design/microluna-parallel.md) | Design / assessment | Microluna v7: parallel sessions and a contract that decides the task |
| [coder/design/microluna-v18.md](coder/design/microluna-v18.md) | Design / assessment | Microluna v18: the executed briefing, and what characterizes a run |
| [coder/design/microluna-v8.md](coder/design/microluna-v8.md) | Design / assessment | Microluna v8: guards that give way, faster suites, and general guidance |
| [coder/design/microluna.md](coder/design/microluna.md) | Design / assessment | Microluna: how to reach Luna on a logged-in Codex session |
| [coder/design/networked-coder-plan.md](coder/design/networked-coder-plan.md) | Design / assessment | A measured path to the best coding agent |
| [coder/design/pattern-components.md](coder/design/pattern-components.md) | Design / assessment | Patterns as components: tuning Coder without fitting wording |
| [coder/design/prompt-audit.md](coder/design/prompt-audit.md) | Design / assessment | Prompt audit against the determinism thesis |
| [coder/design/rebuild-plan.md](coder/design/rebuild-plan.md) | Design / assessment | Coder shared runtime and product-suite plan |
| [coder/design/relay-backend-plan.md](coder/design/relay-backend-plan.md) | Design / assessment | Relay backend plan: coder on relay.openagents.com |
| [coder/design/rust-mobile-feasibility.md](coder/design/rust-mobile-feasibility.md) | Design / assessment | Rust-first Coder client feasibility |
| [coder/design/service-spec.md](coder/design/service-spec.md) | Design / assessment | Coder service: Nostr auth, free usage, and deployment |
| [coder/design/teardown-nostr-integration.md](coder/design/teardown-nostr-integration.md) | Design / assessment | Incorporating the teardown archive into Coder through Nostr |
| [coder/design/thesis.md](coder/design/thesis.md) | Design / assessment | The determinism thesis |
| [coder/design/thoughts-on-a-typesafe-coding-agent.md](coder/design/thoughts-on-a-typesafe-coding-agent.md) | Design / assessment | [public] thoughts on a typesafe coding agent |
| [coder/design/typesafe-agent-analysis.md](coder/design/typesafe-agent-analysis.md) | Design / assessment | A TypeSafe-native Coder |
| [coder/design/typesafe-agent-protocol-addendum.md](coder/design/typesafe-agent-protocol-addendum.md) | Design / assessment | TypeSafe agent: protocol, host, and client responsibilities |
| [coder/design/typesafe-agent-roadmap.md](coder/design/typesafe-agent-roadmap.md) | Design / assessment | Roadmap for a TypeSafe-native Coder |
| [coder/design/typesafe-product-suite.md](coder/design/typesafe-product-suite.md) | Design / assessment | Applying the TypeSafe design across the Coder suite |
| [coder/design/x402-lightning-nostr-integration.md](coder/design/x402-lightning-nostr-integration.md) | Design / assessment | Integrating x402 Lightning with Nostr and Coder |
| [coder/guides/README.md](coder/guides/README.md) | Index | Coder guides |
| [coder/guides/artifact-verification.md](coder/guides/artifact-verification.md) | Guide / contract | Verify a retained artifact |
| [coder/guides/coder-one-ask.md](coder/guides/coder-one-ask.md) | Guide / contract | Ask Coder One about runs |
| [coder/guides/coder-one-checks.md](coder/guides/coder-one-checks.md) | Guide / contract | Check claimed behavior with admitted scenarios |
| [coder/guides/coder-one-components.md](coder/guides/coder-one-components.md) | Guide / contract | Run a Coder One component alone |
| [coder/guides/coder-one-issue-eval.md](coder/guides/coder-one-issue-eval.md) | Guide / contract | Measure an issue-flow change on past issues |
| [coder/guides/coder-one-minitasks.md](coder/guides/coder-one-minitasks.md) | Guide / contract | Run Coder One on a mini-task |
| [coder/guides/coder-one-repair.md](coder/guides/coder-one-repair.md) | Guide / contract | Repair once from a diagnostic packet |
| [coder/guides/coder-one-support.md](coder/guides/coder-one-support.md) | Guide / contract | Judge requirement support with paired Jev questions |
| [coder/guides/coder-one-tunable.md](coder/guides/coder-one-tunable.md) | Guide / contract | Run the tunable composition on Terminal-Bench |
| [coder/guides/contribute-knowledge.md](coder/guides/contribute-knowledge.md) | Guide / contract | Contribute knowledge and take a quest |
| [coder/guides/decision-profiles.md](coder/guides/decision-profiles.md) | Guide / contract | Configure Coder's decision client |
| [coder/guides/devin-delegation-runbook.md](coder/guides/devin-delegation-runbook.md) | Guide / contract | Delegate work to Devin with Coder |
| [coder/guides/fire-loop.md](coder/guides/fire-loop.md) | Guide / contract | Run the fire loop |
| [coder/guides/headless.md](coder/guides/headless.md) | Guide / contract | Headless mode |
| [coder/guides/install.md](coder/guides/install.md) | Guide / contract | Installing Coder |
| [coder/guides/knowledge-base.md](coder/guides/knowledge-base.md) | Guide / contract | Use the shared knowledge base |
| [coder/guides/microcoder.md](coder/guides/microcoder.md) | Guide / contract | Microcoder |
| [coder/guides/mobile-readonly.md](coder/guides/mobile-readonly.md) | Guide / contract | Read saved Codex and Claude chats on iPhone |
| [coder/guides/program-authority.md](coder/guides/program-authority.md) | Guide / contract | Program authority |
| [coder/guides/project-supervision.md](coder/guides/project-supervision.md) | Guide / contract | Capacity-aware project supervision |
| [coder/guides/tasks.md](coder/guides/tasks.md) | Guide / contract | Durable local tasks |
| [coder/guides/tracker-intake.md](coder/guides/tracker-intake.md) | Guide / contract | Scoped tracker intake |
| [coder/guides/worker-executor.md](coder/guides/worker-executor.md) | Guide / contract | Running coder-worker on a local executor |
| [coder/guides/xp.md](coder/guides/xp.md) | Guide / contract | Referee quests and read the XP ledger |
| [coder/measurements/2026-09-20-observed-fanout.md](coder/measurements/2026-09-20-observed-fanout.md) | Retained evidence / audit | The observed six-Devin episode |
| [coder/measurements/2026-09-21-evidence-select.md](coder/measurements/2026-09-21-evidence-select.md) | Retained evidence / audit | Evidence selection on a held-out suite |
| [coder/measurements/2026-09-21-independence-v2-eval.md](coder/measurements/2026-09-21-independence-v2-eval.md) | Retained evidence / audit | Independence v1 against v2 on twelve task lists |
| [coder/measurements/2026-09-21-review-policy-eval.md](coder/measurements/2026-09-21-review-policy-eval.md) | Retained evidence / audit | Review policy on a labeled panel |
| [coder/measurements/2026-09-21-wave-vs-refill.md](coder/measurements/2026-09-21-wave-vs-refill.md) | Retained evidence / audit | Fixed waves against completion-driven refill and separate resource lanes |
| [coder/measurements/2026-09-22-consumer-conformance.md](coder/measurements/2026-09-22-consumer-conformance.md) | Retained evidence / audit | The consumer conformance suite and the deterministic baseline |
| [coder/measurements/2026-09-22-evidence-select-dev-threshold.md](coder/measurements/2026-09-22-evidence-select-dev-threshold.md) | Retained evidence / audit | Abstention-floor selection for evidence relevance, and a failed held-out confirmation |
| [coder/measurements/2026-09-22-review-finding-baseline.md](coder/measurements/2026-09-22-review-finding-baseline.md) | Retained evidence / audit | Wording baseline for the per-finding review question |
| [coder/measurements/2026-09-22-review-policy-selection.md](coder/measurements/2026-09-22-review-policy-selection.md) | Retained evidence / audit | Review policy selection on development and held-out halves |
| [coder/measurements/2026-09-22-terminal-bench-chat-fit.md](coder/measurements/2026-09-22-terminal-bench-chat-fit.md) | Retained evidence / audit | Terminal-Bench tasks that resemble recent Coder work |
| [coder/measurements/2026-09-22-terminal-bench-trace-analysis.md](coder/measurements/2026-09-22-terminal-bench-trace-analysis.md) | Retained evidence / audit | Terminal-Bench trace analysis: what three agents actually do |
| [coder/measurements/2026-09-23-delegate-vs-gemini.md](coder/measurements/2026-09-23-delegate-vs-gemini.md) | Retained evidence / audit | The delegate door against the Gemini fallback |
| [coder/measurements/2026-09-24-microluna-terminal.md](coder/measurements/2026-09-24-microluna-terminal.md) | Retained evidence / audit | Microluna on the eight delegate prompts |
| [coder/measurements/2026-09-25-issue-flow-policies.md](coder/measurements/2026-09-25-issue-flow-policies.md) | Retained evidence / audit | Issue-flow policy comparison: retain requirements |
| [coder/measurements/README.md](coder/measurements/README.md) | Evidence index | Coder measurements |
| [coder/measurements/relay-transport.md](coder/measurements/relay-transport.md) | Retained evidence / audit | The relay transport, under a recorded episode |
| [coder/migration-status.md](coder/migration-status.md) | Reference | Coder suite migration: implementation tracker |
| [coder/runtime/README.md](coder/runtime/README.md) | Index | Coder runtime |
| [coder/runtime/delegate-door.md](coder/runtime/delegate-door.md) | Guide / contract | The delegate door |
| [coder/runtime/delegate.md](coder/runtime/delegate.md) | Guide / contract | Delegation |
| [coder/runtime/free-labor.md](coder/runtime/free-labor.md) | Guide / contract | Recoverable free coding orders |
| [coder/runtime/frozen-task-context.md](coder/runtime/frozen-task-context.md) | Guide / contract | Frozen task knowledge and check lineage |
| [coder/runtime/knowledge-bundles.md](coder/runtime/knowledge-bundles.md) | Guide / contract | Pinned knowledge snapshots and private delivery |
| [coder/runtime/knowledge-evidence.md](coder/runtime/knowledge-evidence.md) | Guide / contract | Knowledge evidence intake |
| [coder/runtime/knowledge-studies.md](coder/runtime/knowledge-studies.md) | Guide / contract | Frozen knowledge comparisons |
| [coder/runtime/microcoder-repository.md](coder/runtime/microcoder-repository.md) | Guide / contract | Microcoder repository adapter |
| [coder/runtime/nostr-task-control.md](coder/runtime/nostr-task-control.md) | Guide / contract | Scoped Nostr task control |
| [coder/runtime/portable-host.md](coder/runtime/portable-host.md) | Guide / contract | Portable host bundle infrastructure |
| [coder/runtime/repository-evidence.md](coder/runtime/repository-evidence.md) | Guide / contract | Repository source references |
| [coder/runtime/shell-loop.md](coder/runtime/shell-loop.md) | Guide / contract | The shell loop |
| [coder/runtime/subprocesses.md](coder/runtime/subprocesses.md) | Guide / contract | Subprocess supervision |
| [coder/runtime/task-owner.md](coder/runtime/task-owner.md) | Guide / contract | Local task ownership and evidence |
| [coder/runtime/terminal.md](coder/runtime/terminal.md) | Guide / contract | The terminal's lifecycle, lanes, and scrollback |
| [coder/runtime/traces.md](coder/runtime/traces.md) | Guide / contract | Traces |
| [coder/terminal-bench-contract.md](coder/terminal-bench-contract.md) | Reference | The v0.5 headless episode contract |
| [coder/terminal-bench.md](coder/terminal-bench.md) | Reference | Terminal-Bench comparisons |
| [coder/thoughts-on-a-typesafe-coding-agent/README.md](coder/thoughts-on-a-typesafe-coding-agent/README.md) | Index | Source and verification notes |
| [coder/rust-native/README.md](coder/rust-native/README.md) | Index | Rust Native adoption in Coder |
| [coder/rust-native/adoption.md](coder/rust-native/adoption.md) | Design / assessment | Adopting Rust Native in Coder |
| [coder/rust-native/architecture.md](coder/rust-native/architecture.md) | Design / assessment | Coder shared UI architecture |
| [coder/rust-native/build-order.md](coder/rust-native/build-order.md) | Design / assessment | Rust Native build order |
| [coder/rust-native/references.md](coder/rust-native/references.md) | Design / assessment | Reference architecture review |
| [coder/rust-native/styling-design.md](coder/rust-native/styling-design.md) | Design / assessment | Coder styling design with Rust Native |
| [coder/verification/2026-09-20-devin-runbook.md](coder/verification/2026-09-20-devin-runbook.md) | Retained evidence / audit | Workstation validation of the Devin delegation runbook |
| [coder/verification/2026-09-20-execution-boundary.md](coder/verification/2026-09-20-execution-boundary.md) | Retained evidence / audit | Execution boundary and workspace snapshots |
| [coder/verification/2026-09-20-terminal-lifecycle.md](coder/verification/2026-09-20-terminal-lifecycle.md) | Retained evidence / audit | Terminal lifecycle smoke test, 2026-09-20 |
| [coder/verification/2026-09-20-worktree-lifecycle.md](coder/verification/2026-09-20-worktree-lifecycle.md) | Retained evidence / audit | Worktree lifecycle regression |
| [coder/verification/2026-09-21-project-supervisor.md](coder/verification/2026-09-21-project-supervisor.md) | Retained evidence / audit | Project supervisor verification, 2026-09-21 |
| [coder/verification/2026-09-22-relay-interoperability.md](coder/verification/2026-09-22-relay-interoperability.md) | Retained evidence / audit | Relay, worker, and program interoperability |
| [coder/verification/2026-09-25-issue-eval-read-isolation.md](coder/verification/2026-09-25-issue-eval-read-isolation.md) | Retained evidence / audit | Confine issue-evaluation reads |
| [coder/verification/2026-09-26-free-labor/README.md](coder/verification/2026-09-26-free-labor/README.md) | Evidence index | Free labor acceptance evidence |
| [coder/verification/2026-09-26-frozen-context/README.md](coder/verification/2026-09-26-frozen-context/README.md) | Evidence index | Frozen context and shared transport acceptance |
| [coder/verification/2026-09-26-knowledge/README.md](coder/verification/2026-09-26-knowledge/README.md) | Evidence index | Knowledge integrity and permissioned input verification |
| [coder/verification/2026-09-26-mobile-reader.md](coder/verification/2026-09-26-mobile-reader.md) | Retained evidence / audit | Coder iOS reader verification — September 26, 2026 |
| [coder/verification/2026-09-26-verse-mobile.md](coder/verification/2026-09-26-verse-mobile.md) | Verification | Shared Verse desktop/iOS delivery |
| [coder/verification/2026-09-26-verse-gym.md](coder/verification/2026-09-26-verse-gym.md) | Verification | Verse Gym building and scoped observation |
| [coder/verification/2026-09-26-world-pairing.md](coder/verification/2026-09-26-world-pairing.md) | Verification | Verse home and QR pairing delivery |
| [coder/verification/2026-09-26-portable-host/README.md](coder/verification/2026-09-26-portable-host/README.md) | Evidence index | Portable host packaging evidence |
| [coder/verification/2026-09-26-repository-adapter/README.md](coder/verification/2026-09-26-repository-adapter/README.md) | Evidence index | Repository adapter acceptance, 2026-09-26 |
| [coder/verification/2026-09-26-task-inbox.md](coder/verification/2026-09-26-task-inbox.md) | Retained evidence / audit | Durable local task inbox verification |
| [coder/verification/2026-09-26-task-owner.md](coder/verification/2026-09-26-task-owner.md) | Retained evidence / audit | Local task ownership and evidence verification |
| [coder/verification/gates.md](coder/verification/gates.md) | Retained evidence / audit | Verification gates and what they cost |
| [coder/verification/2026-09-26-task-control/README.md](coder/verification/2026-09-26-task-control/README.md) | Evidence index | Scoped task control: code verification |

## decision-models

| Document | Role | Topic |
| --- | --- | --- |
| [decision-models/2026-09-20-state-budget-choice/choice-state-tables.md](decision-models/2026-09-20-state-budget-choice/choice-state-tables.md) | Reference | choice-state-tables |
| [decision-models/README.md](decision-models/README.md) | Index | Decision models |
| [decision-models/api/README.md](decision-models/api/README.md) | Index | Decision API contracts |
| [decision-models/api/classification-http.md](decision-models/api/classification-http.md) | Guide / contract | Classify through the gateway |
| [decision-models/api/decision-api.md](decision-models/api/decision-api.md) | Guide / contract | OpenAgents Decision API |
| [decision-models/api/relay-decision-contract.md](decision-models/api/relay-decision-contract.md) | Guide / contract | The relay decision contract |
| [decision-models/examples/README.md](decision-models/examples/README.md) | Index | Caller examples |
| [decision-models/fixtures/README.md](decision-models/fixtures/README.md) | Index | Contract fixtures |
| [decision-models/fixtures/classify-v1/README.md](decision-models/fixtures/classify-v1/README.md) | Index | Classification v1 fixtures |
| [decision-models/guides/README.md](decision-models/guides/README.md) | Index | Decision model guides |
| [decision-models/guides/caller.md](decision-models/guides/caller.md) | Guide / contract | Calling the decision API |
| [decision-models/guides/choosing.md](decision-models/guides/choosing.md) | Guide / contract | Choosing a decision model |
| [decision-models/guides/classification-callers.md](decision-models/guides/classification-callers.md) | Guide / contract | Classification callers |
| [decision-models/guides/clients.md](decision-models/guides/clients.md) | Guide / contract | Client packages |
| [decision-models/guides/mcp-documentation.md](decision-models/guides/mcp-documentation.md) | Guide / contract | MCP documentation tools |
| [decision-models/guides/mcp-server.md](decision-models/guides/mcp-server.md) | Guide / contract | MCP server |
| [decision-models/jev/knowledge-base.md](decision-models/jev/knowledge-base.md) | Reference | Jev and TypeSafe: internal knowledge base |
| [decision-models/jev/rust-sdk.md](decision-models/jev/rust-sdk.md) | Reference | Jev Rust SDK design |
| [decision-models/measurements/2026-09-19-coder-turns.md](decision-models/measurements/2026-09-19-coder-turns.md) | Retained evidence / audit | Scoring the workload that actually exists |
| [decision-models/measurements/2026-09-19-frozen-embedding-baseline.md](decision-models/measurements/2026-09-19-frozen-embedding-baseline.md) | Retained evidence / audit | The cheap baseline, measured |
| [decision-models/measurements/2026-09-19-program-selection.md](decision-models/measurements/2026-09-19-program-selection.md) | Retained evidence / audit | Asking every turn whether it is a program request |
| [decision-models/measurements/2026-09-19-restatement-and-polarity.md](decision-models/measurements/2026-09-19-restatement-and-polarity.md) | Retained evidence / audit | Restatement, polarity, and the independence gate |
| [decision-models/measurements/2026-09-19-score-ordinality.md](decision-models/measurements/2026-09-19-score-ordinality.md) | Retained evidence / audit | Does a Score's ordering mean anything? |
| [decision-models/measurements/2026-09-20-coder-question-baselines.md](decision-models/measurements/2026-09-20-coder-question-baselines.md) | Retained evidence / audit | Each production question against the constant that would replace it |
| [decision-models/measurements/2026-09-20-compiled-functions.md](decision-models/measurements/2026-09-20-compiled-functions.md) | Retained evidence / audit | What 98 labels bought, against a zero-label compile |
| [decision-models/measurements/2026-09-20-frozen-embedding-door.md](decision-models/measurements/2026-09-20-frozen-embedding-door.md) | Retained evidence / audit | The frozen-embedding baseline, scored through the store |
| [decision-models/measurements/2026-09-20-instrument-validity.md](decision-models/measurements/2026-09-20-instrument-validity.md) | Retained evidence / audit | Validating the instrument |
| [decision-models/measurements/2026-09-20-lev-domain-gap.md](decision-models/measurements/2026-09-20-lev-domain-gap.md) | Retained evidence / audit | Lev adapters outside the support domain |
| [decision-models/measurements/2026-09-20-program-selection-local-doors.md](decision-models/measurements/2026-09-20-program-selection-local-doors.md) | Retained evidence / audit | Program selection through a local door |
| [decision-models/measurements/2026-09-20-program-selection-v2.md](decision-models/measurements/2026-09-20-program-selection-v2.md) | Retained evidence / audit | Current program-selection baseline |
| [decision-models/measurements/2026-09-20-risk-respecification.md](decision-models/measurements/2026-09-20-risk-respecification.md) | Retained evidence / audit | Three rewordings of risk, scored against the constant they would have to beat |
| [decision-models/measurements/2026-09-20-score-contract.md](decision-models/measurements/2026-09-20-score-contract.md) | Retained evidence / audit | Score positions, selected answers, and calibration |
| [decision-models/measurements/2026-09-20-state-budget-choice.md](decision-models/measurements/2026-09-20-state-budget-choice.md) | Retained evidence / audit | Choice adapter state-budget sweep |
| [decision-models/measurements/2026-09-20-state-budget.md](decision-models/measurements/2026-09-20-state-budget.md) | Retained evidence / audit | A state budget for the smallest door |
| [decision-models/measurements/2026-09-23-program-selection-v3.md](decision-models/measurements/2026-09-23-program-selection-v3.md) | Retained evidence / audit | Program selection with review-runs |
| [decision-models/measurements/2026-09-24-tenant-training-demo.md](decision-models/measurements/2026-09-24-tenant-training-demo.md) | Retained evidence / audit | The tenant-training rehearsal, 2026-09-24 |
| [decision-models/measurements/README.md](decision-models/measurements/README.md) | Evidence index | Decision model measurements |
| [decision-models/others/2026-09-19-laya.md](decision-models/others/2026-09-19-laya.md) | Reference | Laya, and what to do about it |
| [decision-models/others/2026-09-20-jevbench.md](decision-models/others/2026-09-20-jevbench.md) | Reference | JevBench, and the public tier vendored from it |
| [decision-models/others/laya-adapter-prerequisites.md](decision-models/others/laya-adapter-prerequisites.md) | Reference | Laya adapter prerequisites |
| [decision-models/research/2026-09-19-capability-sockets.md](decision-models/research/2026-09-19-capability-sockets.md) | Design / assessment | Is a compiled function just another plugin? |
| [decision-models/research/2026-09-19-compiled-functions.md](decision-models/research/2026-09-19-compiled-functions.md) | Design / assessment | Compiled functions: settled enough to act on |
| [decision-models/research/2026-09-19-inference-side-scoring.md](decision-models/research/2026-09-19-inference-side-scoring.md) | Design / assessment | Inference-side scoring: settled, and not the way the claim says |
| [decision-models/research/2026-09-19-question-text-optimization.md](decision-models/research/2026-09-19-question-text-optimization.md) | Design / assessment | Is the question text a tunable parameter? |
| [decision-models/research/2026-09-19-specialist-classifiers.md](decision-models/research/2026-09-19-specialist-classifiers.md) | Design / assessment | Specialist classifiers: settled, including against our own number |
| [decision-models/research/README.md](decision-models/research/README.md) | Index | Research |
| [decision-models/research/abstention.md](decision-models/research/abstention.md) | Design / assessment | Abstention: should a model be able to say it does not know? |
| [decision-models/service/README.md](decision-models/service/README.md) | Index | Decision API service |
| [decision-models/service/batch-execution.md](decision-models/service/batch-execution.md) | Guide / contract | Bounded batch execution for classify |
| [decision-models/service/billing-terms.md](decision-models/service/billing-terms.md) | Guide / contract | Purchase terms for billed plans |
| [decision-models/service/billing.md](decision-models/service/billing.md) | Guide / contract | Plans, checkout, and entitlements |
| [decision-models/service/candidate-admission.md](decision-models/service/candidate-admission.md) | Guide / contract | Candidate admission |
| [decision-models/service/compatibility.md](decision-models/service/compatibility.md) | Guide / contract | Compatibility and deprecation |
| [decision-models/service/confidential-inference.md](decision-models/service/confidential-inference.md) | Guide / contract | Confidential hosted inference |
| [decision-models/service/decision-advertise.md](decision-models/service/decision-advertise.md) | Guide / contract | Decision service discovery |
| [decision-models/service/decision-worker.md](decision-models/service/decision-worker.md) | Guide / contract | The decision worker |
| [decision-models/service/deployment.md](decision-models/service/deployment.md) | Guide / contract | Deploying the gateway |
| [decision-models/service/durable-jobs.md](decision-models/service/durable-jobs.md) | Guide / contract | Durable classification jobs |
| [decision-models/service/gateway.md](decision-models/service/gateway.md) | Guide / contract | The gateway |
| [decision-models/service/image-decisions.md](decision-models/service/image-decisions.md) | Guide / contract | Image-capable decisions |
| [decision-models/service/monetary-accounting.md](decision-models/service/monetary-accounting.md) | Guide / contract | Monetary accounting |
| [decision-models/service/monetary-ledger.md](decision-models/service/monetary-ledger.md) | Guide / contract | Monetary ledger |
| [decision-models/service/operations.md](decision-models/service/operations.md) | Guide / contract | Operating policies |
| [decision-models/service/playground.md](decision-models/service/playground.md) | Guide / contract | Playground and chat demo |
| [decision-models/service/privacy.md](decision-models/service/privacy.md) | Guide / contract | Privacy and retention |
| [decision-models/service/recipes.md](decision-models/service/recipes.md) | Guide / contract | Recipe library |
| [decision-models/service/review-policies.md](decision-models/service/review-policies.md) | Guide / contract | Review and fallback policies for classification |
| [decision-models/service/sdk-language-decision.md](decision-models/service/sdk-language-decision.md) | Guide / contract | SDK language boundary decision |
| [decision-models/service/skill-directory.md](decision-models/service/skill-directory.md) | Guide / contract | The skill directory |
| [decision-models/service/tenant-training.md](decision-models/service/tenant-training.md) | Guide / contract | Tenant training |
| [decision-models/service/usage-dashboard.md](decision-models/service/usage-dashboard.md) | Guide / contract | Usage, activity, and the dashboard |
| [decision-models/service/workspace-membership.md](decision-models/service/workspace-membership.md) | Guide / contract | Workspace membership |

## deployment

| Document | Role | Topic |
| --- | --- | --- |
| [deployment/README.md](deployment/README.md) | Index | Deployment and host operations |
| [deployment/configuration.md](deployment/configuration.md) | Operations | Relay configuration contract |
| [deployment/database.md](deployment/database.md) | Operations | Postgres store and roles |
| [deployment/import-jsonl.md](deployment/import-jsonl.md) | Operations | Signed-event JSONL import |
| [deployment/runbook-cloud-run.md](deployment/runbook-cloud-run.md) | Operations | Runbook: production relay on Cloud Run |
| [deployment/runbook-debian-vps.md](deployment/runbook-debian-vps.md) | Operations | Debian VPS relay deployment |
| [deployment/runbook-local-dev.md](deployment/runbook-local-dev.md) | Operations | Local relay development |

## extensions

| Document | Role | Topic |
| --- | --- | --- |
| [extensions/README.md](extensions/README.md) | Index | OpenAgents programs and extensions |
| [extensions/architecture.md](extensions/architecture.md) | Reference | Architecture and terminology |
| [extensions/delivery.md](extensions/delivery.md) | Reference | Delivery and evaluation |
| [extensions/opportunities.md](extensions/opportunities.md) | Reference | TypeSafe opportunities |
| [extensions/packages.md](extensions/packages.md) | Reference | Packages and distribution |
| [extensions/plugins.md](extensions/plugins.md) | Reference | Wasm plugins |
| [extensions/programs.md](extensions/programs.md) | Reference | Programs and decisions |

## game

| Document | Role | Topic |
| --- | --- | --- |
| [game/README.md](game/README.md) | Index | Games, MMORPGs, and 3D worlds in OpenAgents |

## gym

| Document | Role | Topic |
| --- | --- | --- |
| [gym/README.md](gym/README.md) | Index | Gym |
| [gym/deployment-from-store.md](gym/deployment-from-store.md) | Reference | Read deployment verdicts from stored passes |
| [gym/gate-digests.md](gym/gate-digests.md) | Reference | Gate digests |
| [gym/head-to-head.md](gym/head-to-head.md) | Reference | Replay traces head to head |
| [gym/ledger.md](gym/ledger.md) | Reference | The locked-partition ledger |
| [gym/measured-records.md](gym/measured-records.md) | Reference | Measured records for a caller's labels |
| [gym/measurements/2026-09-19-calibration-and-the-argmax.md](gym/measurements/2026-09-19-calibration-and-the-argmax.md) | Retained evidence / audit | Calibration and the argmax |
| [gym/measurements/2026-09-19-deployment-ranking.md](gym/measurements/2026-09-19-deployment-ranking.md) | Retained evidence / audit | The four-way table, re-read with time and money as criteria |
| [gym/measurements/2026-09-19-latency-noise-floor.md](gym/measurements/2026-09-19-latency-noise-floor.md) | Retained evidence / audit | Latency's noise floor, and why this machine could not measure it |
| [gym/measurements/2026-09-19-question-text-routing.md](gym/measurements/2026-09-19-question-text-routing.md) | Retained evidence / audit | Rewording the routing question: the tenth attempt, which ran |
| [gym/measurements/2026-09-19-reproducing-the-week.md](gym/measurements/2026-09-19-reproducing-the-week.md) | Retained evidence / audit | Reproducing the week's claims from rows |
| [gym/measurements/2026-09-20-frozen-embedding-door.md](gym/measurements/2026-09-20-frozen-embedding-door.md) | Retained evidence / audit | A door that refuses two primitives, through the store |
| [gym/measurements/2026-09-20-hosted-jev-quiet-latency.md](gym/measurements/2026-09-20-hosted-jev-quiet-latency.md) | Retained evidence / audit | Hosted Jev on a quiet machine: latency, and a second pass on the same items |
| [gym/measurements/2026-09-20-kev-4b-metal-pass.md](gym/measurements/2026-09-20-kev-4b-metal-pass.md) | Retained evidence / audit | Kev 4B: one retained Metal evaluation pass |
| [gym/measurements/2026-09-20-kev-quiet-latency.md](gym/measurements/2026-09-20-kev-quiet-latency.md) | Retained evidence / audit | Kev on a quiet machine: the latency floor, a p95 per door, and the ranking with time in it |
| [gym/measurements/2026-09-20-raw-floors-and-mapped-claims.md](gym/measurements/2026-09-20-raw-floors-and-mapped-claims.md) | Retained evidence / audit | The raw floors and mapped claims, re-derived |
| [gym/measurements/2026-09-20-relocking-support-v2.md](gym/measurements/2026-09-20-relocking-support-v2.md) | Retained evidence / audit | Re-locking support-v2 from the items no adapter saw |
| [gym/measurements/2026-09-24-caller-pilot/outcome.md](gym/measurements/2026-09-24-caller-pilot/outcome.md) | Retained evidence / audit | Caller pilot: outcome |
| [gym/measurements/2026-09-24-caller-pilot/plan.md](gym/measurements/2026-09-24-caller-pilot/plan.md) | Retained evidence / audit | Caller pilot: frozen evaluation plan |
| [gym/measurements/2026-09-24-caller-pilot/report.md](gym/measurements/2026-09-24-caller-pilot/report.md) | Retained evidence / audit | caller-acme-returns-v1 — the measured record |
| [gym/measurements/2026-09-24-file-viewer/README.md](gym/measurements/2026-09-24-file-viewer/README.md) | Evidence index | Retained-file viewer verification |
| [gym/measurements/README.md](gym/measurements/README.md) | Evidence index | Gym measurements |
| [gym/model-identity.md](gym/model-identity.md) | Reference | Model content identity |
| [gym/published-snapshots.md](gym/published-snapshots.md) | Reference | Published snapshots: the public benchmark and status view |
| [gym/regression.md](gym/regression.md) | Reference | Catching a door that regresses against itself |
| [gym/retained-files.md](gym/retained-files.md) | Reference | Inspect files from a transcript |
| [gym/run-analysis.md](gym/run-analysis.md) | Reference | Analyze a run when it ends |
| [gym/run-card.md](gym/run-card.md) | Reference | Characterize a run with its card |
| [gym/terminal-bench-cli.md](gym/terminal-bench-cli.md) | Reference | Use Terminal-Bench from the Gym CLI |
| [gym/terminal-bench-tui.md](gym/terminal-bench-tui.md) | Reference | View Terminal-Bench runs in the Gym terminal |
| [gym/terminal-bench.md](gym/terminal-bench.md) | Reference | Testing coder against terminal-bench, through the Gym |

## history

| Document | Role | Topic |
| --- | --- | --- |
| [history/2026-09-25-transcript-roadmap.md](history/2026-09-25-transcript-roadmap.md) | Historical | Roadmap: from the transcript archive to one plan |
| [history/2026-09-26-retired-gym-migration.md](history/2026-09-26-retired-gym-migration.md) | Historical | The Gym |
| [history/README.md](history/README.md) | Historical | Historical surveys |

## kev

| Document | Role | Topic |
| --- | --- | --- |
| [kev/2026-09-20-upstream-review.md](kev/2026-09-20-upstream-review.md) | Reference | Kev release review and integration priorities |
| [kev/README.md](kev/README.md) | Index | Kev |
| [kev/architecture.md](kev/architecture.md) | Reference | Kev architecture |
| [kev/artifacts.md](kev/artifacts.md) | Reference | Reproducible Kev artifacts |
| [kev/jev-comparison.md](kev/jev-comparison.md) | Reference | Kev vs Jev, measured side by side |
| [kev/jev-unmasked.md](kev/jev-unmasked.md) | Reference | Jev's Architecture Unmasked, digested |
| [kev/measurements/2026-09-19-variant-scores.md](kev/measurements/2026-09-19-variant-scores.md) | Retained evidence / audit | Every kev checkpoint on one suite |
| [kev/measurements/2026-09-20-candidate-4b.md](kev/measurements/2026-09-20-candidate-4b.md) | Retained evidence / audit | Replacement 4B evaluation |
| [kev/measurements/2026-09-20-merge-precision.md](kev/measurements/2026-09-20-merge-precision.md) | Retained evidence / audit | LoRA merge precision and loading memory |
| [kev/measurements/2026-09-20-metal-attention.md](kev/measurements/2026-09-20-metal-attention.md) | Retained evidence / audit | Metal attention and sequence padding |
| [kev/measurements/2026-09-20-per-variant-admission.md](kev/measurements/2026-09-20-per-variant-admission.md) | Retained evidence / audit | Per-variant forward admission: the estimate beside a real forward |
| [kev/measurements/2026-09-20-program-selection-latency.md](kev/measurements/2026-09-20-program-selection-latency.md) | Retained evidence / audit | Program selection latency, per Kev checkpoint |
| [kev/measurements/2026-09-20-real-weights-and-domain-gap.md](kev/measurements/2026-09-20-real-weights-and-domain-gap.md) | Retained evidence / audit | Two checkpoints with real weights: the gated suite, and our suite against public labels |
| [kev/measurements/2026-09-20-state-reuse.md](kev/measurements/2026-09-20-state-reuse.md) | Retained evidence / audit | Exact-state reuse and the cache decision |
| [kev/measurements/2026-09-20-training-decision.md](kev/measurements/2026-09-20-training-decision.md) | Retained evidence / audit | Coder-specific training decision |
| [kev/measurements/2026-09-22-serving-reconciliation.md](kev/measurements/2026-09-22-serving-reconciliation.md) | Retained evidence / audit | Serving records reconciliation — 2026-09-22 |
| [kev/measurements/README.md](kev/measurements/README.md) | Evidence index | Kev measurement records |
| [kev/mesh-plan.md](kev/mesh-plan.md) | Reference | Decision models on the mesh |
| [kev/model-cards.md](kev/model-cards.md) | Reference | Kev model cards and measurements |
| [kev/port-roadmap.md](kev/port-roadmap.md) | Design / assessment | Kev port roadmap |

## laya

| Document | Role | Topic |
| --- | --- | --- |
| [laya/README.md](laya/README.md) | Index | Laya |
| [laya/conformance.md](laya/conformance.md) | Reference | Laya conformance |
| [laya/measurements/2026-09-22-port-baseline.md](laya/measurements/2026-09-22-port-baseline.md) | Retained evidence / audit | Laya port baseline |
| [laya/measurements/README.md](laya/measurements/README.md) | Evidence index | Laya measurement records |

## lev

| Document | Role | Topic |
| --- | --- | --- |
| [lev/README.md](lev/README.md) | Index | Lev |
| [lev/apple-fm-surface.md](lev/apple-fm-surface.md) | Reference | The Apple FM surface, as this workspace already knows it |
| [lev/architecture.md](lev/architecture.md) | Reference | Lev architecture |
| [lev/calibration.md](lev/calibration.md) | Reference | Calibrating Lev |
| [lev/disposition.md](lev/disposition.md) | Reference | Lev: what it is good for, and how to make it better |
| [lev/helper-build.md](lev/helper-build.md) | Reference | Build the Lev helper from source |
| [lev/improvement-strategy.md](lev/improvement-strategy.md) | Reference | How to make Lev better |
| [lev/manifest.md](lev/manifest.md) | Reference | The decision model manifest |
| [lev/measurements/2026-09-19-adapter-band.md](lev/measurements/2026-09-19-adapter-band.md) | Retained evidence / audit | The band adapter |
| [lev/measurements/2026-09-19-adapter-v1.md](lev/measurements/2026-09-19-adapter-v1.md) | Retained evidence / audit | Suite scores: support-v2 |
| [lev/measurements/2026-09-19-behavior.md](lev/measurements/2026-09-19-behavior.md) | Retained evidence / audit | Apple FM behavior record |
| [lev/measurements/2026-09-19-calibration-variance.md](lev/measurements/2026-09-19-calibration-variance.md) | Retained evidence / audit | The spread the calibration metrics carry across seed blocks |
| [lev/measurements/2026-09-19-comparison.md](lev/measurements/2026-09-19-comparison.md) | Retained evidence / audit | One contract, three implementations |
| [lev/measurements/2026-09-19-flip-rate-variance.md](lev/measurements/2026-09-19-flip-rate-variance.md) | Retained evidence / audit | The spread a flip rate carries across option orders |
| [lev/measurements/2026-09-19-seed-variance.md](lev/measurements/2026-09-19-seed-variance.md) | Retained evidence / audit | The spread an L2 estimate carries across seed blocks |
| [lev/measurements/2026-09-19-suite-scores.md](lev/measurements/2026-09-19-suite-scores.md) | Retained evidence / audit | Suite scores: support-v1 |
| [lev/measurements/2026-09-19-suite-v2-scores.md](lev/measurements/2026-09-19-suite-v2-scores.md) | Retained evidence / audit | Suite scores: support-v2 |
| [lev/measurements/2026-09-19-three-way-first-rows.md](lev/measurements/2026-09-19-three-way-first-rows.md) | Retained evidence / audit | The first run that left a record |
| [lev/measurements/2026-09-20-admission-live.md](lev/measurements/2026-09-20-admission-live.md) | Retained evidence / audit | Admission on Apple's live runtime |
| [lev/measurements/2026-09-20-apple-serving-matrix.md](lev/measurements/2026-09-20-apple-serving-matrix.md) | Retained evidence / audit | Apple serving verification: partial matrix |
| [lev/measurements/2026-09-20-final-workspace-gate.md](lev/measurements/2026-09-20-final-workspace-gate.md) | Retained evidence / audit | Apple workspace gate: feature-enabled Coder tests failed |
| [lev/measurements/README.md](lev/measurements/README.md) | Evidence index | Lev measurement records |
| [lev/mesh-plan.md](lev/mesh-plan.md) | Reference | Lev on the mesh |
| [lev/revocation.md](lev/revocation.md) | Reference | Revocation |
| [lev/roadmap.md](lev/roadmap.md) | Design / assessment | Lev roadmap |

## maintenance

| Document | Role | Topic |
| --- | --- | --- |
| [maintenance/2026-09-26-documentation-review.md](maintenance/2026-09-26-documentation-review.md) | Reference | Documentation review, September 26, 2026 |

## minecraft

| Document | Role | Topic |
| --- | --- | --- |
| [minecraft/README.md](minecraft/README.md) | Index | Minecraft guilds |
| [minecraft/agents-and-learning.md](minecraft/agents-and-learning.md) | Reference | Agent decisions and learning |
| [minecraft/architecture.md](minecraft/architecture.md) | Reference | Runtime and authority |
| [minecraft/arena-operations.md](minecraft/arena-operations.md) | Reference | Operate the arena episode |
| [minecraft/coding-quests.md](minecraft/coding-quests.md) | Reference | Coding quests and world changes |
| [minecraft/delivery-and-sources.md](minecraft/delivery-and-sources.md) | Reference | Delivery, decisions, and sources |
| [minecraft/demo-2026-09-22.md](minecraft/demo-2026-09-22.md) | Reference | September 22 demo plan |
| [minecraft/economy.md](minecraft/economy.md) | Reference | Compute credits and reputation |
| [minecraft/evaluation.md](minecraft/evaluation.md) | Reference | Evaluation and throughput |
| [minecraft/experience.md](minecraft/experience.md) | Reference | The guild foundry experience |
| [minecraft/protocols.md](minecraft/protocols.md) | Reference | Minecraft protocol profile |
| [minecraft/voyager-runbook.md](minecraft/voyager-runbook.md) | Reference | Watch a Voyager episode live |

## nostr

| Document | Role | Topic |
| --- | --- | --- |
| [nostr/README.md](nostr/README.md) | Index | Nostr implementation notes |
| [nostr/crypto-primitives.md](nostr/crypto-primitives.md) | Reference | Nostr cryptographic primitives |

## optimization

| Document | Role | Topic |
| --- | --- | --- |
| [optimization/README.md](optimization/README.md) | Index | Programming and improving agent systems |
| [optimization/architecture.md](optimization/architecture.md) | Design / assessment | Architecture and opportunity map |
| [optimization/coder-components.md](optimization/coder-components.md) | Design / assessment | Coder as a tunable system |
| [optimization/concepts.md](optimization/concepts.md) | Design / assessment | The talk, DSPy, and GEPA |
| [optimization/experiments.md](optimization/experiments.md) | Design / assessment | Experiments, Gym, and promotion |
| [optimization/proposed-issues.md](optimization/proposed-issues.md) | Design / assessment | Proposed issues for full integration |
| [optimization/sources.md](optimization/sources.md) | Design / assessment | Conceptual references |

## protocol

| Document | Role | Topic |
| --- | --- | --- |
| [protocol/2026-09-26-nip-implementation-coverage.md](protocol/2026-09-26-nip-implementation-coverage.md) | Protocol support / assessment | NIP implementation coverage after the September 26 sync |
| [protocol/2026-09-26-openagents-gap-review.md](protocol/2026-09-26-openagents-gap-review.md) | Protocol support / assessment | OpenAgents NIP coverage review |
| [protocol/2026-09-26-teardown-coverage.md](protocol/2026-09-26-teardown-coverage.md) | Protocol support / assessment | Archived teardowns: Nostr coverage review |
| [protocol/2026-09-26-upstream-nip-sync-block.md](protocol/2026-09-26-upstream-nip-sync-block.md) | Protocol support / assessment | Block NIP implementation review, September 26, 2026 |
| [protocol/2026-09-26-upstream-nip-sync-official.md](protocol/2026-09-26-upstream-nip-sync-official.md) | Protocol support / assessment | Official NIP implementation review, September 26, 2026 |
| [protocol/2026-09-26-upstream-nip-sync.md](protocol/2026-09-26-upstream-nip-sync.md) | Protocol support / assessment | Upstream NIP sync and implementation assessment |
| [protocol/README.md](protocol/README.md) | Index | Nostr protocol implementation |
| [protocol/block-nip-ledger.md](protocol/block-nip-ledger.md) | Protocol support / assessment | Block NIP fixture ledger |
| [protocol/block-nips.md](protocol/block-nips.md) | Protocol support / assessment | Block NIP implementation status |
| [protocol/implementation-plan.md](protocol/implementation-plan.md) | Protocol support / assessment | Nostr protocol implementation plan |
| [protocol/media.md](protocol/media.md) | Protocol support / assessment | M7 Media Contract |
| [protocol/nip-expansion.md](protocol/nip-expansion.md) | Protocol support / assessment | M6 Protocol Expansion |
| [protocol/official-nip-ledger.md](protocol/official-nip-ledger.md) | Protocol support / assessment | Official NIP ledger |
| [protocol/openagents-retention.md](protocol/openagents-retention.md) | Protocol support / assessment | OpenAgents relay retention |
| [protocol/verification/2026-09-26-nips/README.md](protocol/verification/2026-09-26-nips/README.md) | Evidence index | NIP implementation verification, September 26, 2026 |

## terminal-bench

| Document | Role | Topic |
| --- | --- | --- |
| [terminal-bench/2026-09-26-config-sweep.md](terminal-bench/2026-09-26-config-sweep.md) | Retained evidence / assessment | Microcoder configuration sweep for Round 3 |
| [terminal-bench/2026-09-26-kb-154-check.md](terminal-bench/2026-09-26-kb-154-check.md) | Retained evidence / assessment | Did the Round 3 knowledge entries hurt Microcoder? Check blocked by Jev credit |
| [terminal-bench/2026-09-26-round2-loop.md](terminal-bench/2026-09-26-round2-loop.md) | Retained evidence / assessment | Round 2 loop work: ending a green run, and models to run it on |
| [terminal-bench/2026-09-26-tb21-oos-study.md](terminal-bench/2026-09-26-tb21-oos-study.md) | Retained evidence / assessment | Terminal-Bench 2.1 out-of-sample study, knowledge off (pre-registration) |
| [terminal-bench/2026-09-26-tb21-sweep.md](terminal-bench/2026-09-26-tb21-sweep.md) | Retained evidence / assessment | Terminal-Bench 2.1 configuration sweep: step cap and model |
| [terminal-bench/tb21-dev-set.md](terminal-bench/tb21-dev-set.md) | Retained evidence / assessment | Terminal-Bench 2.1 development set |
| [terminal-bench/2026-09-22-luna-jevprobe-upgrade.md](terminal-bench/2026-09-22-luna-jevprobe-upgrade.md) | Dated report / protocol | Coder One upgrade: give Luna evidence for every requirement |
| [terminal-bench/2026-09-22-pack-study.md](terminal-bench/2026-09-22-pack-study.md) | Dated report / protocol | The first study: evidence.pack parameters |
| [terminal-bench/2026-09-22-routing.md](terminal-bench/2026-09-22-routing.md) | Dated report / protocol | Route each task: outcome matrix and first router |
| [terminal-bench/2026-09-22-run-analyses.md](terminal-bench/2026-09-22-run-analyses.md) | Dated report / protocol | Coder One run analyses, 2026-09-22 |
| [terminal-bench/2026-09-23-check-recall.md](terminal-bench/2026-09-23-check-recall.md) | Dated report / protocol | Check recall on retained TB4 trials |
| [terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md](terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md) | Dated report / protocol | Coder One against Claude Code on Terminal-Bench 4.0 |
| [terminal-bench/2026-09-23-matched-controller-targeted.md](terminal-bench/2026-09-23-matched-controller-targeted.md) | Dated report / protocol | Coder One's controller against the same executor on 10 TB4 tasks |
| [terminal-bench/2026-09-23-matched-opus-controller.md](terminal-bench/2026-09-23-matched-opus-controller.md) | Dated report / protocol | Matched Opus comparison: Coder controller on and off |
| [terminal-bench/2026-09-23-persist-v8.md](terminal-bench/2026-09-23-persist-v8.md) | Dated report / protocol | Tunable v8: persistence that stops, and runs cheaper |
| [terminal-bench/2026-09-23-task-win-analysis.md](terminal-bench/2026-09-23-task-win-analysis.md) | Dated report / protocol | What the TB4 winning and failing traces actually show |
| [terminal-bench/2026-09-23-tb4-failure-analysis.md](terminal-bench/2026-09-23-tb4-failure-analysis.md) | Dated report / protocol | Terminal-Bench 4.0 failure analysis, 2026-09-23 |
| [terminal-bench/2026-09-23-tunable-results.md](terminal-bench/2026-09-23-tunable-results.md) | Dated report / protocol | Tunable Coder One development results, 2026-09-23 |
| [terminal-bench/2026-09-23-what-we-have-learned.md](terminal-bench/2026-09-23-what-we-have-learned.md) | Dated report / protocol | Terminal-Bench 4.0: what the runs so far show |
| [terminal-bench/2026-09-24-acceptance-first.md](terminal-bench/2026-09-24-acceptance-first.md) | Dated report / protocol | Acceptance first: does a green frozen suite predict a pass? |
| [terminal-bench/2026-09-24-best-of-n-luna.md](terminal-bench/2026-09-24-best-of-n-luna.md) | Dated report / protocol | Best-of-N Luna, selected by the combined verdict |
| [terminal-bench/2026-09-24-effort-routing.md](terminal-bench/2026-09-24-effort-routing.md) | Dated report / protocol | Per-task effort on six TB4 tasks: routed against fixed medium and fixed xhigh |
| [terminal-bench/2026-09-24-escalation-on-failed-check.md](terminal-bench/2026-09-24-escalation-on-failed-check.md) | Dated report / protocol | Escalation to GPT-6 Astra on a failed check |
| [terminal-bench/2026-09-24-issue-review.md](terminal-bench/2026-09-24-issue-review.md) | Dated report / protocol | Terminal-Bench issue review and experiment safeguards |
| [terminal-bench/2026-09-24-luna-tb4-baseline.md](terminal-bench/2026-09-24-luna-tb4-baseline.md) | Dated report / protocol | GPT-6 Luna on 14 TB4 tasks, direct and with Jev structure |
| [terminal-bench/2026-09-24-microluna-candidate-evidence.md](terminal-bench/2026-09-24-microluna-candidate-evidence.md) | Dated report / protocol | Microluna candidate evidence: the review can rescue or ruin a solution |
| [terminal-bench/2026-09-24-microluna-iteration-speed.md](terminal-bench/2026-09-24-microluna-iteration-speed.md) | Dated report / protocol | Faster, observable Microluna iterations |
| [terminal-bench/2026-09-24-microluna-iterations.md](terminal-bench/2026-09-24-microluna-iterations.md) | Dated report / protocol | Microluna iterations: v9 and on, dev and test sets |
| [terminal-bench/2026-09-24-microluna-overnight.md](terminal-bench/2026-09-24-microluna-overnight.md) | Dated report / protocol | Microluna overnight: the determinism thesis on TB4 |
| [terminal-bench/2026-09-24-microluna-two-targets.md](terminal-bench/2026-09-24-microluna-two-targets.md) | Dated report / protocol | Microluna: preserve good work, then measure two kinds of win |
| [terminal-bench/2026-09-24-microluna-v6-embedding-definitive.md](terminal-bench/2026-09-24-microluna-v6-embedding-definitive.md) | Dated report / protocol | Microluna v6 on embedding-drift-monitor: definitive analysis |
| [terminal-bench/2026-09-24-microluna-v6-embedding-preliminary.md](terminal-bench/2026-09-24-microluna-v6-embedding-preliminary.md) | Dated report / protocol | Microluna v6 on embedding-drift-monitor: preliminary analysis |
| [terminal-bench/2026-09-24-microluna-v6-v8-report.md](terminal-bench/2026-09-24-microluna-v6-v8-report.md) | Dated report / protocol | Microluna v6 to v8: what happened, and what it means |
| [terminal-bench/2026-09-24-microluna-v7-embedding-definitive.md](terminal-bench/2026-09-24-microluna-v7-embedding-definitive.md) | Dated report / protocol | Microluna v7 on embedding-drift-monitor: definitive analysis |
| [terminal-bench/2026-09-24-microluna.md](terminal-bench/2026-09-24-microluna.md) | Dated report / protocol | Microluna against Luna-in-Codex |
| [terminal-bench/2026-09-24-persist-v10.md](terminal-bench/2026-09-24-persist-v10.md) | Dated report / protocol | Tunable v10 against v7 on four near-miss tasks |
| [terminal-bench/2026-09-24-strategy-fingerprints.md](terminal-bench/2026-09-24-strategy-fingerprints.md) | Dated report / protocol | Strategy fingerprints: what Fable's winners do that Luna and Coder One don't |
| [terminal-bench/2026-09-24-task-anatomy.md](terminal-bench/2026-09-24-task-anatomy.md) | Dated report / protocol | Task anatomy for the Microluna overnight effort |
| [terminal-bench/2026-09-24-truthful-checks.md](terminal-bench/2026-09-24-truthful-checks.md) | Dated report / protocol | Truthful checks, calibrated against graded runs |
| [terminal-bench/2026-09-24-version-arc.md](terminal-bench/2026-09-24-version-arc.md) | Dated report / protocol | Coder One's version arc, from the Gemini loop to tunable v10 |
| [terminal-bench/2026-09-25-archive-check-confirmation.md](terminal-bench/2026-09-25-archive-check-confirmation.md) | Dated report / protocol | Archive confirmation: executed checks and grader disagreement |
| [terminal-bench/2026-09-25-baseline-offline.md](terminal-bench/2026-09-25-baseline-offline.md) | Dated report / protocol | Baseline behavior offline: the host runs the task's own program |
| [terminal-bench/2026-09-25-candidate-review-validation.md](terminal-bench/2026-09-25-candidate-review-validation.md) | Dated report / protocol | Candidate review and executable-check validation |
| [terminal-bench/2026-09-25-check-grades-offline.md](terminal-bench/2026-09-25-check-grades-offline.md) | Dated report / protocol | Check-line grades offline: not admitted |
| [terminal-bench/2026-09-25-data-profile.md](terminal-bench/2026-09-25-data-profile.md) | Dated report / protocol | Data profile and wide entry points offline |
| [terminal-bench/2026-09-25-decision-fit.md](terminal-bench/2026-09-25-decision-fit.md) | Dated report / protocol | Fitting Jev decision settings on recorded answers |
| [terminal-bench/2026-09-25-departures-offline.md](terminal-bench/2026-09-25-departures-offline.md) | Dated report / protocol | Departure miners offline: no source admitted |
| [terminal-bench/2026-09-25-environment-facts-offline.md](terminal-bench/2026-09-25-environment-facts-offline.md) | Dated report / protocol | Environment facts in the briefing, measured offline |
| [terminal-bench/2026-09-25-executed-contract-checks.md](terminal-bench/2026-09-25-executed-contract-checks.md) | Dated report / protocol | Executed contract checks: the task's own contract, run |
| [terminal-bench/2026-09-25-executed-contract-supplement.md](terminal-bench/2026-09-25-executed-contract-supplement.md) | Dated report / protocol | Executed contract checks: the remaining sixteen candidates |
| [terminal-bench/2026-09-25-fable-pattern-map.md](terminal-bench/2026-09-25-fable-pattern-map.md) | Dated report / protocol | Fable 5.1's winning runs, mapped to components |
| [terminal-bench/2026-09-25-failure-localization.md](terminal-bench/2026-09-25-failure-localization.md) | Dated report / protocol | Failure localization offline: error context, first mismatch, and phase timing |
| [terminal-bench/2026-09-25-finish-rule-offline.md](terminal-bench/2026-09-25-finish-rule-offline.md) | Dated report / protocol | The finish rule for Microluna, measured offline and on mini-tasks |
| [terminal-bench/2026-09-25-lexicon-free-suspects.md](terminal-bench/2026-09-25-lexicon-free-suspects.md) | Dated report / protocol | Lexicon-free suspects offline: more hits, more false alarms |
| [terminal-bench/2026-09-25-literal-artifact-checks.md](terminal-bench/2026-09-25-literal-artifact-checks.md) | Dated report / protocol | Literal artifact checks: development results |
| [terminal-bench/2026-09-25-literal-lifecycle.md](terminal-bench/2026-09-25-literal-lifecycle.md) | Dated report / protocol | Literal artifact lifecycle correction |
| [terminal-bench/2026-09-25-method-conformance.md](terminal-bench/2026-09-25-method-conformance.md) | Dated report / protocol | verify.method_conformance offline: well-known methods, checked by their definitions |
| [terminal-bench/2026-09-25-metric-target.md](terminal-bench/2026-09-25-metric-target.md) | Dated report / protocol | checks.metric_target and control.optimize: measure the stated target, keep only passing improvements |
| [terminal-bench/2026-09-25-microluna-v13-embedding-trials.md](terminal-bench/2026-09-25-microluna-v13-embedding-trials.md) | Dated report / protocol | Microluna v13 on embedding-drift-monitor: the three trials, step by step |
| [terminal-bench/2026-09-25-microluna-v18-family.md](terminal-bench/2026-09-25-microluna-v18-family.md) | Dated report / protocol | Microluna v18: the completed family run |
| [terminal-bench/2026-09-25-oracle.md](terminal-bench/2026-09-25-oracle.md) | Dated report / protocol | checks.oracle: an acceptance check that doesn't depend on the candidate |
| [terminal-bench/2026-09-25-retention-budget-repairs.md](terminal-bench/2026-09-25-retention-budget-repairs.md) | Dated report / protocol | Candidate retention and cohort budget repairs |
| [terminal-bench/2026-09-25-review-rule-offline.md](terminal-bench/2026-09-25-review-rule-offline.md) | Dated report / protocol | The review rule for Microluna, measured offline |
| [terminal-bench/2026-09-25-run-card-v13-reproduction.md](terminal-bench/2026-09-25-run-card-v13-reproduction.md) | Dated report / protocol | The run card reproduces the v13 embedding reconstruction |
| [terminal-bench/2026-09-25-setup-time.md](terminal-bench/2026-09-25-setup-time.md) | Dated report / protocol | Where trial setup time goes |
| [terminal-bench/2026-09-25-stall-detection.md](terminal-bench/2026-09-25-stall-detection.md) | Dated report / protocol | Stall detection and next-step choice for Microluna, measured offline |
| [terminal-bench/2026-09-25-status-snapshot.md](terminal-bench/2026-09-25-status-snapshot.md) | Historical | Terminal-Bench status snapshot, 2026-09-25 |
| [terminal-bench/2026-09-25-tiered-acceptance.md](terminal-bench/2026-09-25-tiered-acceptance.md) | Dated report / protocol | Tiered acceptance: does any test class separate passing from failing candidates? |
| [terminal-bench/2026-09-25-truthful-checks-microluna.md](terminal-bench/2026-09-25-truthful-checks-microluna.md) | Dated report / protocol | Truthful checks: Microluna evidence and corroboration |
| [terminal-bench/2026-09-25-verify-executed-offline.md](terminal-bench/2026-09-25-verify-executed-offline.md) | Dated report / protocol | verify.executed offline: rerun the task's commands, reject a regression |
| [terminal-bench/2026-09-26-out-of-sample-study-results.md](terminal-bench/2026-09-26-out-of-sample-study-results.md) | Dated report / protocol | Out-of-sample study: results |
| [terminal-bench/2026-09-26-out-of-sample-study.md](terminal-bench/2026-09-26-out-of-sample-study.md) | Dated report / protocol | Out-of-sample knowledge and Microcoder study (pre-registration) |
| [terminal-bench/2026-09-26-round2-knowledge.md](terminal-bench/2026-09-26-round2-knowledge.md) | Dated report / protocol | Round 2 knowledge: what was added, and from where |
| [terminal-bench/README.md](terminal-bench/README.md) | Index | Terminal-Bench |
| [terminal-bench/capability-gaps.md](terminal-bench/capability-gaps.md) | Reference | Capability-gap log |
| [terminal-bench/claude-code-delegate-prompt/README.md](terminal-bench/claude-code-delegate-prompt/README.md) | Captured prompt / reference | What Claude Code sends as the Opus delegate |
| [terminal-bench/claude-code-delegate-prompt/system-prompt.md](terminal-bench/claude-code-delegate-prompt/system-prompt.md) | Captured prompt / reference | Harness |
| [terminal-bench/coder-one-delegate-runbook.md](terminal-bench/coder-one-delegate-runbook.md) | Reference | Coder One delegate runbook |
| [terminal-bench/data-quality.md](terminal-bench/data-quality.md) | Reference | Terminal-Bench data quality and run incidents |
| [terminal-bench/delegate-prompts/README.md](terminal-bench/delegate-prompts/README.md) | Captured prompt / reference | What each executor sends, by system prompt variant |
| [terminal-bench/development-results.md](terminal-bench/development-results.md) | Reference | Development results, 2026-09-22 |
| [terminal-bench/measurement.md](terminal-bench/measurement.md) | Reference | Terminal-Bench measurement and pricing |
| [terminal-bench/quest-board.md](terminal-bench/quest-board.md) | Reference | Terminal-Bench 4 quest board |
| [terminal-bench/reports.md](terminal-bench/reports.md) | Index | Terminal-Bench report catalog |
| [terminal-bench/resilience.md](terminal-bench/resilience.md) | Reference | Terminal-Bench resilience |
| [terminal-bench/runbook.md](terminal-bench/runbook.md) | Reference | Terminal-Bench runbook: operating notes |
| [terminal-bench/targeted-experiment-template.md](terminal-bench/targeted-experiment-template.md) | Reference | Targeted experiment results template |
| [terminal-bench/tb4-leaderboard.md](terminal-bench/tb4-leaderboard.md) | Reference | Terminal-Bench 4.0 leaderboard reference, 2026-09-23 |
| [terminal-bench/tb4-results.md](terminal-bench/tb4-results.md) | Reference | Terminal-Bench 4.0 results |
| [terminal-bench/winning-runs-analysis.md](terminal-bench/winning-runs-analysis.md) | Reference | Why the two winning Coder One runs were cheap and fast |
| [terminal-bench/2026-09-26-round3-knowledge.md](terminal-bench/2026-09-26-round3-knowledge.md) | Dated report / protocol | Round 3 knowledge: what was added, and from where |

## verse

| Document | Role | Topic |
| --- | --- | --- |
| [verse/README.md](verse/README.md) | Index | Verse |
| [verse/gym.md](verse/gym.md) | Guide | Spatial Gym observation and launch recipes |
| [verse/mobile.md](verse/mobile.md) | Guide | Verse in Coder for iOS |
| [verse/chat.md](verse/chat.md) | Reference | Verse chat |
| [verse/gdd.md](verse/gdd.md) | Reference | Verse game design document (draft) |

## voyager

| Document | Role | Topic |
| --- | --- | --- |
| [voyager/README.md](voyager/README.md) | Index | Voyager |
