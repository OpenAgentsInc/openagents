---
id: openagents.pricing
version: 5
kind: product
title: "Pricing"
summary: >-
  The chat and Coder on your own computer are free; the Pro plan ($20 a month
  for cloud environments, 100 machine-hours included) is decided but not on
  sale yet.
tags: [pricing, cost, free, plans, subscription]
applies_when: >-
  The user asks what OpenAgents costs, whether it's free, or about plans,
  the Pro plan, subscriptions, prices, paid tiers, cloud environment hours,
  extra hours, fair-use caps, per-message charges, or whether they need
  their own API key; not the Wallet's network fees, and not the chat's
  message limits alone.
answer: >-
  Chatting with us is free, and Coder on your own computer is free. Our Pro
  plan, coming soon and not on sale yet, is $20 a month for cloud
  environments: 100 machine-hours a month on a 2 vCPU, 8 GB machine, 2
  machines at once, and 20 GB of saved environments, with no time limit on a
  run. Unused hours don't roll over. Extra hours are $0.18 each from your
  credits, only if you turn them on in Settings, up to a monthly limit you
  set. Models run on your own Claude or Codex key or subscription.
status: admitted
author: openagents
provenance:
  written_from: [reference]
  cites:
    - docs/deployment/chat-worker.md
    - docs/roadmap/2026-09-29-launch-roadmap.md
    - docs/cloud/retail-environment-contract.md
evidence:
  - "2026-09-28: written from the cited documents and checked against them (#9923); the answer text awaits the owner's copy review."
  - "2026-10-01: the chat's message limits are gone (#10120); the answer says so."
  - "2026-10-02: the answer says outright that there is no plan, subscription, account, cap, quota, rate limit, throttle, or API key to bring for the chat, and that Coder's coding agents run under the user's own accounts and we bill nothing (#10135)."
  - "2026-10-02: BYOK (#10176) ships on computers: the answer says a person may bring their own OpenRouter, Vercel AI Gateway, or TypeSafe key, optionally, and that with it on nothing runs on ours."
  - "2026-10-09: the owner decided the Pro plan (#11006): $20 a month with 100 machine-hours of cloud environments on a 2 vCPU, 8 GB machine, 2 at once, 20 GB of saved environments (up to 10 versions), no run time limit, extra hours at $0.18 only when turned on and up to the person's cap, no rollover, models on the person's own key. Checkout isn't open yet, so the answer says it isn't on sale."
---

## Answer

Chatting with us is free, and Coder on your own computer is free. Our Pro plan, coming soon and not on sale yet, is $20 a month for cloud environments: 100 machine-hours a month on a 2 vCPU, 8 GB machine, 2 machines at once, and 20 GB of saved environments, with no time limit on a run. Unused hours don't roll over. Extra hours are $0.18 each from your credits, only if you turn them on in Settings, up to a monthly limit you set. Models run on your own Claude or Codex key or subscription.

## Details

- The Pro plan (#11006, `docs/cloud/retail-environment-contract.md`): $20 a month. It includes 100 machine-hours a month of cloud environment time on the standard machine (2 vCPU, 8 GB), 2 machines at once, and 20 GB of saved environment images (up to 10 saved versions). We set no time limit on a run; a limit you set yourself is respected.
- When the month's hours run out, setups stop until the next month unless you turn on extra hours in Settings. Extra hours cost $0.18 per machine-hour from your account's credits, up to the monthly limit you set. Extra hours are off until you turn them on.
- Unused hours don't roll over to the next month.
- Model usage isn't included: environments run the setup agent on your own Claude or Codex key or subscription.
- The plan isn't on sale yet: checkout opens when it is published.
- Wallet payments carry network and routing fees, shown on the confirm screen before you send.
- The chat needs no API key: the chat worker holds its own model keys, and the app holds none.
- Your own keys are optional (#10176): add them in the desktop app's Settings (Model providers), OpenAgents Terminal's `/settings`, or with `openagents settings provider-key set openrouter` (or `vercel`, `typesafe`). **Use my keys for everything** needs an OpenRouter or Vercel AI Gateway key; with it on, a call your keys can't make fails with one plain line and never falls back to ours. The phone doesn't take keys yet.

## Sources

- `docs/deployment/chat-worker.md`
- `docs/roadmap/2026-09-29-launch-roadmap.md`
- `docs/cloud/retail-environment-contract.md`
