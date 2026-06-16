# Business Slack Connect Intake

Date: 2026-06-16

Issue: #5095

## What landed

`/business` now posts to `POST /api/public/business-signup` instead of a
placeholder target. The form captures:

- business name
- work email
- optional website
- phone number
- requested work notes
- opt-in shared Slack channel checkbox

The endpoint stores each request in D1 table `business_signup_requests`. When a
lead checks the Slack option, the stored `slack_connect_status` is
`manual_invite_pending`; otherwise it is `not_requested`.

The JSON response is deliberately public-safe. It returns the request id, source
route, `requestedSlackChannel`, `slackConnectStatus`, `nextAction`, generated
timestamp, and authority boundary. It does not echo email, phone, website, or
freeform request text.

## Slack Connect boundary

This slice does not auto-create Slack channels for every signup. It only records
the opt-in and queues the operator handoff.

Slack Connect cannot be fully completed by OpenAgents alone:

- an operator must create or choose the Slack channel and send the invite
- the other Slack workspace must accept the invite
- acceptance can be tracked later by moving `slack_connect_status` to
  `invite_sent`, `accepted`, or `declined`

Until a Slack app token and acceptance webhook are added, the correct operator
workflow is:

```sql
SELECT id, business_name, contact_email, website, phone, help_with, created_at
FROM business_signup_requests
WHERE request_slack_channel = 1
  AND slack_connect_status = 'manual_invite_pending'
ORDER BY created_at ASC;
```

After the invite is manually sent:

```sql
UPDATE business_signup_requests
SET slack_connect_status = 'invite_sent',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE id = '<business_signup_id>';
```

## Authority boundary

The intake receipt grants no Slack, workspace, spend, payout, or agent authority.
Workspace creation remains operator-owned through the workspace API and invite
flow; Slack Connect completion remains external until the invited workspace
accepts.
