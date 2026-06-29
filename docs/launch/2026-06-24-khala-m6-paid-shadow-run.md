# Khala M6 paid shadow run

Date: 2026-06-24

Issue: OpenAgentsInc/openagents#6014

## Result

The owner-armed M6 paid shadow run completed against the live OpenAgents
buy-mode eval endpoint and live Pylon NIP-90 providers.

- Schema: `psionic.khala_m6.paid_shadow_run.v1`
- Campaign: `buy_mode_campaign_khala_m6_issue6014_20260624160649`
- Worker deploy: `b821cab6-81fd-4947-816d-5268f9faeafe`
- Dispatch endpoint ref: `openagents.worker.operator_buy_mode_eval`
- Verdict class ref: `training.verification_classes.v1.exact_trace_replay`
- Spend authority ref: `openagents.buy_mode_campaign.daily_cap_msats`
- Run cap: 10,000 msats
- Run spend: 9,000 msats
- Learned lane: 3/3 verified, cost 3 sats, verified-work-per-sat 1.0
- Heuristic lane: 3/3 verified, cost 6 sats, verified-work-per-sat 0.5
- Decision: `promotion_eligible` / `promote_candidate`

The candidate remains a shadow candidate. Runtime promotion is still
approval-gated and does not happen automatically.

## Settlement Refs

Learned lane:

- `receipt.public.buy_mode.m6.50b9e65f129f0b16f3eceebe`
- `settlement.public.buy_mode.m6.28243b5ec404c27381c00fbf`
- `receipt.public.buy_mode.m6.57d6cfeb442a1460a917c18f`
- `settlement.public.buy_mode.m6.2ea92e2f9093bd8751c4832c`
- `receipt.public.buy_mode.m6.2276621c58367e594fac34ba`
- `settlement.public.buy_mode.m6.84b51fdd3bba5f1bfc394dc2`

Heuristic lane:

- `receipt.public.buy_mode.m6.6fee0502464ab026954920fd`
- `settlement.public.buy_mode.m6.00840ae39783b2484a0a237e`
- `receipt.public.buy_mode.m6.15f656359089febb3e119ba1`
- `settlement.public.buy_mode.m6.cc1905e459718b99e7bd55d9`
- `receipt.public.buy_mode.m6.e81a389a627ce4538e036d0f`
- `settlement.public.buy_mode.m6.be40b84484d9eb19fa405b88`

## Notes

The live closeout required three hardening fixes:

- the Worker NIP-42 publisher now allows a longer auth-challenge window;
- the Psionic HTTP dispatcher retries public-safe relay publish blockers with
  minute-scale backoff;
- the Pylon NIP-90 provider quotes the accepted bid amount while still
  enforcing its price floor, preserving learned-vs-heuristic cost separation.

No raw invoices, preimages, wallet mnemonics, bearer tokens, or private relay
payloads are recorded here.
