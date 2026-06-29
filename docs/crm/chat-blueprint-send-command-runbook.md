# Chat → Blueprint `send_email{channel}` Command Runbook

The approval-gated path that lets chat (or an agent program) propose a CRM send
and an operator approve it before anything goes out (epic #5980, sub-issue
#5986). This is the CRM projection of the Blueprint `send_email` effect kind
(`blueprint-routes.ts`): **propose → approve → execute**, nothing sends without
an explicit approval.

## Flow

```
chat / agent ──propose──►  crm_contact_commands (kind 'send_email', pending_approval)
                                   │
                          operator approves  (desktop pane #5987 / approval UI)
                                   ▼
                     dispatchCrmSend({channel})  ──►  gmail_gws (queue) | resend (send)
                                   │
                          command status: applied | failed   (+ outcome in result_json)
```

## Endpoints (admin-gated)

```
POST /api/operator/crm/contacts/:id/commands/send-email
     { channel: 'gmail_gws'|'resend', templateSlug, sendReason?, proposedByRef?, tenant? }
     -> 201 { command }            (status 'proposed', approval_state 'pending_approval')

GET  /api/operator/crm/commands?status=proposed
     -> { commands: [...] }         (the approval queue)

POST /api/operator/crm/commands/:id/approve   { approvedByRef? }
     -> { result: { kind:'executed', outcome, command } }   (runs dispatchCrmSend)
        409 not_pending if already actioned, 404 if unknown

POST /api/operator/crm/commands/:id/reject    { reason? }
     -> { result }                 (status 'rejected', nothing sent)
```

## How chat proposes

The chat UI / agent calls `POST .../commands/send-email` with the contact, the
chosen `channel`, and a `templateSlug`. That records a Blueprint-style proposal;
it does **not** send. The operator reviews the queue (`GET .../commands`) and
approves or rejects. On approve, the unified `dispatchCrmSend` runs over the
chosen channel — Resend sends server-side; `gmail_gws` queues for the local
executor (#5987). The shared suppression/unsubscribe gate still applies at
execution, so a suppressed address yields a `failed` command (gate held).

The chat-frontend affordance is a thin caller of the propose endpoint; the
operator approval surface is the Autopilot Desktop CRM pane (#5987). The
server-side proposal/approval/execute contract above is the source of truth.
