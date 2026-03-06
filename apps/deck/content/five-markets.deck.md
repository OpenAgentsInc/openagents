+++
title = "Autopilot + Five Markets"
slug = "autopilot-five-markets"
theme = "hud"
+++

---
+++
id = "autopilot-wedge"
title = "Autopilot connects to five markets"
eyebrow = "01 / Product Wedge"
summary = "Autopilot is the personal agent users run. Under the product surface, it plugs into one machine-work economy with five interlocking markets on one shared kernel."
layout = "title"
theme = "hud"
diagram = "market-map"
footer = "Visible product wedge"
sources = ["README.md", "docs/plans/deck-five-markets-presentation.md", "docs/kernel/diagram.md"]
notes = """
Start from the product the audience can picture.
Autopilot is the wedge: personal agent, wallet, local runtime, first earning loop.
Then show that the five markets are not siloed products. They all terminate in one kernel.
"""
+++
- personal agent
- wallet
- local runtime
- first earning loop
- gateway into five markets on one kernel

Autopilot is the product surface. The markets underneath are different views of one shared machine-work economy.

---
+++
id = "compute-market"
title = "Compute market"
eyebrow = "02 / Compute"
summary = "The compute market allocates machine capacity. It is the first visible wedge because Autopilot Earn already turns spare CPU and GPU into paid work."
layout = "two-column"
theme = "compute"
diagram = "compute-flow"
footer = "Current MVP wedge"
sources = ["README.md", "docs/MVP.md", "docs/kernel/economy-kernel.md"]
notes = """
Tie directly to the MVP promise: go online, receive paid work, see sats land in the wallet.
Compute is the first user-visible market because it makes the earning loop legible immediately.
"""
+++
- buys and sells machine capacity
- current product wedge for `Autopilot Earn`
- prices delivery, uptime, and proof
- foundation for broader machine work

Compute is the first market people can feel: spare capacity turns into work, receipts, and bitcoin.

---
+++
id = "data-market"
title = "Data market"
eyebrow = "03 / Data"
summary = "Machine work needs context, not just compute. The data market prices useful context under explicit permission."
layout = "two-column"
theme = "data"
diagram = "access-grant"
footer = "Permissioned context"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/kernel/diagram.md"]
notes = """
Emphasize permissioned access rather than vague data marketplace language.
The important idea is controlled context: grants, revocation, and receipts.
"""
+++
- context, permissions, access
- datasets, artifacts, stored conversations, local context
- explicit grants and revocation
- makes machine work more useful and more controllable

Useful context should be rentable without losing control of the asset or the policy around it.

---
+++
id = "labor-market"
title = "Labor market"
eyebrow = "04 / Labor"
summary = "The labor market is where machine work is bought and sold. It consumes compute and data, then settles against verified outcomes."
layout = "two-column"
theme = "labor"
diagram = "contract-chain"
footer = "Software can hire software"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/kernel/diagram.md"]
notes = """
This is the slide that turns the story from inputs into actual work.
Stress the contract chain: define work, submit work, verify work, settle work.
"""
+++
- buy and sell machine work
- work units, contracts, submissions, verdicts
- settlement tied to verified outcomes
- software can hire software only if trust scales

The labor market is the operational layer where verifiable outcomes become payable work.

---
+++
id = "liquidity-market"
title = "Liquidity market"
eyebrow = "05 / Liquidity"
summary = "The liquidity market moves value between participants and rails: quotes, routing, FX, envelopes, and reserves."
layout = "two-column"
theme = "liquidity"
diagram = "liquidity-route"
footer = "Bounded money movement"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/MVP.md"]
notes = """
Explain that payment movement is also a market.
Keep it concrete: value routing, bounded envelopes, settlement proofs, and no blank checks.
"""
+++
- routes, FX, settlement, reserves
- moves value between participants and rails
- bounded envelopes instead of blank checks
- critical plumbing for every other market

Liquidity is the plumbing layer that makes machine work payable without hiding the risk.

---
+++
id = "risk-market"
title = "Risk market"
eyebrow = "06 / Risk"
summary = "The risk market prices uncertainty across labor and compute: coverage, underwriting, prediction, and policy signals that shape verification and autonomy."
layout = "two-column"
theme = "risk"
diagram = "risk-loop"
footer = "Priced uncertainty"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/kernel/diagram.md"]
notes = """
Make the point that prediction and underwriting are not side features.
Risk prices uncertainty, and those prices feed back into verification depth, policy, and autonomy.
"""
+++
- prices uncertainty across labor and compute
- coverage, underwriting, prediction, policy signals
- shapes verification depth and autonomy throttles
- turns unknowns into explicit priced signals

Risk is how the system stops pretending uncertainty does not exist and starts pricing it directly.
