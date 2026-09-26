# Coder documentation

Coder is a coding product across interfaces and execution locations. Today,
it routes turns through typed decisions, generates responses, and runs
approved work through a shared terminal and headless runtime. Mobile, web,
managed cloud execution, and Coder OS are part of the intended suite; their
historical demonstrations are not current implementations in this workspace.

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

The [migration implementation tracker](migration-status.md) records the active
task foundation, the complete M0–M20 backlog, dependencies, ownership, and
acceptance gates. It separates shipped foundations from the work still needed
for durable tasks, mobile control, CoderOS, and agent labor.

The [local task inbox](guides/tasks.md) records, inspects, and cancels queued
requests across process restarts. It is the first task-contract slice;
execution ownership and cross-device control remain in the tracker.

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

Start with the [headless guide](guides/headless.md) to run a turn or the
[Devin delegation runbook](guides/devin-delegation-runbook.md) to assign work.
The [rebuild plan](design/rebuild-plan.md) describes the target architecture
and links to the proposed delivery roadmap.

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
