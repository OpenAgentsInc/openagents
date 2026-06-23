---
title: Tempo Session Intent for HTTP Payment Authentication
abbrev: Tempo Session
docname: draft-tempo-session-00
version: 00
category: info
ipr: noModificationTrust200902
submissiontype: IETF
consensus: true

author:
  - name: Liam Horne
    ins: L. Horne
    email: liam@tempo.xyz
    org: Tempo Labs
  - name: Georgios Konstantopoulos
    ins: G. Konstantopoulos
    email: georgios@tempo.xyz
    org: Tempo Labs
  - name: Dan Robinson
    ins: D. Robinson
    email: dan@tempo.xyz
    org: Tempo Labs
  - name: Brendan Ryan
    ins: B. Ryan
    email: brendan@tempo.xyz
    org: Tempo Labs
  - name: Jake Moxey
    ins: J. Moxey
    email: jake@tempo.xyz
    org: Tempo Labs

normative:
  RFC2119:
  RFC3339:
  RFC4648:
  RFC8174:
  RFC8259:
  RFC9110:
  RFC9111:
  RFC9457:
  I-D.httpauth-payment:
    title: "The 'Payment' HTTP Authentication Scheme"
    target: https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/
    author:
      - name: Jake Moxey
    date: 2026-01

informative:
  RFC8610:
  EIP-712:
    title: "Typed structured data hashing and signing"
    target: https://eips.ethereum.org/EIPS/eip-712
    author:
      - name: Remco Bloemen
    date: 2017-09
  SSE:
    title: "Server-Sent Events"
    target: https://html.spec.whatwg.org/multipage/server-sent-events.html
    author:
      - org: WHATWG
  TEMPO-TX-SPEC:
    title: "Tempo Transaction Specification"
    target: https://docs.tempo.xyz/protocol/transactions/spec-tempo-transaction
    author:
      - org: Tempo Labs
  TIP-20:
    title: "TIP-20 Token Standard"
    target: https://docs.tempo.xyz/protocol/tip20/spec
    author:
      - org: Tempo Labs
---

--- abstract

This document defines the "session" intent for the "tempo" payment method
in the Payment HTTP Authentication Scheme. It specifies unidirectional
streaming payment channels for incremental, voucher-based payments
suitable for low-cost the metered services.

--- middle

# Introduction

This document is published as Informational but contains normative requirements using BCP 14 keywords {{RFC2119}} {{RFC8174}} to ensure interoperability between implementations. Payment method specifications that reference this document inherit these requirements.

The `session` intent establishes a unidirectional streaming payment channel
using on-chain escrow and off-chain {{EIP-712}} vouchers. This enables high-
frequency, low-cost payments by batching many off-chain voucher signatures
into periodic on-chain settlements.

Unlike the `charge` intent which requires the full payment amount upfront, the
`session` intent allows clients to pay incrementally as they consume
services, paying exactly for resources received.

## Use Case: LLM Token Streaming

Consider an LLM inference API that charges per output token:

1. Client requests a streaming completion (SSE response)
2. Server returns 402 with a `session` challenge
3. Client opens a payment channel on-chain, depositing funds
4. Server begins streaming response
5. As response streams, or over incremental requests, client signs vouchers with increasing amounts
6. Server settles periodically or at stream completion

The client pays exactly for tokens received, with no worst-case reservation.

## Session Flow

The following diagram illustrates the Tempo session flow:

~~~
   Client                        Server                     Tempo Network
      |                             |                             |
      |  (1) GET /api/resource      |                             |
      |-------------------------->  |                             |
      |                             |                             |
      |  (2) 402 Payment Required   |                             |
      |      intent="session"        |                             |
      |      (includes challengeId) |                             |
      |<--------------------------  |                             |
      |                             |                             |
      |  (3) GET /api/resource      |                             |
      |      Authorization: Payment |                             |
      |      action="open"          |                             |
      |      (includes signed tx)   |                             |
      |-------------------------->  |                             |
      |                             |                             |
      |                             |  (4) open(...)               |
      |                             |-------------------------->  |
      |                             |                             |
      |  (5) 200 OK + Receipt       |                             |
      |      (streaming response)   |                             |
      |<--------------------------  |                             |
      |                             |                             |
      |  (6) HEAD /api/resource     |                             |
      |      action="voucher"       |                             |
      |      (top-up, same URI)     |                             |
      |-------------------------->  |                             |
      |                             |                             |
      |  (7) 200 OK + Receipt       |                             |
      |<--------------------------  |                             |
      |                             |                             |
      |  (8) GET /api/resource      |                             |
      |      action="voucher"       |                             |
      |      (incremental request)  |                             |
      |-------------------------->  |                             |
      |                             |                             |
      |  (9) 200 OK + Receipt       |                             |
      |      (additional response)  |                             |
      |<--------------------------  |                             |
      |                             |                             |
      |  (10) GET /api/resource     |                             |
      |       action="close"        |                             |
      |-------------------------->  |                             |
      |                             |  (11) close(voucher)        |
      |                             |-------------------------->  |
      |                             |                             |
      |  (12) 200 OK + Receipt      |                             |
      |       (includes txHash)     |                             |
      |<--------------------------  |                             |
      |                             |                             |
~~~

Voucher updates and close requests are submitted to the **same resource
URI** that requires payment. This allows sessions to work on any endpoint
without dedicated payment control plane routes. Servers SHOULD support
voucher updates via any HTTP method; clients MAY use `HEAD` for pure
voucher top-ups when no response body is needed.

## Concurrency Model {#concurrency}

A channel supports one active session at a time. The cumulative voucher
semantics ensure correctness—each voucher advances a single monotonic
counter. The channel is the unit of concurrency; no additional session
locking is required.

When a client sends a new streaming request on a channel that already
has an active session, servers SHOULD terminate the previous session and
start a new one. Voucher updates MAY arrive on separate HTTP connections
(including HTTP/2 streams) and MUST be processed atomically with respect
to balance updates.

Servers MUST ensure that voucher acceptance and balance deduction are
serialized per channel to prevent race conditions.

# Requirements Language

{::boilerplate bcp14-tagged}

# Terminology

Streaming Payment Channel
: A unidirectional off-chain payment mechanism where the payer deposits
  funds into an escrow contract and signs cumulative vouchers authorizing
  increasing payment amounts.

Voucher
: An {{EIP-712}} signed message authorizing a cumulative payment amount for
  a specific channel. Vouchers are monotonically increasing in amount.

Channel
: A payment relationship between a payer and payee, identified by a
  unique `channelId`. The channel holds deposited funds and tracks
  cumulative settlements.

Settlement
: The on-chain {{TIP-20}} transfer that converts off-chain voucher
  authorizations into actual token movement.

Authorized Signer
: An address delegated to sign vouchers on behalf of the payer.
  Defaults to the payer if not specified.

Base Units
: The smallest indivisible unit of a TIP-20 token. TIP-20 tokens use
  6 decimal places; one million base units equals 1.00 tokens.

# Encoding Conventions {#encoding}

This section defines normative encoding rules for interoperability.

## Hexadecimal Values

All byte arrays (addresses, hashes, signatures, channelId) use:

- Lowercase hexadecimal encoding
- `0x` prefix
- No padding or truncation

| Type | Length | Example |
|------|--------|---------|
| address | 42 chars (0x + 40 hex) | `0x742d35cc6634c0532925a3b844bc9e7595f8fe00` |
| bytes32 | 66 chars (0x + 64 hex) | `0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f` |
| signature | 130-132 chars (0x + 128-130 hex) | 65-byte (r‖s‖v) or 64-byte EIP-2098 compact |

Implementations MUST use lowercase hex. Implementations SHOULD accept
mixed-case input but normalize to lowercase before comparison.

## Numeric Values

Integer values (amounts, timestamps) are encoded as decimal strings in
JSON to avoid precision loss with large numbers:

| Field | Encoding | Example |
|-------|----------|---------|
| `cumulativeAmount` | Decimal string | `"250000"` |
| `requestedAt` | Decimal string (Unix seconds) | `"1736165100"` |
| `chainId` | JSON number | `4217` |

The `chainId` uses JSON number encoding as values are small enough to
avoid precision issues.

## Timestamp Format

HTTP headers and receipt fields use {{RFC3339}} formatted timestamps:
`2025-01-06T12:05:00Z`. Timestamps in EIP-712 signed data use Unix
seconds as decimal strings.

# Channel Escrow Contract

Streaming payment channels require an on-chain escrow contract that holds
user deposits and enforces voucher-based withdrawals.

## Channel State {#channel-state}

Each channel is identified by a unique `channelId` and stores:

| Field | Type | Description |
|-------|------|-------------|
| `payer` | address | User who deposited funds |
| `payee` | address | Server authorized to withdraw |
| `token` | address | {{TIP-20}} token address |
| `authorizedSigner` | address | Authorized signer (0 = payer) |
| `deposit` | uint128 | Total amount deposited |
| `settled` | uint128 | Cumulative amount already withdrawn by payee |
| `closeRequestedAt` | uint64 | Timestamp when close was requested (0 if not) |
| `finalized` | bool | Whether channel is closed |

The `channelId` MUST be computed deterministically using the escrow
contract's `computeChannelId()` function:

~~~
channelId = keccak256(abi.encode(
    payer,
    payee,
    token,
    salt,
    authorizedSigner,
    address(this),
    block.chainid
))
~~~

Note: The `channelId` includes `address(this)` (the escrow contract
address) and `block.chainid`, explicitly binding the channel to a
specific contract deployment and chain. Clients MUST use the contract's
`computeChannelId()` function or equivalent logic to ensure
interoperability.

## Channel Lifecycle

Channels have no expiry—they remain open until explicitly closed.

~~~
┌─────────────────────────────────────────────────────────────────┐
│                          CHANNEL OPEN                           │
│       Client deposits tokens, channel created with unique ID    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       SESSION PAYMENTS                           │
│          Client signs vouchers, server provides service         │
│          Server may periodically settle() to claim funds        │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────────────┐
│   COOPERATIVE CLOSE     │     │          FORCED CLOSE           │
│  Server calls close()   │     │  1. Client calls requestClose() │
│   with final voucher    │     │  2. Wait 15 min grace period    │
│                         │     │  3. Client calls withdraw()     │
└─────────────────────────┘     └─────────────────────────────────┘
              │                               │
              └───────────────┬───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        CHANNEL CLOSED                           │
│           Funds distributed, channel finalized                  │
└─────────────────────────────────────────────────────────────────┘
~~~

## Contract Functions

Compliant escrow contracts MUST implement the following functions. The
signatures shown are a reference implementation; alternative implementations
MAY use different parameter types (e.g., `uint256` instead of `uint128`)
as long as the semantics are preserved.

### open

Opens a new channel with escrowed funds.

| Parameter | Type | Description |
|-----------|------|-------------|
| `payee` | address | Server's address authorized to withdraw funds |
| `token` | address | {{TIP-20}} token contract address |
| `deposit` | uint128 | Amount to deposit in base units (6 decimals) |
| `salt` | bytes32 | Random value for deterministic channelId computation |
| `authorizedSigner` | address | Delegated signer; use `0x0` to default to payer |

Returns the computed `channelId`.

~~~solidity
function open(
    address payee,
    address token,
    uint128 deposit,
    bytes32 salt,
    address authorizedSigner
) external returns (bytes32 channelId);
~~~

### settle

Server withdraws funds using a signed voucher without closing the channel.

| Parameter | Type | Description |
|-----------|------|-------------|
| `channelId` | bytes32 | Unique channel identifier |
| `cumulativeAmount` | uint128 | Cumulative total authorized (not delta) |
| `signature` | bytes | EIP-712 signature from authorized signer |

The contract computes `delta = cumulativeAmount - channel.settled` and
transfers `delta` tokens to the payee.

~~~solidity
function settle(
    bytes32 channelId,
    uint128 cumulativeAmount,
    bytes calldata signature
) external;
~~~

### topUp

User adds more funds to an existing channel. If a close request is
pending (`closeRequestedAt != 0`), calling `topUp()` MUST cancel it by
resetting `closeRequestedAt` to zero and emitting a
`CloseRequestCancelled` event.

| Parameter | Type | Description |
|-----------|------|-------------|
| `channelId` | bytes32 | Existing channel identifier |
| `additionalDeposit` | uint128 | Additional amount to deposit in base units |

~~~solidity
function topUp(
    bytes32 channelId,
    uint128 additionalDeposit
) external;
~~~

### close

Server closes the channel, settling any outstanding voucher and refunding
the remainder to the payer. Only callable by the payee.

| Parameter | Type | Description |
|-----------|------|-------------|
| `channelId` | bytes32 | Channel to close |
| `cumulativeAmount` | uint128 | Final cumulative amount for settlement |
| `signature` | bytes | EIP-712 signature from authorized signer |

Transfers `cumulativeAmount - channel.settled` to payee, refunds
`channel.deposit - cumulativeAmount` to payer, and marks channel finalized.

~~~solidity
function close(
    bytes32 channelId,
    uint128 cumulativeAmount,
    bytes calldata signature
) external;
~~~

### requestClose

User requests channel closure, starting a grace period of at least 15 minutes.

| Parameter | Type | Description |
|-----------|------|-------------|
| `channelId` | bytes32 | Channel for which to request closure |

Sets `channel.closeRequestedAt` to current block timestamp. The grace period
allows the payee time to submit any outstanding vouchers before forced closure.

~~~solidity
function requestClose(bytes32 channelId) external;
~~~

### withdraw

User withdraws remaining funds after the grace period expires.

| Parameter | Type | Description |
|-----------|------|-------------|
| `channelId` | bytes32 | Channel to withdraw from |

Requires `block.timestamp >= channel.closeRequestedAt + CLOSE_GRACE_PERIOD`.
Refunds all remaining deposit to payer and marks channel finalized.

~~~solidity
function withdraw(bytes32 channelId) external;
~~~

## Access Control

The escrow contract MUST enforce the following access control:

| Function | Caller | Description |
|----------|--------|-------------|
| `open` | Anyone | Creates channel; caller becomes payer |
| `settle` | Payee only | Withdraws funds using voucher |
| `topUp` | Payer only | Adds funds to existing channel |
| `close` | Payee only | Closes channel with final voucher |
| `requestClose` | Payer only | Initiates forced close |
| `withdraw` | Payer only | Withdraws after grace period |

## Signature Verification

The escrow contract MUST perform the following signature verification for
all functions that accept voucher signatures (`settle`, `close`):

1. **Canonical signatures**: The contract MUST reject ECDSA signatures
   with non-canonical (high-s) values. Signatures MUST have
   `s <= secp256k1_order / 2` where the half-order is
   `0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0`.
   See {{signature-malleability}} for rationale.

2. **Authorized signer verification**: The contract MUST recover the
   signer address from the EIP-712 signature and verify it matches the
   expected signer for the channel:
   - If `channel.authorizedSigner` is non-zero, the recovered signer
     MUST equal `channel.authorizedSigner`
   - Otherwise, the recovered signer MUST equal `channel.payer`

3. **Domain binding**: The contract MUST use its own address as the
   `verifyingContract` in the EIP-712 domain separator, ensuring
   vouchers cannot be replayed across different escrow deployments.

Failure to enforce these requirements on-chain would allow attackers to
bypass server-side validation by submitting transactions directly to
the contract.

# Request Schema

The `request` parameter in the `WWW-Authenticate` challenge contains a
base64url-encoded JSON object.

## Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | string | REQUIRED | Price per unit in base units (see note below) |
| `unitType` | string | OPTIONAL | Unit being priced (e.g., `"llm_token"`, `"byte"`, `"request"`) |
| `suggestedDeposit` | string | OPTIONAL | Suggested channel deposit amount in base units |
| `currency` | string | REQUIRED | {{TIP-20}} token address (e.g., `"0x20c0..."`) |
| `recipient` | string | REQUIRED | Payee address (server's withdrawal address)—equivalent to the on-chain `payee` |

For the `session` intent, `amount` specifies the price per unit of service
in base units (6 decimals), not a total charge. When `unitType` is present,
clients can use it together with `amount` to estimate costs before streaming
begins. The total cost depends on consumption:
`total = amount × units_consumed`.

The optional `suggestedDeposit` indicates the server's recommended
channel deposit for typical usage. Clients MAY deposit less (if they
expect limited usage) or more (for extended sessions). The minimum
viable deposit is implementation-defined but SHOULD be at least
`amount` to cover one unit of service.

Challenge expiry is specified via the `expires` auth-param in the
`WWW-Authenticate` header per {{I-D.httpauth-payment}}, using {{RFC3339}}
timestamp format. Unlike the `charge` intent, the session request JSON
does not include an `expires` field—expiry is conveyed solely via the
HTTP header.

## Method Details

As of version 00, session-specific request fields are placed in
`methodDetails`. A future high-level "session" intent definition may
promote common fields to the core schema.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `methodDetails.escrowContract` | string | REQUIRED | Address of the channel escrow contract |
| `methodDetails.channelId` | string | OPTIONAL | Channel ID if resuming an existing channel |
| `methodDetails.minVoucherDelta` | string | OPTIONAL | Minimum amount increase between vouchers (server policy hint) |
| `methodDetails.feePayer` | boolean | OPTIONAL | If `true`, server pays transaction fees (default: `false`) |
| `methodDetails.chainId` | number | OPTIONAL | Tempo chain ID (default: 4217) |

Channel reuse is OPTIONAL. Servers MAY include `channelId` to suggest
resuming an existing channel:

- **New channel** (no `channelId`): Client generates a random salt locally,
  computes `channelId` using the formula in {{channel-state}}, opens the channel
  on-chain, and returns the `channelId` in the credential.
- **Existing channel** (`channelId` provided): Client MUST verify
  `channel.deposit - channel.settled >= amount` before resuming. If
  insufficient, client SHOULD either call `topUp()` with the difference
  or open a new channel.

Servers MAY cache `(payer address, payee address, token) → channelId`
mappings to suggest channel reuse, reducing on-chain transactions.

**Example (new channel):**

~~~json
{
  "amount": "25",
  "unitType": "llm_token",
  "suggestedDeposit": "10000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35cc6634c0532925a3b844bc9e7595f8fe00",
  "methodDetails": {
    "escrowContract": "0x1234567890abcdef1234567890abcdef12345678",
    "chainId": 4217
  }
}
~~~

This requests a price of 0.000025 tokens per LLM token, with a suggested
deposit of 10.00 tokens. The client generates a random salt locally.

**Example (existing channel):**

~~~json
{
  "amount": "25",
  "unitType": "llm_token",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35cc6634c0532925a3b844bc9e7595f8fe00",
  "methodDetails": {
    "escrowContract": "0x1234567890abcdef1234567890abcdef12345678",
    "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
    "chainId": 4217
  }
}
~~~

For existing channels, `suggestedDeposit` is omitted since the channel
already has funds. The `channelId` tells the client to resume this channel.

# Fee Payment {#fee-payment}

When a challenge includes `methodDetails.feePayer: true`, the server
commits to paying transaction fees on behalf of the client. In the
`session` intent, `feePayer` affects only the client-originated channel
funding transactions (`open` and `topUp`).

## Server-Paid Fees

When `feePayer: true` for `open` or `topUp`:

1. **Client signs with placeholder**: The client signs the Tempo Transaction
   {{TEMPO-TX-SPEC}} with `fee_payer_signature` set to a placeholder value
   (`0x00`) and `fee_token` left empty. The client uses signature domain
   `0x76`.

2. **Server receives credential**: The server extracts the client-signed
   transaction from the credential payload.

3. **Server adds fee payment signature**: The server selects a `fee_token`
   (any USD-denominated TIP-20 stablecoin) and signs the transaction using
   signature domain `0x78`. This signature commits to the transaction
   including the `fee_token` and client's address.

4. **Server broadcasts**: The final transaction contains both signatures:
   - Client's signature (authorizing the channel operation)
   - Server's `fee_payer_signature` (committing to pay fees)

## Client-Paid Fees

When `feePayer: false` or omitted, the client MUST set `fee_token` to a valid
USD TIP-20 token address and include valid fee payment fields so the
transaction is executable without server fee sponsorship. The server
broadcasts the transaction as-is.

## Server-Initiated Operations

The `settle` and `close` contract functions are server-originated on-chain
transactions. The server pays transaction fees for these operations
regardless of the `feePayer` setting:

- **Voucher updates** (`action="voucher"`) are off-chain and incur no
  transaction fees.
- **Settlement** (`settle()`) and channel **close** (`close` invocation) are initiated by
  the server using the highest valid voucher. The server covers the fees for
  these transactions.
- Servers MAY recover settlement costs through pricing or other business
  logic.

The `feePayer` field applies only to `open` and `topUp` operations where
the client provides a signed transaction.

## Server Requirements

When acting as fee payer for `open` or `topUp`:

- Servers MUST maintain sufficient balance of a USD TIP-20 token to pay
  transaction fees
- Servers MAY use any USD-denominated TIP-20 token with sufficient AMM
  liquidity as the fee token
- Servers MUST validate the transaction matches challenge and channel
  parameters before adding fee payer signature
- Servers MUST reject credentials with unknown `action` values

## Client Requirements

- When `feePayer: true`: Clients MUST sign with `fee_payer_signature`
  set to `0x00` and `fee_token` empty or `0x80` (RLP null)
- When `feePayer: false` or omitted: Clients MUST set `fee_token` to a
  valid USD TIP-20 token and have sufficient balance to pay fees

# Credential Schema

The credential in the `Authorization` header contains a base64url-encoded
JSON object per {{I-D.httpauth-payment}}.

## Credential Structure

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `challenge` | object | REQUIRED | Echo of the challenge parameters from the server's WWW-Authenticate header |
| `payload` | object | REQUIRED | Session-specific payload object |

Implementations MUST ignore unknown fields in credential payloads, request
objects, and receipts to allow forward-compatible extensions.

## Credential Lifecycle

A streaming payment session progresses through distinct phases, each
corresponding to a payload action:

1. **Open**: Client deposits funds on-chain and presents the `open` action
   to begin the session. The server verifies the on-chain deposit and
   validates the initial zero-amount voucher.

2. **Streaming**: Client submits `voucher` actions with increasing
   cumulative amounts as service is consumed. The server may periodically
   settle vouchers on-chain.

3. **Close**: Client sends the `close` action with the final voucher. The
   server settles on-chain and returns a receipt.

Each action carries action-specific fields directly in the `payload` object,
with the `action` field discriminating between phases.

## Payload Actions

The `payload` object uses an `action` discriminator with action-specific
fields at the same level:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `action` | string | REQUIRED | One of `"open"`, `"topUp"`, `"voucher"`, `"close"` |

Action-specific fields are placed directly in the `payload` object alongside
`action`. See each action's definition for required fields.

| Action | Description |
|--------|-------------|
| `open` | Confirms channel is open on-chain; begins streaming |
| `topUp` | Adds funds to an existing channel |
| `voucher` | Submits an updated cumulative voucher |
| `close` | Requests server to close the channel |

### Open Payload {#open-payload}

The `open` action confirms an on-chain channel opening and begins the
streaming session. The client provides a signed transaction for the server
to broadcast.

**Payload fields (in addition to `action`):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | REQUIRED | `"transaction"` |
| `channelId` | string | REQUIRED | Channel identifier (hex-encoded bytes32) |
| `transaction` | string | REQUIRED | Signed transaction bytes |
| `authorizedSigner` | string | OPTIONAL | Address delegated to sign vouchers |
| `cumulativeAmount` | string | REQUIRED | Initial cumulative amount (typically `"0"`) |
| `signature` | string | REQUIRED | EIP-712 voucher signature for the initial amount |

The `transaction` field contains the complete signed Tempo Transaction
(type 0x76) {{TEMPO-TX-SPEC}} serialized as RLP and hex-encoded. The server
broadcasts the transaction, optionally adding a fee payer signature if
`feePayer: true` was specified in the challenge (see {{fee-payment}}).

The server recovers the `payer` address from the signed transaction and
uses it to compute the `channelId` deterministically (see {{channel-state}}).
The `authorizedSigner` is inferred from the calldata inside `transaction`
and verified when the transaction is signed.

The initial voucher (`cumulativeAmount` and `signature`) proves the client
controls the signing key and establishes the voucher chain.

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "open",
    "type": "transaction",
    "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
    "transaction": "0x76f901...signed transaction bytes...",
    "cumulativeAmount": "0",
    "signature": "0xabcdef1234567890..."
  }
}
~~~

Note: The `transaction` field contains RLP-encoded transaction bytes.
When provided, the `signature` field is the EIP-712 voucher signature
(65 bytes r‖s‖v or 64 bytes EIP-2098 compact).

The `challenge` object MUST echo the challenge parameters from the server's
`WWW-Authenticate` header per {{I-D.httpauth-payment}}.

### TopUp Payload {#topup-payload}

The `topUp` action adds funds to an existing channel during a streaming
session. Like `open`, the client provides a signed transaction for the
server to broadcast.

Clients MUST include a `challenge` object in the Payment credential for `topUp`
actions. To obtain a challenge for a top-up outside an active streaming
response, clients MAY send a `HEAD` request to the protected resource;
the server returns 402 with a `WWW-Authenticate` challenge (no body).
Servers MUST reject `topUp` actions referencing an unknown or expired
challenge `id` with problem type `challenge-not-found`.

**Payload fields (in addition to `action`):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | REQUIRED | `"transaction"` |
| `channelId` | string | REQUIRED | Channel ID |
| `transaction` | string | REQUIRED | Signed transaction bytes |
| `additionalDeposit` | string | REQUIRED | Additional amount to deposit in base units |

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "topUp",
    "type": "transaction",
    "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
    "transaction": "0x76f901...signed topUp transaction bytes...",
    "additionalDeposit": "5000000"
  }
}
~~~

Upon successful verification, the server updates the channel's available
balance. The new deposit is immediately available for voucher authorization.

### Voucher Payload {#voucher-payload}

The `voucher` action submits an updated cumulative voucher during streaming.

**Payload fields (in addition to `action`):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `channelId` | string | REQUIRED | Channel identifier |
| `cumulativeAmount` | string | REQUIRED | Cumulative amount authorized |
| `signature` | string | REQUIRED | EIP-712 voucher signature |

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "voucher",
    "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
    "cumulativeAmount": "250000",
    "signature": "0xabcdef1234567890..."
  }
}
~~~

### Close Payload {#close-payload}

The `close` action requests the server to close the channel and settle
on-chain.

**Payload fields (in addition to `action`):**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `channelId` | string | REQUIRED | Channel identifier |
| `cumulativeAmount` | string | REQUIRED | Final cumulative amount for settlement |
| `signature` | string | REQUIRED | EIP-712 voucher signature |

The server uses the voucher fields (channelId, cumulativeAmount, signature)
to call `close(channelId, cumulativeAmount, signature)` on-chain.

**Example:**

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "close",
    "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
    "cumulativeAmount": "500000",
    "signature": "0xabcdef1234567890..."
  }
}
~~~

# Voucher Signing Format {#voucher-format}

Vouchers use typed structured data signing compatible with {{EIP-712}}.
This section normatively defines the signing procedure; {{EIP-712}} is
referenced for background only.

## Wire Format

Voucher fields are placed directly in the credential `payload` object
(alongside `action`) rather than in a nested structure:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `channelId` | string | REQUIRED | Channel identifier (hex-encoded bytes32) |
| `cumulativeAmount` | string | REQUIRED | Cumulative amount authorized (decimal string) |
| `signature` | string | REQUIRED | EIP-712 signature (hex-encoded) |

The EIP-712 domain and type definitions are fixed by this specification.
Implementations MUST reconstruct the full typed data structure using the
domain parameters from the challenge (`chainId`, `escrowContract`) before
signature verification.

## Type Definitions

The `types` object MUST contain exactly:

~~~json
{
  "Voucher": [
    { "name": "channelId", "type": "bytes32" },
    { "name": "cumulativeAmount", "type": "uint128" }
  ]
}
~~~

Note: The `EIP712Domain` type is implicit per EIP-712 and SHOULD NOT be
included in the `types` object. The domain separator is computed from
the `domain` object using the canonical type string
`EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)`.

## Domain Separator

The `domain` object MUST contain:

| Field | Type | Value |
|-------|------|-------|
| `name` | string | `"Tempo Stream Channel"` |
| `version` | string | `"1"` |
| `chainId` | number | Tempo chain ID (e.g., `4217`) |
| `verifyingContract` | string | Escrow contract address from challenge |

## Signing Procedure

To sign a voucher, implementations MUST:

1. Construct the domain separator hash:

   ~~~
   domainSeparator = keccak256(
     abi.encode(
       keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
       keccak256(bytes(name)),
       keccak256(bytes(version)),
       chainId,
       verifyingContract
     )
   )
   ~~~

2. Construct the struct hash:

   ~~~
   structHash = keccak256(
     abi.encode(
       keccak256("Voucher(bytes32 channelId,uint128 cumulativeAmount)"),
       channelId,
       cumulativeAmount
     )
   )
   ~~~

3. Compute the signing hash:

   ~~~
   signingHash = keccak256("\x19\x01" || domainSeparator || structHash)
   ~~~

4. Sign with ECDSA using secp256k1 curve

5. Encode signature as 65-byte `r || s || v` where `v` is 27 or 28

## Cumulative Semantics

Vouchers specify cumulative totals, not incremental deltas:

- Voucher #1: `cumulativeAmount = 100` (authorizes 100 total)
- Voucher #2: `cumulativeAmount = 250` (authorizes 250 total)
- Voucher #3: `cumulativeAmount = 400` (authorizes 400 total)

When settling, the contract computes: `delta = cumulativeAmount - settled`

# Verification Procedure

## Open Verification

On `action="open"`, servers MUST:

1. **Transaction verification**: Decode the signed transaction from
   `transaction`, verify it calls `open()` on the expected escrow contract
   with correct parameters. Recover the `payer` address from the
   transaction, infer `authorizedSigner` from the calldata, and compute
   `channelId` deterministically (see {{channel-state}}). If `feePayer: true`,
   add fee payer signature using domain `0x78` (see {{fee-payment}}) and
   broadcast. Otherwise, broadcast as-is.
2. Query the escrow contract to verify channel state:
   - Channel exists with the computed `channelId`
   - `channel.payee` matches server's address
   - `channel.token` matches `request.currency`
   - `channel.deposit - channel.settled >= amount` (sufficient available balance)
   - Channel is not finalized
   - `channel.closeRequestedAt == 0` (no pending close request)
3. If `cumulativeAmount` and `signature` are provided, verify the initial
   voucher:
   - Recover signer from EIP-712 signature
   - Verify signature uses canonical low-s values (see {{signature-malleability}})
   - Signer matches `channel.payer` or `channel.authorizedSigner`
   - `voucher.channelId` matches
   - `voucher.cumulativeAmount >= channel.settled` (at or above current settlement)
4. Initialize server-side channel state

## TopUp Verification

On `action="topUp"`, servers MUST:

1. **Transaction verification**: Decode the signed transaction from
   `transaction`, verify it calls `topUp()` on the expected escrow contract
   with the specified `additionalDeposit` amount. If
   `feePayer: true`, add fee payer signature using domain `0x78` (see
   {{fee-payment}}) and broadcast. Otherwise, broadcast as-is.
2. Query the escrow contract to verify updated channel state:
   - `channel.deposit` increased by `additionalDeposit`
   - Channel is not finalized
3. Update server-side accounting:
   - Increase available balance by `additionalDeposit`

## Voucher Verification {#voucher-verification}

On `action="voucher"`, servers MUST:

1. Verify voucher signature using EIP-712 recovery
2. Verify signature uses canonical low-s values (see {{signature-malleability}})
3. Recover signer and MUST verify it matches expected signer from on-chain state
4. Verify `channel.closeRequestedAt == 0` (no pending close request).
   Servers MUST reject vouchers on channels with a pending forced close
   to prevent service delivery that cannot be settled.
5. Verify monotonicity:
   - `cumulativeAmount > highestVoucherAmount`
   - `(cumulativeAmount - highestVoucherAmount) >= minVoucherDelta`
6. Verify `cumulativeAmount <= channel.deposit`
7. Persist voucher to durable storage before providing service
8. Update `highestVoucherAmount = cumulativeAmount`

Servers MUST derive the expected signer from on-chain channel state by
querying the escrow contract. The expected signer is `channel.authorizedSigner`
if non-zero, otherwise `channel.payer`. Servers MUST NOT trust signer
claims in HTTP payloads.

Servers MUST persist the highest voucher to durable storage before
providing the corresponding service. Failure to do so may result in
unrecoverable fund loss if the server crashes after service delivery.

## Idempotency {#idempotency}

Servers MUST treat voucher submissions idempotently:

- Resubmitting a voucher with the same `cumulativeAmount` as the highest
  accepted MUST return 200 OK with the current `highestAmount`
- Submitting a voucher with lower `cumulativeAmount` than highest accepted
  MUST return 200 OK with the current `highestAmount` (not an error)
- Clients MAY safely retry voucher submissions after network failures

## Rejection and Error Responses {#error-responses}

If verification fails, servers MUST return an appropriate HTTP status
code with a Problem Details {{RFC9457}} response body:

| Status | When |
|--------|------|
| 400 Bad Request | Malformed payload or missing fields |
| 402 Payment Required | Invalid signature or signer mismatch |
| 410 Gone | Channel finalized or not found |

Error responses use Problem Details format:

~~~json
{
  "type": "https://paymentauth.org/problems/session/invalid-signature",
  "title": "Invalid Signature",
  "status": 402,
  "detail": "Voucher signature could not be verified",
  "channelId": "0x6d0f4fdf..."
}
~~~

Problem type URIs:

| Type URI | Description |
|----------|-------------|
| `https://paymentauth.org/problems/session/invalid-signature` | Voucher or close request signature invalid |
| `https://paymentauth.org/problems/session/signer-mismatch` | Signer is not authorized for this channel |
| `https://paymentauth.org/problems/session/amount-exceeds-deposit` | Voucher amount exceeds channel deposit |
| `https://paymentauth.org/problems/session/delta-too-small` | Amount increase below `minVoucherDelta` |
| `https://paymentauth.org/problems/session/channel-not-found` | No channel with this ID exists |
| `https://paymentauth.org/problems/session/channel-finalized` | Channel has been closed |
| `https://paymentauth.org/problems/session/challenge-not-found` | Challenge ID unknown or expired |
| `https://paymentauth.org/problems/session/insufficient-balance` | Insufficient authorized balance for request |

For errors on the Payment Auth protected resource (the initial request
carrying `Authorization: Payment`), servers MUST return 402 with a fresh
`WWW-Authenticate: Payment` challenge per {{I-D.httpauth-payment}}.

# Server-Side Accounting {#server-accounting}

Servers MUST maintain per-session accounting state to track authorized
funds versus consumed service. This section defines the normative
requirements for balance tracking, crash safety, and idempotency.

## Accounting State

For each active session identified by `(challengeId, channelId)`, servers
MUST maintain:

| Field | Type | Description |
|-------|------|-------------|
| `acceptedCumulative` | uint128 | Highest valid voucher amount accepted (monotonically increasing) |
| `spent` | uint128 | Cumulative amount charged for delivered service (monotonically increasing) |
| `settledOnChain` | uint128 | Last cumulative amount settled on-chain (informational) |

The `available` balance is computed as:

~~~
available = acceptedCumulative - spent
~~~

## Per-Request Processing

For each request carrying a Payment credential with `intent="session"`,
servers MUST follow this procedure:

1. **Voucher acceptance** (if a voucher is provided in the credential):
   - Verify signature and monotonicity per {{voucher-verification}}
   - If valid, persist the new `acceptedCumulative` value to durable storage
   - If invalid, return 402 with a fresh challenge

2. **Balance check**:
   - Compute `available = acceptedCumulative - spent`
   - Compute `cost` for this request (see {{cost-calculation}})
   - If `available < cost`: return 402 with Problem Details including
     `requiredTopUp = cost - available`

3. **Charge and deliver** (if `available >= cost`):
   - **MUST persist** `spent := spent + cost` to durable storage BEFORE
     or atomically with delivering the metered service
   - Deliver the response (or next chunk/token window for streaming)
   - Return `Payment-Receipt` header with current balance state

4. **Receipt generation**:
   - Include balance state in receipt (see {{receipt-generation}})

## Crash Safety

To prevent fund loss from server crashes:

- Servers MUST persist `spent` increments BEFORE delivering corresponding
  service. If the server crashes after persisting but before delivery,
  the client may retry and be charged again (see Idempotency below).

- Servers MUST persist `acceptedCumulative` BEFORE relying on the new
  balance for service authorization.

- Implementations SHOULD use transactional storage or write-ahead logging
  to ensure atomicity between state updates and service delivery.

## Request Idempotency {#request-idempotency}

To prevent double-charging on retries and network failures:

- Clients SHOULD include an `Idempotency-Key` header on paid requests
- Servers SHOULD track `(challengeId, idempotencyKey)` pairs and return
  the cached response (including receipt) for duplicate requests
- Servers MUST NOT increment `spent` for duplicate idempotent requests

If idempotency is not implemented, servers MUST document this limitation
and warn clients that retries may incur additional charges.

**Example idempotent request:**

~~~http
GET /api/chat HTTP/1.1
Host: api.example.com
Idempotency-Key: req_a1b2c3d4e5f6
Authorization: Payment eyJ...
~~~

## Cost Calculation {#cost-calculation}

The `cost` for a request depends on the pricing model declared in the
challenge. Servers MUST support at least one of:

- **Fixed cost**: A predetermined amount per request
- **Usage-based fees**: Pricing proportional to resource consumption (e.g.,
  tokens generated, bytes transferred, compute time)

For metered resources, servers compute cost during or after service
delivery. For streaming responses (SSE, chunked), servers SHOULD:

1. Reserve an estimated cost before starting delivery
2. Adjust `spent` as actual consumption is measured
3. Pause delivery if `available` is exhausted (client must top-up)

## Insufficient Balance During Streaming

When a streaming response exhausts `available` balance:

1. Server MUST stop delivering additional metered content
2. Server MAY hold the connection open awaiting a voucher top-up
3. Server MAY close the response; client then retries with higher voucher
4. If client submits a voucher update (request to same URI or any
   endpoint protected by the same payment handler), server SHOULD
   resume delivery on the original connection if still open

For SSE responses, servers MUST emit an `payment-need-voucher` event when
available balance is exhausted:

~~~
event: payment-need-voucher
data: {"channelId":"0x6d0f4fdf...","requiredCumulative":"250025","acceptedCumulative":"250000","deposit":"500000"}
~~~

The `payment-need-voucher` event data MUST be a JSON object containing:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `acceptedCumulative` | string | REQUIRED | Current highest accepted voucher amount (base units) |
| `channelId` | string | REQUIRED | Channel identifier (hex-encoded bytes32) |
| `deposit` | string | REQUIRED | Current on-chain deposit in the escrow contract (base units) |
| `requiredCumulative` | string | REQUIRED | Minimum cumulative amount the next voucher must authorize (base units) |

The `deposit` field allows the client to determine the correct recovery
action. When `requiredCumulative` exceeds `deposit`, the client MUST
submit `action="topUp"` to increase the on-chain deposit before sending
a new voucher. When `requiredCumulative` is within `deposit`, the client
can submit `action="voucher"` directly.

After emitting `payment-need-voucher`, the server MUST pause delivery
until a valid voucher advancing `acceptedCumulative` is accepted.
Servers SHOULD close the stream if no voucher is received within a
reasonable timeout (for example, 60 seconds). Clients SHOULD respond
by sending a voucher credential to any endpoint protected by the same
payment handler.

Servers SHOULD NOT deliver service beyond the authorized balance under
any circumstances. See {{dos-mitigation}} for rate limiting requirements.

# Settlement Procedure

## Settlement Timing

Servers MAY settle at any time using their own criteria:

- Periodically (e.g., every N seconds or M base units accrued)
- When `action="close"` is received
- When accumulated unsettled amount exceeds a threshold
- Based on gas cost optimization

Settlement frequency is an implementation detail left to servers.

The `close()` function settles any delta between the provided
`cumulativeAmount` and `channel.settled`. If the server has already
settled the highest voucher via `settle()`, calling `close()` with the
same amount will only refund the payer the remaining deposit.

## Cooperative Close

When the client sends `action="close"`:

1. Server receives the signed close request
2. Server calls `close(channelId, cumulativeAmount, signature)` on-chain
3. Contract settles any delta and refunds remainder to payer
4. Server returns receipt with transaction hash

Servers SHOULD close promptly when clients request—the economic
incentive is to claim earned funds immediately.

## Forced Close

If the server does not respond to close requests:

1. Client calls `requestClose(channelId)` on-chain
2. 15-minute grace period begins (wall-clock time via `block.timestamp`)
3. Server can still `settle()` or `close()` during grace period
4. After grace period, client calls `withdraw(channelId)`
5. Client receives all remaining (unsettled) funds

Clients SHOULD wait at least 16 minutes after `requestClose()` before
calling `withdraw()` to account for block time variance.

## Sequential Sessions

A single channel supports sequential sessions. Each session uses the same
cumulative voucher counter. When a new session begins on a channel, the
previous session's spending state is irrelevant—the channel's
`highestVoucherAmount` is the source of truth for the next voucher's
minimum value.

## Voucher Submission Transport

Vouchers are submitted via HTTP requests to the **same resource URI** that
requires payment. There is no separate session endpoint. Clients SHOULD use
HTTP/2 multiplexing or maintain separate connections for voucher updates
and content streaming when topping up during a long-lived response.

For voucher-only updates (no response body needed), clients MAY use `HEAD`
requests. Servers SHOULD support voucher credentials on `HEAD` requests
for resources that require session payment.

## Receipt Generation {#receipt-generation}

Servers MUST return a `Payment-Receipt` header on **every successful
paid request**. For streaming responses (SSE, chunked transfer), servers
MUST include the receipt in the initial response headers AND in the final
message of the stream. This ensures clients receive at least one receipt
even if the stream is interrupted, while also providing accurate final
state when the stream completes normally.

For SSE responses, the final receipt SHOULD be delivered as an event:

~~~
event: payment-receipt
data: {"method":"tempo","intent":"session","status":"success",...}
~~~

For chunked responses, the final receipt MAY be delivered as an HTTP
trailer if the client advertises trailer support via `TE: trailers`.

The base Payment Auth spec defines core receipt fields. The session intent
extends the receipt with balance tracking:

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | `"tempo"` |
| `intent` | string | `"session"` |
| `status` | string | `"success"` |
| `timestamp` | string | {{RFC3339}} response time |
| `challengeId` | string | Challenge identifier for audit correlation |
| `channelId` | string | The channel identifier |
| `acceptedCumulative` | string | Highest voucher amount accepted |
| `spent` | string | Total amount charged so far |
| `units` | number | OPTIONAL: Units consumed this request (e.g., tokens, bytes) |
| `txHash` | string | OPTIONAL: On-chain transaction hash (present on settlement/close) |

The `txHash` field serves as the core spec's `reference` field in
{{I-D.httpauth-payment}}. It is OPTIONAL because not every
response involves an on-chain settlement—voucher updates are off-chain.

The `units` field indicates what was consumed for **this specific request**.
When the challenge includes `unitType`, clients can use it to interpret the
unit of measure. Clients can compute cost as `units × amount` from the
challenge.

**Example receipt (per-request with metering):**

~~~json
{
  "method": "tempo",
  "intent": "session",
  "status": "success",
  "timestamp": "2025-01-06T12:08:30Z",
  "challengeId": "c_8d0e3b5a9f2c1d4e",
  "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
  "acceptedCumulative": "250000",
  "spent": "237500",
  "units": 500
}
~~~

**Example receipt (on close with settlement):**

~~~json
{
  "method": "tempo",
  "intent": "session",
  "status": "success",
  "timestamp": "2025-01-06T12:10:00Z",
  "challengeId": "c_8d0e3b5a9f2c1d4e",
  "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
  "acceptedCumulative": "250000",
  "spent": "250000",
  "txHash": "0x1a2b3c4d5e6f7890abcdef1234567890abcdef1234567890abcdef1234567890"
}
~~~

# Security Considerations

## Replay Prevention

Vouchers are bound to a specific channel and contract via:

- `channelId` in the voucher message
- `verifyingContract` in EIP-712 domain
- `chainId` in EIP-712 domain
- Cumulative amount semantics (can only increase)

The escrow contract enforces:

- `cumulativeAmount > channel.settled` (monotonicity)
- `cumulativeAmount <= channel.deposit` (cap)

## No Voucher Expiry

Vouchers have no `validUntil` field. This simplifies the protocol:

- Channels have no expiry—they are closed explicitly
- Vouchers remain valid until the channel closes
- The close grace period protects against clients disappearing

**Operational guidance:** Servers SHOULD settle and close channels that
have been inactive for extended periods (e.g., 30+ days) to reduce
storage requirements and operational liability. Servers MAY refuse to
accept vouchers for channels with no activity exceeding a configured
threshold.

## Denial of Service {#dos-mitigation}

To mitigate voucher flooding, servers MUST implement rate limiting:

- Servers SHOULD limit voucher submissions to 10 per second per session
- Servers MAY implement additional IP-based rate limiting for
  unauthenticated requests
- Servers MUST enforce `minVoucherDelta` when present to prevent tiny increments
- Servers SHOULD skip expensive signature verification for vouchers that
  do not advance state (return 200 OK with current `highestAmount` per
  {{idempotency}})

Servers SHOULD perform format validation (field presence, hex encoding,
length checks) before expensive ECDSA signature recovery to minimize
computational cost of malformed requests.

To mitigate channel griefing via dust deposits:

- Servers SHOULD enforce a minimum deposit (e.g., 1 USD equivalent)
- Servers MAY reject channels below this threshold

## Front-Running Protection

Cumulative voucher semantics prevent front-running attacks. If a client
submits a higher voucher while a server's `settle()` transaction is
pending, the settlement will still succeed—it merely leaves additional
unsettled funds that the server can claim later.

## Cross-Contract Replay Prevention

The EIP-712 domain includes `verifyingContract`, binding vouchers to a
specific escrow contract address. This prevents replay of vouchers
across different escrow contract deployments.

## Escrow Guarantees

The escrow contract provides:

- **Payer protection**: Funds only withdrawn with valid voucher signature
- **Payee protection**: Deposited funds guaranteed (cannot be drained)
- **Forced close**: 15-minute grace period protects both parties

## Authorized Signer

The `authorizedSigner` field allows delegation of signing authority
to a hot wallet while the main wallet only deposits funds. This reduces
exposure of the primary key during streaming sessions.

**Security considerations for delegated signing:**

- Clients using `authorizedSigner` delegation SHOULD limit channel
  deposits to acceptable loss amounts
- Clients SHOULD rotate authorized signers periodically
- Clients SHOULD NOT reuse signers across multiple high-value channels
- If the authorized signer key is compromised, an attacker can drain
  the entire channel deposit

## Signature Malleability {#signature-malleability}

ECDSA signatures are malleable: for any valid signature `(r, s)`, the
signature `(r, -s mod n)` is also valid for the same message. To prevent
signature substitution attacks, implementations MUST enforce canonical
signatures:

- Signatures MUST use "low-s" values with `s <= secp256k1_order / 2`
- The secp256k1 half-order is:
  `0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0`
- Servers MUST reject signatures with `s` values exceeding this threshold

Accepted signature formats:

- 65-byte `(r, s, v)` format where `v` is 27 or 28
- 64-byte EIP-2098 compact format

Implementations SHOULD use established libraries (e.g., OpenZeppelin ECDSA)
that enforce these requirements.

## Voucher Context and User Experience

The voucher message contains only `channelId` and `cumulativeAmount`. The
`channelId` is derived from channel parameters including payer, payee,
token, salt, and authorized signer, cryptographically binding these values.

However, wallet signing interfaces may only display the raw `channelId`
bytes, making it difficult for users to verify payment details. Wallet
implementations are encouraged to:

- Decode `channelId` components when the derivation formula is known
- Display the payee address and token in human-readable form
- Show cumulative vs. incremental amounts clearly

## Session Attribution

Vouchers are bound to channels but not to specific HTTP sessions or API
requests. When a payee operates multiple services using the same channel,
voucher-to-service attribution is an implementation concern.

The `challengeId` in the challenge provides correlation across requests.
Servers MUST implement challenge-to-voucher mapping for:

- Dispute resolution
- Usage accounting
- Audit trails

## Cross-Session Replay Prevention {#session-binding}

Vouchers use cumulative amount semantics: each voucher authorizes a total
payment up to `cumulativeAmount`, and the on-chain contract enforces strict
monotonicity (`cumulativeAmount > channel.settled`). This means a voucher
can only ever advance the channel state forward -- it cannot be "replayed"
to extract additional funds because the settlement watermark only moves in
one direction.

A separate `sessionHash` binding is therefore unnecessary:

- **Cross-session replay is harmless**: If a voucher from session A is
  presented in session B, it can only authorize funds up to the amount
  already committed. The server tracks `highestVoucherAmount` per session
  and rejects vouchers that do not advance state.
- **Cross-resource replay**: Vouchers authorize cumulative payment on a
  channel, not access to specific resources. Resource authorization is
  handled at the application layer via `challengeId` correlation.

This simplification aligns the spec with the deployed `TempoStreamChannel`
contract and the `@tempo/stream-channels` package, neither of which
include a session hash in the voucher type.

## Chain Reorganization {#chain-reorg}

On Tempo networks, finality is achieved within approximately 500ms.
However, for high-value channels, servers SHOULD:

1. Re-verify channel state periodically during long-lived sessions
2. Monitor for `ChannelClosed` or `CloseRequested` events
3. Cease service delivery if the channel becomes invalid

If a chain reorganization invalidates an accepted transaction, the
server SHOULD:

1. Stop accepting vouchers for that channel
2. Return 410 Gone with problem type `channel-not-found`
3. Log the incident for investigation

## Grace Period Rationale

The 15-minute forced close grace period balances competing concerns:

- **Payer protection**: Ensures timely fund recovery if the server becomes
  unresponsive
- **Payee protection**: Provides time to detect close requests and submit
  final settlements, even during network congestion or maintenance windows
- **Block time variance**: Allows margin for timestamp variations in
  on-chain enforcement

Implementations MAY use different grace periods in their escrow contracts,
but MUST clearly document the value and ensure clients are aware.

# IANA Considerations

## Payment Intent Registration

This document registers the following payment intent in the "HTTP Payment
Intents" registry established by {{I-D.httpauth-payment}}:

| Intent | Applicable Methods | Description | Reference |
|--------|-------------------|-------------|-----------|
| `session` | `tempo` | Streaming payment channel | This document |

Contact: Tempo Labs (<contact@tempo.xyz>)

## Problem Type Registration

This document registers the following problem types in the "HTTP
Problem Types" registry established by {{RFC9457}}:

| Type URI | Title | Status | Reference |
|----------|-------|--------|-----------|
| `https://paymentauth.org/problems/session/invalid-signature` | Invalid Signature | 402 | This document |
| `https://paymentauth.org/problems/session/signer-mismatch` | Signer Mismatch | 402 | This document |
| `https://paymentauth.org/problems/session/amount-exceeds-deposit` | Amount Exceeds Deposit | 402 | This document |
| `https://paymentauth.org/problems/session/delta-too-small` | Delta Too Small | 402 | This document |
| `https://paymentauth.org/problems/session/channel-not-found` | Channel Not Found | 410 | This document |
| `https://paymentauth.org/problems/session/channel-finalized` | Channel Finalized | 410 | This document |
| `https://paymentauth.org/problems/session/challenge-not-found` | Challenge Not Found | 402 | This document |
| `https://paymentauth.org/problems/session/insufficient-balance` | Insufficient Balance | 402 | This document |

Each problem type is defined in {{error-responses}}.

--- back

# Example

Note: In examples throughout this appendix, hex values shown with `...`
(e.g., `"0x6d0f4fdf..."`) are abbreviated for readability. Actual values
MUST be full-length as specified in {{encoding}}.

## Challenge

~~~http
HTTP/1.1 402 Payment Required
WWW-Authenticate: Payment id="kM9xPqWvT2nJrHsY4aDfEb",
  realm="api.llm-service.com",
  method="tempo",
  intent="session",
  expires="2025-01-06T12:05:00Z",
  request="<base64url-encoded JSON below>"
~~~

The `request` decodes to:

~~~json
{
  "amount": "25",
  "unitType": "llm_token",
  "suggestedDeposit": "10000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35cc6634c0532925a3b844bc9e7595f8fe00",
  "methodDetails": {
    "escrowContract": "0x9d136eEa063eDE5418A6BC7bEafF009bBb6CFa70",
    "chainId": 4217
  }
}
~~~

Note: Challenge expiry is in the header `expires` auth-param, not in the
request JSON. The client generates a random salt locally for new channels.

This requests a price of 0.000025 tokens per LLM token, with a suggested
deposit of 10.00 pathUSD (10000000 base units).

## Open Credential

The client retries the **same resource URI** with the open credential:

~~~http
GET /api/chat HTTP/1.1
Host: api.llm-service.com
Authorization: Payment <base64url-encoded credential>
~~~

The credential payload for an open action:

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "open",
    "type": "transaction",
    "channelId": "0x6d0f4fdf1f2f6a1f6c1b0fbd6a7d5c2c0a8d3d7b1f6a9c1b3e2d4a5b6c7d8e9f",
    "transaction": "0x76f901...signed transaction bytes...",
    "cumulativeAmount": "0",
    "signature": "0xabcdef1234567890..."
  }
}
~~~

## Voucher Top-Up (Same Resource URI)

During streaming, clients submit updated vouchers to the **same resource
URI**. This can use any HTTP method; `HEAD` is recommended for pure
top-ups when no response body is needed:

~~~http
HEAD /api/chat HTTP/1.1
Host: api.llm-service.com
Authorization: Payment <base64url-encoded credential with action="voucher">
~~~

Or with a regular request that also retrieves content:

~~~http
GET /api/chat HTTP/1.1
Host: api.llm-service.com
Authorization: Payment <base64url-encoded credential with action="voucher">
~~~

The credential payload for a voucher update:

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "voucher",
    "channelId": "0x6d0f4fdf...",
    "cumulativeAmount": "250000",
    "signature": "0x1234567890abcdef..."
  }
}
~~~

## Close Request (Same Resource URI)

Close requests are also sent to the same resource URI:

~~~http
GET /api/chat HTTP/1.1
Host: api.llm-service.com
Authorization: Payment <base64url-encoded credential with action="close">
~~~

The credential payload for a close request:

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.llm-service.com",
    "method": "tempo",
    "intent": "session",
    "request": "eyJ...",
    "expires": "2025-01-06T12:05:00Z"
  },
  "payload": {
    "action": "close",
    "channelId": "0x6d0f4fdf...",
    "cumulativeAmount": "500000",
    "signature": "0xabcdef1234567890..."
  }
}
~~~

The voucher fields contain the final cumulative amount for on-chain settlement.

# Reference Implementation

This appendix provides reference implementation details. These are
informative and not normative.

## Solidity Interface

~~~solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface ITempoStreamChannel {
    struct Channel {
        address payer;
        address payee;
        address token;
        address authorizedSigner;
        uint128 deposit;
        uint128 settled;
        uint64 closeRequestedAt;
        bool finalized;
    }

    function CLOSE_GRACE_PERIOD() external view returns (uint64);
    function VOUCHER_TYPEHASH() external view returns (bytes32);
    function CLOSE_REQUEST_TYPEHASH() external view returns (bytes32);

    function open(
        address payee,
        address token,
        uint128 deposit,
        bytes32 salt,
        address authorizedSigner
    ) external returns (bytes32 channelId);

    function settle(
        bytes32 channelId,
        uint128 cumulativeAmount,
        bytes calldata signature
    ) external;

    function topUp(
        bytes32 channelId,
        uint128 additionalDeposit
    ) external;

    function close(
        bytes32 channelId,
        uint128 cumulativeAmount,
        bytes calldata signature
    ) external;

    function requestClose(bytes32 channelId) external;

    function withdraw(bytes32 channelId) external;

    function getChannel(bytes32 channelId)
        external view returns (Channel memory);

    function getChannelsBatch(bytes32[] calldata channelIds)
        external view returns (Channel[] memory);

    function computeChannelId(
        address payer,
        address payee,
        address token,
        bytes32 salt,
        address authorizedSigner
    ) external view returns (bytes32);

    function getVoucherDigest(
        bytes32 channelId,
        uint128 cumulativeAmount
    ) external view returns (bytes32);

    function getCloseRequestDigest(
        bytes32 channelId,
        uint64 requestedAt
    ) external view returns (bytes32);

    function domainSeparator() external view returns (bytes32);

    event ChannelOpened(
        bytes32 indexed channelId,
        address indexed payer,
        address indexed payee,
        address token,
        address authorizedSigner,
        uint256 deposit
    );

    event Settled(
        bytes32 indexed channelId,
        address indexed payer,
        address indexed payee,
        uint256 cumulativeAmount,
        uint256 deltaPaid,
        uint256 newSettled
    );

    event CloseRequested(
        bytes32 indexed channelId,
        address indexed payer,
        address indexed payee,
        uint256 closeGraceEnd
    );

    event CloseRequestCancelled(
        bytes32 indexed channelId,
        address indexed payer,
        address indexed payee
    );

    event TopUp(
        bytes32 indexed channelId,
        address indexed payer,
        address indexed payee,
        uint256 additionalDeposit,
        uint256 newDeposit
    );

    event ChannelClosed(
        bytes32 indexed channelId,
        address indexed payer,
        address indexed payee,
        uint256 settledToPayee,
        uint256 refundedToPayer
    );

    error ChannelAlreadyExists();
    error ChannelNotFound();
    error ChannelFinalized();
    error InvalidSignature();
    error AmountExceedsDeposit();
    error AmountNotIncreasing();
    error NotPayer();
    error NotPayee();
    error TransferFailed();
    error CloseNotReady();
}
~~~

## Deployed Contracts

| Network | Chain ID | Contract Address |
|---------|----------|------------------|
| Moderato (Testnet) | 42431 | `0x9d136eEa063eDE5418A6BC7bEafF009bBb6CFa70` |

## Contract Source

The reference implementation is available at:
https://github.com/tempoxyz/tempo/tree/main/tips/ref-impls/src/TempoStreamChannel.sol

# Schema Definitions (JSON Schema)

This appendix provides JSON Schema definitions for implementations that
prefer JSON Schema over CDDL.

## Session Request Schema

~~~json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://paymentauth.org/schemas/session-request.json",
  "title": "Session Request",
  "type": "object",
  "required": ["amount", "currency", "recipient", "methodDetails"],
  "properties": {
    "amount": {
      "type": "string",
      "pattern": "^[0-9]+$",
      "description": "Price per unit in base units (decimal string)"
    },
    "unitType": {
      "type": "string",
      "description": "Unit type being priced (e.g., llm_token, byte)"
    },
    "suggestedDeposit": {
      "type": "string",
      "pattern": "^[0-9]+$",
      "description": "Suggested channel deposit in base units"
    },
    "currency": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]{40}$",
      "description": "TIP-20 token address (mixed-case accepted, normalized to lowercase)"
    },
    "recipient": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]{40}$",
      "description": "Payee address (mixed-case accepted, normalized to lowercase)"
    },
    "methodDetails": { "$ref": "#/$defs/methodDetails" }
  },
  "$defs": {
    "methodDetails": {
      "type": "object",
      "required": ["escrowContract"],
      "properties": {
        "escrowContract": {
          "type": "string",
          "pattern": "^0x[0-9a-fA-F]{40}$"
        },
        "channelId": {
          "type": "string",
          "pattern": "^0x[0-9a-fA-F]{64}$",
          "description": "OPTIONAL: for channel reuse"
        },
        "minVoucherDelta": {
          "type": "string",
          "pattern": "^[0-9]+$",
          "description": "OPTIONAL: server policy hint"
        },
        "feePayer": {
          "type": "boolean",
          "default": false,
          "description": "If true, server pays transaction fees"
        },
        "chainId": { "type": "integer" }
      }
    }
  }
}
~~~

## Session Payload Schema

~~~json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://paymentauth.org/schemas/session-payload.json",
  "title": "Session Payload",
  "type": "object",
  "required": ["action"],
  "properties": {
    "action": { "enum": ["open", "topUp", "voucher", "close"] },
    "transaction": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]+$",
      "description": "Signed transaction bytes"
    },
    "channelId": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]{64}$",
      "description": "Channel identifier"
    },
    "cumulativeAmount": {
      "type": "string",
      "pattern": "^[0-9]+$",
      "description": "Cumulative amount authorized (decimal string)"
    },
    "signature": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]{128,130}$",
      "description": "EIP-712 voucher signature"
    }
  }
}
~~~

## Session Receipt Schema

Servers MUST include `Payment-Receipt` only on successful processing of a
session action (2xx responses). On error responses (4xx/5xx), servers MUST
return Problem Details and MUST NOT include a `Payment-Receipt` header.
The `status` field is always `"success"` because receipts represent
successful acceptance; failures are communicated via HTTP status codes
and Problem Details.

~~~json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://paymentauth.org/schemas/session-receipt.json",
  "title": "Session Receipt",
  "type": "object",
  "required": ["method", "intent", "status", "timestamp", "challengeId", "channelId", "acceptedCumulative", "spent"],
  "properties": {
    "method": { "const": "tempo" },
    "intent": { "const": "session" },
    "status": { "const": "success" },
    "timestamp": {
      "type": "string",
      "format": "date-time"
    },
    "challengeId": { "type": "string" },
    "channelId": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]{64}$"
    },
    "acceptedCumulative": {
      "type": "string",
      "pattern": "^[0-9]+$",
      "description": "Highest voucher amount accepted"
    },
    "spent": {
      "type": "string",
      "pattern": "^[0-9]+$",
      "description": "Total amount charged so far"
    },
    "units": {
      "type": "integer",
      "description": "OPTIONAL: Units consumed this request"
    },
    "txHash": {
      "type": "string",
      "pattern": "^0x[0-9a-fA-F]{64}$",
      "description": "OPTIONAL: On-chain transaction hash (present on settlement/close)"
    }
  }
}
~~~

# Acknowledgements

The authors thank the Tempo community for their feedback on session
payment design.
