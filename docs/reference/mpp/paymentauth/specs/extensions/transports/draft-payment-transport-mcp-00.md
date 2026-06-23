---
title: "Payment Authentication Scheme: JSON-RPC & MCP Transport"
abbrev: Payment JSON-RPC & MCP Transport
docname: draft-payment-transport-mcp-00
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

normative:
  RFC2119:
  RFC3339:
  RFC5246:
  RFC6455:
  RFC8174:
  RFC8259:
  RFC8446:
  RFC8785:
  I-D.httpauth-payment:
    title: "The 'Payment' HTTP Authentication Scheme"
    target: https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/
    author:
      - name: Jake Moxey
    date: 2026-01
  MCP:
    title: "Model Context Protocol Specification"
    target: https://modelcontextprotocol.io/specification/2025-11-25
  JSON-RPC:
    title: "JSON-RPC 2.0 Specification"
    target: https://www.jsonrpc.org/specification

informative:
---

--- abstract

This document defines how the Payment HTTP Authentication Scheme
operates over JSON-RPC 2.0 transports. It specifies the mapping
of payment challenges to JSON-RPC error responses using
implementation-defined error codes, credential transmission via
metadata fields, receipt delivery in successful responses, and
error handling conventions. This specification applies to any
transport carrying JSON-RPC 2.0 messages, including WebSocket,
HTTP, stdio, and protocol frameworks such as the Model Context
Protocol (MCP).

--- middle

# Introduction

JSON-RPC 2.0 {{JSON-RPC}} is a stateless, lightweight remote
procedure call protocol using JSON {{RFC8259}}. Many modern
protocols layer JSON-RPC over various transports including HTTP,
WebSocket {{RFC6455}}, and stdio. Protocol frameworks such as the
Model Context Protocol (MCP) {{MCP}} also use JSON-RPC 2.0 as
their message format. This document defines how the Payment HTTP
Authentication Scheme {{I-D.httpauth-payment}} operates within
JSON-RPC 2.0 messages, independent of the underlying transport.

This specification defines:

- Error codes for payment signaling
- Challenge structure in JSON-RPC error responses
- Credential transmission via `_meta` metadata fields
- Receipt delivery via `_meta` metadata fields
- Error handling conventions
- Notification behavior
- Capability advertisement

## Applicability

This specification applies to any system that exchanges JSON-RPC
2.0 messages, including but not limited to:

- **WebSocket**: JSON-RPC over persistent `wss://` connections
  for real-time APIs, streaming services, and subscriptions.

- **HTTP**: JSON-RPC 2.0 over standard HTTP request-response
  exchanges.

- **stdio**: JSON-RPC 2.0 over standard input/output streams
  for local process communication.

This specification also defines MCP-specific conventions for
the Model Context Protocol {{MCP}}, which uses JSON-RPC 2.0
for tool invocations (`tools/call`), resource access
(`resources/read`), and prompt retrieval (`prompts/get`).

Transport-specific security requirements (e.g., TLS for
WebSocket, process isolation for stdio) are addressed in
{{transport-security}}.

## Design Goals

1. **Native JSON**: Use JSON objects directly rather than base64url
   encoding, leveraging JSON-RPC's native capabilities.

2. **Transport Independent**: Define payment semantics at the
   JSON-RPC layer, applicable to any transport carrying JSON-RPC
   messages.

3. **Minimal Overhead**: Add payment data only when needed via
   the `_meta` extension mechanism.

4. **Multiple Options**: Support servers offering multiple payment
   methods in a single challenge.

# Requirements Language

{::boilerplate bcp14-tagged}

# Terminology

This document uses terminology from {{I-D.httpauth-payment}}:

Challenge
: Payment requirements communicated by the server.

Credential
: Payment authorization data sent by the client.

Receipt
: Server acknowledgment of successful payment.

# Protocol Overview

The payment flow follows three phases within JSON-RPC message
exchanges:

~~~
Client                                                 Server
   │                                                      │
   │  (1) JSON-RPC Request                                │
   │      {method: "...", params: {...}}                   │
   ├─────────────────────────────────────────────────────>│
   │                                                      │
   │  (2) JSON-RPC Error                                  │
   │      {code: -32042, data: {challenges: [...]}}       │
   │<─────────────────────────────────────────────────────┤
   │                                                      │
   │  (3) Client fulfills challenge                       │
   │                                                      │
   │  (4) JSON-RPC Request                                │
   │      {method: "...",                                 │
   │       _meta: {credential: {...}}}                    │
   ├─────────────────────────────────────────────────────>│
   │                                                      │
   │  (5) JSON-RPC Result                                 │
   │      {result: {...}, _meta: {receipt: {...}}}         │
   │<─────────────────────────────────────────────────────┤
~~~

# Capability Advertisement

Servers and clients SHOULD advertise supported payment methods
and intents before payment flows begin. The capability object
SHOULD contain:

**`methods`** (REQUIRED): Object mapping payment method
  identifiers (as registered in the IANA HTTP Payment Methods
  registry) to their configuration. Each method object MUST
  contain an `intents` array listing the supported payment
  intent types (as registered in the IANA HTTP Payment Intents
  registry) for that method.

Example capability object:

~~~json
{
  "methods": {
    "tempo": { "intents": ["charge"] },
    "stripe": { "intents": ["charge"] }
  }
}
~~~

The mechanism for advertising capabilities depends on the
transport:

- **WebSocket**: Servers MAY send a `payment.capabilities`
  JSON-RPC notification after connection establishment.

- **MCP**: See {{mcp-capability-advertisement}}.

Clients MAY use capability information to determine
compatibility before invoking paid methods. Clients MUST NOT
rely solely on capability advertisement to determine payment
support; malicious servers could claim capabilities they don't
properly implement. Clients SHOULD validate challenge structure
before fulfilling payment.

## MCP Capability Advertisement {#mcp-capability-advertisement}

For MCP specifically, servers SHOULD advertise payment support
in the `InitializeResult`:

~~~json
{
  "protocolVersion": "2025-11-25",
  "capabilities": {
    "tools": {},
    "resources": {},
    "experimental": {
      "payment": {
        "methods": {
          "tempo": { "intents": ["charge"] },
          "stripe": { "intents": ["charge"] }
        }
      }
    }
  },
  "serverInfo": {
    "name": "example-server",
    "version": "1.0.0"
  }
}
~~~

Clients SHOULD advertise in the `InitializeRequest`:

~~~json
{
  "protocolVersion": "2025-11-25",
  "capabilities": {
    "experimental": {
      "payment": {
        "methods": {
          "tempo": { "intents": ["charge"] }
        }
      }
    }
  },
  "clientInfo": {
    "name": "example-client",
    "version": "1.0.0"
  }
}
~~~

Servers MAY use client capabilities to filter which payment
options to offer in challenges.

# Payment Challenge

## Signaling Payment Required

When a JSON-RPC method requires payment, the server MUST respond
with a JSON-RPC error using code `-32042` (Payment Required).
This code is within the JSON-RPC implementation-defined server
error range (-32000 to -32099) per {{JSON-RPC}}:

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [
        {
          "id": "qB3wErTyU7iOpAsD9fGhJk",
          "realm": "api.example.com",
          "method": "tempo",
          "intent": "charge",
          "request": {
            "amount": "1000",
            "currency": "usd",
            "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00"
          },
          "expires": "2025-01-15T12:05:00Z",
          "description": "API call fee"
        }
      ],
      "problem": {
        "type": "https://paymentauth.org/problems/payment-required",
        "title": "Payment Required",
        "status": 402,
        "detail": "Payment required for access."
      }
    }
  }
}
~~~

The `error.data.httpStatus` field SHOULD be included with value
`402` to indicate the corresponding HTTP status code for
transports that bridge to HTTP (e.g., MCP Streamable HTTP).

## Challenge Structure

The `error.data` object MUST contain:

**`challenges`** (REQUIRED): Array of one or more challenge objects.

**`problem`** (OPTIONAL): An RFC 9457 Problem Details object providing
  additional error context. When present, contains `type`, `title`,
  `status`, `detail`, and optionally `challengeId`.

Each challenge object MUST contain:

**`id`** (REQUIRED): Unique challenge identifier. Servers MUST
  cryptographically bind this value to at minimum the following
  parameters: `realm`, `method`, `intent`, `request` (canonical hash),
  and `expires`. Clients MUST include this value unchanged in the
  credential.

**`realm`** (REQUIRED): Protection space identifier defining the scope
  of the payment requirement.

**`method`** (REQUIRED): Payment method identifier as registered in
  the IANA HTTP Payment Methods registry.

**`intent`** (REQUIRED): Payment intent type as registered in the
  IANA HTTP Payment Intents registry.

**`request`** (REQUIRED): Method-specific payment request data
  as a native JSON object. Servers MUST NOT base64url-encode the
  request when using JSON-RPC transport. For challenge binding
  and challenge ID verification, both parties MUST canonicalize
  `request` using JSON Canonicalization Scheme (JCS) {{RFC8785}}
  and hash the canonicalized bytes. The schema is defined by the
  payment method specification.

Each challenge object MAY contain:

**`expires`** (OPTIONAL): Timestamp in {{RFC3339}} format after which
  the challenge is no longer valid. Clients SHOULD NOT attempt to
  fulfill challenges past their expiry. If absent, servers define the
  validity period.

**`description`** (OPTIONAL): Human-readable description of what the
  payment is for.

## Multiple Payment Options

When multiple challenges are present, they represent **alternative**
payment options. Clients MUST select exactly one challenge to fulfill.
Servers MUST NOT require multiple simultaneous payments via this
mechanism.

Servers MAY offer multiple payment options by including multiple
challenge objects in the `challenges` array:

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [
        {
          "id": "pT7yHnKmQ2wErXsZ5vCbNl",
          "realm": "api.example.com",
          "method": "tempo",
          "intent": "charge",
          "request": {
            "amount": "1000",
            "currency": "usd"
          }
        },
        {
          "id": "mF8uJkLpO3qRtYsA6wDcVb",
          "realm": "api.example.com",
          "method": "stripe",
          "intent": "charge",
          "request": {
            "amount": "1000",
            "currency": "usd"
          }
        }
      ]
    }
  }
}
~~~

Clients SHOULD select one challenge based on their capabilities and
user preferences. Clients MUST send only one credential corresponding
to a single selected challenge.

# Payment Credential

## Metadata Placement {#metadata-placement}

This specification defines two placement strategies for the
`_meta` field, depending on the protocol:

**Root-level `_meta`** (Generic JSON-RPC): The `_meta` field
  is placed at the root of the JSON-RPC message object. This
  approach works with any JSON-RPC method regardless of whether
  `params` is an object or an array:

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "eth_getBlockByNumber",
  "params": ["latest", false],
  "_meta": {
    "org.paymentauth/credential": { ... }
  }
}
~~~

**Nested `_meta`** (MCP): The `_meta` field is placed inside
  `params` (for requests) or `result` (for responses), per MCP
  conventions where `params` is always an object:

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "expensive-api",
    "_meta": {
      "org.paymentauth/credential": { ... }
    }
  }
}
~~~

Servers MUST check both locations for `_meta` and MUST NOT
require clients to use a specific placement. Servers MUST
ignore `org.paymentauth/credential` on methods that do not
require payment.

## Transmitting Payment Data

Clients send payment credentials using the `_meta` field with
key `org.paymentauth/credential`. The key uses reverse-DNS
naming to avoid collisions with other extensions:

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "eth_getBlockByNumber",
  "params": ["latest", false],
  "_meta": {
    "org.paymentauth/credential": {
      "challenge": {
        "id": "qB3wErTyU7iOpAsD9fGhJk",
        "realm": "api.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {
          "amount": "1000",
          "currency": "usd",
          "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00"
        },
        "expires": "2025-01-15T12:05:00Z"
      },
      "payload": {
        "signature": "0x1b2c3d4e5f..."
      }
    }
  }
}
~~~

## Credential Structure

The `org.paymentauth/credential` object MUST contain:

**`challenge`** (REQUIRED): The complete challenge object from the
  server's `-32042` error response. Clients MUST echo the challenge
  unchanged.

**`payload`** (REQUIRED): Method-specific payment proof as a JSON
  object. The schema is defined by the payment method specification.

The credential object MAY contain:

**`source`** (OPTIONAL): Identifier of the payment source (e.g., a
  DID or address).

# Payment Receipt

## Successful Payment Response

After successful payment verification and settlement, servers
MUST include a receipt using `_meta` with key
`org.paymentauth/receipt`. The `_meta` field placement follows
the same rules as {{metadata-placement}}: root-level for generic
JSON-RPC, nested in `result` for MCP.

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "number": "0x1348c9",
    "hash": "0x7736fab79e05dc611604d22470dadad2..."
  },
  "_meta": {
    "org.paymentauth/receipt": {
      "status": "success",
      "method": "tempo",
      "timestamp": "2025-01-15T12:00:30Z",
      "reference": "tx_abc123...",
      "challengeId": "qB3wErTyU7iOpAsD9fGhJk"
    }
  }
}
~~~

Servers MUST return `org.paymentauth/receipt` on every successful
response to a paid request. Servers MUST NOT return receipts for
unpaid requests.

## Receipt Structure

The `org.paymentauth/receipt` object MUST contain:

**`status`** (REQUIRED): Settlement status. MUST be `"success"` for
  successful payments.

**`method`** (REQUIRED): Payment method that was used.

**`timestamp`** (REQUIRED): {{RFC3339}} timestamp of settlement.

**`challengeId`** (REQUIRED): The `id` from the fulfilled challenge.

The receipt object MAY contain:

**`reference`** (OPTIONAL): Method-specific settlement reference
  (e.g., transaction hash, invoice ID).

# MCP Covered Operations

The following MCP operations support payment flows.

## Tool Calls

Tool invocations via `tools/call` MAY require payment:

**Challenge:**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "premium-analysis"
  }
}
~~~

**Response:**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [{
        "id": "tool-pay-123",
        "realm": "tools.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {"amount": "500", "currency": "usd"}
      }]
    }
  }
}
~~~

## Resource Access

Resource reads via `resources/read` MAY require payment:

**Challenge:**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": {
    "uri": "data://premium/market-data"
  }
}
~~~

**Response:**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [{
        "id": "resource-pay-456",
        "realm": "data.example.com",
        "method": "stripe",
        "intent": "charge",
        "request": {"amount": "100", "currency": "usd"}
      }]
    }
  }
}
~~~

## Prompt Retrieval

Prompt retrieval via `prompts/get` MAY require payment:

**Response:**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [{
        "id": "prompt-pay-789",
        "realm": "prompts.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {"amount": "50", "currency": "usd"}
      }]
    }
  }
}
~~~

# Error Handling

## Error Code Mapping

Servers MUST map payment errors to JSON-RPC error codes within the
server error range (-32000 to -32099) per {{JSON-RPC}}:

| Condition | Code | Description |
|-----------|------|-------------|
| Payment required | -32042 | Payment challenge in `error.data` |
| Payment verification failed | -32043 | Fresh challenge + failure reason |
| Malformed credential | -32602 | Invalid params (bad JSON structure) |
| Internal payment error | -32603 | Payment processor failure |

Note: `-32700` (Parse error) applies only when the entire JSON-RPC
message is unparseable, not for malformed `_meta` subfields. Use
`-32602` for credential structure errors.

## Payment Verification Failure

When payment verification fails, servers MUST return code `-32043`
with a fresh challenge and failure details:

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "error": {
    "code": -32043,
    "message": "Payment Verification Failed",
    "data": {
      "httpStatus": 402,
      "challenges": [{
        "id": "retry-challenge-abc",
        "realm": "api.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {"amount": "1000", "currency": "usd"}
      }],
      "failure": {
        "reason": "signature-invalid",
        "detail": "Signature verification failed"
      }
    }
  }
}
~~~

On verification failure, servers MAY return the same challenge if it
remains valid, or issue a fresh challenge. Clients SHOULD treat any
`-32043` response as requiring a new payment attempt.

The `failure` object MAY contain:

**`reason`** (OPTIONAL): Machine-readable failure code.

**`detail`** (OPTIONAL): Human-readable failure description.

## Protocol Errors

Technical errors use standard JSON-RPC error codes:

**Invalid Params (-32602):**

Used for malformed credentials or missing required fields:

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": {
      "detail": "Missing required field: challenge.id"
    }
  }
}
~~~

# Notifications

JSON-RPC notifications are requests without an `id` field that
expect no response. Since payment challenges require a response,
notifications cannot support payment flows.

## Server Behavior

Servers MUST NOT process payment-gated operations invoked as
notifications. Servers SHOULD silently drop such notifications
per JSON-RPC 2.0 semantics.

Servers MAY log dropped payment-required notifications for
debugging purposes.

Servers MAY send JSON-RPC notifications to deliver data after
a client has fulfilled a prior payment challenge (e.g.,
streaming subscription updates over WebSocket).

## Client Guidance

Clients SHOULD NOT invoke payment-gated operations as
notifications. Operations that may require payment SHOULD always
include a request `id` to receive payment challenges and results.

# Security Considerations

## Challenge Binding

Servers MUST cryptographically bind challenge IDs to their parameters
(at minimum: `realm`, `method`, `intent`, `request` hash, `expires`).
The `request` hash MUST be computed over the JCS-canonicalized
representation of `request` per {{RFC8785}}.
This prevents clients from reusing a challenge ID with modified
payment terms.

Servers SHOULD also bind challenges to the specific operation being
requested (e.g., tool name, resource URI) to prevent a challenge
issued for one operation being used for another.

## Replay Protection

Servers MUST reject credentials for:

- Unknown challenge IDs
- Expired challenges (past `expires` timestamp)
- Previously-used challenge IDs

Servers SHOULD maintain a record of used challenge IDs for at least
the challenge validity period. For high-throughput scenarios, servers
MAY use stateless challenge tokens (e.g., MAC over canonical
parameters) and maintain only a post-use replay set.

When two concurrent requests race using the same challenge, servers
MUST ensure only one succeeds via atomic check-and-mark operations.

## Transport Security {#transport-security}

When using network transports (HTTP, WebSocket), all
communication MUST occur over TLS 1.2 {{RFC5246}} or later
(TLS 1.3 {{RFC8446}} RECOMMENDED).

For stdio transport, security depends on the process isolation
provided by the operating system.

For persistent transports (e.g., WebSocket), servers SHOULD
additionally rate limit connection establishment and close
connections that exceed challenge request thresholds.

## Credential Confidentiality

Payment credentials MAY contain sensitive data (signatures, tokens).
Clients MUST NOT log or persist credentials beyond immediate use.
Servers MUST NOT log full credential payloads. This includes crash
dumps, distributed tracing, and analytics telemetry.

## Metadata Stripping

On-path attackers or malicious intermediaries could strip
`org.paymentauth/credential` from requests (causing repeated
payment challenges) or `org.paymentauth/receipt` from responses
(affecting auditability). For network transports, TLS provides
integrity. For stdio transport, rely on process isolation.

## Confused Deputy

Clients may be tricked into paying for unintended operations if
method names or realms are misleading. Client implementations
SHOULD:

- Display `realm`, `amount`, `currency`, and `recipient` to users
  before fulfilling payment challenges
- Allow users to configure payment policies per realm
- Validate that challenge parameters match the requested operation

## Denial of Service

Attackers may trigger many payment challenges to exhaust server
resources or payment processor rate limits. Servers SHOULD:

- Rate limit challenge issuance per client
- Use stateless challenge encoding where possible
- Implement exponential backoff for repeated failures

# IANA Considerations

This document has no IANA actions. Payment methods and intents are
registered per {{I-D.httpauth-payment}}.

Servers MUST use the following JSON-RPC error codes:

| Code | Name | Description |
|------|------|-------------|
| -32042 | Payment Required | Server requires payment to proceed |
| -32043 | Payment Verification Failed | Payment credential invalid |

These codes are within the JSON-RPC server error range (-32000 to
-32099). Implementations MUST use these exact codes for interoperability.

--- back

# Example: Ethereum JSON-RPC over WebSocket

An `eth_getBlockByNumber` call over WebSocket with payment
required for RPC access:

**Connection:**

~~~http
GET / HTTP/1.1
Host: rpc.example.com
Upgrade: websocket
Connection: Upgrade
~~~

**Step 1: Initial Request**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "eth_getBlockByNumber",
  "params": ["latest", false]
}
~~~

**Step 2: Payment Challenge**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [{
        "id": "ch_ws_789",
        "realm": "rpc.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {
          "amount": "1",
          "currency": "usd",
          "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00"
        },
        "expires": "2025-01-15T12:05:00Z",
        "description": "Ethereum RPC call"
      }]
    }
  }
}
~~~

**Step 3: Request with Credential**

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "eth_getBlockByNumber",
  "params": ["latest", false],
  "_meta": {
    "org.paymentauth/credential": {
      "challenge": {
        "id": "ch_ws_789",
        "realm": "rpc.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {
          "amount": "1",
          "currency": "usd",
          "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00"
        },
        "expires": "2025-01-15T12:05:00Z"
      },
      "source": "0x1234567890abcdef...",
      "payload": {
        "signature": "0xabc123..."
      }
    }
  }
}
~~~

**Step 4: Success with Receipt**

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "number": "0x1348c9",
    "hash": "0x7736fab79e05dc611604d22470dadad2...",
    "parentHash": "0x61cdb2a09ab99abf791d474f20c2ea...",
    "timestamp": "0x56ffeff8",
    "gasLimit": "0x47e7c4",
    "gasUsed": "0x38658",
    "miner": "0xf8b483dba2c3b7176a3da549ad41a48b...",
    "transactions": []
  },
  "_meta": {
    "org.paymentauth/receipt": {
      "status": "success",
      "method": "tempo",
      "timestamp": "2025-01-15T12:00:15Z",
      "reference": "0xtx789...",
      "challengeId": "ch_ws_789"
    }
  }
}
~~~

# Example: MCP Tool Call

A complete MCP tool call with payment, using nested `_meta`
per MCP conventions:

**Step 1: Initial Request**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "web-search",
    "arguments": {"query": "MCP protocol"}
  }
}
~~~

**Step 2: Payment Challenge**

~~~json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32042,
    "message": "Payment Required",
    "data": {
      "httpStatus": 402,
      "challenges": [{
        "id": "ch_abc123",
        "realm": "search.example.com",
        "method": "tempo",
        "intent": "charge",
        "request": {
          "amount": "10",
          "currency": "usd",
          "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00"
        },
        "expires": "2025-01-15T12:05:00Z",
        "description": "Web search query"
      }]
    }
  }
}
~~~

**Step 3: Request with Payment**

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "web-search",
    "arguments": {"query": "MCP protocol"},
    "_meta": {
      "org.paymentauth/credential": {
        "challenge": {
          "id": "ch_abc123",
          "realm": "search.example.com",
          "method": "tempo",
          "intent": "charge",
          "request": {
            "amount": "10",
            "currency": "usd",
            "recipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fE00"
          },
          "expires": "2025-01-15T12:05:00Z"
        },
        "source": "0x1234567890abcdef...",
        "payload": {
          "signature": "0xabc123..."
        }
      }
    }
  }
}
~~~

**Step 4: Success with Receipt**

~~~json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{
      "type": "text",
      "text": "Search results for 'MCP protocol'..."
    }],
    "_meta": {
      "org.paymentauth/receipt": {
        "status": "success",
        "method": "tempo",
        "timestamp": "2025-01-15T12:00:15Z",
        "reference": "0xtx789...",
        "challengeId": "ch_abc123"
      }
    }
  }
}
~~~

# References to MCP Specification

- MCP Base Protocol: <https://modelcontextprotocol.io/specification/2025-11-25/basic>
- MCP Transports: <https://modelcontextprotocol.io/specification/2025-11-25/basic/transports>
- MCP Tools: <https://modelcontextprotocol.io/specification/2025-11-25/server/tools>
- MCP Resources: <https://modelcontextprotocol.io/specification/2025-11-25/server/resources>
- MCP `_meta` Field: <https://modelcontextprotocol.io/specification/2025-11-25/basic>
