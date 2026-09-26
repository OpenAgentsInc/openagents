# Coder documentation

For cross-project priorities and dependencies, see the [master roadmap](../roadmap.md).

Coder is a coding product across interfaces and execution locations. Today,
it routes turns through typed decisions, generates responses, and runs
approved work through a shared terminal and headless runtime. Mobile, web, managed cloud execution, and CoderOS are part of the intended
suite. The public workspace now has bounded mobile and host prototypes; those
fixtures and the historical private demonstrations do not establish complete
product releases.

The [TypeSafe product-suite plan](design/typesafe-product-suite.md) maps the
complete source proposal and episodes 275–281 to runtime work, current gaps,
and release tests. The [networked Coder plan](design/networked-coder-plan.md)
connects that product to measured coding quality, reusable knowledge, and the
high-priority [agent labor track](../agents/market-infrastructure.md).

The [Coder suite migration roadmap](design/coder-suite-migration.md) reviews
the private Coder implementation as design reference. It identifies what to
reimplement for mobile, desktop, web, CoderOS, and remote execution, with
source evidence, current public gaps, dependencies, and release gates. The
shared task host and measured TypeSafe/Microcoder integration connect those
interfaces without importing the private backend.

Use the [migration implementation tracker](migration-status.md) for current
delivery status. It records the active
task foundation, the complete M0–M20 backlog, dependencies, ownership, and
acceptance gates. It separates shipped foundations from the work still needed
for durable tasks, mobile control, CoderOS, and agent labor.

[Beating Fable together](beat-fable-together.md) is the plan after Episode
288: make the Microcoder wins visible in the Gym, run a pre-registered
out-of-sample knowledge test, open the knowledge base to contributors, and
bring that work into Verse as quests and XP. The
[showcase](beat-fable-showcase.md) is its public write-up: where Microcoder
beats Fable 5.1 low, with the Gym's labels, and how to join.

## Use the current implementation

| Task | Start here |
| --- | --- |
| Install or run a turn | [Install](guides/install.md), [headless mode](guides/headless.md), [shared turn contract](runtime/shell-loop.md) |
| Submit and inspect durable work | [Task commands](guides/tasks.md), [execution owner](runtime/task-owner.md), [frozen context](runtime/frozen-task-context.md) |
| Use Microcoder outside the benchmark harness | [Repository adapter](runtime/microcoder-repository.md), including admitted configurations and incomplete live acceptance |
| Inspect runs and evidence | [Traces](runtime/traces.md), [artifact verification](guides/artifact-verification.md), [Terminal-Bench index](../terminal-bench/README.md) |
| Reuse or share knowledge | [Knowledge guide](guides/knowledge-base.md), [private bundles](runtime/knowledge-bundles.md), [evidence integrity](runtime/knowledge-evidence.md) |
| Review the suite roadmap | [Delivery tracker](migration-status.md), [design index](design/README.md), [migration assessment](design/coder-suite-migration.md) |

The [local task commands](guides/tasks.md) retain requests across process
restarts. The [execution owner](runtime/task-owner.md) adds explicit bounded
commands, detached execution, conservative recovery, paged ATIF evidence,
retained candidate artifacts, and independently checked completion. This local
OS-user path does not establish cross-device control. The
[repository Microcoder adapter](runtime/microcoder-repository.md) defines its
separately admitted model support. [Frozen context](runtime/frozen-task-context.md)
pins exact knowledge and independent-check source lineage before execution. The [free labor host](runtime/free-labor.md)
uses the same execution boundary for a recoverable buyer/provider order.
The [portable host helper](runtime/portable-host.md) installs digest-pinned
bundles, retains rollback, and generates one-shot services for explicit task
grants. Its acceptance includes real macOS tasks and Linux service wiring;
clean-host and CoderOS release acceptance remain separate work.

The [Rust mobile feasibility decision](design/rust-mobile-feasibility.md)
selects shared evidence state with native text controls and Rust-rendered HTML,
based on simulator, emulator, and browser observations. Device release gates
remain explicit. [Knowledge bundles](runtime/knowledge-bundles.md),
[evidence integrity](runtime/knowledge-evidence.md), and
[prospective study bookkeeping](runtime/knowledge-studies.md) distinguish
permissioned reusable inputs from measured improvements.

## Plans and reference material

The [teardown integration plan](design/teardown-nostr-integration.md) brings
81 archived research documents into that direction. It links the complete
source ledger, six new draft Nostr profiles, and ordered implementation gates
for sessions, workspaces, tracked work, automation, environments, and live media.

| Directory | Contents |
| --- | --- |
| [Guides](guides/) | Configuration, headless use, delegation, project supervision, and artifact verification. |
| [Runtime](runtime/) | Execution contracts, subprocesses, terminal behavior, traces, and repository evidence. |
| [x402 Lightning](design/x402-lightning-nostr-integration.md) | Designed paid operations, Nostr bindings, wallet admission, and recovery. |
| [Design](design/) | Product direction, architecture analysis, roadmap, and historical proposals. |
| [Programs and extensions](../extensions/) | Target specification for programs, Wasm plugins, skills, discovery, and packages. |
| [Measurements](measurements/) | Observed results, task-selection studies, and supporting evidence. |
| [Verification](verification/) | Dated acceptance records and supporting evidence. |
| [Examples](examples/) | Work lists and project supervisor configuration. |
| [Source archive](thoughts-on-a-typesafe-coding-agent/) | Original exports and images for the coding-agent proposal. |

The [rebuild plan](design/rebuild-plan.md) describes target architecture. Use
the [runtime index](runtime/README.md) for implemented contracts and the
[delivery tracker](migration-status.md) for acceptance limits. Older dated
assessments and benchmark version plans remain historical evidence; their
missing-feature lists and queued experiments are not current execution orders.

The [Terminal-Bench task comparison](measurements/2026-09-22-terminal-bench-chat-fit.md)
locates the CoderBench golden and ranks upstream tasks against recent work.
The [v0.5 algorithm and golden-trace proposal](design/coder-terminal-v05-algorithm-and-goldens.md)
connects those tasks to task state, evidence, execution, and the OpenAgents NIPs.
The [Terminal-Bench runbook](terminal-bench.md) covers the pinned local
harness, and the [episode contract](terminal-bench-contract.md) defines how
a v0.5 artifact plugs into it.

[Decision model documentation](../decision-models/README.md) covers the
models, API, and measurements that inform Coder's decisions.

The [issue-flow policy comparison](measurements/2026-09-25-issue-flow-policies.md)
records the retained requirements default, all completed traces, and the
operator-directed stop of the second round.
