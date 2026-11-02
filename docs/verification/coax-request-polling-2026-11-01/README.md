# A frame poll stays pending after stdout closes

Follow-up for [#12625](https://github.com/OpenAgentsInc/openagents/issues/12625).
The [previous owner slice](../coax-request-owner-2026-11-01/README.md) preserved
active request state. This slice removes an intentional five-second wait from
its nonblocking polling path.

The check-first native child wrote a response and HTTP trailer, closed stdout,
then stayed alive for three seconds. The old `so_step(call, 0)` waited for it
to exit and returned terminal. The new focused oracle failed in 3.979 seconds.
Closed stdout is not proof that the process has exited.

`so_step` now normalizes nonpositive waits to zero for reads and process-exit
polls. A still-running child keeps its handle, buffers and unpublished result;
a later poll completes normally, or cancellation kills/reaps it. Positive
calls retain the existing five-second finish wait. No process host, ABI or
buffer-lifetime rule changed.

[Native qualification](polling.json) uses the actual generated Coax and actual
native process host with an owned offline child, no network or key. Only the
new polling cases execute: delayed completion, cancellation after EOF and a
negative wait. All four O0/O2 plain/ASan+UBSan profiles pass; every observed
poll took at most 1 ms. Caller descriptors, guards, delayed body publication,
child reaping and safe slot reuse are checked. This fixture observation is not
a hard real-time or production scheduling guarantee.

The [small authority policy](policy.json) passes debug/release checks and
**9/9 root mutants killed**. The first scalar-clamp formulation left one
boundary mutant surviving; it was replaced with an explicit Boolean wait
decision and rechecked. No equivalent mark or waiver was used. Old failures
and source snapshots remain in the archive.

The new qualified compiler `1fac3750…` was reused; no compiler build or broad
model/bootstrap gate ran. Only changed transport/policy paths and their test
harness were executed. Earlier unchanged owner/model results keep their own
commit and scope.

## Full desktop boundary investigated

The [separate diagnosis](desktop-diagnosis.json) retains full `settings.coax`
emission and actual Lab debug/release attempts on that compiler, with all
61-source and compiler/provenance hashes unchanged. They still refuse with
E0381 before producing desktop C or running the new Lab check.

Read-only debugger observations locate the exact refusal: attempting a ninth
borrow-path step exceeds the unchanged eight-step guard, during B1 retained
leaf analysis. The Lab path reaches nested server/tool fields; its first
failing RHS fact has zero known origins and remains incomplete. It is not a
complete-empty or static-only proof that permits skipping the address query.
Settings fails inside its RHS origin projection too. No quota, application
catalogue or analysis requirement was changed to force a full-app pass.

[proof.tar.xz](proof.tar.xz) retains raw outputs, failures, generated sources and
the frozen diagnosis. [manifest.json](manifest.json) maps original paths to
archive members; every member was read back and hash-checked. Executable bytes
are omitted, with observed hashes retained. This work does not close R2, B2,
full B3 or full desktop qualification.

```shell
# After emitting wrappers for the selected compiler:
coax/tests/systemone-process-lifecycle.sh \
  /tmp/polling-evidence /tmp/generated-systemone-wrappers polling
```
