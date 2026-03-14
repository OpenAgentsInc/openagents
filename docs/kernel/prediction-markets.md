# Risk Market

> Canonical market-status doc: [markets/risk-market.md](./markets/risk-market.md)
>
> This file remains the deeper background note for prediction, coverage, and
> underwriting mechanics inside the broader Risk Market.

This document describes the `Risk Market` companion surface of the OpenAgents Economy Kernel: prediction, coverage, and underwriting.

Prediction markets in OpenAgents are **not primarily speculative betting venues**.
They are used as **risk-pricing and verification-capacity instruments** that help the system determine how much confidence to place in agent outputs and infrastructure.

They function as a **market-based signal layer** sitting between verification and underwriting.

---

## Current implementation status

- `implemented`: starter authority flows now exist for coverage offers, coverage bindings, prediction positions, claims, claim resolution, and policy-bearing risk signals in `openagents-kernel-core` and `apps/nexus-control`
- `local prototype`: richer incidents, premiums, calibration, and risk-control modeling still live in docs and desktop-local kernel receipts or snapshots
- `planned`: underwriter accounts, broader market depth, claim payout productization, and full live policy integration

Prediction positions are economically important in the architecture because agents can explicitly take bounded PASS or FAIL positions on outcomes. That gives providers another possible earning lane for Bitcoin alongside compute, data, labor, and liquidity. It is one lane among several, not the whole product.

---

# 1. The Core Idea

Every unit of work in the system ultimately resolves to a question:

> **Did the outcome meet the contract?**

Prediction markets allow participants to **take positions on that outcome before the truth is known**.

Participants post collateral backing their belief that a result will:

* **PASS**
* **FAIL**

Those positions produce a **market-implied probability of failure**.

Example:

| Contract                      | Market probability of failure |
| ----------------------------- | ----------------------------- |
| AI code patch correctness     | 3%                            |
| Security analysis             | 22%                           |
| Long feedback ML training run | 40%                           |

This probability becomes a **machine-legible risk signal**.

---

# 2. What the Market Is Actually Pricing

Prediction markets in the kernel price **three things simultaneously**.

### Outcome risk

Will the work succeed or fail?

Example:

* code compiles and tests pass
* legal contract review missed clause
* ML training produced correct model

---

### Verification difficulty

Some work is **cheap to verify**, others are extremely expensive.

Prediction markets reveal where:

* verification is easy
* verification is uncertain
* verification capacity is scarce

---

### Liability cost

If an outcome fails, someone must pay.

Markets estimate **expected loss**.

Example:

```
expected loss = failure probability × claim payout
```

This directly informs:

* warranty pricing
* collateral requirements
* credit envelope limits

---

# 3. How Markets Plug Into the Kernel

Prediction markets interact with **four kernel modules**.

---

## Verification

Market signals influence verification requirements.

Example policy:

```
If implied_fail_probability > 15%
    raise required verification tier
```

This prevents cheap verification when risk is high.

---

## Liability underwriting

Markets help price warranties.

Example:

```
liability_premium = base_rate × implied_fail_probability
```

Underwriters earn premiums when markets predict correctly.

---

## Autonomy throttling

Market signals feed into autonomy controls.

If markets signal rising risk:

```
autonomy_mode → DEGRADED
```

This may:

* require human verification
* reduce envelope issuance
* disable warranties

---

## Observability

Market signals appear in `/stats`:

Example metrics:

* implied failure probability
* market calibration score
* underwriter diversity
* coverage concentration

These help operators evaluate **economic health of the system**.

---

# 4. Why Prediction / Coverage / Risk Markets Help Verification Scale

Verification is expensive and limited.

Prediction markets create **economic incentives for early detection of failure**.

Participants profit when they:

* identify weak work
* detect fraud
* detect verification blind spots
* predict failures earlier than others

This means:

```
market participants become distributed auditors
```

without needing centralized QA teams.

---

# 5. Why Markets Are Bounded

OpenAgents **does not allow unlimited speculation**.

Markets are restricted by policy:

* positions must be collateralized
* settlement must reference deterministic outcomes
* exposure is bounded
* correlation controls apply
* manipulation signals trigger breakers

Markets therefore act as:

> **risk signals and underwriting tools**, not casinos.

---

# 6. Relationship to Coverage Markets

In practice the system often uses **coverage markets first**.

Coverage markets are simpler:

Participants post collateral offering to **insure outcomes**.

Example:

```
Underwriter offers $100k coverage
Premium: 2%
```

Multiple offers form a **coverage binding**.

Prediction markets can then produce **secondary signals** like:

```
implied_fail_probability
confidence score
market calibration
```

Coverage markets provide **real liability**, while prediction markets provide **information signals**.

---

# 7. How This Affects the Economy

Prediction markets turn uncertain work into **quantified economic risk**.

This enables:

* automated underwriting
* dynamic verification policies
* compute market hedging
* capital-efficient warranties

Without prediction markets:

```
verification must be manual
risk pricing is guesswork
automation stalls
```

With them:

```
risk becomes a tradable signal
```

---

# 8. Simple Mental Model

Think of the system as three layers:

### Execution layer

Agents perform work.

---

### Verification layer

Checkers and adjudicators evaluate outputs.

---

### Market layer

Prediction markets answer:

> “How confident should we be in this result?”

The kernel combines all three signals:

```
verification evidence
market signals
policy rules
```

to determine whether money moves.

---

# The Key Insight

Prediction markets turn **uncertainty into a measurable economic signal**.

That signal lets the OpenAgents kernel:

* scale autonomy safely
* price risk dynamically
* allocate verification capacity efficiently

Without markets, verification bottlenecks remain manual.

With markets, **verification becomes economically scalable**.
