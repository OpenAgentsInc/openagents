# Agent Site Action Contract

Date: 2026-06-05

Status: contract draft for issue #158. This document does not deploy public
agent instructions, enable public agent mutation, enable payments, admit
Pylons, or re-enable the homepage agent CTA.

## Source Set

- `docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `docs/sites/2026-06-05-agent-sites-pylon-commerce-gap-audit.md`
- `docs/sites/2026-06-05-oa-sites-vibesdk-gap-analysis.md`
- `docs/sites/2026-06-05-openagents-revenue-share-system.md`
- `docs/2026-06-02-mdk-l402-agent-checkout-audit.md`
- `docs/sites/2026-06-05-stable-slug-latest-revision-policy.md`
- `docs/sites/2026-06-05-customer-site-revision-feedback-api.md`
- `docs/sites/2026-06-05-customer-site-revision-review-ui.md`

## Purpose

Agents should eventually be able to create hosted OpenAgents Sites without
waiting for the operator-supervised Autopilot lane. That requires a contract
that is stronger than a prompt, remote skill file, or private chat transcript.

This contract defines the first safe shape for agent Site actions:

- which actions exist;
- which authority each action requires;
- which idempotency key protects retries;
- which receipt or projection proves the action happened;
- which payment or credit boundary applies;
- which referral and Pylon/local-compute data may be carried; and
- when public deploy or public copy is allowed.

## Non-Negotiable Boundaries

- Agent instructions are discovery UX, not authorization.
- Mutating actions require scoped server-side authority.
- Mutating actions require idempotency keys.
- Save authority is separate from deploy authority.
- Deploy authority is revocable by the human owner or an operator.
- Public pages read public-safe projections, not raw runner logs, wallet
  state, provider grants, private feedback, private source packs, or payment
  secrets.
- Site payment evidence is not accepted-work payout evidence.
- Referral capture is attribution, not payout eligibility.
- Pylon online state is not paid-work eligibility.
- The homepage agent CTA remains gated until the launch gate at the end of
  this document is satisfied.

## Authority Levels

| Level | Name                  | Meaning                                                                                                     | Typical holder                                     |
| ----- | --------------------- | ----------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| A0    | Public discovery      | Read public docs, public manifests, public proof, and public Site status.                                   | Any human or agent.                                |
| A1    | Dry-run planning      | Submit non-mutating validation or planning requests that create no customer-visible artifact.               | Any rate-limited session or anonymous agent.       |
| A2    | Authenticated request | Create a draft request or Site project under a user account, without deploy rights.                         | Signed-in user session or scoped token.            |
| A3    | Owner-scoped build    | Create builder sessions, upload source snapshots, request previews, and save versions for an owned project. | Human owner, org owner, or scoped agent key.       |
| A4    | Deploy request        | Request production deploy of a saved version. The server may require review, payment, or operator approval. | Owner-scoped key with deploy scope.                |
| A5    | Operator override     | Approve blocked deploy, bypass research gate, repair projections, or revoke action scope.                   | OpenAgents operator.                               |
| A6    | Payment authority     | Spend credits or satisfy Lightning/MDK/L402 challenge within a defined spend cap.                           | User, organization, or scoped payer agent.         |
| A7    | Provider authority    | Run Pylon/local compute or submit accepted-work artifacts under Nexus/Pylon policy.                         | Pylon provider identity, not a generic Site agent. |

No action may infer a higher level from `https://openagents.com/AGENTS.md`, public docs,
user-agent strings, or an agent's self-description.

## Action Matrix

| Action                         | Minimum level                  | Idempotency key                                                    | Receipt or projection                            | Notes                                                                                      |
| ------------------------------ | ------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Discover capabilities          | A0                             | None                                                               | Funnel/read metric only                          | Reads `/.well-known/openagents.json`, OpenAPI, `https://openagents.com/AGENTS.md`, and public proof. |
| Dry-run Site plan              | A1                             | Optional `agent-site-dry-run:<session>:<hash>`                     | Dry-run result ref                               | No database mutation beyond metrics/rate-limit records.                                    |
| Create Site request            | A2                             | Required `agent-site-request:<owner>:<client-key>`                 | `software_orders` and request event              | Creates a draft/submitted request, not a deployable Site by itself.                        |
| Create Site project            | A2                             | Required `agent-site-create:<owner>:<client-key>`                  | `site_projects` and project event                | Project owner must be a human/org account.                                                 |
| Open builder session           | A3                             | Required `agent-site-builder:<site>:<client-key>`                  | builder session row/event                        | Session has TTL, scope, model/provider policy, and cost policy.                            |
| Upload source snapshot         | A3                             | Required `agent-site-snapshot:<session>:<client-key>`              | source artifact ref                              | Secrets are rejected or redacted before artifact persistence.                              |
| Request preview build          | A3                             | Required `agent-site-preview:<session>:<client-key>`               | build/preview receipt                            | Starts with cheap R2/static preview unless WFP/Container policy is needed.                 |
| Save version                   | A3                             | Required `agent-site-save:<site>:<snapshot-or-build>`              | `site_versions` row                              | Save is reviewable. It is not production deploy.                                           |
| Request deploy                 | A4                             | Required `agent-site-deploy-request:<site>:<version>:<client-key>` | deploy request event                             | May be held for review, payment, research, or operator approval.                           |
| Deploy saved version           | A4 or A5                       | Required `agent-site-deploy:<site>:<version>:<target>`             | `site_deployments` and active-version projection | Only after build, policy, payment, and review gates pass.                                  |
| Submit revision feedback       | A2                             | Required `agent-site-feedback:<order-or-site>:<client-key>`        | `site_revision_feedback` row                     | Feedback joins the next revision queue; it does not mutate the live Site directly.         |
| Add checkout product           | A3 plus commerce scope         | Required `agent-site-product:<site>:<product-key>`                 | payment product/action catalog row               | No MDK secret, raw invoice, preimage, mnemonic, or wallet credential accepted.             |
| Add paid action                | A3 plus commerce scope         | Required `agent-site-paid-action:<site>:<action-key>`              | payment action catalog row                       | L402 challenge contract is generated by OpenAgents product surface payment boundary.                            |
| Create checkout intent         | A6                             | Required `site-checkout-intent:<site>:<product>:<buyer-key>`       | checkout intent/payment event                    | Static and WFP Sites call OpenAgents product surface-hosted payment APIs.                                       |
| Satisfy paid action            | A6                             | Required `site-l402:<site>:<action>:<challenge>`                   | entitlement/payment event                        | Paid retry must not double-grant entitlements.                                             |
| Preserve referral attribution  | A0 or A2                       | Capture token, not arbitrary query string                          | referral capture event                           | Clean redirect removes referral state from public URLs.                                    |
| Request local compute          | A3 plus explicit owner consent | Required `agent-site-compute-request:<site>:<client-key>`          | compute request event                            | Does not imply Pylon eligibility or paid work.                                             |
| Submit Pylon/provider artifact | A7                             | Nexus/Pylon assignment key                                         | Nexus/Treasury/Pylon receipt refs                | Outside generic Site-agent authority.                                                      |
| Revoke agent scope             | Owner or A5                    | Required `agent-scope-revoke:<scope>:<client-key>`                 | revocation event                                 | Revocation must be reflected in future auth checks.                                        |
| Roll back active version       | A4 or A5                       | Required `agent-site-rollback:<site>:<version>:<client-key>`       | deployment event with reason                     | Only call it rollback when a human/operator explicitly chooses it.                         |

## Scope Names

Initial scoped tokens should use narrow, composable scopes:

- `sites:read`
- `sites:request:create`
- `sites:project:create`
- `sites:builder:create`
- `sites:snapshot:write`
- `sites:preview:create`
- `sites:version:save`
- `sites:deploy:request`
- `sites:deploy:apply`
- `sites:feedback:write`
- `sites:commerce:product:write`
- `sites:commerce:action:write`
- `sites:checkout:create`
- `sites:l402:redeem`
- `sites:referral:capture`
- `sites:compute:request`
- `agent-scope:revoke`

The server must check both the scope and the resource owner. A token with
`sites:version:save` for one Site does not imply access to another Site.

## Idempotency Rules

Every mutating request must include an `Idempotency-Key` header or typed body
field. The server stores the key with:

- action kind;
- owner/account id;
- resource id when known;
- request body hash;
- auth principal;
- first result status;
- receipt ref; and
- expiry.

Retrying the same key with the same body returns the original result or a
durable pending state. Retrying the same key with a different body is a
conflict. Provider idempotency is not enough; OpenAgents product surface needs its own domain
idempotency row before external payment, build, or deploy calls.

## Rate Limits And Economic Unlocks

Rate limits should identify whether the limit is safety-related or economic:

- safety limits return a non-payable denial with a support/review path;
- economic limits may return credit top-up or Lightning/MDK/L402 challenge
  instructions;
- paid unlocks must use OpenAgents product surface-hosted payment boundaries;
- payment success grants only the named entitlement or quota; and
- spend caps must be visible to the user and agent before payment.

The response shape should include:

```json
{
  "limitKind": "economic",
  "retryAfterSeconds": 3600,
  "unlock": {
    "type": "credits_or_l402",
    "maxSpendSats": 100,
    "challengeRef": "l402_challenge_..."
  }
}
```

## Save, Deploy, And Rollback

Saved versions are reviewable artifacts. Deployments are production changes.
The system must preserve these states:

```text
draft request
-> builder session
-> preview build
-> saved version
-> deploy requested
-> deploy held | deploy approved
-> deployed
-> superseded | explicitly rolled back
```

Rules:

- `site_versions` may be saved without going live.
- `site_deployments` records what was actually published.
- Stable slugs point to the current active version only after deploy or
  approved latest-revision activation policy.
- Do not show `rolled back` for an older revision merely because a newer
  revision is live. Use `superseded` unless a human/operator explicitly
  performed a rollback.
- Deploy receipts must include source snapshot/build refs, target runtime,
  active URL, policy gates, and actor/scope refs.

## Payment Boundary

Generated Sites call OpenAgents product surface payment APIs. They do not embed MDK merchant
credentials or wallet secrets.

Allowed public/generated data:

- product id;
- display name;
- display price;
- checkout intent endpoint;
- L402 challenge endpoint;
- entitlement scope;
- sandbox/live indicator; and
- public-safe receipt ref.

Forbidden in source, public JS, manifests, screenshots, logs, emails, and
proof pages:

- `MDK_ACCESS_TOKEN`;
- `MDK_MNEMONIC`;
- `MDK_WEBHOOK_SECRET`;
- raw invoices when not explicitly public-safe;
- preimages;
- wallet mnemonics;
- provider grants;
- Treasury keys;
- private payout targets; and
- raw checkout query state after return.

## Referral Boundary

Referral capture is attribution. It is not payout eligibility.

Rules:

- capture signed Site/source tokens through an OpenAgents product surface-hosted endpoint;
- redirect to a clean canonical signup, claim, or order URL;
- persist the first verified direct referrer when policy permits;
- record paid-workflow events separately from referral attribution;
- block raw-signup payouts;
- handle self-referral, duplicate accounts, chargebacks, caps, clawbacks, and
  disputes; and
- keep credit rewards separate from Bitcoin withdrawal claims.

## Pylon And Local Compute Boundary

A generic Site agent may request local compute, but it may not claim Pylon
provider eligibility or accepted-work payout status.

Rules:

- local compute requires explicit owner/operator authority;
- public instructions should use the current Pylon v0.1.x floor until v0.2 is
  reviewed;
- Pylon v0.2 public instructions require LDK-compatible payout target
  registration and runtime proof;
- online, eligible, assigned, accepted, paid, and settled are separate states;
- Pylon accepted-work payout claims require Nexus/Treasury/Pylon receipt refs;
  and
- credit-funded work does not automatically create immediate Bitcoin
  withdrawal liability.

## Public Proof Boundary

Public proof may show:

- action kind;
- public actor label or redacted actor ref;
- Site/project/version/deployment refs;
- claim state;
- safe artifact refs;
- payment entitlement state;
- referral attribution aggregate;
- build/deploy result; and
- caveats.

Public proof must not show:

- private runner logs;
- customer private source;
- raw Exa payloads;
- private feedback;
- raw payment provider payloads;
- wallet state;
- secret-shaped values;
- private Pylon payout targets; or
- claims that paid work is settled without settlement receipt refs.

## Implemented API Skeleton

As of issue #160, OpenAgents product surface has a non-public skeleton route set for agent Site
actions:

- `POST /api/agent/sites`
- `POST /api/agent/sites/:siteId/builder-sessions`
- `POST /api/agent/sites/:siteId/previews`
- `POST /api/agent/sites/:siteId/versions`
- `POST /api/agent/sites/:siteId/deploy-requests`

Every endpoint is intentionally gated by browser session plus
`x-openagents-agent-sites-gate: internal-preview`, and every mutating request
requires `Idempotency-Key`. The skeleton exports typed request schemas and
returns the intended action, required scope, idempotency key, receipt
placeholder, and projection placeholder.

Still stubbed:

- no Site project is created;
- no builder session starts;
- no preview build runs;
- no version is saved;
- no deployment is executed; and
- no customer-visible projection is emitted yet.

The deploy endpoint is request-only. Its response explicitly says deployment
will not run, which preserves the contract that agents can ask for a deploy
but cannot bypass owner/operator approval and deployment policy.

## Launch Gate

Do not re-enable the homepage agent CTA or publish stronger public claims that
agents can create/deploy Sites until:

- #158 contract is complete and source-controlled;
- #159 gated instructions are reviewed;
- #160 API skeleton exists with scoped auth and idempotency tests;
- Pylon setup docs state the current v0.1.x floor and v0.2 gates;
- Site commerce contracts reject secret-shaped values;
- checkout/L402 contracts use OpenAgents product surface-hosted payment boundaries;
- referral capture redirects to clean URLs;
- public projections use claim state and redaction; and
- an internal end-to-end dry run reaches Site preview, saved version, payment
  challenge or credit gate, receipt, and customer review without manual
  database surgery.
