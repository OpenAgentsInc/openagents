# LG-8 referral/affiliate attribution receipt

Status: LG-8 source receipt for issue
[#8269](https://github.com/OpenAgentsInc/openagents/issues/8269), recorded
2026-07-04. This is attribution infrastructure only, not an affiliate payout
claim.

## What landed

- Operator-issued affiliate-code registry:
  `business_affiliate_codes` (`0298_business_affiliate_attribution.sql`).
- Attribution rows:
  `business_affiliate_attributions`, linking code -> business signup ->
  pipeline -> eventual payment receipt ref.
- Operator endpoints:
  `POST /api/operator/business/affiliate-codes` and
  `GET /api/operator/business/affiliate-attribution?code=...`.
- `/business` server-rendered form now preserves `?ref=` into a hidden
  `referralCode` field, matching the Foldkit business page.
- Business signup records `affiliate_<code>` source refs through the existing
  LG-6 source-attribution shape.
- Pipeline creation backfills `pipelineRef` onto the affiliate attribution row
  when the row was linked to a signup.

## Measurement Boundary

The conversion report is exact-only:

- intake leg: `business_signup:<id>` when an attributed signup row exists;
- pipeline leg: the linked `business_pipeline_rows.pipeline_ref`, otherwise
  `not_measured`;
- payment leg: an exact `business_checkout_kickoffs.public_receipt_ref`,
  otherwise `not_measured`.

No raw UTM, URL, prospect name, contact email, raw referrer identity, payment
payload, or payout destination is projected.

## Explicit Non-Claims

- No payout mechanics changed.
- No public affiliate signup exists.
- No earning copy was added.
- `referral.refer_once_earn_forever.v1` remains the red overclaim marker until
  a real settled payout receipt exists and the normal owner-signed green gate
  is satisfied.

## Verification Targets

- `apps/openagents.com/workers/api/src/business-affiliate-attribution.test.ts`
- `apps/openagents.com/workers/api/src/business-new-routes.test.ts`
- `apps/openagents.com/workers/api/src/business-signup-routes.test.ts`
- `apps/openagents.com/workers/api/src/business-pipeline-routes.test.ts`
