# Migration away from NIP-90 as a canonical market protocol

Date: 2026-08-04

Status: accepted design direction. This document changes forward protocol
guidance; it does not delete historical events, fixtures, packages, receipts,
or evidence.

## Decision

The official Nostr NIP repository now marks NIP-90 **unrecommended** with the
warning that it “got totally out of control” and implementers should prefer
use-case-specific microstandards. OpenAgents previously used the generic
`5000–5999` request, `6000–6999` result, and `7000` feedback ranges as the
common substrate for compute, labor, dataset access, skill search, and market
receipts.

That is no longer the forward design.

OpenAgents will:

1. retain NIP-90 decoding, validation, replay, and read compatibility for
   already-published and independently interoperable events;
2. freeze existing NIP-90 profiles—no new market semantics or job types;
3. treat `packages/nip90` as a compatibility package, not the place where new
   market protocols are designed;
4. design narrowly scoped market microstandards with their own actors,
   invariants, privacy rules, state machines, error codes, and fixture corpora;
5. provide explicit translators where an old NIP-90 event can be represented
   safely in a new profile, without claiming wire equivalence when semantics
   differ.

Immortal's full-three-lane implementation directive includes NIP-90. Here,
“implemented” means exact decoding, validation, relay/client interoperability,
historical replay, translation, negative cases, and pinned compatibility
fixtures. It does **not** reverse this design decision or put new liquidity,
labor, data, or compute protocols back into the generic DVM ranges.

## Why

The generic DVM envelope made it easy to start markets but hard to state what
an event actually authorizes. Compute output, labor acceptance, dataset
licensing, liquidity reservation, fiat settlement, and insurance claims do
not share one safe state machine. Reusing the same request/result/feedback
ranges encouraged overloaded tags, ambiguous `kind:7000` statuses, public
payload leakage, and application-specific rules hidden outside the protocol.

Use-case microstandards allow each market to answer the questions that matter:

- Who may make an offer or accept it?
- Is the quote indicative or is capacity reserved?
- What is private, and which participants may decrypt it?
- What makes an order idempotent?
- Which transitions are monotonic or terminal?
- What proves delivery or settlement?
- What remains an external authority?
- How does a participant recover after relay, client, or provider failure?

## Compatibility policy

Historical truth stays historical truth.

- Existing `5934`/`6934` labor events, `5960`/`6960` dataset events, and
  `kind:7000` feedback remain valid under the version of the profile that
  created them.
- Existing receipt digests are not rewritten. A new projection may reference
  the old event and add an explicit `legacy:nip90` provenance marker.
- Relays may continue to transport NIP-90 events. Transport support does not
  make NIP-90 the canonical application protocol.
- Existing tests remain useful compatibility tests. New conformance suites
  must target the replacement microstandard directly.
- No old event is silently upgraded. Translation is deterministic,
  versioned, and loss-accounted; an unrepresentable field fails closed.

## Replacement map

| Existing use      | Current wire                                          | Forward direction                                                                                                                            |
| ----------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Agentic labor     | NIP-LBR v1 over `5934`/`6934`/`7000`                  | Standalone NIP-LBR v2 microstandard: request, quote, acceptance, progress, delivery, verification, close; kinds reserved only after review   |
| Dataset access    | NIP-DS core plus optional DS-DVM `5960`/`6960`/`7000` | Keep DS core addressable listing/offer/access-contract kinds; replace DS-DVM with a DS-specific private negotiation profile                  |
| Skill search      | NIP-SKL manifests plus NIP-90 search                  | Query manifests directly; use NIP-51 lists, NIP-89 handlers, NIP-99 listings, and a focused private capability-RFQ profile if needed         |
| Compute/inference | Generic DVM jobs                                      | Separate compute microstandard with model/input privacy, deterministic parameters, metering, cancellation, evidence, and streaming semantics |
| Liquidity/swaps   | Previously proposed as another DVM-style service      | [NIP-MKT](MKT.md) negotiated-market base plus MKT-SWP/P2P/PFI/MINT/LSP profiles; never NIP-90                                                |
| Public receipts   | `kind:7000` plus app projections                      | Use the owning microstandard's close event plus NIP-EV/NIP-OC evidence and outcome references                                                |

NIP-MKT is now a drafted OpenAgents NIP with collision-reviewed base kinds
`39600-39609`. Its separately reviewed profile drafts allocate `39610` to
[MKT-SWP](MKT-SWP.md) and `39630` to [MKT-PFI](MKT-PFI.md); every other kind
in `39610-39699` remains unallocated. Those upstream drafts are not an
official numbered NIP or an admitted executable-profile claim. `NIP-LBR v2`
remains a candidate design label with no allocated kinds.

## Common negotiated-market spine

Labor, data, compute, and liquidity can share a small semantic spine without
sharing one giant event kind range:

```text
Offering → private RFQ → signed Quote → signed Order/Acceptance
         → sequenced Status/Evidence → terminal Close
```

[NIP-MKT](MKT.md) now specifies the market form of that shared part:
correlation, idempotency, expiry, quote reservation, cancellation, sequencing,
privacy, recovery, and evidence references. It uses public Provider
Profile/Offering heads and independently signed private records transported
inside NIP-59 gift wraps. Each focused profile defines its own payload,
authority, custody, verification, and settlement laws.

Labor, data, and compute may reuse envelope helpers or the narrow spine only
through their owning microstandards. They do not silently become MKT profiles,
and MKT does not absorb their identity, delivery, acceptance, or outcome
semantics.

## Translation into NIP-MKT

Legacy market history is projected, never upgraded in place. A translator
into a focused NIP-MKT profile must:

1. name the source protocol and revision and preserve the exact source digest;
2. name a deterministic mapping version and the target profile descriptor;
3. list every dropped, defaulted, inferred, or ambiguous field;
4. retain `legacy:nip90` or the corresponding source provenance; and
5. fail closed when the source cannot represent the target signers,
   reservation, custody, state, evidence, or settlement authority.

A translation does not acquire the NIP-MKT signatures, reservation,
idempotency, custody, or settlement meaning that the source never had.

## Document migration rules

When updating historical records:

- preserve statements about what was built or observed at the time;
- add a dated supersession note instead of rewriting old evidence;
- replace forward recommendations that say “extend NIP-90” with the applicable
  microstandard;
- distinguish compatibility support from recommended new implementation;
- do not claim a replacement is live until code, fixtures, receipts, and
  runtime evidence exist.

## Exit criteria

NIP-90 can leave the forward critical path when:

1. each active market has an owned microstandard and pinned fixtures;
2. read-only legacy projection covers all retained NIP-90 evidence;
3. new clients and providers negotiate without publishing new NIP-90 events;
4. public product copy and roadmaps no longer call NIP-90 the universal agent
   market;
5. conformance proves old events remain readable and new events fail closed
   under the wrong profile.

Until then, the correct description is: **NIP-90 compatibility remains;
NIP-90 expansion is frozen.**
