# Execution records

These records accompany the [matched Opus experiment](../README.md) and its
[results](../../../../../docs/terminal-bench/2026-09-23-matched-opus-controller.md).
The JSON files were copied from the execution host after scanning for known
credential values. [The manifest](manifest.json) records their SHA-256 digests.
Environment fields in job configurations contain variable references, not
credential values.

| Record | Purpose |
| --- | --- |
| `matched-20260923--*.json` | All 12 materialized Harbor job configurations, including the executor entry point, task, pinned artifact, retry policy, and timeout. |
| [Completion record](completion.json) | All 12 completed attempts and the grader-only replay, final task and binary checks, and execution-source digests. |
| [Protocol pins](pins.json) | Final runner, policy, protocol, and system-prompt digests. |
| [Initial execution pins](initial-execution-pins.json) | Bytes at the start of inference, before the collection and accounting fixes. |
| [Admission amendment](admission-amendment.json) | The pre-inference decision to reserve a sixth host-wide Claude slot for one serial experiment trial. |
| [Prelaunch pins](prelaunch-pins.json), [five-slot pins](prelaunch-five-slot-pins.json) | Earlier staging records, before the admission amendment. The prelaunch record also lists macOS metadata sidecars; they are not prompt inputs. |
| [Collection fix](collection-fix.json), [pins after that fix](after-collection-fix-pins.json) | Parent-directory creation for native-session downloads, with old and new runner digests. |
| [Prompt-token accounting fix](prompt-token-fix.json) | Include cache reads and creation in Harbor's input-token total; native usage and costs remain unchanged. |
| [Host and task pins](host-and-task-pins.json) | Task revision and clean checkout, instruction and task-configuration digests, Harbor version, and host architecture. |
| [First disk cleanup](disk-admission.json), [second disk cleanup](disk-admission-second.json), [third disk cleanup](disk-admission-third.json) | Unused build-cache removal to restore the admission floor. |
| [Regrade configuration](regrade-config.json) | Replay of the first plain candidate without another model call; file-digest proof is in its retained trial bundle. |
| [Grader review](grader-contract-review.json) | Next.js grader excerpt and digest, inspected after the first Coder failure. It was never supplied to the executing agents. |

The policy, prompt, task selection, model, effort, tools, cache setting, budgets,
and trial order did not change after inference began. The two adapter fixes
changed collection and accounting only. Their source revisions and the original
exception remain available; the report does not silently replace the affected
trial records.
