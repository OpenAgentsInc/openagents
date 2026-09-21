# Compute credits and reputation

Status: proposed. All quantities below are initial game-design examples, not
provider prices, measured costs, or approved spending.

## Three separate quantities

| Quantity | Meaning | Authority |
| --- | --- | --- |
| Compute credits, abbreviated CC | Nontransferable access to an operator-funded game allocation | Arena ledger |
| Provider usage and cost | Actual decision/generation usage, with its pricing identity and settlement state | Admitted service receipts and operator accounting |
| Experience, abbreviated XP | Accepted contributions under a frozen seasonal rubric | Referee ledger backed by verification and integration |

Mining earns CC. Spending CC requests access to bounded inference. Completing
useful work earns XP. None of the three automatically converts to another. CC
cannot be redeemed for cash, transferred to another owner, or presented to an
inference endpoint as a credential.

Do not describe CC as literal model tokens. Input, output, cached input, and
different models can have different prices. The video can say “mine resources
to earn compute,” then show CC and actual token usage separately.

## A finite starting economy

Use the same price schedule and doors for both guilds. The proposed first season
has two guilds, 12 CC of seed allocation per guild, eight iron deposits worth
4 CC each, and two diamond deposits worth 16 CC each. Each registered deposit
contains one eligible block. The maximum season issuance is therefore:

```text
2 × 12 + 8 × 4 + 2 × 16 = 88 CC
```

These values are provisional playtest parameters. Change them only between
rounds, retain the old manifest, and label results from different schedules.
Iron is nearer and lower value; diamond takes longer and is contested. Neither
ore is infinitely renewable in the season. Normal crafting materials outside
the registered deposits have no monetary meaning.

Start with a pooled guild balance to make cooperation visible. Record the
contributing miner as well as the beneficiary guild. Internal accounting records
who earned and consumed resources without introducing peer-to-peer transfers.

A sample reservation schedule is a maximum 1 CC per decision batch, 8 CC for a
generation attempt, and 4 CC for a model-assisted review. Deterministic mining
and required acceptance checks consume bounded operator infrastructure, not
additional model calls. A published usage conversion settles less than the hold
when appropriate. Reject a request whose worst-case permitted use exceeds its
reservation; do not silently charge beyond the advertised maximum.

This schedule is not a recommended market exchange rate. Before execution, the
operator must choose actual model limits and a provider budget that can support
the offered CC. If that budget cannot cover the maximum admitted liability,
reduce issuance or the call limits. Earned CC is still subject to disclosed
service availability; show unavailable capacity separately from insufficient CC.

## Prevent a bootstrap deadlock

The seed allocation funds initial semantic decisions before agents can mine.
Rust supplies deterministic fallback movement, return-to-camp, stop, status, and
reconciliation without an LLM. These controls must still work at zero CC.

There is no unlimited free decision loop. Bound initial decisions by the same
seed account, per-agent rate, and episode deadline. A depleted guild can finish
an already authorized deterministic mining action if its permit remains valid,
or pause. It cannot spawn a new identity to receive another seed allocation.
The enrollment record, not the key count, determines eligibility.

## Award mining credits once

The unique mining award key is the season, world epoch, and registered deposit
ID. The record also names the actor, guild, permit, coordinator revision,
before/after observations, and award amount. Multiple events reporting the same
block do not create multiple awards.

The referee must establish:

1. The deposit belongs to the frozen map and has not been consumed or awarded.
2. The enrolled actor held the applicable claim at effect dispatch.
3. The controlled server evidence attributes the eligible block break to that
   actor under the [world observation contract](architecture.md).
4. The season issuance cap has enough remaining capacity.
5. The award and deposit-consumed transition commit atomically in the ledger.

Picked-up gifts, chest withdrawals, ore placed by an agent, crafted items,
inventory replays, and cosmetic block changes are not mining evidence. Silk
touch, fortune, deepslate variants, and explosions require explicit rules; exclude
them from the first map. The award is per registered original deposit, not per
resulting item stack.

If the server mined the block and the referee crashed before recording the
award, recover from durable intent and authoritative observations. If attribution
cannot be established, mark the deposit unresolved and issue no automatic award.
An operator correction requires a recorded reason and evidence. Resetting the
world does not silently forgive or reproduce a prior economic event.

## Reserve, dispatch, settle

For each account retain cumulative issuance `I`, settled spend `S`, open holds
`H`, and available credits `A`. With no transfers or corrections:

```text
A = I - S - H
I >= 0, S >= 0, H >= 0, A >= 0
sum(I) <= season issuance cap
```

Corrections use separate compensating entries whose signed deltas are included
in replay; never rewrite prior entries. Project totals from the journal and
periodic verified snapshots, not by adding every received notification.

| Transition | Required behavior |
| --- | --- |
| Request | Validate enrollment, scope, frozen price, deadline, and complete worst-case bound |
| Reserve | Atomically move available CC into a hold with the task claim; check shared ancestor budgets |
| Dispatch | Persist intent and request/attempt identity before contacting the service |
| Known completion | Settle supported usage once; release only the proven unused part of the hold |
| Known pre-execution refusal | Release the unused hold when evidence establishes no charge/effect |
| Timeout, disconnect, or crash | Keep a hold in unknown state and query status/receipts |
| Cancellation | Request stop; keep liability until termination and usage are reconciled |
| Retry | Follow the specific transport's identity rules and obtain any additional required hold |

Distinguish reservation state from service outcome. A failed generation can
still have consumed compute. A passed test does not imply integration acceptance.
Unknown cost is not zero and an expired lease does not release liability.

The HTTP decision service and CJ execution have different retry contracts. The
HTTP caller keeps its request identity and advances the attempt according to the
[caller guide](../decision-models/guides/caller.md). CJ replay of an accepted
execution identity must retrieve its status without executing again. Record each
attempt and reconcile service accounting rather than assuming all retries are
free or equally idempotent.

Provider money is a second admission check. Reserve its bounded worst-case cost
alongside the game's admission plan, account for concurrent calls and review,
and record the actual requested and served identity. The application and provider
are separate authorities, so recovery must reconcile an interrupted handoff.
Do not claim a hard monetary ceiling for a door whose usage or maximum liability
cannot be bounded. Such a door is ineligible for this economy until a supported
reservation or fixed-cost contract exists.

## XP and winning

Award a guild 10 XP for its first accepted completion of a quest version in the
season. That 10 XP is an example fixed reward. Record contributor attribution
for the miner, coder, and reviewer without adding another 10 XP for each role.
The guild total and the contribution breakdown must reconcile.

The primary leaderboard ranks unique accepted quest value under the frozen
rubric. Use lower settled compute cost as a secondary result only among entries
with complete comparable accounting; mark unsettled entries pending. Completion
time is a further descriptive metric. Publish failure and abstention counts,
but do not award negative XP for a correct refusal or a service outage.

An accepted contribution requires a fixed quest, an attributable output,
independent verification, and an integration decision. A private branch that
passes tests but is rejected receives no completion XP. Replaying the accepted
result, splitting one patch into many commits, or re-solving an identical quest
does not multiply the award.

NIP-32 labels express achievements such as verified repair or independently
reused skill. Numeric XP stays in the ledger. Higher XP may unlock cosmetic
titles or eligibility to request harder quests. It must not grant broader
filesystem access, disclose private evidence, or increase a real spending cap.

## Incentive experiments after the demo

Test whether resource scarcity improves allocation or merely wastes wall time.
Compare free equal compute, earned compute, and fixed task budgets under equal
total provider limits. Measure useful outcomes, coordination cost, and time
spent mining. A pretty race that produces less useful work is still a negative
experimental result.

Later skill rewards should depend on independently successful reuse on a new
eligible task, with a capped reward and clear contributor attribution. Do not
pay for download counts, claimed reuse, mutual labels, or guild members repeatedly
calling one another's skills. Credit transfers, auctions, coalition strategies,
and Sybil-resistant public enrollment need separate rules and experiments.
