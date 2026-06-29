---
title: Solana Charge Intent for HTTP Payment Authentication
abbrev: Solana Charge
docname: draft-solana-charge-00
version: 00
category: info
ipr: trust200902
submissiontype: independent
consensus: false

author:
  - name: Ludo Galabru
    ins: L. Galabru
    email: ludo.galabru@solana.org
    org: Solana Foundation

  - name: Ilan Gitter
    ins: I. Gitter
    email: ilan.gitter@solana.org
    org: Solana Foundation

normative:
  RFC2119:
  RFC3339:
  RFC4648:
  RFC8174:
  RFC8259:
  RFC8785:
  RFC9457:
  I-D.payment-intent-charge:
    title: "'charge' Intent for HTTP Payment Authentication"
    target: https://datatracker.ietf.org/doc/draft-payment-intent-charge/
    author:
      - name: Jake Moxey
      - name: Brendan Ryan
      - name: Tom Meagher
    date: 2026
  I-D.httpauth-payment:
    title: "The 'Payment' HTTP Authentication Scheme"
    target: https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/
    author:
      - name: Jake Moxey
    date: 2026-01

informative:
  SOLANA-DOCS:
    title: "Solana Documentation"
    target: https://solana.com/docs
    author:
      - org: Solana Foundation
    date: 2026
  SPL-TOKEN:
    title: "SPL Token Program"
    target: https://solana.com/docs/tokens
    author:
      - org: Solana Foundation
    date: 2026
  SPL-TOKEN-2022:
    title: "SPL Token-2022 Program"
    target: https://solana.com/docs/tokens/extensions
    author:
      - org: Solana Foundation
    date: 2026
  BASE58:
    title: "Base58 Encoding Scheme"
    target: https://datatracker.ietf.org/doc/html/draft-msporny-base58-03
    author:
      - name: Manu Sporny
    date: 2023
---

--- abstract

This document defines the "charge" intent for the "solana" payment
method within the Payment HTTP Authentication Scheme
{{I-D.httpauth-payment}}. The client constructs and signs a native SOL
or SPL token transfer on the Solana blockchain; the server verifies the
payment and presents the transaction signature as proof of payment.

Two credential types are supported: `type="transaction"` (default),
where the client sends the signed transaction to the server for
broadcast, and `type="signature"` (fallback), where the client
broadcasts the transaction itself and presents the on-chain transaction
signature for server verification.

--- middle

# Introduction

HTTP Payment Authentication {{I-D.httpauth-payment}} defines a
challenge-response mechanism that gates access to resources behind
payments. This document registers the "charge" intent for the
"solana" payment method.

Solana is a high-throughput blockchain with sub-second finality
and low transaction fees {{SOLANA-DOCS}}. This specification
supports payments in both native SOL and SPL tokens (including
Token-2022 {{SPL-TOKEN-2022}}), making it suitable for
micropayment use cases where fast confirmation and low overhead
are important.

## Pull Mode (Default) {#pull-mode}

The default flow, called "pull mode", uses `type="transaction"`
credentials. The client signs the transaction and the server
"pulls" it for broadcast to the Solana network:

~~~
   Client                     Server              Solana Network
      |                          |                        |
      |  (1) GET /resource       |                        |
      |----------------------->  |                        |
      |                          |                        |
      |  (2) 402 Payment Required|                        |
      |      (recipient, amount, |                        |
      |       feePayerKey?)      |                        |
      |<-----------------------  |                        |
      |                          |                        |
      |  (3) Build tx, set fee   |                        |
      |      payer, sign         |                        |
      |                          |                        |
      |  (4) Authorization:      |                        |
      |      Payment <credential>|                        |
      |      (signed tx bytes)   |                        |
      |----------------------->  |                        |
      |                          |  (5) Co-sign (if fee   |
      |                          |      payer) + send     |
      |                          |----------------------> |
      |                          |  (6) Confirmation      |
      |                          |<---------------------- |
      |                          |                        |
      |  (7) 200 OK + Receipt    |                        |
      |<-----------------------  |                        |
      |                          |                        |
~~~

In this model the server controls transaction broadcast, enabling
fee sponsorship ({{fee-sponsorship}}) and server-side retry logic.
When `feePayer` is `true`, the challenge includes `feePayerKey`
so the client sets the server as fee payer. The server co-signs
with its fee payer key before broadcasting.

## Push Mode (Fallback) {#push-mode}

The fallback flow, called "push mode", uses `type="signature"`
credentials. The client "pushes" the transaction to the network
itself and presents the confirmed signature. The client
broadcasts the transaction itself and presents the confirmed
transaction signature:

~~~
   Client                     Server              Solana Network
      |                          |                        |
      |  (1) GET /resource       |                        |
      |----------------------->  |                        |
      |                          |                        |
      |  (2) 402 Payment Required|                        |
      |      (recipient, amount) |                        |
      |<-----------------------  |                        |
      |                          |                        |
      |  (3) Build & sign tx     |                        |
      |                          |                        |
      |  (4) Send transaction    |                        |
      |----------------------------------------------->   |
      |  (5) Confirmation        |                        |
      |<-----------------------------------------------   |
      |                          |                        |
      |  (6) Authorization:      |                        |
      |      Payment <credential>|                        |
      |      (tx signature)      |                        |
      |----------------------->  |                        |
      |                          |  (7) getTransaction    |
      |                          |----------------------> |
      |                          |  (8) Parsed tx data    |
      |                          |<---------------------- |
      |                          |                        |
      |  (9) 200 OK + Receipt    |                        |
      |<-----------------------  |                        |
      |                          |                        |
~~~

This flow is useful when the client cannot or does not wish to
delegate broadcast to the server. The server verifies the payment
by fetching and inspecting the on-chain transaction via RPC.

## Relationship to the Charge Intent

This document inherits the shared request semantics of the
"charge" intent from {{I-D.payment-intent-charge}}. It defines
only the Solana-specific `methodDetails`, `payload`, and
verification procedures for the "solana" payment method.

# Requirements Language

{::boilerplate bcp14-tagged}

# Terminology

Transaction Signature
: A base58-encoded {{BASE58}} unique identifier for a Solana
  transaction, produced by the first signer. Serves as both
  the transaction identifier and proof of payment in this
  specification.

SPL Token
: A fungible token on Solana conforming to the SPL Token
  program {{SPL-TOKEN}} or the Token-2022 program
  {{SPL-TOKEN-2022}}.

Associated Token Account (ATA)
: A deterministically derived token account for a given
  owner and mint, per the Associated Token Program. The
  address is a Program Derived Address (PDA) seeded by
  the owner's public key, the token mint, and the token
  program ID.

Lamports
: The smallest unit of native SOL. 1 SOL = 1,000,000,000
  lamports.

Base Units
: The smallest transferable unit of an SPL token, determined
  by the token's decimal precision. For example, USDC uses
  6 decimals, so 1 USDC = 1,000,000 base units.

Fee Payer
: An account that pays Solana transaction fees. When the server
  acts as fee payer, it adds its signature to the transaction
  before broadcasting, covering the transaction fee on behalf
  of the client.

Pull Mode
: The default settlement flow where the client signs the
  transaction and the server broadcasts it
  (`type="transaction"`). The server "pulls" the signed
  transaction from the credential. Enables fee sponsorship
  and server-side retry logic.

Push Mode
: The fallback settlement flow where the client broadcasts
  the transaction itself and presents the confirmed
  signature (`type="signature"`). The client "pushes" the
  transaction to the network directly. Cannot be used with
  fee sponsorship.

# Intent Identifier

The intent identifier for this specification is "charge".
It MUST be lowercase.

# Intent: "charge"

The "charge" intent represents a one-time payment gating access
to a resource. The client builds and signs a Solana transfer
transaction, then either sends the signed transaction bytes to
the server for broadcast (`type="transaction"`) or broadcasts the
transaction itself and sends the on-chain signature
(`type="signature"`). The server verifies the transfer details
and returns a receipt.

# Encoding Conventions {#encoding}

All JSON {{RFC8259}} objects carried in auth-params or HTTP
headers in this specification MUST be serialized using the JSON
Canonicalization Scheme (JCS) {{RFC8785}} before encoding. JCS
produces a deterministic byte sequence, which is required for
any digest or signature operations defined by the base spec
{{I-D.httpauth-payment}}.

The resulting bytes MUST then be encoded using base64url
{{RFC4648}} Section 5 without padding characters (`=`).
Implementations MUST NOT append `=` padding when encoding,
and MUST accept input with or without padding when decoding.

This encoding convention applies to: the `request` auth-param
in `WWW-Authenticate`, the credential token in `Authorization`,
and the receipt token in `Payment-Receipt`.

# Request Schema

## Shared Fields

The `request` auth-param of the `WWW-Authenticate: Payment`
header contains a JCS-serialized, base64url-encoded JSON
object (see {{encoding}}). The following shared fields are
included in that object:

amount
: REQUIRED. The payment amount in base units, encoded as a
  decimal string. For native SOL, the amount is in lamports.
  For SPL tokens, the amount is in the token's smallest unit
  (e.g., for USDC with 6 decimals, "1000000" represents
  1 USDC). The value MUST be a positive integer that fits
  in a 64-bit unsigned integer (max 18,446,744,073,709,551,615).

currency
: REQUIRED. For native SOL, MUST be the lowercase
  string `"sol"`. For SPL tokens, MUST be the base58-encoded
  mint address (e.g.,
  `"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"`
  for USDC). The mint address uniquely identifies the token
  and is used by the client to construct the transfer
  instruction. MUST NOT exceed 128 characters.

description
: OPTIONAL. A human-readable memo describing the resource or
  service being paid for. MUST NOT exceed 256 characters.

recipient
: REQUIRED. The base58-encoded public key of the account
  receiving the payment. For native SOL transfers, this is the
  destination account. For SPL token transfers, this is the
  owner of the destination associated token account, not the
  ATA address itself.

externalId
: OPTIONAL. Merchant's reference (e.g., order ID, invoice
  number), per {{I-D.payment-intent-charge}}. May be used
  for reconciliation or idempotency. MUST NOT exceed 566
  bytes (Solana Memo Program limit). When present, clients
  SHOULD include this value as a Memo Program instruction
  in the transaction, making it visible on-chain for
  auditing and reconciliation. Servers MAY verify the memo
  matches the `externalId` from the challenge.

## Method Details

The following fields are nested under `methodDetails` in
the request JSON:

network
: OPTIONAL. Identifies which Solana cluster the payment
  should be made on. MUST be one of "mainnet",
  "devnet", or "localnet". Defaults to "mainnet"
  if omitted. Clients MUST reject challenges whose
  network does not match their configured cluster.

decimals
: Conditionally REQUIRED. The number of decimal places
  for the token (0–9). MUST be present when `currency`
  is a mint address; MUST be absent when `currency` is
  `"sol"`. Used by the client to construct a
  `TransferChecked` instruction.

tokenProgram
: OPTIONAL. The base58-encoded program ID of the token
  program governing the token. MUST be either the
  Token Program
  (`TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`) or
  the Token-2022 Program
  (`TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb`).
  If omitted, clients MUST determine the correct token
  program by fetching the mint account from the network
  and inspecting its owner program. If that lookup
  fails, returns an unexpected owner, or cannot be
  verified, clients MUST reject the challenge rather
  than falling back to the Token Program. Servers
  SHOULD include this field as a hint to avoid the
  extra RPC lookup. MUST NOT be present when
  `currency` is `"sol"`.

feePayer
: OPTIONAL. A boolean indicating whether the server will
  pay transaction fees on behalf of the client. Defaults
  to `false` if omitted. When `true`, the `feePayerKey`
  field MUST also be present. See {{fee-sponsorship}}.

feePayerKey
: Conditionally REQUIRED. The base58-encoded public key
  of the server's fee payer account. MUST be present when
  `feePayer` is `true`; MUST be absent when `feePayer` is
  `false` or omitted. The client uses this key as the
  transaction fee payer when constructing the transaction.

splits
: OPTIONAL. An array of at most 8 additional payment
  splits. Each entry is a JSON object with the following
  fields:

  - `recipient` (REQUIRED): Base58-encoded public key of
    the split recipient.
  - `amount` (REQUIRED): Amount in the same base units
    and asset as the primary `amount`.
  - `memo` (OPTIONAL): Human-readable label for this
    split (e.g., "platform fee", "referral"). MUST NOT
    exceed 566 bytes (Solana Memo Program limit).
  - `ataCreationRequired` (OPTIONAL): Boolean. Defaults
    to `false`. When `true`, the client MUST include an
    idempotent Associated Token Account creation instruction
    for this split recipient's ATA before the split transfer.
    This field MUST NOT be `true` unless `currency` is an
    SPL token mint address. In fee-sponsored pull mode
    (`feePayer: true`), this field is the only authorization
    for the server fee payer to fund split-recipient ATA
    creation. See {{split-recipient-ata-creation}}.

  When present, the client MUST include a transfer
  instruction for each split in addition to the primary
  transfer to `recipient`. All splits use the same asset
  as the primary payment (native SOL or the token from `currency`).

  The top-level `amount` is the total the client pays.
  The sum of all split amounts MUST NOT exceed `amount`.
  The primary `recipient` receives `amount` minus the
  sum of all split amounts; this remainder MUST be
  greater than zero. Servers MUST reject challenges
  where splits consume the entire amount. Servers MUST
  verify each split transfer on-chain during credential
  verification. If the same recipient appears more than
  once in `splits`, each occurrence is a distinct
  payment leg and MUST be verified separately; servers
  MUST NOT implicitly aggregate such entries.

  This mechanism is a Solana-specific extension to the
  base `charge` intent. It can be used for fee payer cost
  recovery, platform fees, revenue sharing, or referral
  commissions.

recentBlockhash
: OPTIONAL. A base58-encoded recent blockhash for the
  client to use when constructing the transaction. When
  provided, clients SHOULD use this blockhash instead of
  fetching one from an RPC node. This avoids an extra
  RPC round-trip and ensures the server can verify
  blockhash freshness. This field is advisory and
  short-lived; it MUST NOT be assumed to remain valid for
  the full lifetime of the payment challenge. If omitted,
  clients MUST fetch a recent blockhash themselves.

### Native SOL Example

~~~json
{
  "amount": "10000000",
  "currency": "sol",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Weather API access",
  "methodDetails": {
    "network": "mainnet"
  }
}
~~~

This requests a transfer of 0.01 SOL (10,000,000 lamports).

### SPL Token Example

~~~json
{
  "amount": "1000000",
  "currency": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Premium API call",
  "methodDetails": {
    "network": "mainnet",
    "decimals": 6,
    "tokenProgram": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
  }
}
~~~

This requests a transfer of 1 USDC (1,000,000 base units).

### Fee Sponsorship Example

~~~json
{
  "amount": "10000000",
  "currency": "sol",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Weather API access",
  "methodDetails": {
    "network": "mainnet",
    "feePayer": true,
    "feePayerKey": "9aE3Fg7HjKLmNpQr5TuVwXyZ2AbCdEf8GhIjKlMnOp1R"
  }
}
~~~

This requests a transfer of 0.01 SOL where the server pays
transaction fees.

### Payment Splits Example

~~~json
{
  "amount": "1050000",
  "currency": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Marketplace purchase",
  "methodDetails": {
    "network": "mainnet",
    "decimals": 6,
    "splits": [
      { "recipient": "3pF8Kg2aHbNvJkLMwEqR7YtDxZ5sGhJn4UV6mWcXrT9A", "amount": "50000", "memo": "platform fee" }
    ]
  }
}
~~~

This requests a total payment of 1.05 USDC. The platform
receives 0.05 USDC and the primary recipient (seller)
receives 1.00 USDC.

### Split Recipient ATA Creation Example

~~~json
{
  "amount": "1000000",
  "currency": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Marketplace purchase with bridge settlement",
  "methodDetails": {
    "network": "mainnet",
    "decimals": 6,
    "tokenProgram": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    "feePayer": true,
    "feePayerKey": "Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr",
    "splits": [
      {
        "recipient": "3pF8Kg2aHbNvJkLMwEqR7YtDxZ5sGhJn4UV6mWcXrT9A",
        "amount": "990000",
        "memo": "bridge deposit",
        "ataCreationRequired": true
      }
    ]
  }
}
~~~

This requests a total payment of 1 USDC. The bridge deposit
split receives 0.99 USDC and the primary recipient receives
0.01 USDC. Because the split sets `ataCreationRequired: true`,
the fee payer authorizes funding the split recipient's ATA if
it does not already exist. The top-level recipient is not
covered by this authorization.

# Credential Schema

The `Authorization` header carries a single base64url-encoded
JSON token (no auth-params). The decoded object contains the
following top-level fields:

challenge
: REQUIRED. An echo of the challenge auth-params from the
  `WWW-Authenticate` header: `id`, `realm`, `method`,
  `intent`, `request`, and (if present) `expires`. This
  binds the credential to the exact challenge that was
  issued.

source
: OPTIONAL. A payer identifier string, as defined by
  {{I-D.httpauth-payment}}. Solana implementations MAY
  use the payer's base58-encoded public key or a DID.

payload
: REQUIRED. A JSON object containing the Solana-specific
  credential fields. The `type` field determines which
  additional fields are present. Two payload types are
  defined: `"transaction"` (default) and `"signature"`
  (fallback).

## Transaction Payload — Pull Mode {#transaction-payload}

In pull mode (`type="transaction"`), the client sends the
signed transaction bytes to the server for broadcast. The
`transaction` field contains the base64-encoded serialized
signed transaction.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | REQUIRED | `"transaction"` |
| `transaction` | string | REQUIRED | Base64-encoded serialized signed transaction bytes (max 1232 bytes decoded) |

The transaction MUST be a valid Solana versioned transaction
that does not exceed the 1232-byte transaction size limit.
containing the transfer instruction(s) matching the challenge
parameters. The client MUST sign the transaction with the
transfer authority key. When `feePayer` is `false` or absent,
the client MUST also be the fee payer and the transaction MUST
be fully signed. When `feePayer` is `true`, the transaction
MUST set the server's `feePayerKey` as fee payer, and the
client signs only as transfer authority; the server adds the
fee payer signature before broadcasting (see
{{fee-sponsorship}}).

Example (decoded):

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.example.com",
    "method": "solana",
    "intent": "charge",
    "request": "eyJ...",
    "expires": "2026-03-15T12:05:00Z"
  },
  "payload": {
    "type": "transaction",
    "transaction": "AQAAAA...base64-encoded-signed-tx..."
  }
}
~~~

## Signature Payload — Push Mode {#signature-payload}

In push mode (`type="signature"`), the client has already
broadcast the transaction to the Solana network. The
`signature` field contains the base58-encoded transaction
signature for the server to verify on-chain.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | REQUIRED | `"signature"` |
| `signature` | string | REQUIRED | Base58-encoded Solana transaction signature |

Example (decoded):

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.example.com",
    "method": "solana",
    "intent": "charge",
    "request": "eyJ...",
    "expires": "2026-03-15T12:05:00Z"
  },
  "payload": {
    "type": "signature",
    "signature": "5UfDuX7hXbPjGUpTmt9PHRLsNGJe4dEny..."
  }
}
~~~

## Limitations of Push Mode {#signature-limitations}

The `type="signature"` credential has the following limitations:

- MUST NOT be used when `feePayer` is `true` in the challenge
  request. Since the client has already broadcast the
  transaction, the server cannot add its fee payer signature.
  Servers MUST reject `type="signature"` credentials when
  the challenge specifies `feePayer: true`.

- The server cannot modify or enhance the transaction (e.g.,
  add priority fees, adjust compute units, or retry with
  different parameters).

# Fee Sponsorship {#fee-sponsorship}

When a challenge includes `feePayer: true` in `methodDetails`,
the server commits to paying Solana transaction fees on behalf
of the client. This section describes the fee sponsorship
mechanism.

## Server-Paid Fees

When `feePayer` is `true`:

1. **Client constructs transaction**: The client builds the
   transfer transaction with the server's `feePayerKey` set
   as the transaction fee payer. The client's account is the
   transfer authority but NOT the fee payer.

2. **Client partially signs**: The client signs the transaction
   with only its own key (the transfer authority). The fee
   payer signature slot remains empty.

3. **Client sends credential**: The client sends the partially
   signed transaction as a `type="transaction"` credential.

4. **Server adds fee payer signature**: The server verifies the
   transaction contents, then signs with the fee payer key to
   complete the transaction.

5. **Server broadcasts**: The fully signed transaction
   (containing both the client's transfer authority signature
   and the server's fee payer signature) is broadcast to the
   Solana network.

## Client-Paid Fees

When `feePayer` is `false` or omitted, the client MUST set
itself as the fee payer and fully sign the transaction. The
server broadcasts the transaction as-is without adding any
signatures.

## Server Requirements

When acting as fee payer, servers:

- MUST maintain sufficient SOL balance in the fee payer
  account to cover transaction fees
- MUST verify the transaction contents before signing
  (see {{transaction-verification}})
- SHOULD implement rate limiting to mitigate fee
  exhaustion attacks (see {{fee-payer-risks}})

## Client Requirements

- When `feePayer` is `true`: clients MUST set `feePayerKey`
  from `methodDetails` as the transaction fee payer and MUST
  sign only with the transfer authority key. Clients MUST
  use `type="transaction"` credentials.
- When `feePayer` is `false` or omitted: clients MUST set
  themselves as the fee payer and fully sign the transaction.
  Clients MAY use either `type="transaction"` or
  `type="signature"` credentials.

## Split Recipient ATA Creation {#split-recipient-ata-creation}

ATA creation is permitted only as setup for payment
recipients. It does not authorize the creation of token
accounts for unrelated owners. The challenge expresses a
required split-recipient ATA setup on the split entry itself
with `ataCreationRequired: true`.

Every ATA creation instruction in an SPL token payment
transaction MUST satisfy all of the following:

- The instruction MUST be the Associated Token Program's
  idempotent create instruction.
- The instruction owner MUST be a recipient listed in
  `splits`, subject to the fee payer restrictions below.
- The instruction mint MUST be the challenge `currency`.
- The instruction token program MUST be the challenge
  `tokenProgram`. If `tokenProgram` is omitted, the token
  program resolved from the mint account is used.
- The ATA address MUST be the canonical Associated Token
  Account PDA for the owner, mint, and token program.
- The instruction payer MUST be the transaction fee payer.

When a split sets `ataCreationRequired: true`:

- The challenge `currency` MUST be an SPL token mint address.
- The client MUST include an ATA creation instruction for that
  split recipient before the split transfer.
- The ATA creation instruction does not create an additional
  payment recipient. The client MUST still include the split's
  `transferChecked` payment instruction.

In fee-sponsored pull mode (`feePayer: true`), the server fee
payer only authorizes ATA creation for split recipients whose
split entry sets `ataCreationRequired: true`. Clients MUST NOT
include fee-payer-funded ATA creation instructions for the
top-level `recipient`, unmarked split recipients, or arbitrary
owners. If the top-level recipient's ATA does not exist, the
server MUST NOT issue a challenge that requires creating it.

When the client is the transaction fee payer (`feePayer` is
`false` or omitted), clients MAY include ATA creation
instructions only for split recipients, and MUST include one
for each split whose entry sets `ataCreationRequired: true`.
Clients MUST NOT include ATA creation instructions for the
top-level `recipient` or any other owner.

# Verification Procedure {#verification}

Upon receiving a request with a credential, the server MUST:

1. Decode the base64url credential and parse the JSON.

2. Verify that `payload.type` is present and is either
   `"transaction"` or `"signature"`.

3. Look up the stored challenge using
   `credential.challenge.id`. If no matching challenge
   is found, reject the request.

4. Verify that all fields in `credential.challenge`
   exactly match the stored challenge auth-params.

5. If `payload.type` is `"signature"` and the challenge
   specifies `feePayer: true`, reject the request (see
   {{signature-limitations}}).

6. Proceed with type-specific verification:
   - For `type="transaction"`: see {{transaction-verification}}.
   - For `type="signature"`: see {{signature-verification}}.

## Pull Mode Verification {#transaction-verification}

For credentials with `type="transaction"`:

1. Decode the base64 `payload.transaction` value.

2. Deserialize the transaction and verify that it
   structurally matches the challenge request:
   - the fee payer matches the challenge policy;
   - the transfer authority is signed by the client;
   - the transaction contains only expected transfer,
     ATA-creation, memo, and compute-budget instructions;
   - when `feePayer` is `true`, ATA-creation instructions
     funded by the server fee payer are limited to split
     recipients whose split entry sets `ataCreationRequired`
     to `true`, as described in {{split-recipient-ata-creation}};
   - when `feePayer` is `false` or omitted, ATA-creation
     instructions are limited to split recipients;
   - the payment semantics match the challenge request,
     as described in {{sol-verification}} or
     {{spl-verification}}.

3. If `feePayer` is `true`, add the server's fee payer
   signature using the `feePayerKey` and re-serialize.
   The transaction MUST have the server's `feePayerKey`
   set as the fee payer account.

4. If `feePayer` is `true`, simulate the transaction
   using the `simulateTransaction` RPC method. The
   server MUST reject the credential if simulation
   fails. If `feePayer` is `false` or omitted, the
   server SHOULD simulate the transaction before
   broadcast and SHOULD reject the credential if
   simulation indicates the transaction will fail.
   This catches invalid transactions without spending
   fees, which is especially important in fee payer
   mode (see {{fee-payer-risks}}).

5. Broadcast the transaction to the Solana network using
   `sendTransaction`.

6. Wait for confirmation at the required commitment level.

7. Fetch the confirmed transaction using `getTransaction`
   with `jsonParsed` encoding and verify the transfer
   details still match the challenge request, as described in
   {{sol-verification}} or {{spl-verification}}.

8. Record the transaction signature as consumed to
   prevent replay (see {{replay-protection}}).

9. Return the resource with a Payment-Receipt header.

## Push Mode Verification {#signature-verification}

For credentials with `type="signature"`:

1. Verify that `payload.signature` is present and is a
   valid base58-encoded string.

2. Verify the transaction signature has not been
   previously consumed (see {{replay-protection}}).

3. Fetch the transaction from the Solana network using
   the RPC `getTransaction` method with `jsonParsed`
   encoding and the `confirmed` commitment level.

4. Verify the transaction was successful (no error in
   the transaction metadata).

5. Verify the transfer details match the challenge
   request, as described in {{sol-verification}} or
   {{spl-verification}}.

6. Mark the transaction signature as consumed to
   prevent replay.

7. Return the resource with a Payment-Receipt header.

Note: both credential types reuse the same on-chain
transfer verification logic defined in
{{sol-verification}} and {{spl-verification}}.

## Native SOL Verification {#sol-verification}

For native SOL payments (`currency` is `"sol"`),
the server MUST:

1. Compute the primary payment amount as the top-level
   `amount` minus the sum of all `splits`, if any.

2. Locate a System Program `transfer` instruction in the
   transaction's parsed instructions whose `destination`
   matches the top-level `recipient` and whose `lamports`
   field matches that primary payment amount.

3. For each split in `splits`, if any, locate an
   additional System Program `transfer` instruction whose
   `destination` and `lamports` fields match that split.

   Each required payment leg MUST be matched to a
   distinct transfer instruction. A single transfer
   instruction MUST NOT satisfy more than one required
   payment leg, even if multiple legs share the same
   recipient.

If any required transfer instruction is missing, the
server MUST reject the credential.

## SPL Token Verification {#spl-verification}

For SPL token payments (`currency` is a mint address,
not `"sol"`), the server MUST:

1. Compute the primary payment amount as the top-level
   `amount` minus the sum of all `splits`, if any.

2. Locate a `transferChecked` instruction from the
   appropriate token program (Token Program or
   Token-2022) in the transaction's parsed instructions
   whose `mint` field matches the top-level `currency`
   field from the challenge request.

3. Derive the expected destination associated token
   account for the top-level `recipient` from the
   `recipient`, `currency`, and `tokenProgram` in the
   challenge request. Verify that at least one matching
   `transferChecked` instruction uses that derived ATA
   as `destination` and has `tokenAmount.amount` equal
   to the primary payment amount.

4. For each split in `splits`, if any, derive the
   expected destination ATA for that split recipient and
   verify that at least one additional `transferChecked`
   instruction uses that ATA as `destination` and has
   `tokenAmount.amount` equal to the split amount.

   Each required payment leg MUST be matched to a
   distinct `transferChecked` instruction. A single
   instruction MUST NOT satisfy more than one required
   payment leg, even if multiple legs resolve to the
   same destination ATA.

If any required `transferChecked` instruction is missing,
the server MUST reject the credential.

Split recipient ATA creation does not alter SPL transfer
verification. The selected challenge request defines the full
set of required payment legs. A `transferChecked` instruction
to an ATA created by the transaction satisfies a required
payment leg only if that ATA owner appears in `splits` for the
selected challenge.

## Replay Protection {#replay-protection}

Servers MUST maintain a set of consumed transaction
signatures. Before accepting a credential, the server
MUST check whether the signature has already been
consumed. After successful verification, the server
MUST atomically mark the signature as consumed.

The transaction signature is globally unique on the
Solana network, making it a natural replay prevention
token. A signature that has been consumed MUST NOT be
accepted again, even if presented with a different
challenge ID.

For `type="transaction"` credentials, the transaction
signature is derived after broadcast. For
`type="signature"` credentials, the signature is
provided directly by the client.

# Settlement Procedure

Two settlement flows are supported, corresponding to
the two credential types.

## Pull Mode Settlement (type="transaction")

For `type="transaction"` credentials, the client signs
the transaction and sends it to the server. The server
optionally adds a fee payer signature and broadcasts:

~~~
   Client                        Server                   Solana Network
      |                             |                           |
      |  (1) Authorization:         |                           |
      |      Payment <credential>   |                           |
      |      (signed tx bytes)      |                           |
      |-------------------------->  |                           |
      |                             |                           |
      |                             |  (2) If feePayer: true,   |
      |                             |      co-sign as fee payer |
      |                             |                           |
      |                             |  (3) simulateTransaction  |
      |                             |------------------------>  |
      |                             |  (4) Simulation OK        |
      |                             |<------------------------  |
      |                             |                           |
      |                             |  (5) sendTransaction      |
      |                             |------------------------>  |
      |                             |  (6) Confirmation         |
      |                             |<------------------------  |
      |                             |                           |
      |                             |  (7) getTransaction       |
      |                             |      (verify transfer)    |
      |                             |------------------------>  |
      |                             |  (8) Parsed tx data       |
      |                             |<------------------------  |
      |                             |                           |
      |  (9) 200 OK + Receipt       |                           |
      |<--------------------------  |                           |
      |                             |                           |
~~~

1. Client submits credential containing signed transaction
   bytes.
2. If `feePayer` is `true`, the server co-signs with its
   fee payer key.
3. Server simulates the transaction to catch failures
   without spending fees.
4. Server broadcasts the transaction to Solana.
5. Transaction reaches the required commitment level.
6. Server fetches the confirmed transaction and verifies
   the transfer details match the challenge request.
7. Server records the signature as consumed and returns
   the resource with a Payment-Receipt header whose
   `reference` field is the transaction signature.

## Push Mode Settlement (type="signature")

For `type="signature"` credentials, the client broadcasts
the transaction itself and presents the confirmed signature:

~~~
   Client                     Server              Solana Network
      |                          |                        |
      |  (1) Build & sign tx     |                        |
      |                          |                        |
      |  (2) sendTransaction     |                        |
      |----------------------------------------------->   |
      |                          |                        |
      |  (3) Poll confirmation   |                        |
      |----------------------------------------------->   |
      |  (4) Confirmed           |                        |
      |<-----------------------------------------------   |
      |                          |                        |
      |  (5) Authorization:      |                        |
      |      Payment <credential>|                        |
      |      (tx signature)      |                        |
      |----------------------->  |                        |
      |                          |  (6) getTransaction    |
      |                          |----------------------> |
      |                          |  (7) Verified          |
      |                          |<---------------------- |
      |                          |                        |
      |  (8) 200 OK + Receipt    |                        |
      |<-----------------------  |                        |
~~~

1. Client builds a transfer transaction and signs it.
2. Client sends the transaction to the Solana network.
3. Client polls for confirmation status.
4. Transaction reaches `confirmed` commitment level.
5. Client presents the transaction signature as the
   credential.
6. Server fetches the transaction via RPC and verifies
   transfer details.
7. Server confirms the payment matches the challenge.
8. Server returns the resource with a Payment-Receipt.

## Client Transaction Construction

### Native SOL

The client MUST construct a transaction containing a
System Program `transfer` instruction with:

- `source`: the client's signing account
- `destination`: the `recipient` from the challenge
- `lamports`: the `amount` from the challenge

### SPL Tokens

The client MUST construct a transaction containing:

1. Zero or more idempotent Associated Token Account creation
   instructions permitted by {{split-recipient-ata-creation}}:
   - When `feePayer` is `true`, the client MUST include an
     idempotent ATA creation instruction for each split
     recipient whose split entry sets `ataCreationRequired` to
     `true`, and MUST NOT include ATA creation instructions
     for the top-level `recipient`, unmarked split recipients,
     or arbitrary owners.
   - When `feePayer` is `false` or omitted, the client MAY
     include idempotent ATA creation instructions only for split
     recipients, and MUST include one for each split whose entry
     sets `ataCreationRequired` to `true`. The client MUST NOT
     include ATA creation instructions for the top-level
     `recipient`.

   The transaction fee payer covers the rent-exempt minimum
   (~0.002 SOL) if the account does not exist.

2. A `transferChecked` instruction on the appropriate
   token program for the primary payment, and one additional
   `transferChecked` instruction for each split. Each transfer
   uses:
   - `source`: the client's associated token account
   - `mint`: the `currency` field
   - `destination`: the payment recipient's derived ATA
   - `authority`: the client's signing account
   - `amount`: the primary remainder or split amount
   - `decimals`: the `decimals` from `methodDetails`

### Fee Payer Configuration

When `feePayer` is `true` in the challenge:

- The client MUST set the server's `feePayerKey` as the
  transaction fee payer.
- The client MUST sign the transaction only with its own
  key (transfer authority).
- The fee payer signature slot MUST be left empty for the
  server to fill.

When `feePayer` is `false` or absent:

- The client MUST set itself as the transaction fee payer.
- The client MUST fully sign the transaction.

Clients SHOULD set a compute unit limit and priority
fee appropriate for current network conditions.

## Confirmation Requirements

For `type="signature"` credentials, clients MUST wait for
at least the `confirmed` commitment level before presenting
the credential. Servers MUST fetch the transaction with at
least `confirmed` commitment. Servers MAY require
`finalized` commitment for high-value transactions.

For `type="transaction"` credentials, the server controls
the broadcast and confirmation process. Servers MUST wait
for at least `confirmed` commitment before returning the
receipt.

## Finality

Solana provides two commitment levels relevant to
payment verification:

- `confirmed`: optimistic confirmation from a
  supermajority of validators (~400ms). Sufficient
  for most payment use cases.
- `finalized`: deterministic finality after ~31 slots
  (~12 seconds). Required for high-value transactions
  where rollback risk is unacceptable.

In theory, a `confirmed` transaction could be rolled
back if validators shift consensus to a competing fork
that excludes the confirmed block. In practice, this
has never occurred on Solana mainnet. The `confirmed`
level is RECOMMENDED as the default for payment
verification to minimize latency.

## Receipt Generation

Upon successful verification, the server MUST include
a `Payment-Receipt` header in the 200 response.

The receipt payload for Solana charge:

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | `"solana"` |
| `challengeId` | string | The challenge `id` from `WWW-Authenticate` |
| `reference` | string | The transaction signature (base58-encoded) |
| `status` | string | `"success"` |
| `timestamp` | string | {{RFC3339}} verification time |

Example (decoded):

~~~json
{
  "method": "solana",
  "challengeId": "kM9xPqWvT2nJrHsY4aDfEb",
  "reference": "5UfDuX7hXbPjGUpTmt9PHRLsNGJe4dEny...",
  "status": "success",
  "timestamp": "2026-03-10T21:00:00Z"
}
~~~

# Error Responses

When rejecting a credential, the server MUST return HTTP
402 (Payment Required) with a fresh
`WWW-Authenticate: Payment` challenge per
{{I-D.httpauth-payment}}. The server SHOULD include a
response body conforming to RFC 9457 {{RFC9457}} Problem
Details, with `Content-Type: application/problem+json`.
Servers MUST use the standard problem types defined in
{{I-D.httpauth-payment}}: `malformed-credential`,
`invalid-challenge`, and `verification-failed`. The
`detail` field SHOULD contain a human-readable
description of the specific failure (e.g., "Transaction
not found", "Amount mismatch", "Signature already
consumed").

All error responses MUST include a fresh challenge in
`WWW-Authenticate`.

Example error response body:

~~~json
{
  "type": "https://paymentauth.org/problems/verification-failed",
  "title": "Transfer Mismatch",
  "status": 402,
  "detail": "Destination token account does not belong to expected recipient"
}
~~~

# Security Considerations

## Transport Security

All communication MUST use TLS 1.2 or higher. Solana
credentials MUST only be transmitted over HTTPS
connections.

## Replay Protection Considerations

Servers MUST track consumed transaction signatures and
reject any signature that has already been accepted.
The check-and-consume operation MUST be atomic to
prevent race conditions where concurrent requests
present the same signature. Transaction signatures are
globally unique on the Solana network (derived from the
signer's key and the blockhash), making them natural
replay prevention tokens.

## Client-Side Verification

Clients MUST verify the challenge before signing:

1. `amount` is reasonable for the service
2. `currency` matches the expected asset
3. `recipient` is the expected party
4. If `currency` is a mint address, verify it is a known token
5. `splits`, if present, contain expected recipients
   and amounts — malicious servers could add splits
   to redirect funds
6. `feePayerKey`, if present, is the expected server

Malicious servers could request excessive amounts,
direct payments to unexpected recipients, or add
hidden splits.

## RPC Trust

The server relies on its Solana RPC endpoint to
provide accurate transaction data for on-chain
verification. A compromised RPC could return
fabricated transaction data, causing the server to
accept payments that were never made. Servers SHOULD
use trusted RPC providers or run their own nodes.

## Front-running (Push Mode)

In push mode, the client broadcasts the transaction
before presenting the credential, making it visible
on-chain. A party monitoring the chain could attempt
to present the same signature to the server. The
challenge binding (the credential echoes the challenge
`id`, which is HMAC-verified) and single-use signature
enforcement mitigate this: only the party that received
the challenge can construct a valid credential.

Push mode does not require the on-chain transaction to
carry a challenge-specific marker. It proves that a
payment matching the challenged terms was made, but not
necessarily that the payment was created for one unique
challenge instance. If multiple valid challenges have
identical terms, the same confirmed transaction could
satisfy any one of them, and the first accepted
presentation wins.

Requiring an on-chain marker such as a Memo carrying
the challenge `id` would provide stronger binding, but
would also reveal extra correlation metadata on chain.
This specification does not require such a marker in
the base flow, but implementations MAY define a
backward-compatible profile that does.

Pull mode is not susceptible to front-running because
the transaction is not broadcast until the server
receives and validates the credential.

## Fee Payer Risks {#fee-payer-risks}

Servers acting as fee payers accept financial risk in
exchange for providing a seamless payment experience.

Denial of Service via Bad Transactions
: Malicious clients could submit transactions that
  fail on-chain (insufficient balance, invalid
  instructions), causing the server to pay ~5,000
  lamports per failed transaction. Mitigations:

  - **Transaction simulation**: `simulateTransaction`
    catches most failures before broadcast, without
    spending fees. Servers MUST simulate fee-sponsored
    pull mode transactions before broadcasting.
    Servers SHOULD simulate non-fee-sponsored pull
    mode transactions before broadcasting.
  - **Rate limiting**: per client address, per IP, or
    per time window.
  - **Balance verification**: check the client's
    balance covers the transfer amount before signing.
  - **Client authentication**: require API keys or
    OAuth tokens before accepting fee-sponsored
    transactions.

ATA Rent Drain
: When the fee payer funds creation of an Associated
  Token Account (ATA), it pays ~0.002 SOL in rent.
  The recipient can close the ATA to reclaim rent,
  then the next payment re-creates it at the fee
  payer's expense. Servers SHOULD verify the
  top-level recipient's ATA exists before issuing a
  challenge, because top-level recipient ATA creation is not
  allowed by this specification. For split recipients,
  servers that set `ataCreationRequired: true` are explicitly
  accepting rent risk for those split recipient ATAs and SHOULD
  apply stricter rate limits, authentication, and cost-recovery
  policy.

Fee Payer Balance Exhaustion
: Servers MUST monitor fee payer balance and reject
  new fee-sponsored requests when insufficient. The
  server SHOULD return a 402 with `feePayer: false`,
  allowing the client to pay its own fees as fallback.

## Transaction Payload Security

In pull mode, the server receives raw transaction
bytes from the client. A malicious client could craft
a transaction that transfers funds FROM the server's
fee payer account rather than simply paying fees.

Servers MUST verify that the transaction contains
only the expected instructions: transfer instruction(s)
matching the challenge parameters, ATA creation
(idempotent), and optionally compute budget
instructions. Any unexpected instructions MUST cause
rejection.

## Blockhash Freshness

When the server provides `recentBlockhash` in the
challenge, clients SHOULD verify it is plausible
(not obviously stale). A malicious server could
provide an expired blockhash, causing the client to
sign a transaction that will never land — wasting
the signing effort. However, since the transaction
is not broadcast by the client in pull mode, the
practical risk is limited to a failed payment
attempt that the client can retry.

# IANA Considerations

## Payment Method Registration

This document requests registration of the following
entry in the "HTTP Payment Methods" registry established
by {{I-D.httpauth-payment}}:

| Method Identifier | Description | Reference |
|-------------------|-------------|-----------|
| `solana` | Solana blockchain native SOL and SPL token transfer | This document |

## Payment Intent Registration

This document requests registration of the following
entry in the "HTTP Payment Intents" registry established
by {{I-D.httpauth-payment}}:

| Intent | Applicable Methods | Description | Reference |
|--------|-------------------|-------------|-----------|
| `charge` | `solana` | One-time SOL or SPL token transfer | This document |

--- back

# Examples

The following examples illustrate the complete HTTP exchange
for each flow. Base64url values are shown with their decoded
JSON below.

## Native SOL Charge (Pull Mode)

A 0.01 SOL charge for weather API access.

**1. Challenge (402 response):**

~~~http
HTTP/1.1 402 Payment Required
WWW-Authenticate: Payment id="kM9xPqWvT2nJrHsY4aDfEb",
  realm="api.example.com",
  method="solana",
  intent="charge",
  request="eyJhbW91bnQiOiIxMDAwMDAwMCIsImN1cnJlbmN5Ij
    oiU09MIiwiZGVzY3JpcHRpb24iOiJXZWF0aGVyIEFQSSBhY2
    Nlc3MiLCJtZXRob2REZXRhaWxzIjp7Im5ldHdvcmsiOiJtYWl
    ubmV0LWJldGEiLCJyZWZlcmVuY2UiOiJmNDdhYzEwYi01OGNj
    LTQzNzItYTU2Ny0wZTAyYjJjM2Q0NzkifSwicmVjaXBpZW50I
    joiN3hLWHRnMkNXODdkOTdUWEpTRHBiRDVqQmtoZVRxQTgzVF
    pSdUpvc2dBc1UifQ",
  expires="2026-03-15T12:05:00Z"
Cache-Control: no-store
~~~

Decoded `request`:

~~~json
{
  "amount": "10000000",
  "currency": "sol",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Weather API access",
  "methodDetails": {
    "network": "mainnet"
  }
}
~~~

**2. Credential (retry with signed transaction):**

~~~http
GET /weather HTTP/1.1
Host: api.example.com
Authorization: Payment <base64url-encoded credential>
~~~

Decoded credential:

~~~json
{
  "challenge": {
    "id": "kM9xPqWvT2nJrHsY4aDfEb",
    "realm": "api.example.com",
    "method": "solana",
    "intent": "charge",
    "request": "<base64url-encoded request>",
    "expires": "2026-03-15T12:05:00Z"
  },
  "payload": {
    "type": "transaction",
    "transaction": "<base64-encoded signed transaction>"
  }
}
~~~

**3. Response (with receipt):**

~~~http
HTTP/1.1 200 OK
Payment-Receipt: <base64url-encoded receipt>
Content-Type: application/json

{"temperature": 72, "condition": "sunny"}
~~~

Decoded receipt:

~~~json
{
  "method": "solana",
  "challengeId": "kM9xPqWvT2nJrHsY4aDfEb",
  "reference": "4vJ9YFuPzUgdLkWYJf3KqfNM8cTnBp3jXx...",
  "status": "success",
  "timestamp": "2026-03-15T12:04:58Z"
}
~~~

## SPL Token (USDC) Charge with Fee Sponsorship

A 1 USDC charge where the server sponsors transaction fees
and includes a `recentBlockhash` to eliminate client RPC
dependency.

Decoded `request`:

~~~json
{
  "amount": "1000000",
  "currency": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Premium API call",
  "methodDetails": {
    "network": "mainnet",
    "decimals": 6,
    "tokenProgram": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
    "feePayer": true,
    "feePayerKey": "Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr",
    "recentBlockhash": "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N"
  }
}
~~~

The client uses `recentBlockhash` from the challenge (no RPC
call needed), sets `feePayerKey` as the transaction fee payer,
and partially signs with its own key only. The server
verifies the transaction contents, co-signs as fee payer,
and broadcasts.

Decoded credential:

~~~json
{
  "challenge": { "..." : "echoed challenge" },
  "payload": {
    "type": "transaction",
    "transaction": "<base64-encoded partially-signed tx>"
  }
}
~~~

## Push Mode (type="signature")

The client broadcasts the transaction itself and presents
the confirmed signature. Cannot be used with fee sponsorship.

Decoded credential:

~~~json
{
  "challenge": { "..." : "echoed challenge" },
  "payload": {
    "type": "signature",
    "signature": "4vJ9YFuPzUgdLkWYJf3KqfNM8cTnBp3jXx..."
  }
}
~~~

## Payment Splits

A marketplace charge of 1.05 USDC where 0.05 USDC goes to
the platform as a fee.

Decoded `request`:

~~~json
{
  "amount": "1050000",
  "currency": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
  "recipient": "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU",
  "description": "Marketplace purchase",
  "methodDetails": {
    "network": "mainnet",
    "decimals": 6,
    "splits": [
      {
        "recipient": "3pF8Kg2aHbNvJkLMwEqR7YtDxZ5sGhJn4UV6mWcXrT9A",
        "amount": "50000",
        "memo": "platform fee"
      }
    ]
  }
}
~~~

The client builds a transaction with two transfers: 1,000,000
base units to the primary recipient and 50,000 to the platform.
The total paid remains 1,050,000 base units, matching the
top-level `amount`.

# Acknowledgements

The authors thank the Tempo team for their input on this
specification.
