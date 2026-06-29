# Pylon Operator Views And Recovery

Status: implemented for `0.3.0-rc1` as a bounded OpenTUI and headless
inspection surface.

Pylon remains automated by default. The operator surface is for inspection,
support, and recovery, not for turning the earning node into a broad manual
labor mode.

Implemented surfaces:

- operate: desired mode, intake state, earnings state, recent job refs, market
  activity refs, receipt refs, and blocker refs;
- wallet: MDK readiness, network ref, known/unknown balance state, payout
  target refs, settlement refs, and wallet blocker refs;
- inspect: host inventory freshness, eligible inventory count, backend health
  refs, resource mode, and inventory blockers;
- recovery: headless command refs plus explicit operator opt-in, sandbox
  profile, budget, and no-wallet-secret evidence gates;
- transcript/log: bounded in-memory retention for the latest 1000 log entries,
  mouse wheel routing, page up/down, home/end, and line up/down key handling.

Headless support command:

```sh
pylon operator snapshot --json
```

The snapshot is public-safe and ref-oriented. It must not include raw wallet
material, mnemonics, provider tokens, private repo content, private topology,
raw local model paths, or environment dumps.
