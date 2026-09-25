# Coder documentation

Coder routes turns through typed decisions, generates responses, and runs
approved work through a shared terminal and headless runtime.

| Directory | Contents |
| --- | --- |
| [Guides](guides/) | Configuration, headless use, delegation, project supervision, and artifact verification. |
| [Runtime](runtime/) | Execution contracts, subprocesses, terminal behavior, traces, and repository evidence. |
| [Design](design/) | Product direction, architecture analysis, roadmap, and historical proposals. |
| [Programs and extensions](../extensions/) | Target specification for programs, Wasm plugins, skills, discovery, and packages. |
| [Measurements](measurements/) | Observed results, task-selection studies, and supporting evidence. |
| [Verification](verification/) | Dated acceptance records and supporting evidence. |
| [Examples](examples/) | Work lists and project supervisor configuration. |
| [Source archive](thoughts-on-a-typesafe-coding-agent/) | Original exports and images for the coding-agent proposal. |

Start with the [headless guide](guides/headless.md) to run a turn or the
[Devin delegation runbook](guides/devin-delegation-runbook.md) to assign work.
The [rebuild plan](design/rebuild-plan.md) describes the implementation and
links to the proposed delivery roadmap.

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
