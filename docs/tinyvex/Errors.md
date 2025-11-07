# Error Taxonomy — Tinyvex

Standardize errors across request responses and stream notifications. Align with JSON‑RPC codes where appropriate.

JSON‑RPC Error Codes (requests)

- -32700 Parse error — Invalid JSON
- -32600 Invalid Request — Invalid JSON‑RPC envelope
- -32601 Method not found — Unknown `tinyvex/*` method
- -32602 Invalid params — Schema/validation failure
- -32603 Internal error — Unhandled server error

Tinyvex Application Codes (use in `data` or `tinyvex/error`)

- TVX001 SubscriptionNotFound — `unsubscribe` for unknown `subId`
- TVX002 SubscriptionRejected — `subscribe` refused (e.g., malformed query)
- TVX003 MutationFailed — mutation failed with application error
- TVX004 ResumeFailed — journal expired or `lastSeq` too old
- TVX005 BackpressureDrop — intermediate transitions dropped; stream coalesced
- TVX006 Unauthorized — token missing/invalid
- TVX007 Timeout — heartbeat or request timeout

Shape Guidelines

- Request error response: { code: number, message: string, data?: any }
- Stream error notification (`tinyvex/error`): { subId?: string, code: string, message: string, data?: any }
- Include `requestId` or `subId` in `data` where available.

Observability

- Include `serverTs` in responses/notifications for clock skew debugging.
- Sample and redact large `data` fields in logs.

