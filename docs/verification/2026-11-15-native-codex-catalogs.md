# Native Codex catalog pagination

2026-11-15. Source qualification for #13620, based on
`555efd2d2a41fa7664908b055f40e86077bb9e89`. This extends the existing signed
native offer and resident task owners. It changes no published wire fields,
creates no second model parser and supplies no static execution fallback.

The existing Codex model-field reader now appends pages into a separate owned
candidate. The native engine publishes it only after a terminal `nextCursor`.
The existing client serializes continuation requests; the resident's existing
read-only probe, task preparation and exact model/effort admission consume the
complete observation. The SDK owner is reset between resident probes/tasks.

The [offer profile](../protocol/engine-offer-v1alpha1.md) documents the bounds:
32 visible models, 32 pages, 1,024 decoded cursor bytes and 16 KiB of retained
model strings. Empty/hidden pages can continue. A final empty catalog, decoded
cursor cycle, malformed envelope, duplicate model, unknown effort, failed
request or exhausted bound cannot publish a ready prefix. Earlier published
strings remain intact while a new candidate is incomplete or refused.

Native settings retain the actual offered effort, including `minimal`; the
ordinary desktop projection retains its earlier minimal-to-low behavior.
Before requesting another page, the native reader checks response-tracking
capacity and rejects an exhausted output buffer. The existing client's request
ID is not a capacity result: it can return a positive ID even when tracking is
full. Retained labels/descriptions and the shared decoding arena must also fit
completely.

## Observed checks

Heavy checks run on the owned Linux pool. The final engine gate used the rebased
source; the new paging policy's earlier passing first-pass result applies to
the same unchanged policy procedures.

| Gate | Result |
| --- | --- |
| `coax/remote/check.sh only coax/lib/agent/engine/codex.coax origin/main` | 174/174 checks; 588 changed-procedure mutants, 583 killed, five reviewed equivalents, zero unresolved survivors; 31 seconds on `oa-build-11` |
| `coax mutate coax/lib/agent/engine/codex-model-pages.coax --root-only` | 85/85 killed, zero survivors; two seconds on `oa-build-4` |
| `coax/web/tool.sh coder/tui/task-host-smoke.coax` | 100/100; real local relay, signed/encrypted HOST, resident and SDK fixture children; 130 seconds on `oa-build-11` |
| `tools-check`, `wire-check`, `layer-check` | Passed; no allowlist increase |

The five equivalent mutations are documented at their actual sites: reordering
the default already at index zero changes nothing; copying a zero-byte config
preserves the empty default; an out-of-range unknown outcome normalizes back to
unknown; and both comparisons that additionally include selected index zero
still select zero. Other mutations on those lines remain exercised.

The checks cover decoded cursor ownership, exact cursor/page/model limits,
hidden/empty intermediate pages, failed reads and retries, publication ownership,
complete native metadata, exact-fit/exhausted pools, full RPC tracking/output,
saved selection refresh, native effort settings and existing SDK permission/
interrupt response handling. Early test fixtures omitted the thread required
by an interrupt and carried a Stop flag into a separate refusal case; their
setup was corrected without changing those product semantics.

The signed smoke observes exactly two `model/list` requests, including the
opaque second-page cursor, before any `turn/start`. Its ready offer contains
both pages' exact model IDs and efforts; polling preserves the same observation.
The subsequent four Claude and two Codex task prompts retain their exact
counts across Start/permission retries and restart. Allow/Deny, actual private
file effects, correlated results/continuation, hash-verified transcript reads,
Stop, uncertainty after owner loss and revocation still pass. A separate
resident restart with a repeating cursor produces a failed empty offer and
starts no additional turn. Evidence directory: `/tmp/coder-task-host-eS9h5U`
on the qualification host.

This is a bounded native SDK catalog implementation. Independent provider
accounts, shared gateway tool continuation/strict output, durable server capture
paging, expired Stop renewal and a real staging coding patch/test still belong
to #13620. Production and TestFlight remain deferred under the owner's release
condition. Controlled SDK fixtures establish transport behavior, not external
provider inference or a production rollout.
