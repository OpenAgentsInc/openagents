---
title: Stripe charge Intent for HTTP Payment Authentication
abbrev: Stripe Charge
docname: draft-stripe-charge-00
version: 00
category: info
ipr: noModificationTrust200902
submissiontype: IETF
consensus: true

author:
  - name: Brendan Ryan
    ins: B. Ryan
    email: brendan@tempo.xyz
    org: Tempo Labs
  - name: Steve Kaliski
    ins: S. Kaliski
    email: stevekaliski@stripe.com
    org: Stripe

normative:
  RFC2119:
  RFC3339:
  RFC8174:
  RFC8785:
  RFC7235:
  I-D.httpauth-payment:
    title: "The 'Payment' HTTP Authentication Scheme"
    target: https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/
    author:
      - name: Jake Moxey
    date: 2026-01

informative:
  STRIPE-API:
    target: https://stripe.com/docs/api
    title: Stripe API Reference
    author:
      - org: Stripe, Inc.
  STRIPE-SPT:
    target: https://docs.stripe.com/agentic-commerce/concepts/shared-payment-tokens
    title: Shared payment tokens
    author:
      - org: Stripe, Inc.
---

--- abstract

This document defines the "charge" intent for the Stripe payment method
within the Payment HTTP Authentication Scheme {{I-D.httpauth-payment}}.
It specifies how clients and servers exchange one-time payments using
Shared Payment Tokens (SPTs).

--- middle

# Introduction

This specification defines the "charge" intent for use with the Stripe
payment method in the Payment HTTP Authentication Scheme
{{I-D.httpauth-payment}}. The charge intent enables one-time payments
where the server processes the payment immediately upon receiving a
Shared Payment Token (SPT).

Stripe provides payment processing through SPTs, which are single-use
tokens that represent payment authorization. SPTs abstract away the
complexity of payment method details (cards, bank accounts, wallets)
and provide a unified interface for payment acceptance.

## Stripe Charge Flow

The following diagram illustrates the Stripe charge payment flow:

~~~
   Client                          Server                          Stripe
      |                               |                               |
      |  (1) GET /resource            |                               |
      |---------------------------->  |                               |
      |                               |                               |
      |  (2) 402 Payment Required     |                               |
      |      intent="charge",         |                               |
      |      request=<base64url>      |                               |
      |<----------------------------- |                               |
      |                               |                               |
      |  (3) Collect payment method   |                               |
      |      via Stripe.js and        |                               |
      |      generate SPT             |                               |
      |      (may prompt for 3DS,     |                               |
      |      biometrics, etc.)        |                               |
      |------------------------------------------------------------>  |
      |                               |                               |
      |  (4) Authorization:           |                               |
      |      Payment <credential>     |                               |
      |---------------------------->  |                               |
      |                               |  (5) Create PaymentIntent     |
      |                               |      (Stripe API, using SPT)  |
      |                               |---------------------------->  |
      |                               |                               |
      |  (6) 200 OK                   |                               |
      |      Payment-Receipt:         |                               |
      |      <receipt>                |                               |
      |<----------------------------  |                               |
      |                               |                               |
~~~


## Relationship to the Payment Scheme

This document is a payment method intent specification as defined in
{{I-D.httpauth-payment}}. It defines the `request` and
`payload` structures for the `charge` intent of the `stripe` payment
method, along with verification and settlement procedures.

# Requirements Language

{::boilerplate bcp14-tagged}

# Terminology

Shared Payment Token (SPT)
: A single-use token (prefixed with `spt_`) that represents authorization
  to charge a payment method. SPTs are created by clients using the
  Stripe API and consumed by servers to process payments. Both the Client
  and Server require a Stripe account. In the Stripe API, SPTs are
  referenced as `shared_payment_granted_token` on PaymentIntent creation.
  See {{STRIPE-SPT}}.

Business Network Profile
: A Stripe profile is a business’s public identity on Stripe. With a Stripe profile,
  businesses can find, verify, and connect with each other on Stripe.
  Learn more: https://docs.stripe.com/get-started/account/profile

Payment Intent
: A Stripe API object that tracks the lifecycle of a customer payment,
  from creation through settlement. Not to be confused with the HTTP
  Payment Auth protocol's "payment intent" parameter.
  Learn more: https://docs.stripe.com/payments/payment-intents

# Intent Identifier

This specification defines the following intent for the `stripe` payment
method:

~~~
charge
~~~

The intent identifier is case-sensitive and MUST be lowercase.

# Intent: "charge"

A one-time payment of the specified amount. The server processes the
payment immediately upon receiving the SPT.

**Fulfillment mechanism:**

1. **Shared Payment Token (SPT)**: The payer creates an SPT using the
   Stripe API, which the server uses to create a PaymentIntent via Stripe.

# Request Schema

The `request` parameter in the `WWW-Authenticate` challenge contains a
base64url-encoded JSON object with the following fields. The JSON MUST
be serialized using JSON Canonicalization Scheme (JCS) {{RFC8785}} before
base64url encoding, per {{I-D.httpauth-payment}}.

## Shared Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `amount` | string | REQUIRED | Amount in smallest currency unit (e.g., cents), encoded as a string |
| `currency` | string | REQUIRED | Three-letter ISO currency code (e.g., `"usd"`) |
| `description` | string | OPTIONAL | Human-readable payment description |
| `externalId` | string | OPTIONAL | Merchant's identifier (e.g., order ID, cart ID) |
| `recipient` | string | OPTIONAL | Payment recipient identifier |

## Method Details

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `methodDetails.networkId` | string | REQUIRED | Stripe Business Network Profile ID |
| `methodDetails.paymentMethodTypes` | []string | REQUIRED | The list of payment method types that the seller can process. |
| `methodDetails.metadata` | object | OPTIONAL | Key-value pairs for additional context |

**Example:**

~~~ json
{
  "amount": "5000",
  "currency": "usd",
  "description": "Premium API access for 1 month",
  "externalId": "order_12345",
  "methodDetails": {
    "networkId": "profile_1MqDcVKA5fEO2tZvKQm9g8Yj",
    "paymentMethodTypes": ["card", "link"]
  }
}
~~~

The client fulfills this by creating an SPT using Stripe:

~~~ javascript
const spt = await stripe.sharedPayment.issuedTokens.create({
  payment_method: 'pm_123',
  usage_limits: {
    currency: 'usd',
    max_amount: 5000,
    expires_at: Timestamp
  },
  seller_details: {
    networkId: 'profile_123'
  }
});
// Returns: { id: 'spt_1N...' }
~~~

# Credential Schema

The Payment credential is a base64url-encoded JSON object containing
`challenge` and `payload` fields per
{{I-D.httpauth-payment}}. For Stripe charge, the `payload` object
contains the following fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `spt` | string | REQUIRED | Shared Payment Token ID (starts with `spt_`) |
| `externalId` | string | OPTIONAL | Client's reference ID |

**Example:**

~~~ json
{
  "spt": "spt_1N4Zv32eZvKYlo2CPhVPkJlW",
  "externalId": "client_order_789"
}
~~~

# Verification Procedure {#charge-verification}

Servers MUST verify Payment credentials for charge intent:

1. Verify the challenge ID matches the one issued
2. Verify the challenge has not expired
3. Extract the `spt` from the credential payload
4. Verify the SPT has not been previously used (replay protection)
5. Validate the SPT exists and is valid via Stripe API (optional pre-check)

Servers MUST complete challenge ID validation and expiry checks (steps 1-2)
before processing credential material (steps 3-5). This ensures basic
request validity is established before accessing payment tokens.

## Challenge Binding

Servers MUST verify that the credential corresponds to the exact challenge
issued. This includes validating:

- Challenge ID
- Amount (if specified in request)
- Currency
- Business Network (if specified)
- Any custom metadata

# Settlement Procedure {#charge-settlement}

**Synchronous settlement:**

1. Server receives and verifies the credential ({{charge-verification}})
2. Server creates a Stripe PaymentIntent with `confirm: true` and the
   SPT as `shared_payment_granted_token`. The server MAY include
   Stripe Connect settlement parameters as described in
   {{stripe-connect-settlement}}:

~~~ javascript
const stripeDetails = request.methodDetails || {};
const settlementPolicy = getServerSettlementPolicy(challenge, request);

const paymentIntentParams = {
  amount: Number(request.amount),
  currency: request.currency,
  shared_payment_granted_token: credential.spt,
  confirm: true,
  automatic_payment_methods: {
    enabled: true,
    allow_redirects: 'never'
  },
  metadata: {
    ...(stripeDetails.metadata || {}),
    challenge_id: challenge.id
  }
};

if (settlementPolicy.applicationFeeAmount !== undefined) {
  paymentIntentParams.application_fee_amount =
    settlementPolicy.applicationFeeAmount;
}

if (settlementPolicy.onBehalfOf !== undefined) {
  paymentIntentParams.on_behalf_of = settlementPolicy.onBehalfOf;
}

if (settlementPolicy.transferData !== undefined) {
  paymentIntentParams.transfer_data = {
    destination: settlementPolicy.transferData.destination
  };

  if (settlementPolicy.transferData.amount !== undefined) {
    paymentIntentParams.transfer_data.amount =
      settlementPolicy.transferData.amount;
  }
}

if (settlementPolicy.transferGroup !== undefined) {
  paymentIntentParams.transfer_group = settlementPolicy.transferGroup;
}

const paymentIntentOptions = {
  idempotencyKey: `${challenge.id}_${credential.spt}`
};

if (settlementPolicy.stripeAccount !== undefined) {
  paymentIntentOptions.stripeAccount = settlementPolicy.stripeAccount;
}

const paymentIntent = await stripe.paymentIntents.create(
  paymentIntentParams,
  paymentIntentOptions
);
~~~

3. Server MUST verify the PaymentIntent `status` is `"succeeded"`
   before returning 200 with `Payment-Receipt` header
4. If the PaymentIntent fails or requires additional action, server
   returns 402 with a new challenge

**Idempotency:**

Servers SHOULD include an idempotency key derived from the challenge ID
and SPT when creating PaymentIntents. This prevents duplicate charges
if the client retries a request.

**Settlement timing:**

Stripe processes fund transfers asynchronously. Servers SHOULD return
200 immediately after PaymentIntent confirmation (status `"succeeded"`),
even if final fund settlement to the merchant is pending.

## Stripe Connect Settlement {#stripe-connect-settlement}

Servers that use Stripe Connect MAY apply Connect parameters when
creating the Stripe PaymentIntent. These parameters are settlement
policy inputs controlled by the server, not payment request fields
that clients need in order to create an SPT.

The following Stripe PaymentIntent create parameters are compatible
with this specification:

| Stripe parameter | Description |
|------------------|-------------|
| `Stripe-Account` header or SDK `stripeAccount` option | Connected account used as the Stripe account context for the request |
| `application_fee_amount` | Platform application fee amount in the smallest currency unit |
| `on_behalf_of` | Connected account used as the business of record |
| `transfer_data[destination]` | Connected account that receives transferred funds |
| `transfer_data[amount]` | Amount transferred to `transfer_data[destination]` |
| `transfer_group` | Reconciliation token linking related charges and transfers |

Servers SHOULD derive Connect settlement parameters from server-side
merchant configuration, platform policy, order state, or equivalent
trusted state. Servers MUST NOT include Connect settlement parameters
in the MPP challenge.

## Receipt Generation

Upon successful settlement, servers MUST return a `Payment-Receipt` header
per {{I-D.httpauth-payment}}. Servers MUST NOT include a
`Payment-Receipt` header on error responses; failures are communicated via
HTTP status codes and Problem Details.

The receipt payload for Stripe charge:

| Field | Type | Description |
|-------|------|-------------|
| `method` | string | `"stripe"` |
| `reference` | string | Stripe PaymentIntent ID (e.g., `"pi_1N4..."`) |
| `status` | string | `"success"` |
| `timestamp` | string | {{RFC3339}} confirmation time |
| `externalId` | string | OPTIONAL. Echoed from credential payload |

# Security Considerations

## SPT Single-Use Constraint

SPTs are single-use tokens. Stripe automatically prevents SPT reuse at
the API level, and idempotency keys ({{charge-settlement}}) prevent
duplicate PaymentIntent creation. Servers MUST enforce single-use
challenge IDs per {{I-D.httpauth-payment}} and SHOULD
use Stripe idempotency keys to prevent repeated charges. Servers MAY
additionally maintain a local replay cache of consumed challenge IDs.

## Amount Verification

Clients MUST verify the payment amount in the challenge matches their
expectation before creating an SPT. The SPT usage limits constrain the
currency, maximum amount, and expiration window granted to the seller,
but those limits are derived from the challenge parameters the client
accepts.

**Verification checklist:**

1. Verify the `amount` matches the expected cost
2. Verify the `currency` matches the expected currency
3. Verify the `description` matches the expected service
4. Verify the challenge hasn't expired
5. Verify the server's identity (TLS certificate validation)

## Stripe Connect Parameter Integrity

Servers that use Stripe Connect settlement parameters MUST validate
those parameters before creating the PaymentIntent:

- Connected account identifiers, including `stripeAccount`,
  `on_behalf_of`, and `transfer_data[destination]`, MUST refer to
  connected accounts that the server is authorized to use for the
  current request.
- `transfer_data[destination]` MUST be present when `transfer_data`
  is present.
- `application_fee_amount`, when present, MUST be a non-negative
  integer no greater than the PaymentIntent `amount`.
- `transfer_data[amount]`, when present, MUST be a non-negative
  integer no greater than the PaymentIntent `amount`.
- `transfer_group`, when present, MUST be derived from an order,
  invoice, request, or similar reconciliation identifier and MUST NOT
  be treated as secret.

Servers MUST NOT accept Connect settlement parameters from clients
unless an explicit trust boundary authorizes that client to control
settlement routing or platform fees.

## PCI DSS Compliance

Stripe's SPT model ensures clients never handle raw payment method details,
significantly reducing PCI DSS compliance scope.

## HTTPS Requirement

All communication MUST use TLS 1.2 or higher. Shared Payment Tokens MUST
only be transmitted over HTTPS connections.

# IANA Considerations

## Payment Intent Registration

This specification registers the "charge" intent for the "stripe" payment
method in the Payment Intent Registry established by
{{I-D.httpauth-payment}}:

- **Intent**: charge
- **Method**: stripe
- **Specification**: [this document]

Contact: Stripe (<stevekaliski@stripe.com>) and Tempo Labs (<brendan@tempo.xyz>)

--- back

# ABNF Collected

~~~ abnf
stripe-charge-challenge = "Payment" 1*SP
  "id=" quoted-string ","
  "realm=" quoted-string ","
  "method=" DQUOTE "stripe" DQUOTE ","
  "intent=" DQUOTE "charge" DQUOTE ","
  "request=" base64url-nopad

stripe-charge-credential = "Payment" 1*SP base64url-nopad

; Base64url encoding without padding per RFC 4648 Section 5
base64url-nopad = 1*( ALPHA / DIGIT / "-" / "_" )
~~~

# Examples

## Charge Example (HTTP Transport)

**Step 1: Client requests resource**

~~~ http
GET /api/generate HTTP/1.1
Host: api.example.com
~~~

**Step 2: Server issues payment challenge**

~~~ http
HTTP/1.1 402 Payment Required
WWW-Authenticate: Payment id="ch_1a2b3c4d5e",
  realm="api.example.com",
  method="stripe",
  intent="charge",
  request="eyJhbW91bnQiOiI1MDAwIiwiY3VycmVuY3kiOiJ1c2QiLCJkZXNjcmlwdGlvbiI6IkFJIGdlbmVyYXRpb24ifQ"
Cache-Control: no-store
Content-Type: application/json

{
  "type": "https://paymentauth.org/problems/payment-required",
  "title": "Payment Required",
  "status": 402,
  "detail": "This resource requires payment"
}
~~~

Decoded request:
~~~ json
{
  "amount": "5000",
  "currency": "usd",
  "description": "AI generation"
}
~~~

**Step 3: Client creates SPT and submits credential**

~~~ http
GET /api/generate HTTP/1.1
Host: api.example.com
Authorization: Payment eyJjaGFsbGVuZ2UiOnsiaWQiOiJjaF8xYTJiM2M0ZDVlIiwicmVhbG0iOiJhcGkuZXhhbXBsZS5jb20iLCJtZXRob2QiOiJzdHJpcGUiLCJpbnRlbnQiOiJjaGFyZ2UiLCJyZXF1ZXN0IjoiZXlKaGJXOTFiblFpT2lJMU1EQXdJaXdpWTNWeWNtVnVZM2tpT2lKMWMyUWlMQ0prWlhOamNtbHdkR2x2YmlJNklrRkpJR2RsYm1WeVlYUnBiMjRpZlEiLCJleHBpcmVzIjoiMjAyNS0wMS0xNVQxMjowNTowMFoifSwicGF5bG9hZCI6eyJzcHQiOiJzcHRfMU40WnYzMmVadktZbG8yQ1BoVlBrSmxXIn19

~~~

Decoded credential:
~~~ json
{
  "challenge": {
    "id": "ch_1a2b3c4d5e",
    "realm": "api.example.com",
    "method": "stripe",
    "intent": "charge",
    "request": "eyJhbW91bnQiOiI1MDAwIiwiY3VycmVuY3kiOiJ1c2QiLCJkZXNjcmlwdGlvbiI6IkFJIGdlbmVyYXRpb24ifQ",
    "expires": "2025-01-15T12:05:00Z"
  },
  "payload": {
    "spt": "spt_1N4Zv32eZvKYlo2CPhVPkJlW"
  }
}
~~~

**Step 4: Server processes payment and returns resource**

~~~ http
HTTP/1.1 200 OK
Payment-Receipt: eyJtZXRob2QiOiJzdHJpcGUiLCJyZWZlcmVuY2UiOiJwaV8xTjRadjMyZVp2S1lsbzJDUGhWUGtKbFciLCJzdGF0dXMiOiJzdWNjZXNzIiwidGltZXN0YW1wIjoiMjAyNS0wMS0xNVQxMjowNDozMloifQ
Cache-Control: private
Content-Type: text/plain

Here is your generated content...
~~~

Decoded receipt:
~~~ json
{
  "method": "stripe",
  "reference": "pi_1N4Zv32eZvKYlo2CPhVPkJlW",
  "status": "success",
  "timestamp": "2025-01-15T12:04:32Z"
}
~~~

# Acknowledgements

The authors thank the Tempo community for their feedback on this
specification.
