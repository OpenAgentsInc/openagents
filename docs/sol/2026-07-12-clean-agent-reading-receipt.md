# Sol clean-agent bounded reading receipt

- Class: receipt
- Date: 2026-07-12
- Snapshot: `39a4c9177b` plus the schema-versioned product-issue artifact
- Proof rung: deterministic documentation-policy proof
- Final disposition: passed; retained as a regression gate
- Dispatch: no
- Owner: Sol documentation cleanup

## Question

Can a clean reader begin with only [`README.md`](./README.md), follow no more
than two local links, and recover the same actionable truth as the current
authorities without treating historical prose as dispatch?

## Executable answer

`scripts/check-sol-docs.test.ts` executes the bounded reading contract and
`scripts/check-sol-docs.ts` runs it in the normal offline guard. From the index
the reader reaches the master, claim protocol, and receipt index in one hop.
The test requires:

- the master owner-decision and non-goal sections;
- the exact product-issue projection in `live-roadmap-issues.json`;
- a non-empty dependency-aware current execution order;
- the six distinct proof rungs;
- the `CLAIM` and `CLAIM-RELEASE` coordination shapes; and
- a dispatch reading order with no historical, retired, receipt, or redirect
  target.

## Verification

- `bun run check:sol-docs` — pass;
- `bun run test:sol-docs` — the named clean-agent reading regression passes;
- `bun scripts/check-sol-docs.ts --live` — the pinned non-docs product issue
  set equals live GitHub metadata.

This is a documentation reachability proof, not a claim that an LLM will make
good product decisions from arbitrary historical prose. The guard proves the
bounded entry path contains the required facts and denies known stale routes.

## Observed bounded answer

- Authority is layered: repository law, owning schemas/contracts/promises,
  master product direction, live issues/claims, and current implementation
  evidence each own a distinct kind of truth.
- The pinned product set is #8547, #8566, #8574, #8597, #8636, #8676, #8677,
  #8689, #8707, and the P1-parallel AUDIO-0…AUDIO-8 lane
  (#8733–#8741). The product projection intentionally excludes
  `area:docs`; it is not a claim that the snapshot replaces live GitHub.
- The next-ready structure is a partial order: #8676 and CUT-09 may run in
  parallel while closed CUT-26 remains evidence; CUT-09 precedes #8677;
  CUT-27 closes after those local cutover
  dependencies; #8547 acceptance precedes #8636's live hybrid receipt.
- The six proof rungs remain code-landed, fixture-proven,
  deployed/distributed, live-proven, owner-accepted, and closed.
- Physical iOS plus Android-emulator evidence is accepted; nothing gates on a
  physical Android device. Persona-neutral voice remains required but needs a
  bounded leaf; it is neither paused nor proof of persona/Sarah revival.
- Backroom accepts only fully obsolete narrative after conclusion extraction,
  link migration, and a pushed exact-byte provenance manifest.

## Bounded limitations

Offline reading cannot prove which claim is unowned now, whether GitHub changed
after the pinned snapshot, or whether a remote Backroom commit remains
available. Exact operational selection therefore requires the documented live
issue/comment refresh. The test passes because the entry path states that
boundary explicitly rather than manufacturing live currency.
