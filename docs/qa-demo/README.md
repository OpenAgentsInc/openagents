# QA flow — verifiable Khala-driven QA artifact (2026-06-24)

`run-and-post.sh <PR>` runs a Khala-driven QA session, composes a polished video
(`apps/qa-runner compose`, ffmpeg), and posts it to the PR with **gh-attach**
(`projects/repos/gh-attach`) — the GitHub web-upload path the REST API lacks.
The manual/owner counterpart of the CI loop (#6185). Khala runs on own-infra at
$0 via the operator-credit exemption (#6180).

## The committed chain (all from ONE prod run)

Every file here is the output of a **single** Khala-driven run against
**production** `openagents.com/login` on 2026-06-24 — so the
distill → test → video chain is reviewer-verifiable, not asserted:

| File | Role in the chain |
|---|---|
| `session-trace.json` | the typed, public-safe **session** Khala produced (the steps it drove) |
| `result.json` | the **verdict**: `status: pass`, 5/5 steps (navigate /login · assert "Log in to OpenAgents" · assert URL · screenshot · done) |
| `distilled-login-verify.e2e.test.ts` | the committed **e2e test distilled from that session** |
| `khala-autonomous-qa.mp4` | the composed **video** of that same run (title card → recording → verdict card) |
| `title-card.png` / `verdict-card.png` | the compose-layer cards |

`result.json` also names the run's raw local Playwright artifacts
(`session.mp4`, `trace.zip`, `00-login-page.png`); the **public-safe**
`session-trace.json` and the composed `khala-autonomous-qa.mp4` are the
committed, shareable forms of those.

## Verify it yourself

```bash
cd apps/qa-runner
bun test generated/login-verify-2026-06-24.e2e.test.ts
# -> 1 pass  (runs the distilled test against production)
```

The distilled test is **green against prod** — that's the receipt. This is the
"verify an agent's work by reading the test + its output" thesis (#6192),
applied to our own artifact.

## Relation to behavior contracts

This chain is the receipt shape for the **stated-flow availability** category
of the behavior-contract catalog
(`docs/fable/2026-07-03-behavior-contracts-and-customer-invariants.md` §2.2):
a stated journey ("the login page loads and titles correctly on prod"),
driven end to end, distilled into a committed oracle test, with a
shareable video + trace as the deviation-loop evidence. A customer
engagement's contract oracles produce exactly this artifact chain per sweep;
the registry entry supplies the statement and the enforcement gate, and this
demo supplies the receipt format.
