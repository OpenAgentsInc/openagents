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

Populate a chapter-125 address-index state, then serve both API and page:

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
remain the next #12704 step; regtest qualification does not complete that
requirement.

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
