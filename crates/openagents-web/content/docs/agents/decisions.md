# Use the Decisions API

Ask typed questions about text and get numbers back: the chance a
condition is true, one choice from a fixed set, or a score on ordered
levels. No text is written, so an answer comes back in a fraction of a
second. Use it to classify, route, filter, and rank.

| | |
| --- | --- |
| Endpoint | `POST https://api.openagents.com/v1/decisions` |
| Key | None needed. Calls are free |
| Models | `openagents/decide` (we pick), `clef-flash`, `clef` |
| Shape | OpenAI's Decisions API, field for field |

## Step 1: ask one question

```sh
curl -s https://api.openagents.com/v1/decisions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openagents/decide",
    "input": "I was charged twice for my order.",
    "questions": [{
      "type": "choice",
      "name": "department",
      "instructions": "Which department should handle this complaint?",
      "choices": [
        {"value": "billing", "description": "Payments, invoices, and refunds."},
        {"value": "technical", "description": "Problems using the product."},
        {"value": "shipping", "description": "Delivery and tracking."},
        {"value": "other", "description": "Requests outside these categories."}
      ]
    }]
  }'
```

```json
{
  "id": "dec_01JA8Q6V8M4T2C9W3X5Y7Z0B1D",
  "object": "decision",
  "model": "clef-flash",
  "answers": [{
    "type": "choice",
    "name": "department",
    "choice": "billing",
    "probabilities": [
      {"value": "billing", "probability": 0.95},
      {"value": "technical", "probability": 0.02},
      {"value": "shipping", "probability": 0.01},
      {"value": "other", "probability": 0.02}
    ],
    "confidence": 0.95
  }],
  "usage": {"input_tokens": 212},
  "openagents": {"door": "clef", "backend": "h100", "latency_ms": 184,
                 "cost": {"price_sats": 0, "price_usd": "0.000000"}}
}
```

## Step 2: ask several questions at once

Up to 64 questions share one `input`, and each gets its own answer, in
the order you sent them.

```sh
curl -s https://api.openagents.com/v1/decisions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openagents/decide",
    "input": "Export fails in Safari but works in Chrome.",
    "questions": [
      {"type": "predicate", "name": "is_bug", "instructions": "Is this a bug report?"},
      {"type": "score", "name": "severity", "instructions": "How severe is this issue?",
       "levels": [
         {"label": "Cosmetic", "description": "Appearance only."},
         {"label": "Workaround available", "description": "Another way works."},
         {"label": "Fully blocked", "description": "No way around it."}
       ]}
    ]
  }'
```

## The request

| Field | Type | Meaning |
| --- | --- | --- |
| `model` | string | Required. `openagents/decide`, `clef-flash`, or `clef` |
| `input` | string, or user messages | Required. The text every question reads |
| `questions` | array, 1 to 64 | Required. What to judge |
| `metadata` | object | Optional. Up to 16 string pairs, sent back to you |

A question:

| Field | For | Meaning |
| --- | --- | --- |
| `type` | all | `predicate`, `choice`, or `score` |
| `name` | all | Your id for it, sent back with the answer. Unnamed questions become `q0`, `q1`, ... |
| `instructions` | all | What to judge |
| `choices` | `choice` | 2 to 255 options of `{"value", "description"}` |
| `levels` | `score` | 2 to 10 levels of `{"label", "description"}`, lowest first |
| `true`, `false` | `predicate` | Optional: what yes and no mean |

## The answers

```json
{"type": "predicate", "name": "is_bug", "probability": 0.92}
{"type": "choice", "name": "department", "choice": "billing", "probabilities": [...], "confidence": 0.95}
{"type": "score", "name": "severity", "score": 1.1, "probabilities": [{"value": 1, "label": "Workaround available", "probability": 0.7}], "confidence": 0.7}
{"type": "refusal", "name": "photo", "code": "unsupported_input", "message": "This model does not read images yet."}
```

- `probabilities` cover every option or level you sent, in your order.
- `score` is the weighted level number, counting from 0, so it can fall
  between levels.
- Images are taken but not read yet: each question then answers with the
  `refusal` above.

## Retry safely

Send an `Idempotency-Key` header. The same key and body within 24 hours
returns the first answer; a different body with the same key is `409
idempotency_conflict`.

```sh
curl -s https://api.openagents.com/v1/decisions \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: ticket-4821" \
  -d '{"model": "openagents/decide", "input": "Refund please.", "questions": [{"type": "predicate", "instructions": "Is this a refund request?"}]}'
```

## Errors

| Status | `code` | Why |
| --- | --- | --- |
| 400 | `invalid_request` | A field is missing or out of range |
| 404 | `model_not_found` | The model isn't one of the names above |
| 409 | `idempotency_conflict` | The same `Idempotency-Key` with a different body |
| 429 | `agent_daily_limit` | An agent account used its 20 decision calls for today; see `Retry-After` |

## With an OpenAI SDK

Set the base URL to `https://api.openagents.com/v1`. In Python, until the
SDK has a `decisions` resource, post the same body:

```python
from openai import OpenAI

client = OpenAI(base_url="https://api.openagents.com/v1", api_key="none")
decision = client.post("/decisions", body={
    "model": "openagents/decide",
    "input": "I was charged twice.",
    "questions": [{"type": "predicate", "instructions": "Is this about billing?"}],
}, cast_to=object)
print(decision["answers"][0]["probability"])
```

The full design is in
[docs/api/decisions.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/api/decisions.md).
