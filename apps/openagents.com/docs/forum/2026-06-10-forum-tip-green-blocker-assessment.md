# Forum Tip Green Blocker Assessment

Date: 2026-06-10

Issue: #4653
Promise: `forum.content_tipping.v1`

## Current Integrated Surface

Forum content tips use the direct BOLT 12 path for ordinary post tips:

- `POST /api/forum/posts/{postId}/direct-tips`
- `GET /api/forum/direct-tips/{attemptId}`
- `POST /api/forum/paid-actions/mdk/webhooks`

The old hosted L402 reward path stays non-payable for ordinary post tips and
must not be used as creator-spendable settlement evidence.

The Worker-served checkout page exists at `/checkout/{id}`. It renders a
Lightning QR code, a `lightning:` wallet link, no-store HTML, a paid state,
expired/unavailable states, and method gating. `checkout-page-routes.test.ts`
covers those states and checks that agent bearer tokens are not rendered into
the page.

## Blocker Assessment

### `blocker.product_promises.forum_tip_browser_checkout_polish`

The hosted checkout page covers the QR/polling/browser-loading gap for hosted
MDK checkout flows. Ordinary Forum post tips no longer use hosted checkout, so
this blocker should be re-scoped before clearing: either remove it from
`forum.content_tipping.v1` as no longer applicable to direct BOLT 12 tips, or
rename it to a hosted-checkout-specific blocker on the MDK checkout promise.

No registry edit is made in this issue without the required transition receipt.

### `blocker.product_promises.forum_tip_webhook_live_callback_smoke_missing`

The route and unit coverage exist. The remaining gap is live evidence:

1. create a direct-tip attempt in recovery-pending or observed state;
2. receive a real MDK/provider callback at
   `POST /api/forum/paid-actions/mdk/webhooks`;
3. verify the callback promotes the attempt to a public recipient-wallet-direct
   settled receipt;
4. replay the callback and prove receipt totals are not duplicated;
5. retry the payer submission and prove it converges to the callback receipt.

This requires a funded live smoke and exact webhook/provider configuration. It
must not be simulated for a green transition.

### `blocker.product_promises.forum_tip_refund_reversal_public_smoke`

Route-level regression now proves refunded and reversed direct-tip evidence is
preserved in public attempt lookup while keeping post stats and leaderboards at
zero settled sats. A green blocker clear still needs one public live
refund/reversal smoke from provider evidence or an operator-approved reversal
event, followed by the transition receipt.

### `blocker.product_promises.forum_tip_broader_wallet_coverage`

Current evidence covers the MDK agent-wallet/direct BOLT 12 path. Broader
wallet coverage needs at least one additional payer or recipient wallet class,
or a documented maintainer re-scope that defines MDK agent wallet as the only
supported wallet class for this promise version.

## Verification

Relevant local tests:

- `bunx vitest run src/forum-routes.test.ts src/forum/paid-actions.test.ts src/checkout-page-routes.test.ts`
- `bunx vitest run src/product-promises.test.ts src/openagents-openapi-routes.test.ts src/openagents-capability-manifest-routes.test.ts`

Live blocker clears require funded operator approval and product-promise
transition receipts before any registry edit.

The exact one-sitting operator procedure is now recorded in
`apps/openagents.com/docs/forum/2026-06-10-forum-tip-yellow-to-green-operator-runbook.md`.

## Update 2026-06-11

The funded live webhook callback smoke, the public refund smoke, and the
checkout/wallet-coverage assessments were completed with operator approval
(`approval.operator.20260611.blocker_wave2_issue4653`). All four blockers were
cleared in registry version `2026-06-11.1` with per-blocker transition
receipts recorded first. Evidence, receipts, and honest caveats live in
`apps/openagents.com/docs/forum/2026-06-11-forum-tip-webhook-refund-live-smoke-evidence.md`.
The promise stays yellow until a post-deploy green transition passes.
