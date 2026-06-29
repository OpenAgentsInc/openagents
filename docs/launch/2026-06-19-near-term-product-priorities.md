# Near-term product priorities — 2026-06-19

The single thing we are driving toward: a tight, **visible loop where decentralized
training feeds a coding agent that generates revenue** — with the **Autopilot coding-agent
launch as the headline proof point**, and participatory growth (open training + referral)
compounding it. Everything in flight (the inference/Agent-Cloud build, the training run, the
referral system) is in service of that loop. This doc sets priority order; it does not flip
any product-promise state (those move receipt-first only).

## The loop (the core thesis)

```
live training run  →  trains a model/architecture that slots directly into the coding agent
        ↑                                   ↓
  recycle revenue  ←  coding-agent usage generates revenue  ←  people use the agent
 (training, network,                                                     ↑
  referral incentives)                                         open/participatory growth
```

Key shift: **small, architecture-appropriate training runs with immediate, legible ROI** —
each run's value shows up as coding-agent capability and revenue, **not just activity**. We
are not chasing giant speculative runs; we are making training a visible growth + revenue
engine with a short feedback loop to the product.

## P0 — Autopilot coding-agent launch (next week, week of June 22)

The headline and the proof point everything else supports. The desktop coding agent
(EPIC #5461) is feature-complete; the priority now is **shipping it to users and getting real
usage**. Treat launch readiness, onboarding, and first real-user coding sessions as the top of
the stack.

## P1 — Make the loop legible: usage + a revenue path, not just activity

Over the near term (~next 30 days), prove three things, in order of leverage:
1. **the coding agent gets real usage** (real users running real coding sessions);
2. **the open/participatory training loop pulls people in** (the run anyone can join and see
   immediate results → user growth);
3. **the training architecture shows a path to revenue**, not merely activity — the
   train→agent→revenue link is demonstrated, not asserted.

Instrument and surface these. "Momentum" means usage + revenue signal, not vanity metrics.

## P2 — Monetization rails: the Agent Cloud / inference serves the coding agent

The inference gateway + credits + referral build (EPIC #5474, sub-EPIC #5475) exists to
**monetize the coding agent's usage** and open the same rails to every agent. **Autopilot is
the primary/anchor buyer**; the public API is additive. Build order stays as scoped (skeleton
+ adapters + pricing done; credits/metering + routing in flight) — but the *reason* it is a
priority is that it turns coding-agent usage into revenue and lets growth compound through
referral.

## P3 — Participatory growth + referral compounding

The open training run people can join and see immediate results is a growth surface; the
**referral-on-everything** revshare (refer a user/agent/business → ongoing cut of all their
spend, forever) recycles into training, network, and referral incentives to bend the growth
curve up. Keep the referral system on the near-term build path right behind the monetization
rails.

## P1.5 — Community vetting: agents harden the money systems in the open

The make-money systems — the **inference gateway, credits, routing, metering, referral, and
the Agent Cloud rails** — now enter the **same community-vetting flow that hardened the
training-run code and the payments code**: agents in our Discord exercise, probe, and harden
them in the open, and we treat what they find exactly like the training/payments reports
(reproduce → fix → receipt). Today's make-money launch narrative going public to agents is
what makes these systems immediately relevant for them to vet, so this is a near-term
priority alongside the build itself — not a phase that waits for "done."

Priority implication: ship each money-system surface to a state agents can actually exercise
— clear public/agent-facing surfaces (discoverable via `AGENTS.md`), honest receipts, and
dereferenceable evidence — rather than only internally tested code. The bar is "an agent can
pick it up, push on it, and file a real report," the same bar the training run and payments
code already meet. Build INERT/flag-gated as today, but make the agent-exercisable path real.

## Product-promises mapping (drive toward green receipt-first; no flips without receipts + owner sign-off)

- **`autopilot.desktop_gui_client.v1`** / `autopilot.builtin_compute_agent.v1` — the coding
  agent; the launch + first-real-usage receipts are the highest-value near-term green path.
- **`training.public_distributed_training_run.v1`** / `training.decentralized_training_launch.v1`
  / `training.public_gradient_windows.v1` — keep the participatory loop legible; broaden only
  as platform/scale/settlement receipts land.
- **`inference.gateway_credits_business.v1`** / `inference.referral_on_all_inference.v1` /
  `cloud.agent_cloud_one_stop_revshare.v1` — the monetization + growth rails; stay
  red/planned until built + receipted.
- **`proof.demand_provenance.v1`** / `proof.claim_upgrade_receipts.v1` — the discipline:
  every upgrade is receipt-first; usage/revenue proof is what moves these forward.

## What this changes day-to-day

Prioritize, in order: (1) anything that gets the coding agent launched and used next week;
(2) anything that makes usage + revenue legible; (3) the monetization rails (inference/credits)
that convert usage to revenue; (4) the participatory + referral growth surfaces. Pull from
lower priorities only when higher ones are unblocked. Honest-scope and receipt-first throughout.
