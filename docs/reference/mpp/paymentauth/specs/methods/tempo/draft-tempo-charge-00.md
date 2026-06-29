---
title: Tempo charge Intent for HTTP Payment Authentication
abbrev: Tempo Charge
docname: draft-tempo-charge-00
version: 00
category: info
ipr: noModificationTrust200902
submissiontype: IETF
consensus: true

author:
  - name: Jake Moxey
    ins: J. Moxey
    email: jake@tempo.xyz
    org: Tempo Labs
  - name: Brendan Ryan
    ins: B. Ryan
    email: brendan@tempo.xyz
    org: Tempo Labs
  - name: Tom Meagher
    ins: T. Meagher
    email: tom@tempo.xyz
    org: Tempo Labs

normative:
  RFC2119:
  RFC3339:
  RFC4648:
  RFC8174:
  RFC8259:
  RFC8785:
  I-D.httpauth-payment:
    title: "The 'Payment' HTTP Authentication Scheme"
    target: https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/
    author:
      - name: Jake Moxey
    date: 2026-01

informative:
  EIP-2718:
    title: "Typed Transaction Envelope"
    target: https://eips.ethereum.org/EIPS/eip-2718
    author:
      - name: Micah Zoltu
    date: 2020-10
  EIP-55:
    title: "Mixed-case checksum address encoding"
    target: https://eips.ethereum.org/EIPS/eip-55
    author:
      - name: Vitalik Buterin
    date: 2016-01
  TEMPO-TX-SPEC:
    title: "Tempo Transaction Specification"
    target: https://docs.tempo.xyz/protocol/transactions/spec-tempo-transaction
    author:
      - org: Tempo Labs
---

--- abstract

This document defines the "charge" intent for the "tempo" payment method
in the Payment HTTP Authentication Scheme {{I-D.httpauth-payment}}. It
specifies how clients and servers exchange one-time TIP-20 token transfers
on the Tempo blockchain.

--- middle

# Introduction

The `charge` intent represents a one-time payment of a specified amount.
The server may submit the signed transaction any time before the
challenge `expires` auth-param timestamp.

This specification defines the request schema, credential formats, and
settlement procedures for charge transactions on Tempo.

For non-zero charges, Tempo supports two submission modes:

- `pull`: The client signs a transaction and returns a
  `type="transaction"` credential for the server to broadcast.
- `push`: The client broadcasts the transaction and returns a
  `type="hash"` credential for the server to verify onchain.

Servers SHOULD support `pull` mode. Servers MAY additionally support
`push` mode. Servers that do not support both non-zero modes for a
challenge MUST advertise the supported subset via
`methodDetails.supportedModes`.

## Pull Mode (Default)

The default Tempo charge flow uses `pull` mode:

~~~
   Client                        Server                     Tempo Network
      |                             |                             |
      |  (1) GET /api/resource      |                             |
      |-------------------------->  |                             |
      |                             |                             |
      |  (2) 402 Payment Required   |                             |
      |      intent="charge"        |                             |
      |<--------------------------  |                             |
      |                             |                             |
      |  (3) Sign transfer tx       |                             |
      |                             |                             |
      |  (4) Authorization: Payment |                             |
      |-------------------------->  |                             |
      |                             |  (5) Broadcast tx           |
      |                             |-------------------------->  |
      |                             |  (6) Transfer complete      |
      |                             |<--------------------------  |
      |  (7) 200 OK + Receipt       |                             |
      |<--------------------------  |                             |
      |                             |                             |
~~~

# Requirements Language

{::boilerplate bcp14-tagged}

# Terminology

TIP-20
: Tempo's enshrined token standard, implemented as precompiles rather
  than smart contracts. TIP-20 tokens use 6 decimal places and provide
  `transfer`, `transferWithMemo`, `transferFrom`, and `approve` operations.

Tempo Transaction
: An EIP-2718 transaction with type prefix `0x76`, supporting batched
  calls, multiple signature types (secp256k1, P256, WebAuthn), 2D nonces,
  and validity windows.

2D Nonce
: Tempo's nonce system where each account has multiple independent nonce
  lanes (`nonce_key`), enabling parallel transaction submission.

Fee Payer
: An account that pays transaction fees on behalf of another account.
  Tempo Transactions support fee payment via a separate signature
  domain (`0x78`), allowing the server to pay for fees while the client
  only signs the payment authorization.

# Request Schema

The `request` parameter in the `WWW-Authenticate` challenge contains a
base64url-encoded JSON object. The JSON MUST be serialized using JSON
Canonicalization Scheme (JCS) {{RFC8785}} before base64url encoding,
per {{I-D.httpauth-payment}}.

## Shared Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | string | REQUIRED | Amount in base units (stringified number) |
| `currency` | string | REQUIRED | TIP-20 token address (e.g., `"0x20c0..."`) |
| `recipient` | string | REQUIRED | Recipient address |
| `description` | string | OPTIONAL | Human-readable payment description |
| `externalId` | string | OPTIONAL | Merchant's reference (order ID, invoice number, etc.) |

Challenge expiry is conveyed by the `expires` auth-param in
`WWW-Authenticate` per {{I-D.httpauth-payment}}.

## Method Details

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `methodDetails.chainId` | number | OPTIONAL | Tempo chain ID (default: 42431) |
| `methodDetails.feePayer` | boolean | OPTIONAL | If `true`, server pays transaction fees (default: `false`) |
| `methodDetails.memo` | string | OPTIONAL | A `bytes32` hex value. When present, the client MUST use `transferWithMemo` instead of `transfer`. |
| `methodDetails.splits` | array | OPTIONAL | Additional recipients that receive a portion of `amount`. See {{split-payments}}. |
| `methodDetails.supportedModes` | array | OPTIONAL | Supported non-zero submission modes. Values are `"pull"` and/or `"push"`. |

## Submission Modes

The `supportedModes` field allows a server to advertise which non-zero
charge submission modes it supports for a specific challenge.

- `pull` indicates that the client creates a `type="transaction"`
  credential containing a signed Tempo Transaction for the server to
  broadcast.
- `push` indicates that the client creates a `type="hash"` credential
  after broadcasting the transaction itself.

If `supportedModes` is present, it MUST contain at least one of `pull`
or `push`, and clients MUST choose one of the advertised modes.

If `supportedModes` is omitted, clients MAY assume both `pull` and
`push` are supported for that challenge for backwards compatibility with
version 00 implementations. Servers MUST omit `supportedModes` only when
they support both non-zero modes for the challenge. A server that
supports only `pull` or only `push` for a challenge MUST include
`supportedModes` explicitly and omit the unsupported mode.

For zero-amount charges, mode negotiation does not apply. Clients use a
`type="proof"` credential regardless of `supportedModes`.

**Example:**

~~~json
{
  "amount": "1000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00",
  "methodDetails": {
    "chainId": 4217,
    "feePayer": true,
    "supportedModes": ["pull"]
  }
}
~~~

The client fulfills this by signing a Tempo Transaction with
`transfer(recipient, amount)` or `transferWithMemo(recipient, amount, memo)`
on the specified `currency` (token address),
with `validBefore` no later than the challenge `expires` auth-param. The client MAY use a dedicated
`nonceKey` (2D nonce lane) for payment transactions to avoid blocking
other account activity if the transaction is not immediately settled.

If `methodDetails.feePayer` is `true`, the client signs with
`fee_payer_signature` set to `0x00` and `fee_token` empty, allowing the
server to sponsor fees. If `feePayer` is `false` or omitted, the client
MUST set `fee_token` and pay fees themselves.

## Split Payments {#split-payments}

The `splits` field enables a single charge to distribute payment across
multiple recipients atomically. This is useful for platform fees, revenue
sharing, and marketplace payouts.

### Semantics

The top-level `amount` represents the total amount the client pays. Each
entry in `splits` specifies a recipient and the amount they receive. The
primary recipient (the top-level `recipient`) receives the remainder:
`amount` minus the sum of all split amounts.

### Split Entry Schema

Each entry in the `splits` array is a JSON object:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | string | REQUIRED | Amount in base units for this recipient |
| `memo` | string | OPTIONAL | A `bytes32` hex value for `transferWithMemo` |
| `recipient` | string | REQUIRED | Recipient address |

The `amount` field in each split entry MUST be a base-10 integer string
with no sign, decimal point, exponent, or surrounding whitespace. Each
`splits[i].amount` MUST be greater than zero. The syntax and encoding
requirements for `splits[i].memo` are identical to those for
`methodDetails.memo`, but apply only to that split transfer. Address
fields are compared by decoded 20-byte value, not by string form.

### Constraints

Servers MUST NOT generate a request where the sum of `splits[].amount`
values is greater than or equal to `amount`. Clients MUST reject any
request that violates this constraint. This ensures the primary
recipient always receives a non-zero remainder, avoiding the need to
define zero-value transfer semantics.

Additional constraints:

- If present, `splits` MUST contain at least 1 entry. Servers
  SHOULD limit splits to 10 entries to keep gas usage within a
  single block's budget (~29,000 gas per additional TIP-20
  transfer). Servers MAY reject requests exceeding their supported
  split count.
- All transfers MUST target the same `currency` token address.

### Ordering

The order of entries in `splits` is not significant for verification.
Clients SHOULD emit calls in array order. Servers MUST verify that the
required payment effects are present regardless of call ordering.

### Example

~~~json
{
  "amount": "1000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00",
  "methodDetails": {
    "chainId": 4217,
    "feePayer": true,
    "splits": [
      {
        "amount": "50000",
        "recipient": "0xA1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2"
      },
      {
        "amount": "10000",
        "memo": "0x00000000000000000000000000000000000000000000000000000000deadbeef",
        "recipient": "0xC4D5E6F7A8B9C4D5E6F7A8B9C4D5E6F7A8B9C4D5"
      }
    ]
  }
}
~~~

This requests a total payment of 1.00 pathUSD (1,000,000 base units).
The platform receives 0.05 pathUSD, the affiliate receives 0.01 pathUSD
(with a memo), and the primary recipient receives the remaining
0.94 pathUSD (940,000 base units).

### Client Behavior

When `splits` is present, the client MUST produce a transaction whose
on-chain effects include the following `Transfer` or `TransferWithMemo`
events on the `currency` token address:

1. The primary `recipient` receives `amount - sum(splits[].amount)`.
2. Each `splits[i].recipient` receives `splits[i].amount`. If
   `splits[i].memo` is present, the corresponding transfer MUST use
   `transferWithMemo`.

The top-level `methodDetails.memo`, if present, applies to the primary
transfer.

Clients MAY achieve these effects using any valid transaction structure,
including batched calls, smart contract wallet invocations, or
intermediary operations such as token swaps — provided all required
transfer events are emitted atomically.

# Credential Schema

The credential in the `Authorization` header contains a base64url-encoded
JSON object per {{I-D.httpauth-payment}}.

## Credential Structure

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `challenge` | object | REQUIRED | Echo of the challenge from the server |
| `payload` | object | REQUIRED | Tempo-specific payload object |
| `source` | string | OPTIONAL | Payer identifier as a DID (e.g., `did:pkh:eip155:4217:0x...`) |

The `source` field, if present, SHOULD use the `did:pkh` method with the
chain ID applicable to the challenge and the payer's Ethereum address.

## Transaction Payload (type="transaction")

When `type` is `"transaction"`, `signature` contains the complete signed
Tempo Transaction (type 0x76) serialized as RLP and hex-encoded with
`0x` prefix. The transaction MUST authorize payment in the requested
TIP-20 token sufficient to satisfy the challenge parameters, using one
or more `transfer` and/or `transferWithMemo` calls. When `splits` are
present, the transaction MUST include transfers for each split entry
(see {{split-payments}}). This payload type corresponds to `pull` mode.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `signature` | string | REQUIRED | Hex-encoded RLP-serialized signed transaction |
| `type` | string | REQUIRED | `"transaction"` |

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.example.com",
    "method": "tempo",
    "intent": "charge",
    "request": "eyJ...",
    "expires": "2025-02-05T12:05:00Z"
  },
  "payload": {
    "signature": "0x76f901...signed transaction bytes...",
    "type": "transaction"
  },
  "source": "did:pkh:eip155:4217:0x1234567890abcdef1234567890abcdef12345678"
}
~~~

## Hash Payload (type="hash")

When `type` is `"hash"`, the client has already broadcast the transaction
to the Tempo network. The `hash` field contains the transaction hash for
the server to verify onchain. This payload type corresponds to `push`
mode.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `hash` | string | REQUIRED | Transaction hash with `0x` prefix |
| `type` | string | REQUIRED | `"hash"` |

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.example.com",
    "method": "tempo",
    "intent": "charge",
    "request": "eyJ...",
    "expires": "2025-02-05T12:05:00Z"
  },
  "payload": {
    "hash": "0x1a2b3c4d5e6f7890abcdef1234567890abcdef1234567890abcdef1234567890",
    "type": "hash"
  },
  "source": "did:pkh:eip155:4217:0x1234567890abcdef1234567890abcdef12345678"
}
~~~

## Proof Payload (type="proof") {#proof-payload}

When `amount` is `"0"`, no on-chain transfer is required. Instead of
broadcasting a transaction, the client signs an EIP-712 typed-data
message binding the proof to the challenge identifier. This payload
type is used exclusively for zero-amount charges: clients MUST use
`type="proof"` when `amount` is `"0"`, and MUST NOT use `type="proof"`
when `amount` is non-zero. The `supportedModes` field does not apply
to zero-amount charges.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `signature` | string | REQUIRED | EIP-712 signature with `0x` prefix |
| `type` | string | REQUIRED | `"proof"` |

The `source` field MUST be present on proof credentials and MUST be a
`did:pkh:eip155:<chainId>:<address>` DID identifying the signer.

### EIP-712 Domain and Types

The typed-data domain and types are:

~~~json
{
  "domain": {
    "name": "MPP",
    "version": "1",
    "chainId": <challenge methodDetails.chainId>
  },
  "types": {
    "Proof": [
      { "name": "challengeId", "type": "string" }
    ]
  },
  "primaryType": "Proof",
  "message": {
    "challengeId": "<challenge.id>"
  }
}
~~~

The `challengeId` in the message MUST be the `id` from the challenge
that was issued to the client. This binds the signature to exactly one
challenge, preventing cross-challenge replay.

### Proof Verification

Servers MUST verify proof credentials as follows:

1. Verify `credential.source` is present and parses as
   `did:pkh:eip155:<chainId>:<address>`
2. Verify the chain ID from `source` matches
   `methodDetails.chainId` from the challenge
3. Recover the signer from `payload.signature` using the EIP-712
   domain, types, and message described above
4. Verify the recovered signer matches the address in `source`

### Proof Receipt

Upon successful verification, servers return a receipt per
{{I-D.httpauth-payment}} with `reference` set to the challenge `id`
(since no on-chain transaction exists).

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.example.com",
    "method": "tempo",
    "intent": "charge",
    "request": "eyJ...",
    "expires": "2025-02-05T12:05:00Z"
  },
  "payload": {
    "signature": "0xabcdef1234567890...",
    "type": "proof"
  },
  "source": "did:pkh:eip155:4217:0x1234567890abcdef1234567890abcdef12345678"
}
~~~

# Fee Payment

When a request includes `feePayer: true`, the server commits to paying
transaction fees on behalf of the client.

## Server-Paid Fees

When `feePayer: true`:

1. **Client signs with placeholder**: The client signs the Tempo Transaction
   with `fee_payer_signature` set to a placeholder value (`0x00`) and
   `fee_token` left empty. The client uses signature domain `0x76`.

2. **Server receives credential**: The server extracts the client-signed
   transaction from the credential payload.

3. **Server adds fee payment signature**: The server selects a `fee_token` (any
   USD-denominated TIP-20 stablecoin) and signs the transaction using
   signature domain `0x78`. This signature commits to the transaction
   including the `fee_token` and client's address.

4. **Server broadcasts**: The final transaction contains both signatures:
   - Client's signature (authorizing the payment)
   - Server's `fee_payer_signature` (committing to pay fees)

## Client-Paid Fees

When `feePayer: false` or omitted, the client MUST set `fee_token` to a
valid USD TIP-20 token address and pay fees themselves. The server
broadcasts the transaction as-is without adding a fee payer signature.

## Server Requirements

When acting as fee payer, servers:

- MUST maintain sufficient balance of a USD TIP-20 token to pay
  transaction fees
- MAY use any USD-denominated TIP-20 token with sufficient AMM
  liquidity as the fee token
- MAY recover fee costs through pricing or other business logic

## Client Requirements

- When `feePayer: true`: Clients MUST sign with `fee_payer_signature` set
  to `0x00` and `fee_token` empty or `0x80` (RLP null)
- When `feePayer: false` or omitted: Clients MUST set `fee_token` to a
  valid USD TIP-20 token and have sufficient balance to pay fees

# Settlement Procedure

For `intent="charge"` fulfilled via transaction, the client signs a
transaction containing one or more `transfer` or `transferWithMemo` calls.
When `splits` are present, the transaction contains multiple calls (see
{{split-payments}}). If `feePayer: true`, the server adds its fee payer
signature before broadcasting:

~~~
   Client                           Server                        Tempo Network
      |                                |                                |
      |  (1) Authorization:            |                                |
      |      Payment <credential>      |                                |
      |------------------------------->|                                |
      |                                |                                |
      |                                |  (2) If feePayer: true,        |
      |                                |      add fee payment signature |
      |                                |                                |
      |                                |  (3) eth_sendRawTxSync         |
      |                                |------------------------------->|
      |                                |                                |
      |                                |  (4) Transfer executed         |
      |                                |      (~500ms finality)         |
      |                                |<-------------------------------|
      |                                |                                |
      |  (5) 200 OK                    |                                |
      |      Payment-Receipt: <base64url-receipt> |                    |
      |<-------------------------------|                                |
      |                                |                                |
~~~

1. Client submits credential containing signed `transfer` or `transferWithMemo` transaction
2. If `feePayer: true`, server adds fee sponsorship (signs with `0x78` domain)
3. Server broadcasts transaction to Tempo
4. Transaction included in block with immediate finality (~500ms)
5. Server returns a receipt whose `reference` field is the transaction digest

## Hash Settlement

For credentials with `type="hash"`, the client has already broadcast
the transaction. The server verifies the transaction onchain:

~~~
   Client                        Server                     Tempo Network
      |                             |                             |
      |  (1) Broadcast tx           |                             |
      |------------------------------------------------------>    |
      |                             |                             |
      |  (2) Transaction confirmed  |                             |
      |<------------------------------------------------------    |
      |                             |                             |
      |  (3) Authorization:         |                             |
      |      Payment <credential>   |                             |
      |      (with txHash)          |                             |
      |-------------------------->  |                             |
      |                             |                             |
      |                             |  (4) eth_getTransactionReceipt
      |                             |-------------------------->  |
      |                             |                             |
      |                             |  (5) Receipt returned       |
      |                             |<--------------------------  |
      |                             |                             |
      |                             |  (6) Verify receipt         |
      |                             |                             |
      |  (7) 200 OK                 |                             |
      |      Payment-Receipt:       |                             |
      |      <base64url-receipt>    |                             |
      |<--------------------------  |                             |
      |                             |                             |
~~~

**Limitations:**

- Clients MUST NOT use `type="hash"` when `methodDetails.feePayer` is
  `true`. Servers MUST reject such credentials.
- If `methodDetails.supportedModes` is present and does not include
  `push`, clients MUST NOT use `type="hash"` credentials. Servers MUST
  reject such credentials.
- Server cannot modify or enhance the transaction.

## Transaction Verification {#transaction-verification}

Before broadcasting a transaction credential, servers MUST verify:

1. Deserialize the RLP-encoded transaction from `payload.signature`
2. Verify the transaction contains `transfer` or `transferWithMemo`
   calls on the `currency` token address
3. Verify the `amount` matches the challenge request amount
4. Verify the `recipient` matches the challenge request recipient
5. If `methodDetails.memo` is present, verify the transaction uses
   `transferWithMemo` with the matching memo value
6. If `methodDetails.splits` is present, verify the transaction
   includes transfers satisfying each split entry: the primary
   recipient receives `amount - sum(splits[].amount)`, each split
   recipient receives its specified amount, and any required memo
   values are present
7. If `methodDetails.supportedModes` is present, verify it includes
   `pull`

Servers MAY impose additional structural requirements (such as
exact call count or ordering) as local policy before broadcasting.

## Hash Verification {#hash-verification}

For hash credentials, servers MUST fetch the transaction receipt and
verify that it indicates successful execution. Servers MUST verify
that the receipt contains `Transfer` and/or `TransferWithMemo` event
logs emitted by the `currency` token address whose payment effects
satisfy the challenge parameters, including the primary recipient
amount, any split amounts, and any required memo values.

If `methodDetails.supportedModes` is present, servers MUST verify it
includes `push` before accepting a hash credential.

Servers MAY additionally inspect the transaction call data as a
local-policy check, but call-data decoding is not required for
conformance.

## Receipt Generation

Upon successful settlement, servers MUST return a `Payment-Receipt` header
per {{I-D.httpauth-payment}}. Servers MUST NOT include a
`Payment-Receipt` header on error responses; failures are communicated via
HTTP status codes and Problem Details.

The receipt payload for Tempo charge:

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | `"tempo"` |
| `reference` | string | Transaction hash of the settlement transaction |
| `status` | string | `"success"` |
| `timestamp` | string | {{RFC3339}} settlement time |
| `externalId` | string | OPTIONAL. Echoed from the challenge request |

# Security Considerations

## Transaction Replay

Tempo Transactions include chain ID, nonce, and optional `validBefore`/
`validAfter` timestamps that prevent replay attacks:

- Chain ID binding prevents cross-chain replay
- Nonce consumption prevents same-chain replay
- Validity windows limit temporal replay windows

## Amount Verification

Clients MUST parse and verify the `request` payload before signing:

1. Verify `amount` is reasonable for the service
2. Verify `currency` is the expected token address
3. Verify `recipient` is controlled by the expected party
4. If `splits` is present, verify the sum of split amounts is strictly
   less than `amount` and that all split recipients are expected

## Split Payment Risks

When `splits` are present, additional risks apply:

**Recipient Transparency**: Where a human approval step exists, clients
SHOULD present each split recipient and amount so the user can verify
the payment distribution. Clients SHOULD highlight when the primary
recipient receives a small remainder relative to the total `amount`.

**Gas Overhead**: Each additional split adds approximately 29,000 gas
for the TIP-20 precompile transfer execution. A charge with 10 splits
adds approximately 290,000 gas beyond a single-transfer charge. Servers
sponsoring fees via `feePayer: true` MUST budget for the increased gas
limit.

**Split Count Bound**: Servers SHOULD limit `splits` to 10 entries.
See {{split-payments}} for rationale.

## Server-Paid Fees

Servers acting as fee payers accept financial risk in exchange for
providing a seamless payment experience.

**Denial of Service**: Malicious clients could submit valid-looking
credentials that fail onchain, causing the server to pay fees without
receiving payment. Servers SHOULD implement rate limiting and MAY require
client authentication before accepting payment credentials.

**Fee Token Exhaustion**: Servers MUST monitor their fee token balance
and reject new payment requests when balance is insufficient.

# IANA Considerations

## Payment Method Registration

This document registers the following payment method in the "HTTP Payment
Methods" registry established by {{I-D.httpauth-payment}}:

| Method Identifier | Description | Reference |
|-------------------|-------------|-----------|
| `tempo` | Tempo blockchain TIP-20 token transfer | This document |

Contact: Tempo Labs (<contact@tempo.xyz>)

## Payment Intent Registration

This document registers the following payment intent in the "HTTP Payment
Intents" registry established by {{I-D.httpauth-payment}}:

| Intent | Applicable Methods | Description | Reference |
|--------|-------------------|-------------|-----------|
| `charge` | `tempo` | One-time TIP-20 transfer | This document |

--- back

# ABNF Collected

~~~ abnf
tempo-charge-challenge = "Payment" 1*SP
  "id=" quoted-string ","
  "realm=" quoted-string ","
  "method=" DQUOTE "tempo" DQUOTE ","
  "intent=" DQUOTE "charge" DQUOTE ","
  "request=" base64url-nopad

tempo-charge-credential = "Payment" 1*SP base64url-nopad

; Base64url encoding without padding per RFC 4648 Section 5
base64url-nopad = 1*( ALPHA / DIGIT / "-" / "_" )
~~~

# Example

**Challenge:**

~~~http
HTTP/1.1 402 Payment Required
WWW-Authenticate: Payment id="kM9xPqWvT2nJrHsY4aDfEb",
  realm="api.example.com",
  method="tempo",
  intent="charge",
  request="eyJhbW91bnQiOiIxMDAwMDAwIiwiY3VycmVuY3kiOiIweDIwYzAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAiLCJyZWNpcGllbnQiOiIweDc0MmQzNUNjNjYzNEMwNTMyOTI1YTNiODQ0QmM5ZTc1OTVmOGZFMDAiLCJtZXRob2REZXRhaWxzIjp7ImNoYWluSWQiOjQyMTcsInN1cHBvcnRlZE1vZGVzIjpbInB1bGwiXX19",
  expires="2025-01-06T12:00:00Z"
Cache-Control: no-store
~~~

The `request` decodes to:

~~~json
{
  "amount": "1000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00",
  "methodDetails": {
    "chainId": 4217,
    "supportedModes": ["pull"]
  }
}
~~~

This requests a transfer of 1.00 pathUSD (1000000 base units).

**Credential:**

~~~http
GET /api/resource HTTP/1.1
Host: api.example.com
Authorization: Payment eyJjaGFsbGVuZ2UiOnsiaWQiOiJrTTl4UHFXdlQybkpySHNZNGFEZkViIn0sInBheWxvYWQiOnsic2lnbmF0dXJlIjoiMHg3NmY5MDEuLi4iLCJ0eXBlIjoidHJhbnNhY3Rpb24ifSwic291cmNlIjoiZGlkOnBraDplaXAxNTU6NDIxNzoweDEyMzQ1Njc4OTBhYmNkZWYxMjM0NTY3ODkwYWJjZGVmMTIzNDU2NzgifQ
~~~

# Split Payment Example

**Challenge with splits:**

~~~http
HTTP/1.1 402 Payment Required
WWW-Authenticate: Payment id="sP1itPaym3ntEx4mple",
  realm="marketplace.example.com",
  method="tempo",
  intent="charge",
  request="eyJhbW91bnQiOiIxMDAwMDAwIiwiY3VycmVuY3kiOiIweDIwYzAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAiLCJtZXRob2REZXRhaWxzIjp7ImNoYWluSWQiOjQyMTcsImZlZVBheWVyIjp0cnVlLCJzcGxpdHMiOlt7ImFtb3VudCI6IjUwMDAwIiwicmVjaXBpZW50IjoiMHhBMUIyQzNENEU1RjZBMUIyQzNENEU1RjZBMUIyQzNENEU1RjZBMUIyIn1dfSwicmVjaXBpZW50IjoiMHg3NDJkMzVDYzY2MzRDMDUzMjkyNWEzYjg0NEJjOWU3NTk1ZjhmRTAwIn0",
  expires="2025-06-01T12:00:00Z"
Cache-Control: no-store
~~~

The `request` decodes to:

~~~json
{
  "amount": "1000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00",
  "methodDetails": {
    "chainId": 4217,
    "feePayer": true,
    "splits": [
      {
        "amount": "50000",
        "recipient": "0xA1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2"
      }
    ]
  }
}
~~~

This requests a total payment of 1.00 pathUSD. The platform receives
0.05 pathUSD and the merchant receives 0.95 pathUSD. The resulting
transaction must emit the following transfer events:

1. 950,000 to `0x742d...fE00` — merchant receives remainder
2. 50,000 to `0xA1B2...A1B2` — platform fee

# Acknowledgements

The authors thank the Tempo community for their feedback on this
specification.
