# Strict bug webhook transport

- Date: 2026-08-03
- Status: source implemented; production configuration and proof pending
- Owner: Omega All Work cutover (`OpenAgentsInc/omega#225`)

## Route

GitHub sends strict public bug deliveries to:

```text
POST https://openagents.com/v1/work/webhooks/github/strict-bugs
```

Configure the GitHub webhook for `issues` events only. The route accepts only
the `opened` action for `OpenAgentsInc/openagents` and
`OpenAgentsInc/omega`. It does not accept issue comments, edits, pull requests,
arbitrary repositories, or unsigned requests.

## Secrets and gateway

Mount these values from Google Secret Manager. Do not put values in source,
arguments, logs, receipts, or issue comments.

| Environment variable                 | Purpose                                                                       |
| ------------------------------------ | ----------------------------------------------------------------------------- |
| `STRICT_BUG_GITHUB_WEBHOOK_SECRET`   | Verify `X-Hub-Signature-256` over the exact raw body                          |
| `STRICT_BUG_CANDIDATE_INGRESS_URL`   | HTTPS JSON-frame gateway to the single owner-authority omega-effectd instance |
| `STRICT_BUG_CANDIDATE_INGRESS_TOKEN` | Authenticate the API to that exact gateway                                    |

The gateway URL must use HTTPS and cannot contain credentials or a fragment.
The adapter sends only `strict_bug.candidate.read` and
`strict_bug.candidate.execute` frames. It never forwards the raw webhook or the
GitHub secret.

## Activation checklist

1. Provision dedicated secrets in Google Secret Manager.
2. Deploy the authenticated gateway in front of the single owner-authority
   omega-effectd instance. Do not start a second candidate authority.
3. Mount the three environment values on the OpenAgents Cloud Run service.
4. Register the exact route for `issues` events in both admitted repositories.
5. Deliver the canonical signed strict-bug fixture twice. The first delivery
   must return `202`; the replay must return `200` with `idempotent: true` and
   no second execute effect.
6. Read the candidate through installed Omega. Verify `untrusted: true`,
   `disposition: pending`, matching source/delivery/evidence refs, and zero
   GitHub writes.
7. Exercise explicit reject, duplicate, admit, and link dispositions through
   Omega. A link is provenance only and grants no Work command authority.
8. Record public-safe deployment, delivery, candidate, and zero-write receipt
   refs on #225.

Missing secrets or gateway configuration must return `503`. Bad signatures
return `401`; invalid event, repository, delivery identity, or form shape
returns `400`; oversized bodies return `413`; a conflicting source identity
returns `409`.

This runbook does not activate `native_omega`. The writer cutover remains a
separate explicit and reversible command.
