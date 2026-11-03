# Bitcoin explorer HTTP v1

This is the read-only explorer projection for #12704. Its typed source is
[`explorer.proto`](../../proto/openagents/bitcoin/explorer/v1alpha1/explorer.proto).
It adds a new alpha namespace and changes no published protocol contract.
The implementation uses Coax's Bitcoin readers, light chainstate and address
index; Bitcoin Core, Electrs and third-party HTTP APIs are comparison sources
only. No endpoint signs, broadcasts or decides a payment.

Every successful data object includes `snapshot`: `chain`, `height`, `hash`
and `profile: "pow-only"`. Every field in that response describes this one
committed snapshot. Proof of work, header/transaction identity and tracking
are distinct from full script/consensus validation. A reorg may change the
next response's snapshot; consumers refresh a page rather than combining
different snapshots into one balance.

Hashes are lowercase hexadecimal in conventional display order. Raw byte
strings and scripts use forward-order hexadecimal. Amounts are exact integer
satoshis, including zero. This hand-authored HTTP projection uses snake_case
keys and JSON integer numbers; it is not protobuf JSON. Values stay below
the exact integer range of JavaScript numbers.

## REST

| GET path | Result |
| --- | --- |
| `/api/v1/tip` | Snapshot, under `snapshot` |
| `/api/v1/block/HEIGHT_OR_HASH` | Block with header metadata and transaction IDs |
| `/api/v1/transaction/TXID` | Confirmed or archived transaction, inputs, outputs and spend links |
| `/api/v1/address/ADDRESS` | Confirmed balance, transaction history and output spend links |
| `/api/v1/search/QUERY` | A resolved `kind` and canonical `key` |

Search resolves a decimal height, then a full block hash, then a full txid,
then an address for this network. Decimal text is bounded before arithmetic.
No substring or prefix hash search is implied. Address text is decoded to
its exact script and the canonical spelling is returned. Empty valid address
histories have a zero balance and empty arrays; malformed or foreign-network
addresses refuse.

`Occurrence` contains `txid`, `block`, `height` and transaction `order`.
Active block and transaction results have `confirmations`; archived results
have zero. Output states are `unspent`, `spent`, `replaced` (historical
duplicate txid), `unspendable` or `stale`. Only `spent` includes a `spend`
with the spending txid, block hash, input number and height. A confirmed
balance includes immature coinbase outputs; it is not spendable balance.
Transaction history contains each active transaction occurrence once even
when several inputs or outputs use the same address. Array order is not a
promise of newest-first sorting.

## JSON-RPC

`POST /api/v1/rpc` accepts one JSON-RPC 2.0 object. Methods are
`explorer.tip`, `explorer.block`, `explorer.transaction`, `explorer.address`
and `explorer.search`. `params` is an object with exactly one `key` string,
except `explorer.tip`, which takes an empty object. Results are the same
objects as REST. This is an explorer API, not Bitcoin Core's complete RPC.
Duplicate members, unknown request fields and batch requests are refused.
Valid notifications produce HTTP 204 with no JSON body, including when the
method or its parameters are unsupported. `id` must be a string, an integer or null and is echoed exactly.
Errors use standard parse/invalid-request/method/params codes; missing data
uses `-32004`, unavailable or inconsistent storage uses `-32003`.

REST errors contain `error: {code, message}`. Malformed requests return 400,
unknown routes or missing objects 404, unsupported methods 405, oversized
requests 413, and storage/capacity refusal 503. Results are never truncated
into an apparently complete history. Error responses contain no raw storage
paths or internal diagnostics.

## Service boundary

The local service accepts bounded requests, closes each connection after
one response, binds loopback, and sets `Cache-Control: no-store`. It uses no
cookies, credentials or cross-origin permission. Deployment can place its
HTTP listener behind an existing same-origin proxy. Storage is exclusively
owned by the writer; readers publish only whole committed frames and verify
indexed file/header/transaction identities before returning raw bytes.

The front end must show the current network, snapshot height and pow-only
validation profile. It must display an unavailable-data response when a
request fails, and refresh related views after a changed tip or reorg.
