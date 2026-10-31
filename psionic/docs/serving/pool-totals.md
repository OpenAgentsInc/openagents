# Recomputing a free pool

`psionic/lib/mesh/totals.coax` is the pure Coax reader for Chapter 26's
bounded free profile. It returns a complete `MtResult` from admitted
beacon, receipt and check DTOs plus an explicit reader policy:

```coax
result := mt_recompute(policy, beacons, receipts, checks);
```

The native wire reader must verify the original event ID and signature,
strict version/body/tags, exact content digest, and unsupported-feature
rules before constructing a DTO. This reducer accepts no raw Nostr JSON
and no externally supplied verification flag. Its exported C ABI belongs
behind that admission boundary. A self-signed beacon grants no owner,
buyer or checker trust.

`MtPolicy` contains `from`, `to`, and independent `providers`, `buyers`
and `checkers` key lists. Every list defaults empty. Policy keys must be
lowercase 64-hex. The window is `[from,to)`, aligned to whole minutes, with
a positive length of at most 3600 seconds. This profile supports 32 beacons,
64 receipts and 128 checks, and matching maximum trust-list sizes. An input
over a bound refuses with `MT_LIMIT`; no input is silently truncated.

## Selection

- A beacon needs a listed provider. For each `(provider,pylon)`, select
  the greatest `(generation,observed_at,event_id)` among observations at
  or before `to`. Its expiry must be at least `to`. Newer offline or expired
  observations suppress earlier capacity. Future observations do not change
  an older snapshot. Online and draining observations contribute their
  claimed capacity; an absent beacon contributes no capacity or fault.
- A receipt needs a listed buyer and provider. The buyer differs from the
  provider. Choose the least `(created_at,event_id)` per `(buyer,request)`
  over that policy-eligible supplied cohort, then apply the finished-time
  window and accepted outcome. Only free, accepted remote decisions with
  one `jobs` unit reach this profile. This excludes local fallback and
  refused requests at the producer/admission boundary.
- A check needs a listed checker, a counted receipt, the matching provider,
  `created_at >= receipt.created_at` and `created_at < to`. The checker
  differs from both buyer and provider. Count the greatest
  `(created_at,event_id)` per `(checker,receipt)`. Several trusted checkers
  can express separate observations about one receipt. Check labels from
  unknown keys grant no standing.

Trust filtering precedes per-record validation and duplicate selection, so
out-of-policy records cannot poison eligible IDs. Exact event replays are
harmless. Contradictory DTOs with the same event ID refuse with
`MT_CONFLICT`; they would also violate the original-event admission
precondition. Other malformed eligible DTOs refuse with `MT_RECORD`.
Invalid windows and key policy return `MT_WINDOW` and `MT_POLICY`.
Consumers must check `result.code == MT_OK` before displaying totals.

## Claims and commitments

`jobs_accepted` and its explicit alias `jobs_reported` count the buyer's
accepted-job claims. `units_jobs` counts their one-job units. They do not
prove independent demand, hardware, model execution or result correctness.
`checks_pass`, `checks_fail` and `checks_inconclusive` count the configured
checkers' distinct latest observations. Missing checks never imply failure;
a fail label does not erase the separate buyer claim. No field derives an
owner link from a beacon or converts standing into money.

The capacity fields are `pylons_online`, `slots_total`, `slots_free`, and
`by_family` in `[unified-memory,gpu,cpu]` order. The single rate bucket
`rate` equals accepted reported jobs in this window. The four paid buckets
in `[bitcoin,testnet,signet,regtest]` order are always zero in this profile.

Each input role returns its counted ID array and count, sorted by lowercase
event ID. Its owned 32-byte digest is SHA-256 of those IDs joined with one
newline and no trailing newline. The empty set uses SHA-256 of the empty
string. This matches NIP-PYLON's commitment rule. Fixed inputs and policy
produce identical totals and digests regardless of arrival or replay order.

The counted-ID strings borrow the admitted input storage; keep that
storage alive and unchanged through result serialization. Counters, ID
arrays and digest bytes are values owned by `MtResult`. A wire collector
must retain each event's decoded bytes rather than reuse one parsing
scratch buffer across DTOs.

Recomputation proves arithmetic over the supplied admitted cohort. It
cannot establish relay-history completeness or truthful signer clocks,
discover an omitted earlier receipt, or independently prove the private
job ran. Public records contain digests and metadata; prompts, results and
the checker's private replay witness stay private.

## Run the example and checks

The standalone example is explicitly a synthetic admitted snapshot:

```sh
coax/bin/coax run psionic/examples/mesh-totals.coax
coax/bin/coax test psionic/lib/mesh/totals.coax --library
coax/bin/coax mutate psionic/lib/mesh/totals.coax --library --root-only --jobs 4
psionic/tests/mesh-totals-native.sh /tmp/mesh-totals-native
```

The root-only mutation gate selects this unit's procedures and retains
dependency baseline checks. Its zero-survivor result does not replace the
SHA-256 dependency's separate qualification. The unit checks cover explicit
trust, replay/order, timestamp and capacity boundaries, field/ID collision
binding, sorted commitments, tombstones, self-check exclusion and unknown
signer/provider poison. The independent Chapter 26 harness exercises the
native signed-event path separately.

The native consumer checks run the example and a maximum-size synthetic
cohort through emitted C in optimized plain and ASan/UBSan builds. The
maximum case fills every output ID list, preserves the input arenas,
compares all three commitments to independent SHA-256 vectors, and refuses
each bound plus one.
