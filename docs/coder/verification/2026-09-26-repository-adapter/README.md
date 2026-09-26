# Repository adapter acceptance, 2026-09-26

**Implementation evidence; live acceptance is incomplete.** The operator stopped
all model and benchmark work before independent checks were run on the second
pair of candidates. Both detached controllers had already exited when the stop
was received. No further model, benchmark, or candidate-check runs followed.
[Issue #9674](https://github.com/OpenAgentsInc/openagents/issues/9674) remains
open. A model's completion statement is not a verified solution.

## What ran

Four attempts used original, small Rust repair tasks. They are not Terminal-Bench
tasks or a comparative evaluation. Each task had an isolated Git worktree, an
exact source snapshot, frozen requirements and checker identity, an eight-step
limit, and a 300-second wall bound. Generation requested `gpt-6-luna` at `medium`
effort, with no stronger-model routing, generated acceptance suite, or knowledge
retrieval. The `ceil` task used the local workspace boundary; `range` used a
pinned, network-isolated Docker image. All four grants and full ATIF logs remain
in [the attempt directories](attempts/).

| Attempt | Decision model requested | Actual calls | Controller result | Wall time | Cost evidence | Independent checks |
| --- | --- | --- | --- | --- | --- | --- |
| v1 `ceil` | `jev-latest` | 1 Jev; no generation or shell call | Refused served model `jev-1.13.0` | 0.333 s | Unknown charge | Unavailable: no eligible completed candidate |
| v1 `range` | `jev-latest` | 1 Jev; no generation or shell call | Refused served model `jev-1.13.0` | 0.292 s | Unknown charge | Unavailable: no eligible completed candidate |
| v2 `ceil` | `jev-1.13.0` | 8 Codex, 8 Jev, 8 shell commands | Step limit; incomplete | 44.965 s | $0.003196306 list-price estimate | Not run |
| v2 `range` | `jev-1.13.0` | 5 Codex, 5 Jev, 4 container commands | Model finished | 41.506 s | $0.001833492 list-price estimate | Not run |

The second pair used new tasks and new frozen grants. The model identity was
changed explicitly after the alias refusal; no host fallback accepted the
first pair's responses. Their raw decision bodies and unknown-cost dispositions
remain retained. Generation replies in v2 reported the requested model name;
provider confirmation of effective effort or an immutable model artifact remains
unavailable. The two v2 estimates sum to $0.005029798. That excludes the two
unknown v1 decision charges and is not a provider invoice. Every common task
result retains `cost_status: "unknown"` for billing.

[The machine-readable summary](attempt-summary.json) includes timings, call
counts, task states, trace digests, artifact references, and uncertainty. The
retained launch receipts report admission as pending. The v2 process observations
show each controller with parent PID 1 and its own process group after the
launcher exited. Separate `coder task show` calls observed the same durable
state; no second execution journal was involved.

## Offline controls and verification

Before the live attempts, two actual Docker fixture tests passed, including
workspace-only persistence, denied root and Git-metadata writes, cleared workload
credentials, bounded output, and cancellation of a background writer before
container removal. [Their retained control traces](container-controls/) and
[test output](verification/microcoder-container-final-controls.log) describe
those synthetic cases. The final [code-only repository fixture suite](verification/microcoder-repository-code-only-final.log)
passed 12 tests, with the two Docker tests explicitly excluded from that later
invocation. Final strict [Microcoder all-target Clippy](verification/microcoder-repository-code-only-clippy.log)
and [checker/setup example Clippy](verification/repository-examples-code-only-clippy.log)
passed after the operator's stop. These checks made no model calls and did not
run the fresh candidate checkers.

The protected checker was exercised on intentionally broken and known-correct
copies of both tasks before model execution. Initial controls exposed missing
macOS SDK access; [v1](checker-controls/v1/) and [v2](checker-controls/v2/) retain
those failed infrastructure observations. The checker was then built with exact
Rust compiler, developer-root, linker, and SDK paths. The final
[v3 controls](checker-controls/v3/controls.json) rejected both broken sources and
accepted both corrected controls. These are checker controls, not verification
of the later model candidates.

The example checker and setup programs are original infrastructure:

- [Protected Rust checker](../../../../crates/coder/examples/repository_acceptance_check.rs)
- [Inert task and grant setup](../../../../crates/coder/examples/repository_acceptance_setup.rs)
- [Original source fixtures](../../../../bench/coder/repository-acceptance/README.md)

## Retained scope and limits

[The file inventory](files.json) records exact bytes and SHA-256 digests for the
copied evidence. It includes all four task stores, append-only event logs, ATIF
records, retained artifact manifests and blobs, grants, submissions, capability
manifests, local trust receipts, source files, launch receipts, and observations.
Native successful response objects and decision response bytes are preserved;
authentication headers and credential values are not part of these records.
Opaque provider reasoning fields remain opaque.

Absolute paths describe the original local evidence. This copy is not a runnable
replacement for that machine. Installed checker/controller executables, Git
administrative directories, and generated workspace binaries are not copied;
their identities or retained-artifact availability are recorded where the host
captured them. Original stores remain under
`/tmp/microcoder-repository-live-20260926-v1` and
`/tmp/microcoder-repository-live-20260926-v2` on the originating machine.

All completed host results report successful local cleanup. That is evidence
about these attempts, not a promise that a remote inference request can be
cancelled without charge. Response bodies are bounded when retained, after their
existing transports materialize them; hard controller-memory enforcement remains
unsupported. These observations establish neither a coding-quality win nor a
faster/cheaper comparison with another agent. The operator's stop leaves the live
independent-acceptance gate unmeasured.
