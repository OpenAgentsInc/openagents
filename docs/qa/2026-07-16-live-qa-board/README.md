# Live QA board pixel proof — QA-4 #8909

- Base: `a38f449a61f1dec6de23eb338e077093484e1d02`
- Route: `http://127.0.0.1:3012/qa`
- Evidence endpoint: `GET /api/public/qa-board`
- Capture date: 2026-07-16

## Real evidence rendered

The local Start server loaded the repository's latest durable QA-2 observer
artifact and QA-1 six-lane run, then resolved the QA-1 finding against the live
GitHub issue ledger. The response reported:

- sources: observer `ok`, swarm `ok`, issues `ok`;
- observer: 7/7 passing at `2026-07-16T13:49:02.877Z`;
- swarm: `qa.six-lane.20260716T150054760Z`, six lanes, verdict `findings`;
- finding: high-severity public product-promise registry issue
  [#8912](https://github.com/OpenAgentsInc/openagents/issues/8912), state `open`.

This is not fixture data. The page fetched the same-origin server projection,
which imports the latest committed QA artifacts and enriches only their filed
issue references from GitHub. If either artifact store is absent or GitHub is
unreachable, its source state is `unavailable`; the board does not infer green.

## Captures

| View | Dimensions | SHA-256 |
| --- | --- | --- |
| [`qa-board-desktop.png`](./qa-board-desktop.png) | 1440 × 1625 | `11999e8f0a727eff23305bd7ecb95e93019d0aa536fe82a084a96999bee82bc1` |
| [`qa-board-mobile.png`](./qa-board-mobile.png) | 390 × 2789 | `c4c6600a7660a5e6d4eaea1414e53d441d7acdeb4f9ccb9b69a8adaec69a7703` |

The mobile capture was repeated after the first inspection found that a wide
table deferred check status behind horizontal scrolling. The retained mobile
proof uses compact check rows so state, drift severity, and duration remain
visible at 390 px.

## State and contract verification

`src/routes/-qa-board.test.tsx` covers:

- truthful server-rendered loading state;
- successful live observer/swarm/finding render;
- HTTP failure with exact Unavailable detail and no fake-green summary;
- empty findings with an explicit bounded explanation;
- response-schema rejection and freshness formatting.

`src/qa-board-projection.server.test.ts` covers artifact normalization, live
issue-state enrichment, GitHub degradation, GET-only routing, and cache policy.
