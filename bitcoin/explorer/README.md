# Bitcoin explorer

A Coax/WebGPU interface over the node's read-only
[HTTP v1 projection](../../docs/protocol/bitcoin-explorer-v1.md). It shows the
network, committed snapshot and pow-only profile. Search accepts a block
height or hash, full txid or address. Blocks link to transactions;
transactions link to their block, previous transactions, output addresses
and spending transactions. Address pages show confirmed balances, history
and output states. Balances include immature coinbase outputs.

Build from the repository root:

```sh
coax/web/tool.sh bitcoin/explorer/build.coax
```

Build the full-chain native service:

```sh
coax/web/tool.sh bitcoin/explorer/native-build.coax
```

Fetch headers through our P2P sync, then admit raw block files and build the
complete explorer state. `BLOCK_DIRECTORY` can be a Core `blocks` directory;
Core is an import source, never the query backend. The node checks every
active body through its own header, Merkle, witness and light-coin rules.

```sh
coax/web/tool.sh bitcoin/examples/live/sync-node.coax --chain signet --now UNIX_SECONDS --peer HOST:PORT --blocks 1 --headers-out HEADERS --seconds 660
bitcoin/explorer/build/native-release/explorer-node signet STATE BLOCK_DIRECTORY HEADERS UNIX_SECONDS 8822 bitcoin/explorer
```

The optional trailing arguments are `ARENA_BYTES OVERFLOW_DIRECTORY
SEGMENT_BYTES`. The default arena capacity is 128 GiB of virtual address
space, committed as needed; a small fixture can use 268435456 bytes. Journal
segments target one GiB and preserve the unchanged KV v1 format. New segments
can use a dedicated overflow directory on another filesystem; existing
segments are found in either configured directory. The writer exclusively
locks state and overflow storage. An atomic frame can exceed a smaller
configured segment target. Files and names require exclusive ownership;
raw imported records must remain immutable. A partial interior journal never
becomes a repaired prefix. The node keeps its own `STATE.headers` archive.

Port zero runs one import slice and exits. Each slice has a thirteen-minute
budget and exits 75 at a synced checkpoint when more blocks remain. Re-run
the same command to resume. No data pages are served before the admitted tip
reaches the preferred known header. Once serving, the node watches atomic
replacements of `HEADERS` and follows reorgs using its retained headers and
raw block directory. A reorg is one state frame; the bounded workspace
refuses branches larger than 4,096 blocks, 128 MiB of bodies or a 64 MiB
state frame. Parser bounds are 30,000 inputs, 120,000 outputs, 100,000 witness
items per transaction and 70,000 transactions per block; the header tree
holds 600,000 entries. Exceeding a bound refuses explicitly.

For the smaller chapter-125 teaching adapter, populate an address-index
state, then serve both API and page:

```sh
coax/web/tool.sh bitcoin/examples/live/explorer-api.coax regtest STATE BLOCKFILE 8822 0 bitcoin/explorer
```

Use `signet` for signet state. The listener is loopback. `0` disables the
test connection limit. The final argument enables this repository-local
page: its index, compiled Wasm and generated shader are served beside an
exact allowlist of unchanged shared browser-host modules from `coax/web/`.
No arbitrary path is served. Use the same root layout when packaging it.
Without the final argument the process serves only the API.

The browser updates its tip every ten seconds and refreshes the complete
page when the hash changes, including a same-height reorg. Failed requests
clear visible data; recovery requests a fresh page. Navigation cancels old
requests, and response ownership prevents late replies from replacing newer
snapshots. Engine replacement reissues effects canceled by the host.

Lists show twenty rows per page, with previous/next controls. Hashes and
addresses wrap fully; public identifiers can be copied. `/` focuses search;
Enter submits, Escape leaves it. Text supports caret movement, selection,
paste and copy through the shared keyboard adapter. Tab reaches the mirrored
controls and the canvas draws their focus. The fallback describes the API
when WebGPU is unavailable.

The browser bounds one reply at 16 MiB and 65,536 JSON nodes. A reply that
does not fit becomes unavailable data; it is never presented as a complete
truncated history. The service has its own documented finite bounds. Full
signet writer/storage integration and live Core/address-reference acceptance
remain a separate #12704 acceptance gate; native regtest qualification does
not complete that requirement.

Checks:

```sh
coax/bin/coax mutate bitcoin/explorer/model.coax --root-only --jobs 8
coax/bin/coax mutate coax/lib/bitcoin/explorer-assets.coax --root-only --jobs 8
coax/web/tool.sh wgsl-module bitcoin/explorer/gpu.coax bitcoin/explorer/explorer.wgsl bitcoin/explorer/gpu.module --check
python3 coax/tests/bitcoin/explorer-browser.py --sanitize-with coax/bin/coax
```

The independent fixture oracle computes balances from raw blocks and drives
the real durable writer and API. The headless browser checks search,
transaction spent/unspent states, links, keyboard focus, desktop/mobile,
both reorg directions, unavailable state and recovery. Screenshots are
written under ignored `build/verification/`. See the
[verification record](../../docs/verification/bitcoin-explorer-ui-2026-11-03.md).

Native storage and process checks:

```sh
coax/web/tool.sh bitcoin/explorer/native-build.coax --sanitize
python3 coax/tests/bitcoin/explorer-node.py --node bitcoin/explorer/build/native-sanitized/explorer-node --browser
python3 coax/tests/bitcoin/kvstream.py --coax coax/bin/coax --mode sanitized
coax/bin/coax mutate coax/lib/bitcoin/addrindex-private.coax --root-only --jobs 8
coax/bin/coax mutate coax/lib/bitcoin/kvstream.coax --root-only --jobs 8
```
