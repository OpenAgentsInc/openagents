# Explorer mempool observations — 2026-11-03

Epic #12704's optional chapter 82 profile. The confirmed explorer contract
is unchanged. The new projection is explicitly unvalidated and incomplete;
it tracks local observations, not Core acceptance parity or relay policy.

The `mempool.coax` and `mempool-feed.coax` root mutation gates kill 238 and
57 mutants respectively, with zero survivors. No equivalents are marked.
Checks precede the implementation and cover parent coins, exact identifiers,
fees, duplicate and null inputs, conflicts, maturity, money, coin/table/batch
edges and unchanged pool state after refusal. The existing fee primitive is
reused. The worked example matches its exact transcript in debug and release.

Core 32.1 validates three dependent secretless regtest transactions and
records their mempool entries in
`bitcoin/fixtures/explorer-core-reorg/mempool.json`. Their fees are 1,000,
2,000 and 1,000 satoshis. The independent oracle checks Core's weight,
virtual size and wtxid, then stops Core. Cookies and secrets are not retained.

The independent wire peer checks the actual Coax collector's complete
version addresses and relay request, exact witness getdata entries, ping,
fragmentation, child-before-parent delivery, deduplication, unsolicited
transaction refusal and corrupt-checksum refusal. The collector observes
new announcements during a bounded interval; it does not fetch a peer's
entire mempool.

The native API and one open headless browser pass the observation sequence
`3 → 1 → 0 → 1` through mining and both Core-generated reorgs in release
and checked ASan/UBSan. Every transaction's fee, weight, size and rate matches
the recorded Core values. Wrong-network, malformed, truncated and oversized
feeds return 503 and recover after an atomic valid replacement. POST refuses.
The browser refreshes observations even when the confirmed tip is unchanged.

Desktop and mobile captures are retained under ignored `build/verification/`.
Visual review caught a clipped mobile heading and an early blank desktop
capture. The heading now fits its available width, and capture waits for a
rendered frame. The fees, full identifiers, counts and unvalidated label are
visible; copy controls use only public txids.

The native restart checks also pass 105 independent HTTP comparisons per
profile with bounded replay pause/503/continuation, missing interior journal
segments, unavailable archive links, corrupt sequences, both reorgs and
torn-tail recovery. The bounded file oracle checks 1,220 independent actual
files under checked ASan/UBSan, now advancing one whole frame per call.
The current native binaries also pass 133 Core/Electrs HTTP comparisons per
profile through both real reorgs. The teaching API's entry declares its
compile-time budget explicitly and passes the original 75 HTTP/RPC responses
with both API and index writer under ASan/UBSan.

Compiler: coax0 0.48.0, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
The new protobuf package and owned descriptor were generated with pinned
protoc 29.3 / protobuf 5.29.3; compatibility and consumer profiles pass.

Repeatable commands are in [the explorer README](../../bitcoin/explorer/README.md).
The separate full-signet/Core/Electrs acceptance gate is still pending.

The Bitcoin registry, closed example inventory, local links and 861 exact
source listings pass. tools-check inventories 1,766 tracked tools and reports
15 unrelated missing-reason headers; the new independent oracle has Firm
reason 2. The pre-existing full-book capability-evidence mismatch is outside
this change.
