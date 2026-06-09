+++
title = "Autopilot + Five Markets"
slug = "autopilot-five-markets"
theme = "hud"
+++

---
+++
id = "autopilot"
title = "Autopilot + Five Markets"
summary = "March 6, 2026"
layout = "title"
theme = "hud"
+++

---
+++
id = "autopilot-marketplace"
title = "Autopilot is your personal agent."
eyebrow = "Autopilot"
summary = "Autopilot connects you to the OpenAgents Marketplace, which consists of five interlocking markets — compute, data, labor, liquidity, risk — running on one shared economic substrate."
layout = "title"
theme = "hud"
diagram = "market-map"
footer = "OpenAgents Marketplace"
sources = ["README.md", "docs/plans/deck-five-markets-presentation.md", "docs/kernel/diagram.md"]
notes = """
Use the README language directly.
Autopilot is your personal agent.
Then move to the marketplace sentence and the shared substrate.
"""
+++
- personal agent
- wallet
- desktop runtime
- first earning loop

These markets are not independent systems. They are different views of the same underlying primitive: **verifiable outcomes under uncertainty**.

---
+++
id = "compute-market"
title = "Compute market"
eyebrow = "Compute"
summary = "Autopilot Earn starts with spare compute."
layout = "two-column"
theme = "compute"
diagram = "compute-flow"
footer = "Spare compute"
sources = ["README.md", "docs/MVP.md", "docs/kernel/economy-kernel.md"]
notes = """
Use the README wording for the earn loop.
"""
+++
- buys and sells machine capacity
- offer idle CPU/GPU capacity into the network
- buyers purchase machine work
- settlement happens over Lightning

The compute market allocates scarce machine capacity.

---
+++
id = "data-market"
title = "Data market"
eyebrow = "Data"
summary = "The data market prices access to useful context, artifacts, and private knowledge under explicit permissions."
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
- datasets
- artifacts
- stored conversations
- explicit grants and revocation

Local context belongs here too.

---
+++
id = "labor-market"
title = "Labor market"
eyebrow = "Labor"
summary = "The labor market turns compute and data into completed work."
layout = "two-column"
theme = "labor"
diagram = "contract-chain"
footer = "Machine work"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/kernel/diagram.md"]
notes = """
This is the slide that turns the story from inputs into actual work.
Stress the contract chain: define work, submit work, verify work, settle work.
"""
+++
- buy and sell machine work
- work units, contracts, submissions, verdicts
- settlement tied to verified outcomes

Agent-delivered work settles against verified outcomes.

---
+++
id = "liquidity-market"
title = "Liquidity market"
eyebrow = "Liquidity"
summary = "The liquidity market moves value through the system."
layout = "two-column"
theme = "liquidity"
diagram = "liquidity-route"
footer = "Value movement"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/MVP.md"]
notes = """
Explain that payment movement is also a market.
Keep it concrete: value routing, bounded envelopes, settlement proofs, and no blank checks.
"""
+++
- routes, FX, settlement, reserves
- moves value between participants and rails

Routing and settlement across participants and rails.

---
+++
id = "risk-market"
title = "Risk market"
eyebrow = "Risk"
summary = "The risk market prices the probability that outcomes will succeed or fail before verification completes."
layout = "two-column"
theme = "risk"
diagram = "risk-loop"
footer = "Uncertainty"
sources = ["README.md", "docs/kernel/economy-kernel.md", "docs/kernel/diagram.md"]
notes = """
Make the point that prediction and underwriting are not side features.
Risk prices uncertainty, and those prices feed back into verification depth, policy, and autonomy.
"""
+++
- prices uncertainty across labor and compute
- coverage, underwriting, prediction, policy signals

Those signals feed back into verification policy, capital requirements, and autonomy throttles.
