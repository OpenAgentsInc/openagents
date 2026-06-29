# Episode 228 Launch Claim Ledger

Date: 2026-06-06

Status: implemented contract note for GitHub issue #312 / `OPENAGENTS-064`.

## Purpose

Episode 228 introduced Free Autopilot and the "Get Paid to Code" wedge. The
launch transcript contains several different kinds of claims: some are launch
facts, some are measured beta behavior, some are product intent, and some must
not be presented as proven public facts.

The ledger in `workers/api/src/episode-228-launch-claim-ledger.ts` keeps those
claims separate. It projects each claim through the existing public claim-state
contract so a launch page can show `planned`, `modeled`, `measured`,
`verified`, `settled`, `blocked`, or `prohibited` without reading private
workroom state.

## Current Classifications

| Claim | Public state | Reason |
| --- | --- | --- |
| Autopilot beta launch | `verified` | Backed by the public Episode 228 transcript and live blog route refs. |
| Limited free beta | `verified` | Backed by the public transcript and public blog route refs. |
| Public traces visible | `measured` | There are public proof/activity surfaces, but this is not a claim that every private workroom trace is public. |
| GitHub public-repo request flow | `measured` | The launch described the flow and OpenAgents product surface has route refs for GitHub auth and software-order submission. |
| Private-repo support | `planned` | The transcript frames this as intended future support, not a completed launch fact. |
| Revenue-share model | `modeled` | The transcript describes the economic model, but not a settled payout receipt. |
| Accepted-work payouts settled | `prohibited` | This must not be publicly claimed until accepted-work settlement receipts exist. |
| Best-coding-agent superlative | `prohibited` | This is launch rhetoric, not a public proof claim. |

## Redaction Rules

The ledger stores source and evidence references only. It rejects refs shaped
like raw runner payloads, provider grants, OAuth material, bearer tokens,
payment preimages, invoices, wallet state, customer identifiers, private keys,
or other private/operator-only state.

The tests in `workers/api/src/episode-228-launch-claim-ledger.test.ts` assert:

- the Episode 228 seeded ledger decodes through the Schema;
- verified, measured, modeled, planned, and prohibited claims project with the
  intended copy rules;
- public projections do not expose customer, team, or operator refs;
- unsupported verified claims are lowered to planned;
- private/source/payment/wallet/provider/customer refs fail closed.

## Future Use

`projectOpenAgentsLaunchClaimLedger` accepts an explicit ledger input and can
project future launch pages through the same state and copy-rule model. Future
launches should add only safe source refs and should not mark payout,
settlement, or payment claims as `settled` unless accepted-work settlement
evidence exists.
