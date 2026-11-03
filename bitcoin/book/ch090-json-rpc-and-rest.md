# 90. JSON-RPC and REST

> **Status: partly implemented.** Native CPU, read-only explorer HTTP v1.
Bitcoin Core's full RPC surface, cookie authentication, wallet/miner/mempool
methods, TLS termination and full-node deployment remain planned.

A block explorer needs a small, coherent view of the chain: the tip, a block,
a transaction and an address's history. This chapter implements that subset
for [#12704](https://github.com/OpenAgentsInc/openagents/issues/12704), on
chapter 91's transaction index and chapter 125's address index. Its answers
come from our own durable state and block files. Bitcoin Core and address
indexers are comparison sources, never HTTP backends for the product.

## Start with the contract

The source contract is
[`explorer.proto`](../../proto/openagents/bitcoin/explorer/v1alpha1/explorer.proto),
in a new alpha namespace, and the
[HTTP v1 specification](../../docs/protocol/bitcoin-explorer-v1.md).
No published OpenAgents contract changes. This is a hand-authored JSON
projection, not protobuf JSON and not Bitcoin Core RPC compatibility.

The [service](../examples/live/explorer-api.coax) accepts GET requests for:

| Route | Answer |
| --- | --- |
| `/api/v1/tip` | Network, height, tip hash and validation profile |
| `/api/v1/block/HEIGHT_OR_HASH` | Header metadata, active status, confirmations and txids |
| `/api/v1/transaction/TXID` | Original serialization, inputs, outputs and spend links |
| `/api/v1/address/ADDRESS` | Confirmed balance, deduplicated history and outputs |
| `/api/v1/search/QUERY` | A canonical block, transaction or address key |

Hashes use conventional display order; raw bytes use forward hexadecimal.
Amounts are exact integer satoshis. Genesis is reconstructed from the pinned
network parameters even though Core-style txindex and UTXO state omit its
coinbase. Its output is explicitly unspendable. Stale transactions remain
available, with zero confirmations and stale output states.

## One response, one snapshot

Each data object contains `snapshot`, including `profile: "pow-only"`.
The reader opens only whole KV frames and checks that the address,
transaction and light cursors agree. It never repairs or truncates the
writer's files. A physically incomplete final frame exposes the preceding
whole snapshot. A malformed interior frame, mismatched network or stale
index refuses the request.

The process retains a read-only snapshot between requests. A changed state
file is reopened before serving another data response. Queries use the same
active-block membership as the light chain, so an address history, balance
and all its spend links describe one branch. Front ends refresh related
views when the snapshot hash changes, including a same-height reorg.

Block reads check Core framing, the indexed extent and header identity,
then recompute transaction Merkle and activated witness commitments.
Transaction lookup must land on an actual transaction boundary with the
expected full txid and exact serialized size. The API therefore refuses a
witness-only storage edit before returning raw transaction bytes. None of
these checks executes input scripts or claims full consensus validation.

## JSON-RPC is another envelope

`POST /api/v1/rpc` accepts one JSON-RPC 2.0 request. Its methods are
`explorer.tip`, `explorer.block`, `explorer.transaction`, `explorer.address`
and `explorer.search`. Each takes `params: {"key": "..."}`, except tip's
empty object. Results are identical to REST. IDs preserve their original
JSON spelling; integer IDs must fit JavaScript's exact integer range.

Valid notifications receive HTTP 204 with no JSON body. Duplicate/unknown
envelope members, batches and malformed request parameters refuse. Standard parse/request/method/parameter error codes are
used; absent data is `-32004`, unavailable storage is `-32003`. RPC errors
still return a JSON-RPC object with HTTP 200. REST uses 400, 404, 405, 413
or 503 with a small JSON error. Storage paths and internal diagnostics do
not appear in error responses.

## Run the service

First create and populate a chapter-125 indexed state. Then:

```shell
coax/web/tool.sh bitcoin/examples/live/explorer-api.coax regtest STATE BLOCKFILE 8822
```

Use `signet` for signet state. `BLOCKFILE` may instead be a directory of
unmasked `blkNNNNN.dat` files. The process binds loopback; an optional final
integer limits accepted connections for a bounded test run. Every response
sets `Cache-Control: no-store` and closes its connection. Requests have
overall receive/send deadlines and size bounds.

The optional next argument enables the [Coax explorer interface](../explorer/README.md)
and its exact static asset allowlist. Build it first, then append
`0 bitcoin/explorer` after the port for an unlimited local page service.

This adapter retains a 64 MiB table and reads a state log up to 256 MiB.
Address responses require at most 16,384 output/history rows and all JSON
must fit 32 MiB. Exhaustion returns 503 rather than silently truncating a
history. These are explicit adapter limits. Full signet node/storage
integration remains the epic's end-to-end acceptance work.

## Check both envelopes and real files

[explorer-request.coax](../../coax/lib/bitcoin/explorer-request.coax),
[explorer-rpc.coax](../../coax/lib/bitcoin/explorer-rpc.coax),
[explorer-store.coax](../../coax/lib/bitcoin/explorer-store.coax) and
[explorer-body.coax](../../coax/lib/bitcoin/explorer-body.coax) own request
and stored-data admission. Together their root mutation gates kill 549
mutants with zero survivors. [explorer-json.coax](../../coax/lib/bitcoin/explorer-json.coax)
formats those admitted values into caller-owned buffers.

The [worked program](../examples/explorer-api.coax) prints the genesis tip
and block, then refuses a changed expected header hash. Its
[exact transcript](../fixtures/explorer-api.out) is the golden result.
[explorer.py](../../coax/tests/bitcoin/explorer.py) drives the real HTTP
process and the real durable writer. Independent raw-transaction replay
computes every expected block, transaction, address balance and spend link.
It checks both reorg directions, shared/stale transactions, same-block
spends, genesis, RPC errors, torn state, corrupt block storage and a missing
state file. The same journey runs with both processes under ASan/UBSan.
See the [verification record](../../docs/verification/bitcoin-explorer-api-2026-11-03.md).

## Specifications

The envelopes follow [JSON-RPC 2.0](https://www.jsonrpc.org/specification),
[HTTP/1.1 framing, RFC 9112](https://www.rfc-editor.org/rfc/rfc9112.html) and
[JSON, RFC 8259](https://www.rfc-editor.org/rfc/rfc8259.html), within the
bounded single-object profile documented above.
