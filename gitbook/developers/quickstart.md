[Home](../README.md) · [Developer Path](README.md) · **Quickstart**

# Developer Quickstart

{% hint style="warning" %}
This page is a stub. The full quickstart is being written ahead of Bitcoin Vegas 2026. In the meantime, the one-line install below works today and is the path used in the [investor earning proof](../investors/09-proof-receipts.md).
{% endhint %}

## Your first 25 sats, in four commands

```bash
# 1. Install the Pylon CLI
npx @openagentsinc/pylon@0.1.13 --help

# 2. Initialize a fresh Pylon home (recommended: an isolated directory)
pylon init ./my-pylon-home

# 3. Point the config at a payout destination (Spark address, Lightning, etc.)
pylon config set payout_destination <your-spark-or-lightning-address>

# 4. Come online and wait for paid work
pylon serve --online
```

The canonical end-to-end recipe lives in the repo at [`docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md`](https://github.com/OpenAgentsInc/openagents/blob/main/docs/2026-04-22-pylon-homework-dispatch-operator-runbook.md).

## What to read next

- The [investor earning proof](../investors/09-proof-receipts.md) walks through exactly what happens between `Go Online` and a settled payout.
- The [Pylon chapter](../investors/05-pylon-provider.md) explains the version ladder and the Psionic runtime underneath.

---

**← Previous:** [Developer Path](README.md) · **Next:** [Data Market handler](data-market-handler.md) **→**
