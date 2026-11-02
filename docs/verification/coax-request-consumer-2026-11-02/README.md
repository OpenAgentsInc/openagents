# System One's typed request consumer — 2026-11-02

This qualifies the specific System One/desktop Lab consumer in
[#12625](https://github.com/OpenAgentsInc/openagents/issues/12625), built on the
previous [owner](../coax-request-owner-2026-11-01/README.md),
[polling](../coax-request-polling-2026-11-01/README.md) and
[B2](../coax-b2-early-exit-2026-11-01/README.md) work. Product/test commit:
`a6dedd914cfa1b8ce3b6869fcfec2d16fda1dd7f`.
[The machine record](record.json) pins producers, scope and original receipts.

The Lab now uses the actual
[`outcomes.coax`](../../../coax/lib/systemone/outcomes.coax) adapter over
[`owner.coax`](../../../coax/lib/systemone/owner.coax). The language's direct
`Result` handling and exhaustive nominal unions carry start, poll, snapshot,
cancellation and stale-ticket outcomes. This is an adopted application path;
the raw transport remains available with its earlier manual lifetime duties.

One canonical, exclusively accessed owner preserves a monotonic generation.
Tickets may be copied; the owner must not be copied or reconstructed during
its lifetime. Ready results prevent reuse until snapshot or discard. A stale
ticket cannot poll, cancel or retire a newer generation, even after native
process slot reuse. Exhaustion refuses rather than wrapping.

Snapshot validates every capacity before changing any byte, copies the whole
body/stderr/problem and scalar metadata into independent caller storage,
publishes the result and retires the owner. The caller keeps that snapshot
storage live through every use and supplies disjoint storage. The Lab validates,
formats and routes from local snapshots, copying all displayed strings into
its application-life transcript pool. Completion, cancellation, owner reuse
and process release cannot revoke those copied bytes.

`SoTerminal` distinguishes HTTP success, refusal, cancellation, explicit
no-connection and unknown outcomes. Success still needs `so_check` against the
questions. Only the existing curl exit-7 observation permits the Lab's existing
one-time door fallback. Unknown outcomes never authorize an automatic retry.
Stop and quit explicitly handle discard, including its stale refusal.

| Boundary | Evidence | Observation |
| --- | --- | --- |
| Generation/admission policy | [Checks](owner-policy-checks.json), [mutation](owner-policy-mutation.json) | Five groups, 183 cases; **34/34 killed, zero survivors**, retained unchanged on qualified producer `1fac3750…`. |
| Typed API and refused misuse | [Receipt](typed-api.json), [checks](typed-checks.json), [mutation](typed-mutation.json) | R2 integration image `67e753ae…`: **42 groups**, authority coverage, **32/32 killed, zero survivors**. Unhandled start is E0907, missing terminal case E0904, returned local snapshot backing E0383. |
| Runnable typed request | [Source](../../../coax/tests/systemone-outcomes.coax), [literal transcript](../../../coax/tests/systemone-outcomes.out), [receipt](typed-api.json) | Actual process host plus an owned offline child, at O0/O2/fatal ASan+UBSan: success, HTTP refusal, cancellation, no connection and unknown timeout. Deferred discard handles every normal exit. |
| Actual Lab consumer, real native effects | [Receipt](lab-native.json) | Six generations on the **same canonical Lab** in each native mode: valid answer with usage 7/2 and service identity; stdout EOF while child remains live; stale copied tickets; real Stop kill/reap; unrelated reused slot preservation; timeout/stderr, HTTP busy, malformed answer and no-connection display. All headline, argument, detail and output strings survive wire reuse and later requests. |
| Construction and entry | [Factory](construction.json), [entry](lab-entry.json), [continuity](declaration-continuity.json) | Actual complete defaults and production C factory/quit pass 16 runs; actual `lab_ask` passes eight refusal cases with RED/GREEN O0/O2/sanitizer checks. The 177 factory and 60 entry declarations are unchanged; an added sharp test alone changed the loaded module's file hash. |
| Full application | [Refusal](full-app-refusal.json) | Full settings emission still refuses E0381 at `settings.coax:52`; the full authored Lab check refuses at `claude.coax:900`, before its check runs. **No full application build or UI pass is claimed.** |

The Lab native projection copies complete authored types and actual
`lab_ask`, `lab_pump`, `lab_stop`, routing, validation, formatting and transcript
procedures. Environment reads and downstream engine dispatch are signature
matched test stubs. `process-native.c` is real; an owned executable named curl
is selected through the private test PATH. No model, service, production key,
HTTP request, visible browser or application window is used. This qualifies the
specified request consumer, not the rest of the desktop or a downstream engine.

A surviving `problem.count > 0` mutation exposed a missing one-byte refusal
case. The new check kills it; none was marked equivalent. Earlier test-only
ABI/default assumptions, a headline oracle error and projection/runner failures
remain archived. The raw-owner O0/emission pass is retained through its
runner-only count correction. Typed checks/mutation and the first negative are
retained through a runner-only stdout/stderr diagnostic parser correction;
remaining diagnostics and all literal native transcripts ran in the final
receipt. These are scoped continuities, not retrospective claims that one
script revision produced another's observation.

Reproduce with an existing matching R2 image and new output directories:

```shell
python3 coax/tests/systemone-outcomes.py --compiler /path/to/coax --out /tmp/new-typed-api
python3 apps/openagents-desktop/tests/lab-consumer.py --coax /path/to/coax --out /tmp/new-lab-source --emit-only
python3 apps/openagents-desktop/tests/lab-consumer-process.py --generated-dir /tmp/new-lab-source --out /tmp/new-lab-host
python3 apps/openagents-desktop/tests/check_zeron_defaults.py --coax /path/to/coax --out /tmp/new-defaults
python3 apps/openagents-desktop/tests/lab-owner.py --coax /path/to/coax --out /tmp/new-entry
```

[proof.tar.xz](proof.tar.xz) retains 442 text inputs, generated C/headers, raw
logs, refused attempts and original receipts. Every member was read back against
[the manifest](proof-manifest.json). Binary/object/dSYM bytes are omitted; their
observed hashes remain in receipts. The R2 compiler's normal release/bootstrap
qualification belongs to its separate record; this work did not rebuild it.

The application-life pool has no new free/ownership-transfer contract. Tickets
are not unforgeable native handles, owner values are not linear by compiler
construction, and arbitrary foreign retention and general backing lifetimes
remain outside this runtime protocol. Broader B2 and R0/device/review/golden
gates remain separate. No new owner action was requested or completed.
