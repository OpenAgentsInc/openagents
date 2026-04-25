[Home](../README.md) · [Developer Path](README.md) · **Kernel integration**

# Economy Kernel integration

{% hint style="warning" %}
This page is a stub. The full kernel integration guide is coming. In the meantime, the Economy Kernel architecture is explained in [Investor Chapter 7](../investors/07-economy-kernel.md) and the authority model in [Chapter 8](../investors/08-authority-model.md).
{% endhint %}

## The `sv` control loop

```
WORK  ─▶  VERIFY  ─▶  RECEIPT  ─▶  POLICY  ─▶  THROTTLE  ─▶  WORK
```

Every loop iteration = one settled sat · one signed receipt · one policy delta. See [`assets/graphics/sv-control-loop.svg`](../assets/graphics/sv-control-loop.svg) for the full diagram.

## Authority boundaries — ADR-0001

**Spacetime is not an authority for money or verdicts.** The kernel is. The `TreasuryRouter` cuts and signs every accepted-work payout. The desktop can lag; the money is still correct.

Read the ADR directly at [`docs/adr/0001-authority-boundaries.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/0001-authority-boundaries.md).

## What to read next

- [TreasuryRouter glossary](../shared/glossary/treasury-router.md)
- [Investor Chapter 9 — Proof Receipts](../investors/09-proof-receipts.md) for a worked example of kernel-authored payout ids reconciling against wallet history.

---

**← Previous:** [Data Market handler](data-market-handler.md) · **Next:** [Bounties](bounties.md) **→**
