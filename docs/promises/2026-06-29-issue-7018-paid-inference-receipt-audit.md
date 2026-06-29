# Issue #7018 Paid Inference Receipt Audit

Issue: <https://github.com/OpenAgentsInc/openagents/issues/7018>

## Scope

This audit covers the two product promises named by issue #7018:

- `inference.gateway_credits_business.v1`
- `payments.autopilot_credits_purchase.v1`

The requested green outcome is a dereferenceable end-to-end paid receipt:
card or owner-approved production-equivalent payment -> credited balance ->
metered inference spend -> remaining balance / spend receipt.

## Current Finding

The source path is present, but the paid proof is not present in this checkout.
The relevant shipped machinery includes Stripe checkout creation, Stripe webhook
credit fulfillment into the USD billing ledger, explicit USD-credit bridging
into inference-spendable msat, receipt-first inference metering, and public
card-credit-spend receipt projection.

That is still not enough to green either promise. A green transition needs a
real or owner-approved staging-to-production paid receipt that can be
dereferenced without exposing raw prompts, provider payloads, payment secrets,
wallet material, local auth paths, or private customer data.

## Promise State

`inference.gateway_credits_business.v1` remains red. The gateway and bridge
machinery are not the blocker; the blocker is missing paid receipt evidence for
a card/MPP-funded balance spent through a real metered inference request.

`payments.autopilot_credits_purchase.v1` remains red. The purchase machinery is
wired, but production money collection is not proven without the configured
Stripe production secrets and a dereferenceable real or approved
staging-to-production card-to-credit receipt.

## Narrowed Blockers

- `blocker.product_promises.inference_paid_credits_card_to_credit_not_collectable`
- `blocker.product_promises.inference_paid_receipt_not_yet_supplied`
- `blocker.product_promises.inference_mpp_owner_activation_pending`
- `public_paid_model_gateway_missing`
- `blocker.product_promises.autopilot_credits_prod_stripe_secrets_missing`
- `blocker.product_promises.autopilot_credits_no_real_card_purchase_receipt`
- `blocker.product_promises.autopilot_credits_no_card_credit_spend_receipt`
- `blocker.product_promises.autopilot_credits_no_bitcoin_purchase_path`

## Closure Gate

Close the remaining promise gap only after the operator supplies a
public-safe receipt that links:

1. the card/payment or approved equivalent,
2. the credited ledger entry,
3. the explicit bridge into inference spend,
4. the metered inference usage debit,
5. the remaining balance or spend closeout.

Until then, public copy should say the paid loop is source-wired and
receipt-gated, not broadly live as a collectable paid inference business.
