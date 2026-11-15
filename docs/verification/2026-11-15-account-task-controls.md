# Account task controls

2026-11-15. Follow-up to resident task owner commit
`a0537486176feb6a67f0c33dc02e0404fd9144c7`, for
[#13620](https://github.com/OpenAgentsInc/openagents/issues/13620).

This change exposes an already admitted native task's status, permission reply
and Stop through the account API and public OpenAgents CLI. It reuses the
existing TaskRequest/TaskStatus codecs, NIP-98 account transport and waited
HOST broker adapter. Public dispatch, task transcript paging, mobile task
controls and a real provider task on staging remain outside this checkpoint.
The issue stays open until those end-to-end acceptance gates pass.

The account bridge verifies current connection exclusivity and the returned
Binding's owned conversation/turn before sending a control. A control's reply
must preserve every immutable Binding field. The CLI retains the original
command privately before HTTP; matching retries retain its window and command
ID, while changed permission/revision/decision reuse is refused locally.

## Gates

| Gate | Result |
| --- | --- |
| Task API policy | 159 checks; 69 changed-procedure mutants, 65 killed and four reviewed equivalent boundary changes |
| Task client intent policy | 155 checks; all 22 changed-procedure mutants killed |
| `tools-check` | 0 new tools without a reason |
| `wire-check` | 360 registry rows, 0 failures; no new wire representation or allowlist entry |
| `layer-check` | 0 failures; existing counts unchanged |
| Complete CLI build and changed entry points | All three API/fixture/test roots compiled; the complete native CLI compiled and linked |
| Shared environment/Projects C ABI and process oracle | ASan/UBSan link passed; typed existing environment and new task status returned with SIGCHLD ignored and restored |
| Native API and built CLI integration | 37/37 through the actual signed CLI, HTTP service, database and typed broker child; task-only rerun and all remaining harness checks passed in 418 s |
| Existing history/account/Project/routing integration | 71/71 on the same product sources; reused from the preceding run (only the new test's cache-header assertion changed afterward) |

The API harness compiles the full API under ASan/UBSan, uses a disposable real
Postgres database and starts the typed broker fixture as a separate process.
The process oracle links both environment and Project wrappers to catch shared
C-export collisions before building the full API. It deliberately inherits
SIGCHLD ignored, then verifies that
both existing environment and new task calls receive typed replies and restore
that disposition. The HTTP/CLI fixture covers ownership, ambiguous enrollment,
funding changes, permission controls, Stop, retained command bytes and changed
Binding replies. Broker audit bytes contain only synthetic test identities;
keys stay in private files/child environment and are never printed.

The initial integrated run passed all functional task cases but its cache
assertion expected exactly `no-store`. The common API envelope correctly sends
`no-store, private`; the corrected assertion also checks `Vary: Authorization`.
The task-only rerun passed all 37 cases. Earlier compile/link findings were
resolved before it: host identity validation is exported through checked Coax,
the shared Project budget export is namespaced in the environment C wrapper,
and command windows use the shared wall clock rather than the monotonic timer.
The fixture enforces that wall-clock window before accepting any new control.
The normal pre-push gate also caught the new API policy's missing context
registration. It now belongs to Chat beside the existing environment adapter;
Coder continues to own the native task protocols and authority. Context load
permissions and the layer allowlist are unchanged.

The stand-in does not execute a model or implement the resident's task receipt
store. Real signed/encrypted HOST delivery, native SDK permission/file effects,
Stop, restart uncertainty and exact-once control receipts were qualified by
the unchanged 69-case resident smoke and 19-case filesystem oracle in the
[earlier record](2026-11-15-resident-tasks.md). Their results are reused here,
not represented as a live external provider task.

## Reproduce

Run heavy checks on a build host, through the repository wrappers:

```sh
coax/remote/remote.sh run -- env API_TESTS_ONLY=environment_proxy.c coax/web/api/test.sh
coax/remote/remote.sh run -- coax/remote/check.sh only coax/web/api/task-access-policy.coax,coax/lib/agent/environment/task-client-policy.coax origin/main
coax/remote/remote.sh run -- env API_TESTS_ONLY=task_checks.coax,history_checks.coax coax/web/api/test.sh
```

The CLI examples and exact HTTP admission rules are documented in
[the CLI](../../apps/openagents-cli/README.md#native-task-controls) and
[the task contract](../protocol/environment-execution-v1alpha1.md#account-task-bridge).
No native service, API or TestFlight deployment is claimed by this record.
