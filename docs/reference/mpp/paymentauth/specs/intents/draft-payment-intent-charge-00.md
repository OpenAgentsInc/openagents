---
title: Charge Intent for HTTP Payment Authentication
abbrev: Payment Intent Charge
docname: draft-payment-intent-charge-00
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
  I-D.httpauth-payment:
    title: "The 'Payment' HTTP Authentication Scheme"
    target: https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/
    author:
      - name: Jake Moxey
    date: 2026-01
---

--- abstract

This document defines the "charge" payment intent for use with the Payment
HTTP Authentication Scheme {{I-D.httpauth-payment}}. The "charge" intent
represents a one-time payment where the payer provides proof of payment
immediately in exchange for resource access.

--- middle

# Introduction

The "charge" intent is the most fundamental payment pattern: a one-time
exchange of payment for resource access. The payer provides proof of
payment (or a signed authorization to collect payment), and the server
grants access to the requested resource.

This intent applies to any payment method that supports immediate payment
verification, including:

- Invoice-based systems (preimage revelation)
- Signed transaction authorization
- Token-based payment confirmation
- Traditional payment processor confirmation

## Relationship to Payment Methods

This document defines the abstract semantics of the "charge" intent.
Payment method specifications define how to implement this intent using
their specific payment infrastructure.

# Requirements Language

{::boilerplate bcp14-tagged}

# Terminology

Charge
: A one-time payment where the payer provides proof of payment
  immediately in exchange for resource access.

Base Units
: The smallest denomination of a currency or asset. For USD, this is
  cents (1/100). For tokens, this is the smallest transferable unit
  defined by the token's decimal precision.

# Intent Semantics

## Definition

The "charge" intent represents a request for immediate, one-time payment
of a specified amount in exchange for resource access.

## Properties

| Property | Value |
|----------|-------|
| **Intent Identifier** | `charge` |
| **Payment Timing** | Immediate (before or with request) |
| **Idempotency** | Single-use per challenge |
| **Reversibility** | Method-dependent |

## Flow

1. Server issues a 402 response with `intent="charge"`
2. Client fulfills the payment (method-specific)
3. Client submits credential with proof of payment
4. Server verifies payment and grants access
5. Server returns `Payment-Receipt` header

## Atomicity

The "charge" intent implies atomic exchange: the server SHOULD NOT
provide partial access if payment verification fails. Either the full
resource is provided (payment succeeded) or access is denied (payment
failed).

# Request Schema

The `request` parameter for a "charge" intent is a JSON object with
shared fields defined by this specification and optional method-specific
extensions in the `methodDetails` field. The `request` JSON MUST be
serialized using JSON Canonicalization Scheme (JCS) and base64url-encoded
without padding per {{I-D.httpauth-payment}}.

## Shared Fields

All payment methods implementing the "charge" intent MUST support these
shared fields, enabling clients to parse and display payment requests
consistently across methods. Payment methods MAY elevate OPTIONAL fields
to REQUIRED in their method specification (e.g., `recipient` and
`expires` are REQUIRED for blockchain methods).

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `amount` | string | Payment amount in base units (smallest denomination) |
| `currency` | string | Currency or asset identifier (see {{currency-formats}}) |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `recipient` | string | Payment recipient in method-native format |
| `description` | string | Human-readable payment description |
| `externalId` | string | Merchant's reference (order ID, invoice number, etc.) |
| `methodDetails` | object | Method-specific extension data |

Challenge expiry is conveyed by the `expires` auth-param in
`WWW-Authenticate` per {{I-D.httpauth-payment}}, using {{RFC3339}}
format. Request objects MUST NOT duplicate the expiry value.

## Currency Formats {#currency-formats}

The `currency` field supports multiple formats to accommodate different
payment networks:

| Format | Example | Description |
|--------|---------|-------------|
| ISO 4217 | `"usd"`, `"eur"` | Fiat currencies (lowercase) |
| Token address | `"0x20c0..."` | On-chain token contract address |
| Method-defined | (varies) | Payment method-specific currency identifiers |

Payment method specifications MUST document which currency formats they
support and how to interpret amounts for each format.

## Method Extensions

Payment methods MAY define additional fields in the `methodDetails` object.
These fields are method-specific and MUST be documented in the payment
method specification. Clients that do not recognize a payment method
SHOULD ignore `methodDetails` but MUST still be able to display the
shared fields to users.

## Examples

### Traditional Payment Processor (Stripe)

~~~ json
{
  "amount": "5000",
  "currency": "usd",
  "description": "Premium API access",
  "externalId": "order_12345",
  "methodDetails": {
    "networkId": "profile_1MqDcVKA5fEO2tZvKQm9g8Yj",
    "paymentMethodTypes": ["card", "link"]
  }
}
~~~

### Blockchain Payment (Tempo)

~~~ json
{
  "amount": "1000000",
  "currency": "0x20c0000000000000000000000000000000000000",
  "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00",
  "methodDetails": {
    "chainId": 4217,
    "feePayer": true
  }
}
~~~

### Lightning Network

~~~ json
{
  "amount": "100000",
  "currency": "sat",
  "methodDetails": {
    "invoice": "lnbc1000n1pj9..."
  }
}
~~~

Payment method specifications define the complete `methodDetails` schema
for their implementation of the "charge" intent.

# Credential Requirements

## Payload

The credential structure follows {{I-D.httpauth-payment}},
containing `challenge`, `payload`, and an optional `source` field
identifying the payer. The `payload` for a "charge" intent MUST contain
proof that payment has been made or authorized. The proof type is
method-specific:

| Proof Type | Description | Example Methods |
|------------|-------------|-----------------|
| Preimage | Hash preimage proving invoice payment | Lightning |
| Signature | Signed transaction authorization | Tempo, EVM |
| Confirmation | Payment processor confirmation identifier | Stripe |
| Ledger transaction | Transaction hash on public ledger | Bitcoin, Ethereum |

## Single-Use

Each credential MUST be usable only once per challenge. Servers MUST
reject replayed credentials.

# Verification

## Server Responsibilities

Servers verifying a "charge" credential MUST:

1. Verify the `id` matches an outstanding challenge
2. Verify the challenge has not expired
3. Verify the payment proof using method-specific procedures
4. Verify the payment amount matches the request
5. Verify the payment recipient matches the request

## Settlement

Settlement semantics differ by method:

- **Immediate settlement**: Payment is final upon verification
  (e.g., Lightning preimage, confirmed blockchain transaction)
- **Deferred settlement**: Server submits payment after verification
  (e.g., signed authorization submitted to chain)
- **Processor settlement**: External processor handles settlement
  (e.g., Stripe PaymentIntent)

# Security Considerations

## Amount Verification

Clients MUST verify the requested amount is appropriate for the resource
before authorizing payment. Malicious servers could request excessive
amounts.

## Recipient Verification

Clients SHOULD verify the payment recipient when possible. Not all
payment methods expose an explicit recipient (e.g., processor-based
methods like Stripe route payments internally). For methods that do
expose a recipient (e.g., blockchain addresses), clients SHOULD warn
users about unknown recipients.

## Replay Protection

Servers MUST implement replay protection. Each challenge `id` MUST be
single-use. Servers MUST NOT accept the same credential twice.

## Finality

The finality of a "charge" payment depends on the payment method:

- Some methods provide instant finality (Lightning)
- Some methods may have delayed finality (blockchain confirmations)
- Some methods may be reversible (card chargebacks)

Servers SHOULD understand the finality guarantees of their accepted
payment methods and adjust resource access accordingly.

## Transport Security

All Payment authentication flows MUST use TLS 1.2 or later per
{{I-D.httpauth-payment}}. Payment credentials contain sensitive
authorization data that could result in financial loss if intercepted.

## Currency Verification

Clients MUST verify the `currency` field matches their expectation
before authorizing payment. Malicious servers could request payment
in a different currency or token than expected.

# IANA Considerations

## Payment Intent Registration

This document registers the "charge" intent in the "HTTP Payment Intents"
registry established by {{I-D.httpauth-payment}}:

| Intent | Description | Reference |
|--------|-------------|-----------|
| `charge` | One-time immediate payment | This document |

Contact: Tempo Labs (<contact@tempo.xyz>)
