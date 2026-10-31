# 31. Directions on request

[Chapter 30](ch030-measure-a-guard-decision.md) separated a valid answer from
a useful one. Its quiet/greeting/weather cohort gave the model no pressing
reason to speak. Here we start with a concrete, low-pressure need: a traveller
explicitly asks a guard for directions to a current destination.

The candidate is [`everglade-directions-v1`](../../verse/everglade/guard-guide.coax).
It chooses among five fixed authored tips: quiet, the commons quest board,
the East Gate, the North Trail and the West Gate. It cannot generate dialogue
or change movement, quests, crime, combat or rewards. Directions remains an
isolated candidate; this chapter adds no UI and enables no default or ambient
model behavior.

## Start with something the game knows

Everglade already has three authored journeys. The
[quest definitions](../../verse/everglade/quests.coax) send a traveller through
the East Gate toward the Deep Wood and waystone, along the North Trail to the
ruins, or through the West Gate to the river crossing. Each returns to the
quest board on the commons. The current HUD objective provides a bounded
destination category.

The [observation schema](../../verse/everglade/guard-guide-types.coax) carries
only that current goal and a few local facts: an explicit Directions request,
an eligible guard/player, a known available route, whether the traveller has
already arrived, and whether a guide tip is already visible or cooling down.
`hint_visible` means the guide tip, not the ordinary quest tracker.

The page must not send saved quest history, speech, account identity or precise
coordinates. No current objective means no remote request. In particular,
finishing every quest must not become an invented need to find work. The
`find_work` case requires its own explicit authored navigation intent; it is
not inferred from an empty tracker. A Directions interaction is separate from
`talk_to`, which opens character conversation and can advance a quest.

A route must be known available in the current world. For a first integration,
refusing directions whenever world edits exist is a reasonable bound until
reachability after edits is qualified. An existing landmark alone does not
prove a usable route to it.

## Five contexts, not an inflated dataset

The independently authored
[label manifest](../fixtures/guard-guide-labels-v1.jsonl) is
`guard-guide-editorial-v1`. Its split and labels were fixed before any new model
calls. The [five request artifacts](../fixtures/guard-guide-requests-v1/) record
the exact Coax-written bytes; their [hash inventory](../fixtures/guard-guide-requests-v1/SHA256SUMS)
keeps later prompt changes visible.

| Case | Current requested goal | Authored useful tip | Split |
| --- | --- | --- | --- |
| 0 | Find work | Quest board | Development |
| 1 | Waystone | East Gate | Development |
| 2 | Ruins | North Trail | Held out |
| 3 | River | West Gate | Held out |
| 4 | Return to board | Quest board | Development |

These are five unique requests. Adding day/weather combinations would not add
new information to this decision. Three development cases and two held-out
cases are a smoke check of this fixed task, not evidence of generalization,
calibration or production quality. The labels describe authored route utility,
not measured player outcomes. Neither split justifies automatic promotion.

Eight more cases are local refusals: no explicit help, ineligibility, an unknown
route, arrival, an existing guide tip, no current goal, or an invalid goal code.
They never generate remote requests and never become fabricated failed HTTP
observations.

## Put a strong local rule beside the model

`gg_baseline` maps the current goal to its canonical authored tip and stays
quiet when the need is invalid or already satisfied. It uses no network and
matches all five frozen route labels by construction. Always-quiet is another
baseline: safe and cheap, but unhelpful in these five requested contexts.

The remote model must be compared with both. More useful tips than quiet is
not an advantage over the perfect finite local rule. If the model merely
reproduces that rule after a network round trip, the measurement supports the
rule as a product option; it does not establish a need for the remote model.

Keep raw model choice, semantic admission and fallback separate. A response
can satisfy the wire contract while choosing the wrong gate. `gg_admit`
rejects that route for the current goal. A refusal retains quiet local behavior
in this candidate. If a later integration instead shows the strong local rule
immediately, record it as a local tip; do not credit that useful outcome to a
failed or rejected model answer.

## Compose meaning with ownership

The [runnable example](../examples/guard-directions.coax) uses a checked
synthetic East Gate response and authored timestamps. Its
[transcript](../fixtures/guard-directions.out) demonstrates one-time use,
replaced intent, changed objective, cooldown and exact expiry. No request is
sent and no game state is changed.

```shell
coax/bin/coax run psionic/examples/guard-directions.coax
```

<!-- source: psionic/examples/guard-directions.coax example -->
```jai
main :: () {
    need:=GgObservation.{help_requested=true,goal=2,destination_available=true,eligible=true};
    context:=GdContext.{epoch=1,actor=0,generation=1,revision=1,policy=2,options=2};
    pending:[1]GdState;
    first:=gd_begin(pending[..],context,100,GG_TIMEOUT);
    answer:=gg_decode(GG_RESPONSE);
    staged:=GdCode.stale;
    if answer.ok {staged=gd_offer(pending[..],first.ticket.sequence,101,answer.action);}
    print("synthetic contract: %\n",answer.ok);
    print("staged before frame: %\n",staged==GdCode.ok && pending[0].phase==GD_READY);

    // One frame: current semantic permission, then current ticket ownership.
    taken:=GdChoice.{code=GdCode.waiting};
    if gg_admit(need,answer,2).ok {
        taken=gd_take(pending[..],first.ticket.sequence,context,102,gg_legal(need));
    }
    if taken.code!=GdCode.ok {ignored:=gd_cancel(pending[..],first.ticket.sequence);}
    print("frame action: %\n",taken.action);
    print("authored line: %\n",gg_line(taken.action));
    print("duplicate applies: %\n",gd_take(pending[..],first.ticket.sequence,context,103,gg_legal(need)).code==GdCode.ok);

    old:=gd_begin(pending[..],context,200,GG_TIMEOUT);
    canceled:=gd_cancel(pending[..],old.ticket.sequence);
    context.revision+=1; // a NEW Directions intent, even for the same goal
    current:=gd_begin(pending[..],context,201,GG_TIMEOUT);
    print("old intent stages: %\n",gd_offer(pending[..],old.ticket.sequence,202,answer.action)==GdCode.ok);
    staged=gd_offer(pending[..],current.ticket.sequence,203,answer.action); // new callback, same synthetic bytes

    changed:=need;changed.goal=3;context.revision+=1; // the visible objective moved
    print("current objective admits old tip: %\n",gg_admit(changed,answer,3).ok);
    print("changed ticket consumes: %\n",gd_take(pending[..],current.ticket.sequence,context,204,gg_legal(changed)).code==GdCode.ok);
    canceled=gd_cancel(pending[..],current.ticket.sequence);

    changed.hint_visible=true;
    print("quiet after guide already shown: %\n",gg_admit(changed,GgAnswer.{ok=true,action=0},1).ok);
    changed=need;context.revision+=1; // a fresh eligible intent, now allowed to expire
    last:=gd_begin(pending[..],context,300,GG_TIMEOUT);
    print("at expiry stages: %\n",gd_offer(pending[..],last.ticket.sequence,8300,answer.action)==GdCode.ok);
    expired:=gd_expire(pending[..],8300);
    print("cheap reference action: %\n",gg_baseline(need));
}
```

The callback decodes and stages an enum; it does not display a line. At a
simulation boundary `gg_admit` checks current semantic permission and `gd_take`
checks current ticket ownership. `gg_admit` also refuses remote quiet when the
request itself has been revoked. Quiet is an abstention, not a way around
request eligibility.

In a live consumer, cancellation must reclaim the HTTP slot and suppress its
later callback, as [chapter 28](ch028-a-decision-arrives-while-play-continues.md)
requires. An ordinary frame tick must not change the semantic version, but each
new Directions intent must. Cancel and re-request for the same goal cannot
reuse the old intent's authority. Preserve the ticket sequence across resets.

## Before a page can consume it

A page integration still needs the explicit authored Directions interaction and
a current-frame observation after all gameplay mutations. Bind each intent,
actor generation, world/route version, current goal and guide-tip/cooldown
revision into the Gd context. Goal change, a new intent, actor replacement,
respawn, edits, backgrounding, leaving or an existing tip must revoke pending
authority. Recheck eligibility and route legality when consuming a staged
answer, with real monotonic milliseconds rather than frame time.

The consumer also needs bounded request capacity, transport cancellation,
exactly-once terminal accounting and fallback under authentication, quota,
overload, timeout and malformed-answer failures. Remote Directions calls remain
off by default and in golden/clip/replay. Ambient model behavior stays disabled.
Shadow qualification is a separate step from displaying a remote-selected line.

## A bounded live API result

The [live record](../../docs/verification/guard-directions-2026-10-30.md)
contains the five exact requests sent in development order 0/1/4, then held-out
order 2/3, without prompt or label changes. All five returned the expected
nonquiet tip and passed contract, deadline and frozen-context semantic admission:
3/3 development and 2/2 held out. No useful outcome was supplied by fallback.

| Case | Raw model tip | Browser API fetch ms |
| --- | --- | --- |
| 0 | Quest board | 600 |
| 1 | East Gate | 625 |
| 4 | Quest board | 652 |
| 2 | North Trail | 625 |
| 3 | West Gate | 611 |

The five requests used 236–238 tokens and fit the measured resident capacity of
512. They reached the normal public endpoint through an owned HttpOnly/Secure
browser session after the separate warm-fleet rollout. These API-only timings
exclude login, browser startup and the simulation-frame queue. They do not
claim a live Directions page consumer or actual player-visible admission.
The verification records name the exact fleet image and packaging history.

The strong local rule remains 5/5 with zero remote calls, versus 0/5 useful tips
for quiet. This candidate supplies meaningful directions compared with quiet;
it has shown no advantage over immediate local directions. The two held-out
cases remain a tiny finite smoke check. Neither their success nor confidence
values establish calibration, broader reliability or player benefit.

A sensible next page slice can show the local reference immediately and keep
any remote candidate in explicit shadow mode while qualifying current-frame
ownership, cancellation and edited-world refusal. Showing remote-selected tips
needs additional evidence on a task where the model can add value over a strong
local rule. Default and ambient model behavior remain disabled.
