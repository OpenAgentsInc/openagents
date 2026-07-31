# Sarah LiveKit accounting reconciliation

Use this procedure only when a LiveKit voice session is in
`accounting_uncertain`. That state deliberately preserves the owner's credit
hold because the admitted OpenAI Realtime session may have remained billable
after the worker lost control delivery.

## Evidence boundary

Obtain the complete provider usage export for the provider session. Record only
opaque export, request, response, and transcription references plus numeric
token counts and response terminal statuses. Do not put transcripts, audio,
media URLs, prompts, tool arguments, or user content in the request, reason, or
evidence references.

The request must contain the complete usage set, including usage already
recorded before the failure. Existing rows must match exactly. The server
calculates each usage charge from the configured production rate using ceiling
division; an operator cannot supply a charge.

Send an authenticated `POST` to
`/api/operator/omega/sarah/voice/accounting/reconcile`:

```json
{
  "schema": "openagents.sarah.voice.accounting-reconciliation.v1",
  "reconciliationRef": "incident-1234-reconciliation-1",
  "sessionRef": "voice-session-ref",
  "generation": 1,
  "providerEvidenceRefs": ["openai-usage-export:opaque-export-ref"],
  "usage": [
    {
      "kind": "response",
      "providerResponseRef": "opaque-response-ref",
      "status": "completed",
      "inputTokens": 100,
      "outputTokens": 50,
      "cachedInputTokens": 10,
      "audioInputTokens": 80,
      "audioOutputTokens": 40
    },
    {
      "kind": "transcription",
      "providerTranscriptionRef": "opaque-transcription-ref",
      "inputTokens": 25,
      "outputTokens": 0,
      "cachedInputTokens": 0,
      "audioInputTokens": 25,
      "audioOutputTokens": 0
    }
  ],
  "reason": "Verified against provider usage export"
}
```

A successful transaction inserts the missing numeric usage, verifies the full
persisted set, marks provider accounting exact, debits the exact bounded charge,
releases the full hold, settles the session, and writes a durable reconciliation
receipt. The response includes both reconciliation and settlement receipt
references. Repeating the identical request is safe and returns
`"replayed": true`; changing any evidence under either the same reconciliation
reference or session returns a conflict.

Never guess missing usage or release the hold manually. If provider evidence is
incomplete, leave the session uncertain and escalate the incident.

## Verification

```sql
SELECT s.session_ref, s.state, s.reserved_msat, s.charged_msat,
       s.settlement_receipt_ref, b.provider_accounting_status,
       r.reconciliation_ref, r.reconciliation_receipt_ref,
       r.operator_actor_ref, r.provider_evidence_refs_json
FROM sarah_realtime_voice_sessions AS s
JOIN sarah_livekit_room_bindings AS b USING (session_ref)
LEFT JOIN sarah_livekit_accounting_reconciliations AS r USING (session_ref)
WHERE s.session_ref = '<session-ref>';
```

The session must be `settled` or `released`, provider accounting must be
`exact`, and both receipt references must be present. Verify the owner's
`held_msat` decreased by the full reservation and `balance_msat` decreased only
by `charged_msat`.

## Worker readiness

The API does not return LiveKit credentials until the dispatched worker has
durably claimed its generation. A binding remains under its provisioning lease
while readiness is `waiting`; worker claims retry for approximately 30 seconds.
If no claim arrives, the API settles the provisioning intent before broker
cleanup and returns unavailable. With `JRP_NEVER`, the deployment must not rely
on automatic redispatch: a missing explicit dispatch fails closed and is safe to
retry with the same idempotency key.
