# World-First Claims Qualified Evidence Bundle

- Date: 2026-06-29
- Issue: #7027
- Scope: the four world-first / largest-force product-promise records:
  `claims.world_first_ai_training_paid_bitcoin.v1`,
  `claims.world_first_public_llm_computer_training_run.v1`,
  `claims.pursued_world_first_largest_agentic_sales_force.v1`, and
  `claims.pursued_world_first_largest_sales_force.v1`.
- Result: no state flip. The two world-first records stay red and the two
  largest-force records stay planned until owner-signed receipt-first
  transitions exist.

This bundle is a copy-safety and transition-prep artifact. It does not create a
`promise_transition` receipt and does not authorize unqualified public wording.

## Evidence Map

| Promise | Current state | Evidence status | Remaining blocker |
| --- | --- | --- | --- |
| `claims.world_first_ai_training_paid_bitcoin.v1` | red | Qualified evidence pack exists in this document, backed by the prior-art review and bounded settlement receipts. | `blocker.product_promises.world_first_owner_signed_upgrade_missing` |
| `claims.world_first_public_llm_computer_training_run.v1` | red | Focused definition and evidence pack already exist. This bundle includes it for the unified #7027 review. | `blocker.product_promises.world_first_owner_signed_upgrade_missing` |
| `claims.pursued_world_first_largest_agentic_sales_force.v1` | planned | Not ready. This is an aspiration only; no sized verifiable agentic sales force exists. | `blocker.product_promises.world_first_agentic_sales_force_not_achieved`, `blocker.product_promises.world_first_agentic_sales_force_no_sized_verifiable_force`, `blocker.product_promises.world_first_owner_signed_upgrade_missing` |
| `claims.pursued_world_first_largest_sales_force.v1` | planned | Not ready. This is an aspiration only; no independently verified force has crossed the stated roughly seven-million selling-or-sell-equipped-agent bar. | `blocker.product_promises.world_first_largest_sales_force_not_achieved`, `blocker.product_promises.world_first_largest_sales_force_seven_million_bar_unmet`, `blocker.product_promises.world_first_owner_signed_upgrade_missing` |

## Claim 1: Bitcoin-Paid AI Training Compute

Allowed qualified wording:

> The first AI model training run that pays independent contributors in Bitcoin
> for replay-verified training compute on their own consumer devices.

Load-bearing qualifiers:

- Bitcoin, meaning sats / BTC rather than a project token, platform credit, or
  fiat GPU-rental payout.
- Training compute, not data labeling, data curation, model feedback,
  inference hosting, or generic AI work.
- Replay-verified work, not trust-me marketplace compute.
- Independent contributors' own consumer devices, not rented datacenter
  capacity.

Dereferenceable evidence:

- `docs/launch/2026-06-18-world-firsts-verification.md` records the adversarial
  prior-art review. It checks Spirit of Satoshi, Bittensor/Templar, Gensyn,
  Prime Intellect, Nous/Psyche, Salad, LightPhon, L402, Percepta, and Tracr.
  The review found no prior project satisfying the full conjunction above.
- `docs/launch/2026-06-18-evidence-pack.md` ties the launch narrative to
  public run, verification, settlement, and copy-safety evidence.
- `promise:training.decentralized_training_launch.v1` is the bounded green
  underlying run promise. It proves the public run and bounded real-settlement
  loop, not a world-first transition by itself.
- `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615`
  is the public run summary.
- `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements`
  is the public settlement list. The registry verification for
  `training.decentralized_training_launch.v1` names the counted real
  `realBitcoinMoved:true` settlement receipts and excludes simulation rows.
- `promise:proof.claim_upgrade_receipts.v1` is the transition authority. No
  owner-signed transition receipt exists for this world-first claim as of this
  bundle date.

Why the claim stays red:

The evidence supports only the qualified wording above. It does not itself sign
the product-promise transition. Green still requires an owner-signed
receipt-first upgrade that explicitly accepts the qualifier language and
evidence pack.

Claim 1 refuse-list:

- Bare "world first" with no qualifier language.
- "First to pay Bitcoin for AI."
- "First decentralized training run."
- "First AI compute paid in Bitcoin."
- Any wording that drops Bitcoin, training compute, replay verification, or own
  consumer devices.
- Any scale claim beyond the bounded settlement receipts.

## Claim 2: Public LLM-Computer Training Run

Allowed qualified wording:

> The first public, open-contributor LLM-computer training run - using the
> compiled-program-in-weights paradigm defined by Percepta, run as a bounded
> public network with paid and verified contribution.

Dereferenceable evidence:

- `docs/launch/2026-06-20-llm-computer-training-run-definition.md` defines the
  "training run" sense as executor-construction / exact-trace work rather than
  gradient-descent model training.
- `docs/launch/2026-06-20-world-first-llm-computer-evidence-pack.md` is the
  focused Claim-2 evidence pack.
- `docs/launch/2026-06-18-world-firsts-verification.md` records Percepta and
  Tracr prior-art boundaries and credits Percepta as the paradigm originator.
- `promise:compute.tassadar_executor_poc.v1`,
  `promise:models.tassadar_percepta_executor.v1`, and
  `promise:training.decentralized_training_launch.v1` are scoped underlying
  evidence records.

Why the claim stays red:

The evidence pack and definition are present, but there is still no owner-signed
receipt-first transition for the world-first claim. The underlying public paid
loop is bounded, and the claim must keep Percepta credit plus the
executor-construction qualifier.

Claim 2 refuse-list:

- Bare "world first" with no qualifier language.
- "We invented the LLM-computer."
- "First LLM-computer."
- Any wording that omits Percepta credit.
- Any wording that frames the run as gradient-descent model training.
- Any general LLM-computer capability, performance-parity, or served-product
  claim.

## Largest-Force Pursuits

The Episode 239 records are not evidence-ready world-first claims. They are
only labeled pursuits.

Allowed wording for `claims.pursued_world_first_largest_agentic_sales_force.v1`:

> OpenAgents is pursuing a largest-agentic-sales-force world-first target.

Allowed wording for `claims.pursued_world_first_largest_sales_force.v1`:

> OpenAgents is pursuing a largest-sales-force target, with roughly seven
> million selling-or-sell-equipped agents named as the aspirational bar.

Dereferenceable evidence:

- `docs/transcripts/239.md` is the source of the pursuit language.
- `docs/promises/2026-06-19-episode-239-lets-make-money-registry-reconciliation.md`
  records both as planned, aspirational records.

Dated blocker note:

As of 2026-06-29, there is no sized, independently verifiable agentic sales
force, no independently verified force above the stated roughly seven-million
bar, no independent record / prior-art review for an achieved force claim, and
no owner-signed receipt-first transition. Public copy must present these only as
pursuits or aspirations.

Largest-force refuse-list:

- "OpenAgents has the largest sales force."
- "OpenAgents has the largest agentic sales force."
- "OpenAgents holds the world record."
- "The seven-million-agent bar is met."
- Treating the cited Avon comparison as an OpenAgents-verified statistic.
- Any wording that implies a force exists at scale or is already selling.

## Owner-Signed Transition Request Shape

If the owner chooses to sign a future transition for either red world-first
record, the transition should name:

- the exact promise id;
- the exact allowed qualifier wording above;
- the prior-art review ref;
- the evidence pack ref;
- the underlying run / settlement / promise refs;
- the refuse-list that remains banned after transition;
- the explicit statement that no scale, generic "first AI", generic
  "decentralized training", or unbounded payout claim is created.

The largest-force records are not ready for such a request. They require real,
sized, independently-countable force evidence first.
