# Recorded Bitcoin Lab source

The unlisted `/worlds/bitcoin-lab` renders actual regtest blocks from the
retained Bitcoin Core 32.1 / Electrs recording in
`bitcoin/fixtures/explorer-core-reorg`. This source is **recorded**, not live.
Its observation time is unknown. No Core or Electrs process is needed to
rebuild it or run the realm. No book chapter gains a visual-exercise status.

Rebuild into a **new** directory, check it independently, then freeze the
acknowledged facts into the realm:

```sh
coax/web/tool.sh bitcoin/lab/record.coax /tmp/bitcoin-recording
python3 bitcoin/lab/oracle.py /tmp/bitcoin-recording
coax/web/tool.sh bitcoin/lab/recover.coax /tmp/bitcoin-recording/state.kv /tmp/bitcoin-cut.kv
coax/web/tool.sh bitcoin/lab/project.coax /tmp/bitcoin-recording apps/openagents-web/bitcoin-lab-data.coax
coax/web/tool.sh bitcoin-lab-map
bun coax/web/realms.ts
coax/web/wasm.sh apps/openagents-web/bitcoin-lab.coax apps/openagents-web/bitcoin-lab-face.c apps/openagents-web/build/bitcoin-lab.wasm
```

`record.coax` starts with empty validated state at the actual genesis. It
admits raw headers/bodies, executes the complete block validator and stages
coins, undo, transaction/address archives and V/X metadata together. A private
CoinsCache flush is appended and fsynced before its base, cache generation
and active index are published. The three durable states are:

| Snapshot | Height | Detach | Attach | Retained blocks |
| --- | --- | --- | --- | --- |
| A | 102 | 0 | 102 | 103 |
| B | 103 | 2 | 3 | 106 |
| C | 104 | 3 | 4 | 108 |

Schema-2 metadata (`X`, byte 2) is 48 bytes: schema, network, height,
next archive sequence (four little-endian u32s), then the committed hash.
Only validated V origins enter these writers. L/I/E origins are refused;
there is no conversion or migration from the old pow-only explorer. A
disjoint disposable query view shortens checked H receipts for the existing
read-only codecs. That view contains foreign compatibility metadata, so the
validator refuses it. It is never persisted, cached or used to label a source.

The recorder's JSON is an internal build artifact. `project.coax` writes
immutable typed Coax arrays. The realm changes all facts together when A/B/C
changes; there are no requests or late fixture callbacks. The independent
Python oracle parses raw transactions/blocks, computes txids and weight,
replays active UTXOs, and checks output/spend states and retained Core/Electrs
answers. `recover.coax` writes and reloads physical journal cuts and checks
that V, X, coins and indexes recover at the same complete frame.

This is a deliberately bounded source: 128 headers/blocks, 16 transactions
per block and 16 inputs/outputs per transaction; all excesses refuse the
recording. The realm has 103/106/108 retained blocks and shows a bounded
height window plus stale blocks, with explicit omitted counts. Script/address
search selects an occurrence; it is not a complete script-history report.

The alpha adapter design is in
[`docs/protocol/bitcoin-lab-v1.md`](../../docs/protocol/bitcoin-lab-v1.md).
#12803/#12804 remain open for the allowlisted live adapter, snapshot-bound
paging/tickets, general graph expansion, script history, ordinary explorer
handoff and their broader animation/restore criteria. No live or mainnet
validation claim follows from this recording.
