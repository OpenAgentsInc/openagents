# Bounded project catch-up verification

2026-10-28, task `project-live-catchup` (#db36), coax0 0.15.0.

The old live reader counted `CLOSED` as EOSE and replaced the verified build-time
snapshot after the five initial subscriptions ended. A relay cap of 100 could
therefore replace 156 known roots with 100, and omit older current claims/plans or
statuses. EOSE was being used as a completeness claim it cannot establish.

The changed Coax path keeps its stable watches, then pages issues, comments and
statuses independently. It continues after short pages, handles inclusive
boundary overlap, bounds requests/time/storage/output, and requests exact IDs
missing from the current order or the immutable roots known from the verified
snapshot/last complete view. A failed repair, refusal, ambiguous timestamp
cohort, timeout or exhausted budget leaves the last complete tables displayed
with OFFLINE. Candidate tables have separate storage; failed output generation
cannot damage visible rows/details/plans. New socket generations have new
subscription IDs, checked before frame storage. A snapshot is selected by both
project slug and pubkey. `MAX_ITEMS` and `FOLD_ISSUES` must agree, and the focused
seed check covers their actual combined inventory.

This remains relay-trusted provisional display, not Coax BIP-340 verification,
HOST/COORD authority or a signed CW completeness proof. An empty tail is an
observed relay result; it cannot prove that a malicious relay disclosed every
unknown event. A stalled multi-event timestamp cohort fails closed. An entirely
single-row historical feed also fails closed rather than guessing whether the
relay hid a same-second sibling. Singleton current project/order/repository
views are unaffected. Changing caps are tested, but no arbitrary relay behavior
is described as universally complete. Deadlines use the app's page clock.

## Focused evidence

- `coax test apps/openagents-web/project.coax --library --steps 1000000000`:
  **40 checks passed**. The explicit budget covers existing captured-transcript
  checks; no global compiler step default changed.
- `bun test coax/web/project/live-catchup.test.ts`: **7 tests passed**, including
  native C ABI and an actual headless Chrome WebGPU render of the app's generated
  Wasm, real C face and host. The signed fixture has 155 issue rows and asks for
  pages of 200 while the relay returns at most 37.
- Native/browser items, plans, details, activity and plan-info all match the
  independently signature-verified TypeScript fold byte for byte. Old current
  worker/plan/status state survives the initial cap. Exact-ID repair recovers a
  root deliberately omitted from history queries.
- After catch-up, a newly signed claim arrives through the same open browser
  socket: workers change from 1 to 2 without a reload and all five tables still
  match. A screenshot was inspected (`/tmp/coax-live-catchup-browser.png`).
- Missing known IDs, auth-required CLOSED, ties, a one-row unprovable feed,
  changing caps, reversed event delivery, stale generation frames/EOSE, page
  budget exhaustion, timeout and table-output exhaustion have refusal tests.
- An explicit read-only capture of the actual public relay returned 477 events.
  Its verified TypeScript view has **156 issue rows and 4 workers**. The changed
  native app reader, under artificial cap37, reached exactly the same 156/4 in 21
  historical queries and matched all five tables. This observation is a dated
  capture, not a claim that the live queue remains at those counts.

Repeat the public read explicitly, without signing/publishing anything:

```sh
COAX_LIVE_REAL=1 bun test coax/web/project/live-catchup.test.ts -t 'published public queue'
```

The default fixture run is offline. The optional public capture uses the
existing read-only relay adapter and stores its summary in
`/tmp/coax-live-catchup-public.json`. Test-only native/Wasm probe exports and
browser interception are generated into a temporary directory; no debug API or
JavaScript Nostr decision logic ships in the product. No unrelated compiler
bootstrap, iOS build, TestFlight or mutation suites were rerun for this slice.
