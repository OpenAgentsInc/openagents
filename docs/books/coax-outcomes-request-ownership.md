# Coax outcomes and request ownership

Implementation sequence for [#12625](https://github.com/OpenAgentsInc/openagents/issues/12625),
following the [shared completion plan](completion.md). This maps existing task
identities; it creates no replacement language or ownership milestone. Snapshot:
`a2e4931045`, 2026-11-01 (November 2 UTC).

## Preserve the existing work

| Existing identity | Scope and retained evidence | Remaining exit |
| --- | --- | --- |
| [`book30-analysis-cost`](../../apps/openagents-web/projects/coax/work/book30-clocks.json) | Delivered wide retained-store streaming; [H qualification](../verification/coax-wide-retained-stores-2026-11-01/README.md) and [machine record](../verification/coax-wide-retained-stores-2026-11-01/record.json). The matching source/image used 1,035,932 of 1,048,576 work ticks, leaving 12,644. | Measure any changed compiler input on its matching image. Preserve the failed page goldens and earlier cost candidates. Do not rerun unchanged passing compiler qualification. |
| [`book30-result-language`](../../apps/openagents-web/projects/coax/work/book30-clocks.json) | R2 child: tagged results/unions/optionals, exhaustive handling, propagation, ordered cleanup, sum-aware borrows and a real synchronous resource adapter. A separate agent's local integration is in progress. | Publish the final qualified compiler, source/native/ABI/authority/bootstrap evidence and teaching contract. A private implementation or lower cost measurement is not delivery on main. |
| [`r2a-results`](../../apps/openagents-web/projects/coax/work/coax.json) | Original language umbrella; its first child does not remove the original asynchronous adapter or remaining procedure scope. | Integrate the actual asynchronous request with handled success/refusal/cancellation/unknown outcomes and cleanup. Preserve idempotency/reconciliation rules; unknown outcomes cannot imply safe retry. |
| [`r3a-retain-helper-composition`](../../apps/openagents-web/projects/coax/work/retained-borrow-subtasks.json) (B2) | Precise retained helper writes followed by returned-field reads; [current B1 boundary](../../coax/verification-retained-stores.md#remaining-boundary). | Actual `so_post` body origins select caller output, stderr selects caller error storage, and static problems/default empty values remain distinct from unknown. Reject local backing through the real helper chain; qualify compiler changes separately. |
| [`r3a-retain-request-owner`](../../apps/openagents-web/projects/coax/work/retained-borrow-subtasks.json) (B3) | Actual System One transport and desktop owner, following B2. Existing [caller-buffer harness](../../coax/tests/borrow_summary_systemone.py) qualifies only digits/stderr returns. | Generated owner defaults, single active request, terminal/cancel/poll traces, all borrowed result uses and process-slot reclamation. Caller storage remains alive independently of process release. |

These are stable project slugs. The hexadecimal aliases shown by the Nostr
project are not GitHub issue numbers. Authored seed files describe scope and
dependencies, not live claim or resolution status. The local owner checkpoint
observed at `/tmp/coax-chapter30-full/CURRENT_STATE.json` reports the cost task
resolved and the R2 child active; this is attributed coordination information,
not an independently queried live project inventory. Its provisional R2 cost
and partial qualification are deliberately not promoted here.

## Implementation order

1. Preserve the delivered B1/wide-store evidence and its budgets. The older
   1,010,708-work candidate and B1's 1,047,475-work observation remain historical
   measurements of their own source/images. None is a current compiler estimate.
2. Let the current R2 owner finish the compiler and synchronous adapter. Avoid
   parallel changes to its checker, parser, evaluator, emitters, seeds, spec,
   grammar or teaching examples. Reuse only exactly matching passing inputs.
3. Finish B2's actual helper composition on that integrated compiler. Keep
   complete origin/effect inventory separate from discharged host lifetimes;
   an unknown recursive dependency cannot become safe-empty.
4. Finish B3 on the real polling API. A platform construction fix can proceed
   independently as a prerequisite: after successful root allocation, call the
   actual generated Coax default constructor once before attachment. Never
   reconstruct an attached/active owner or initialize view pointees implicitly.
   After attachment, quitting without starting a request must not release an
   unrelated process slot zero.
5. Enforce single-active-owner admission in Coax and qualify its unchanged-state
   refusal, then success, HTTP refusal, process error, timeout, cancellation,
   duplicate polls and release. Keep borrowed output/error bytes alive through
   every use; cancelling a process cannot revoke a copied descriptor. No live
   service or automatic retry is needed for these offline native checks.
6. Migrate the same consumer to delivered typed outcomes and teach its actual
   diagnostics, transcript, refused misuse and independent oracle. Update the
   chapter marker, registry, contents and README/plan together, only for the
   profile actually delivered.

The construction and admission slices do not by themselves finish B2/B3,
async Result integration, compiler ownership safety or #12625. In particular,
the desktop's selected request pool lives for the application: there is no
new pool-free or copied-active-handle ownership-transfer claim.

## Qualification and independent exits

New checks precede implementation. Use the actual generated Coax header and
code at native O0/O2 and under ASan/UBSan, with independent process-effect
fixtures, named fields/descriptors and guarded byte buffers. C padding is not
an oracle. Changed admission/policy authority gets focused zero-survivor
mutation; unchanged compiler/bootstrap and model checks are retained, not
repeated. Every command stays below fifteen minutes.

The [R0 owner/reviewer entry](../../NEEDS_OWNER.md#r0-adoption-gate) remains
separate: physical iPhone measurements, agreed median/frame-tail budgets and
independent frozen-reference adaptation review. The wide-store record also
retains four failed/unapproved page goldens. Native compilation, a constructor
fix or a successful offline trace cannot close those gates. No owner action
has been completed or newly requested by this coordination document.


## Delivered prerequisite slice

The [retained implementation record](../verification/coax-request-owner-2026-11-01/README.md)
now covers generated desktop defaults, single-active-owner admission, the Lab's
entry guard, and actual offline child completion/cancellation/reaping. Focused
policy mutation killed both mutants; native O0/O2/sanitizer checks pass within
their declared boundaries. The full desktop unit and new full Lab check group
refused with E0381 on the observed images before the application checks ran;
exact source subsets qualify the changed boundaries without calling the full
application passed. B2, typed async outcomes and the full B3 milestone remain
open; those application exits are not inferred from these prerequisites.
