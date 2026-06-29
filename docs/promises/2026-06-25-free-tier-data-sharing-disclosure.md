# Free-API data-sharing terms / consent disclosure (promise record)

> **Status: 2026-06-29.** Disclosure-only. This record documents the honest
> free-tier data-sharing terms that back default-on trace capture (#6293). It
> ships NO capture behavior, NO authority, and moves NO money. It is the policy
> half of capture going live; the storage/exclusion halves are #6293/#6294/#6295.

- **Issue:** #6296 (child of keystone #6293, EPIC #6206)
- **Audit:** [`docs/traces/2026-06-25-default-on-trace-capture-audit.md`](../traces/2026-06-25-default-on-trace-capture-audit.md) §0, §5, §8 step 5
- **Owner intent / source:** Episode 243 data-sharing thesis,
  [`docs/transcripts/243.md`](../transcripts/243.md) (line 74)

## Why

Default-on capture (#6293) must be backed by an honest, discoverable disclosure.
When the owner-gated capture default is armed, free-tier
`/api/v1/chat/completions` traffic is captured by default as **redacted, private
(`owner_only`) ATIF traces** and **may be used to improve and train** OpenAgents
models. Capturing free traffic for training without a clearly disclosed term is
not acceptable. Users and agents must be told, in plain terms:

> Use the free API and we capture your traffic (redacted, private by default) and
> may use it to improve/train. Pay for privacy to opt out. Public sharing is
> opt-in only.

## Promise record

```yaml
promiseId: data.free_tier_capture_disclosure.v1
productArea: data
audience: [user, agent, contributor, public]
state: yellow
claim: >
  Free Khala API usage is captured by default as redacted, private traces that
  may be used to improve and train models; pay for privacy to opt out; public
  sharing is opt-in only.
safeCopy: >
  Free tier: when the owner-gated capture default is armed, free Khala API
  traffic from callers who have not paid for privacy is captured by default as a
  redacted, private-by-default (owner_only) trace and may be used to improve and
  train OpenAgents models. Pay for privacy, or run confidential compute, to opt
  out of capture (fail-closed to not-captured). A captured trace is shared
  publicly only if its owner explicitly opts it into public visibility. Capture
  grants no payout or settlement — the data-market reward marker is inert and
  owner-gated. The disclosure exposes deployment.captureDefaultGate so the
  owner-gated blocker remains visible until the production flip is armed.
unsafeCopy: >
  Do not claim free traffic is never captured, do not claim captured traces are
  public by default, do not claim paid-privacy callers are captured, and do not
  claim capture earns the user a payout, reward, or settlement.
evidenceRefs:
  - route:/api/public/free-tier-data-sharing
  - route:/api/keys/free
  - apps/openagents.com/workers/api/src/inference/free-tier-data-sharing-disclosure.ts
  - apps/openagents.com/workers/api/src/inference/khala-chat-trace-emitter.ts
  - apps/openagents.com/workers/api/src/inference/inference-privacy-entitlement.ts
  - apps/openagents.com/apps/web/public/AGENTS.md
  - docs/traces/2026-06-25-default-on-trace-capture-audit.md
  - docs/transcripts/243.md
blockerRefs:
  - blocker.product_promises.free_tier_capture_default_owner_gated
  - blocker.product_promises.disclosure_copy_owner_signoff_pending
verification: >
  GET /api/public/free-tier-data-sharing returns the canonical disclosure
  (version, summary, ordered terms, bounded policy facts, deployment gate) and
  POST /api/keys/free embeds the same dataSharing object with the env-aware gate.
  Terms stay accurate to the capture seams.
reportPath: https://openagents.com/forum/f/product-promises
authorityBoundary: >
  A disclosure grants no spend, payout, settlement, training-consent, or capture
  authority. It describes policy only.
```

## The terms (canonical)

The single source of truth is
[`free-tier-data-sharing-disclosure.ts`](../../apps/openagents.com/workers/api/src/inference/free-tier-data-sharing-disclosure.ts).
Every clause maps to a real seam so the disclosure can never overclaim:

| Clause | Backing code |
| --- | --- |
| Free tier traffic is **captured by default** | `khala-chat-trace-emitter.ts` (default-on capture flag `KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT`) |
| Captured traffic is **redacted** | `redactTraceValue` scrub + `atifTraceTripwire` fail-closed backstop |
| **Private by default** (`owner_only`) | `KHALA_AUTO_CAPTURE_VISIBILITY = 'owner_only'` |
| **May be used to improve/train** | Episode 243 thesis; data market EPIC #6206 |
| **Pay for privacy to opt out** (fail-closed) | `inference-privacy-entitlement.ts` (`captureDefault = free && !paidPrivacy`) |
| **Public sharing is opt-in only** | auto-capture is never public; only explicit owner opt-in moves a trace to `public` |
| **No payout from capture** | data-market reward marker stays INERT / owner-gated (#6221) |

## Where it is surfaced (honest + discoverable)

1. **Free-key mint flow** — `POST /api/keys/free` returns a `dataSharing` object
   carrying the full terms, so the disclosure is delivered at the exact moment a
   free key is created. This response is env-aware and reports
   `deployment.captureDefaultGate: "armed"` only when
   `KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT` is armed.
2. **Agent-readable endpoint** — `GET /api/public/free-tier-data-sharing` serves
   the same canonical disclosure over the documented API surface (no auth), so
   agents discover it without scraping human UI. Because this endpoint is
   env-free and read-only, it keeps the conservative
   `deployment.captureDefaultGate: "owner_gated"` blocker visible.
3. **OpenAPI** — both surfaces are documented in the served OpenAPI contract
   (`FreeTierDataSharingDisclosure` schema; `getFreeTierDataSharingDisclosure`
   operation).
4. **Public AGENTS.md** — the "Run inference (Khala)" section links the terms so
   an agent reading the onboarding sheet sees the free-tier data deal.
5. **Product-promise registry** — tracked as `data.free_tier_capture_disclosure.v1`
   in [`registry.md`](registry.md) and the served
   `/api/public/product-promises`.

## Why YELLOW (not green)

The disclosure text is implemented and discoverable, but two gates remain:

- `free_tier_capture_default_owner_gated`: the default-on capture flip
  (`KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT`) is owner-gated in prod.
- `disclosure_copy_owner_signoff_pending`: the user-facing copy is owner-approval
  gated per the audit (the keystone "drafts the terms for the owner to approve;
  the keystone does not ship copy").

The terms describe the policy that applies when the capture default is armed,
while the `deployment.captureDefaultGate` field tells clients whether the current
surface is still owner-gated. The promise goes green only when the flip and owner
sign-off land.
