# Product-Promise Next Green-Wave Audit

Date: 2026-06-29
Registry baseline: `2026-06-29.2`
Tracking issue: <https://github.com/OpenAgentsInc/openagents/issues/7014>

## Summary

The `2026-06-29.2` registry has 120 product-promise records. Excluding
`green` and `withdrawn`, 87 records remain non-green:

| State | Count |
| --- | ---: |
| `yellow` | 38 |
| `planned` | 35 |
| `red` | 14 |

Issue #7014 is the public next-wave tracker for moving those records through
receipt-first follow-up. This audit records the wave in-repo so the registry
does not rely only on an issue comment for the work queue.

## Child Wave

The initial child wave opened from #7014 is:

| Issue | Promise scope | Required next proof |
| --- | --- | --- |
| #7015 | `identity.orange_check_forum_signal.v1` | Live orange-check purchase settlement or honest yellow-without-blockers reconciliation. |
| #7016 | `metrics.khala_model_family_mix_public.v1` | Owner-signed live model-family mix projection receipt. |
| #7017 | `api.hosted_gemini.v1` | Production Hosted Gemini registered-agent receipt. |
| #7018 | `inference.gateway_credits_business.v1`, `payments.autopilot_credits_purchase.v1` | Card-to-credit-to-inference paid receipt. |
| #7019 | `data.*`, `privacy.*` | Khala free-tier capture disclosure and paid opt-out proof gates. |
| #7020 | Referral promises | First real purchase-to-Bitcoin-payout receipt. |
| #7021 | `autopilot_sites.partner_payout_ledger.v1` | First real partner payout receipt. |
| #7022 | `autopilot.local_apple_fm_tool_chat.v1` | Signed installer plus supervised Apple FM helper smoke. |
| #7023 | `autopilot.desktop_gui_client.v1`, `autopilot.builtin_compute_agent.v1` | From-DMG clean-Mac proof. |
| #7024 | Artanis responder and labor promises | Unattended receipt accrual gates. |
| #7025 | Business quick-win promises | First paid delivery receipts. |
| #7026 | `training.device_capability_dataset.v1` | Production thermal receipt plus cross-machine replication. |
| #7027 | World-first claims | Qualified evidence bundle or keep red. |
| #7028 | Repo studying promises | Privacy, self-serve, metering, pricing, and payout gates. |
| #7029 | `payments.money_dev_kit.v1` | MDK send-readiness capacity proof. |
| #7030 | Agent-world, payment, and growth visualizations | Default-on decision plus receipt. |

## Registry Effect

This audit flips no promise state, removes no blocker, and broadens no public
copy. Each child issue remains bound to the normal product-promise acceptance
discipline:

- move the scoped promise green only with dereferenceable evidence and an
  owner-signed `promise_transition`;
- narrow blocker refs only when the blocker was actually cleared; or
- leave a dated public-safe blocker note when an owner/product gate remains.

Counter movement, source-level scaffolding, and issue coverage are not green
evidence by themselves. Future registry changes from this wave must cite the
specific child issue, public-safe receipt, and deployed registry version.
