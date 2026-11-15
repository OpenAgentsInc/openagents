# Resident routed tasks

2026-11-15. Source qualification for #13620, extending preparation commit
`f01acefd76453214e05d062f8fcb706211c104b6`. This is the resident task owner and
its internal signed HOST client. No API, mobile, public task CLI, production
deployment or external provider inference is qualified by this record.

The [task profile](../protocol/environment-execution-v1alpha1.md#resident-task-control)
owns the complete field catalog. Native process/model/tool transport is the
existing shared Agent and Claude/Codex SDKs. The provider stand-ins below are
the repository's existing native fixtures; they require no credentials.

## Observations

| Gate | Observed result |
| --- | --- |
| `coax test coder/tui/host.coax --library --run-steps 250000000` | 1,329/1,329 native checks |
| `coax/web/tool.sh coder/tui/task-host-smoke.coax` | 69/69; four Claude tasks and two Codex tasks through a real local relay, signed/encrypted HOST, resident process, native SDK children and private files |
| `coax/web/tool.sh coder/tui/host_smoke.coax` | 49/49 existing pairing/preparation/realm/Stop/revocation checks after the shared driver and typed fixture changes |
| `coax/web/tool.sh coax/lib/agent/environment/task-store-oracle.coax` | 19/19 real filesystem observations |
| Task/preparation/input descriptor and byte oracles | All three passed; ten task request/status fixtures, sole nested router codec, immutable Binding and complete input SHA256 |
| `scripts/proto.sh check` with pinned protoc 29.3 | 85 owned schemas, vendor/profile closures and all 30 compatibility checks passed |
| Changed authority gates through `coax/remote/check.sh only ... origin/main` | Task policy: 108 mutants, 107 killed, one reviewed equivalent; task wire including policy: 395, 384 killed, eleven reviewed equivalents; terminal classifier: all 34 killed |
| Input authority policy | All ten root mutants killed; changed-procedure gate also passed |
| `tools-check`, `wire-check`, `layer-check` | Passed; no allowlist increase |

Heavy commands ran on the owned Linux build pool. The final signed task smoke
ran on `oa-build-12` in 120 seconds (128 seconds including transfer/setup).
The fixture's explicit local relay budget is 1,024 events/REQs per tested
bucket; production relay defaults are unchanged. The existing 49-check smoke
retains its separately documented 240-event fixture budget.

The signed task smoke observes all of the following:

- Operator pairing and explicit workspace enrollment precede preparation.
- The resident pins its own actual report, receives complete canonical input,
  retains Start before process effects, then uses the selected account's native
  auth probe and actual SDK model offers.
- Claude and Codex both hold actual permission requests. Old revisions and
  foreign interaction IDs cannot reach the native SDK. Allow writes the real
  isolated workspace file; Deny leaves it absent. The correlated tool result is
  followed by a native continuation and completion.
- The actual Claude prompt contains every admitted instruction, reference and
  history byte. The status contains the SDK's actual session identity.
- Stop intent is retained independently; an explicit typed Claude
  `aborted_streaming` result confirms cancellation.
- Exact Start and permission-command retries reconcile their durable receipts.
  Native observations count exactly four Claude prompts and two Codex
  `turn/start` requests across the retries and resident restart.
- Restart while a permission is held becomes unknown and does not repeat the
  native task. Consumed preparations cannot be resurrected by relay history;
  subsequent tasks still prepare successfully.
- Revoking enrollment prevents reads of the retained task status.

The filesystem oracle independently verifies 0700/0600 modes, complete input
retention after deleting its upload cache, cross-grant refusal, immutable task
identity, pending permission reconstruction, binding/session-claim corruption,
missing Start marker refusal, global control identity, interrupted local/global
receipt publication, and the actual 512-control bound with a reserved Stop entry.

## Failures resolved and qualification limits

Early native runs exposed a missing request station count in dispatch admission,
incomplete stand-in init/error frames, a fixture assertion expecting the wrong
conflict exit status, and replay recreating a consumed preparation after restart.
These were fixed in their owners. The stand-in's identity and terminal frames
now have a check against the actual SDK decoder; product decoding was not relaxed.

An initial changed gate also found insufficient session reset assertions through
an overly broad policy import. Pure classifiers now load only the shared outcome
constants; session reuse/unknown-outcome assertions were strengthened. Final
changed policy gates have zero unresolved survivors.

These tests do not prove a Codex Stop result against an installed provider,
independent managed-profile execution, OS sandboxing, surviving remote effects,
live account API dispatch, structured transcript catch-up, installed mobile/CLI
permissions, or a real staging patch/test task. Those remain #13620 acceptance
work. In particular, old Codex empty interrupt acknowledgements/idle notices are
uncertain outcomes, never evidence of confirmed cancellation.

The API router must resolve its model alias/default to an actually offered
model/effort before retaining the final Binding and dispatch decision. The
resident refuses a mismatch; it does not silently substitute a display default.
Project membership still supplies no computer or repository authority.

Production and TestFlight deployment remain deferred until the full Claude-port
roadmap and remaining Arcade port satisfy the owner's release condition.
