# Risk Market

The risk market is the lane that prices uncertainty across every other OpenAgents market. The other four markets — Compute, Data, Labor, Liquidity — cannot approach a clearing price on an open network until someone can underwrite the probability that a job fails, a handler misbehaves, or a payout has to be clawed back. The Risk Market is the name for that underwriting surface.

Today, no live risk-market layer ships. The [MVP doc](https://github.com/OpenAgentsInc/openagents/blob/main/docs/MVP.md) is honest about this: starter-grade paid-training work is subsidized by OpenAgents-hosted Nexus, which means OpenAgents itself absorbs almost all of the risk that a submitted contribution is invalid, that a worker artifact is incomplete, or that a Treasury reconciliation lags. That is tolerable at 25-sat / 6,400-cap scale. It is not tolerable at 20 GW.

The roadmap target is _kernel-signed risk receipts_: kind-`5960` requests carry a coverage pool addendum; settled payouts feed a pricing loop for the next job of the same shape; malfunctioning handlers can be priced out or priced in at a discount without breaking the whole marketplace. ADR-0001's [authority boundaries](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md) already specify that Spacetime is not authoritative for money or verdicts — exactly the separation that lets Risk be underwritten by multiple independent parties over time.

The Risk Market is the slowest to build and the most defensible once it exists — because a priced risk loop is what turns Compute, Data, Labor, and Liquidity from single-venue starter markets into open markets with many underwriters.

See also: [Chapter 2 — The Five Markets](../../investors/02-five-markets.md), [Chapter 7 — Economy Kernel](../../investors/07-economy-kernel.md), [Chapter 10 — Roadmap & Ask](../../investors/10-roadmap-and-ask.md).
