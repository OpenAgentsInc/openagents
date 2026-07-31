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

## Provider-disconnect acceptance evidence

The bounded provider-disconnect drill may finish with exact accounting or with
durable uncertainty. Its operator route does not decide settlement. It stores
one directive for the exact active generation and admitted
`providerSessionRefDigest`; the worker acknowledges that same authority before
fencing the generation, draining the usage it has already observed, and closing
its zero-retry provider session.

If every terminal response and transcription event was durably accepted before
close, the normal worker close can mark provider accounting `exact`, charge only
that recorded usage, release the remainder of the hold, and settle. Verify that
path normally; do not submit a redundant reconciliation.

If a response, transcription, or terminal usage delivery cannot be proven
complete, the worker reports uncertain accounting. The control plane preserves
the full hold and records the `provider_disconnect` terminal boundary. Obtain
the complete provider usage export, bind its raw provider session identifier
privately to the stored SHA-256 provider-session digest and the same generation,
then submit the complete usage set through this reconciliation procedure. The
fault request ref and applied-event receipt prove which socket the drill
targeted, but neither proves that the partial recorded usage set is complete.

The drill must not create evidence by rotating the OpenAI key, editing a shared
Secret, blocking egress, or mutating a firewall, NetworkPolicy, LiveKit server,
or shared Deployment. Those actions can affect unrelated sessions and destroy
the one-generation accounting boundary. A fresh Sarah session is allowed only
after the drilled generation is terminal; the old provider connection has no
retry and must never be revived.

## Worker-loss evidence

The worker sends a generation-bound lease every five seconds. If no durable
worker event arrives for 30 seconds, the expiry sweep records
`worker_unavailable` and starts the normal 150-second drain deadline. A worker
that returns during the drain receives that stop reason and closes its provider
without reconnecting. If the worker remains unavailable, the sweep moves the
session to `accounting_uncertain` and preserves the full hold.

For live failure verification, repeat the authenticated settlement read with
`x-openagents-sarah-livekit-acceptance: live-observation-v1`. An uncertain
response then includes `failureEvidence`: the generation; digests of the worker
job, provider session, provider configuration, hold, and recorded usage set;
numeric recorded usage totals and terminal counts; provider and worker counts;
and provider admission and worker-close timestamps. It contains no raw provider
session identifier, transcript, audio, prompt, or tool content.

Use the provider-session digest and recorded usage-set digest to bind the
provider export to the same generation before submitting reconciliation. The
failure evidence proves what OpenAgents durably observed; it does not claim
that the partial set is complete and does not authorize settlement by itself.

## Legacy rate-authority quarantine

Migration `0117_sarah_voice_frozen_accounting_authority.sql` does not assign the
current price to a session admitted before rate freezing existed. It marks those
rows with `accounting_rate_authority = 'legacy_unresolved'`, expires any
unconsumed legacy admission, and prevents any connected legacy session from
settling. An expired reservation that never connected can safely release its
zero-charge hold. A connected session moves to `accounting_uncertain` and keeps
its full hold. A LiveKit binding also receives an `operator_stop` request so its
worker drains before that quarantine transition.

Do not copy the current configured rate into a legacy row and do not use the
normal reconciliation endpoint: neither proves the rate that the owner
accepted. Resolve these rows only through an audited incident procedure that
establishes the original admission terms and complete provider usage. If either
cannot be established, preserve the hold and escalate rather than fabricating a
charge or release.

```sql
SELECT session_ref, owner_actor_ref, transport_kind, state, reserved_msat,
       charged_msat, close_reason, accounting_rate_authority
FROM sarah_realtime_voice_sessions
WHERE accounting_rate_authority = 'legacy_unresolved'
ORDER BY created_at;
```

The expiry sweep isolates failures per session. A quarantined or corrupt legacy
row can therefore never prevent later admitted-rate sessions from expiring and
settling.

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

Explicit interruption is generation-bound and non-terminal. Migration
`0118_sarah_livekit_interrupt_control.sql` stores only an increasing interrupt
sequence and request timestamp. The API sends the same sequence to Sarah over a
reliable, HMAC-authenticated LiveKit server data packet; the periodic worker
lease returns it as a fallback. Replayed or stale sequences do not create a
second interruption, and interruption never changes settlement authority or
permits a provider reconnection.
