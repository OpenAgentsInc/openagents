# 28. A decision arrives while play continues

[Chapter 27](ch027-a-public-mesh.md) made a model reachable through the
site's account boundary. A game has a different boundary: an answer may be
valid for the request and still be wrong to apply now. The guard may have
fallen, the player may have left, or a newer decision may have replaced it.

This chapter builds the small shared consumer in
[`game/decision.coax`](../lib/game/decision.coax). It owns one outstanding
local ticket and no network, model or game state. Its answer is an admitted
enum or a refusal. The [Everglade plan](../../docs/verse/pylon-guard-decisions.md)
uses that gate for an optional authored courtesy line. Ordinary patrol and
combat continue when the model is absent.

Run the [owned lifecycle example](../examples/game-decision.coax):

```shell
coax/bin/coax run psionic/examples/game-decision.coax
```

<!-- source: psionic/examples/game-decision.coax example -->
```jai
main :: () {
    s:[1]GdState;
    context:=GdContext.{epoch=1,actor=0,generation=1,revision=1,policy=1,options=1};
    first:=gd_begin(s[..],context,100,8000);
    waiting:=gd_take(s[..],first.ticket.sequence,context,101,7);
    print("waiting for a reply: %\n",waiting.code==GdCode.waiting);
    offered:=gd_offer(s[..],first.ticket.sequence,102,1);
    accepted:=gd_take(s[..],first.ticket.sequence,context,103,7);
    print("accepted action: %\n",accepted.action);
    print("duplicate applies: %\n",gd_take(s[..],first.ticket.sequence,context,104,7).code==GdCode.ok);

    second:=gd_begin(s[..],context,200,8000);
    offered=gd_offer(s[..],second.ticket.sequence,201,2);
    alarm:=context;alarm.revision+=1;
    print("changed context applies: %\n",gd_take(s[..],second.ticket.sequence,alarm,202,7).code==GdCode.ok);
    revoked:=gd_cancel(s[..],second.ticket.sequence);
    print("old callback stages: %\n",gd_offer(s[..],first.ticket.sequence,203,1)==GdCode.ok);

    third:=gd_begin(s[..],alarm,300,50);
    print("at expiry stages: %\n",gd_offer(s[..],third.ticket.sequence,350,1)==GdCode.ok);
    print("expiry clears pending: %\n",gd_expire(s[..],350));
    print("local fallback needed: %\n",gd_take(s[..],third.ticket.sequence,alarm,351,7).action<0);
}
```

Its [checked transcript](../fixtures/game-decision.out) shows one application,
a refused duplicate, a semantic change and an expired reply. It uses synthetic
local timestamps and no model call.

## A reply is not a frame

An HTTP callback does not run the game action. It stages a decoded proposal.
The next simulation boundary checks the proposal against the world that is
actually present. Keeping those steps separate makes their ordering explicit:

1. Begin a ticket from the local decision context and monotonic deadline.
2. Start the asynchronous effect. Keep ordinary game behavior running.
3. Decode a matching response into a bounded local action enum.
4. Offer that enum to the pending ticket.
5. At a simulation boundary, recheck current context and legal actions, then
   consume the accepted proposal once.

A timeout, cancellation or relevant state change can end the ticket between
any two steps. A second response cannot apply a second action. A response
for an older request cannot overwrite a newer request.

## What stays the same while the world moves

The context names a session/world epoch, actor identity and generation,
semantic revision, policy identity and option-set identity. These are
caller-owned local versions, not values a model can use to grant itself
permission. A query sequence distinguishes repeated requests in one context.

Walking and rendering another frame do not automatically change these
versions. Otherwise every remote answer would arrive after its snapshot had
become unusable. The game advances a semantic version when something relevant
changes: a different order, a guard's death or replacement, an alarm, or a
world reset. It also checks live conditions such as proximity when applying.

For the first guard example, an ordinary clock tick is harmless. A guard
turning hostile is not. There is no reason for a late courtesy line to
survive that transition merely because its signature or JSON was valid.

## One ticket, bounded storage

<!-- source: psionic/lib/game/decision.coax ticket -->
```jai
GdContext :: struct {epoch:int;actor:int;generation:int;revision:int;policy:int;options:int;}
GdTicket :: struct {sequence:int;context:GdContext;expires_ms:int;}
GdState :: struct {sequence:int;phase:int;ticket:GdTicket;action:int;}
GdBegin :: struct {code:GdCode;ticket:GdTicket;}
GdChoice :: struct {code:GdCode;action:int=-1;}
```

The state has an idle, waiting or ready phase. Beginning while another ticket
is owned refuses. A successful begin increments the sequence and sets its
absolute monotonic expiry; cancel and consume do not reset that sequence.
Sequence exhaustion refuses instead of wrapping into an old request ID.

The action set is a bit mask over a bounded enum. The decoder determines what
the response proposes; the game supplies the actions currently legal. A model
probability does not expand that mask. An accepted action clears the active
phase. A refused take returns no action and preserves the owner so its caller
can explicitly cancel or expire it.

<!-- source: psionic/lib/game/decision.coax take -->
```jai
gd_take :: (s:[]GdState,sequence:int,current:GdContext,now_ms:int,legal_mask:int)->GdChoice #export_c {
    if s.count!=1 {return GdChoice.{code=GdCode.state};}
    if s[0].phase==0 || sequence!=s[0].ticket.sequence || !gd_same_context(current,s[0].ticket.context) {return GdChoice.{code=GdCode.stale};}
    if now_ms<0 || now_ms>GD_TIME_MAX || legal_mask<0 || legal_mask>2147483647 {return GdChoice.{code=GdCode.bounds};}
    if now_ms>=s[0].ticket.expires_ms {return GdChoice.{code=GdCode.expired};}
    if s[0].phase!=GD_READY {return GdChoice.{code=GdCode.waiting};}
    if (legal_mask&(1<<s[0].action))==0 {return GdChoice.{code=GdCode.illegal};}
    action:=s[0].action;s[0].phase=0;return GdChoice.{code=GdCode.ok,action=action};
}
```

Keeping state preservation separate from cancellation matters. A mismatching
old callback must not clear a newer pending ticket as a side effect of failing.

## The clock is an effect

A world clock can run faster, pause, or stop while a tab is hidden. It cannot
measure a network deadline. The browser supplies monotonic milliseconds from
its platform clock through a scalar effect. Coax chooses and checks the
deadline. The exact expiry is expired, not one last opportunity to apply.

The teaching gate bounds timeout to 30 seconds and clock values to exactly
representable nonnegative integer milliseconds. The first guard consumer
selects a shorter eight-second timeout. That is a refusal bound, not a promise
that a remote answer arrives in eight seconds or that a game should wait.

## A cancelled HTTP slot belongs to nobody

The generic browser host has a fixed shared set of response slots. Ignoring a
reply in Coax is insufficient if a hanging fetch continues to occupy a slot.
Reusing the integer slot without fencing its old asynchronous completion is
worse: an old fetch could complete into the new owner's callback.

The bounded HTTP effect therefore owns an abortable request record, a timer
and the identity of that record. It invalidates an owned record before abort
and checks the same identity after asynchronous response reads. Explicit
cancellation releases that slot and produces no later callback for it. A
timeout produces at most one terminal failure. Engine replacement drops effects
owned by the previous engine as well.

These are transport effects. The host does not decide that Bram should greet,
that an answer is calibrated or that a player deserves service. Coax keeps the
endpoint, body, timeout, admission, retry/cadence and local action mapping.

Browser cancellation also makes no claim that the remote Pylon stopped its
GPU work. Revoking local application authority and stopping remote execution
are different operations. End-to-end cancellation propagation needs separate
service evidence.

## The next consumer is deliberately small

The first Everglade adapter proposes quiet, a local authored greeting or
a local authored weather line in opt-in shadow mode. It records an admitted
enum without changing Bram’s behavior. Applying those lines comes after
actual shadow measurements and held-out decision-quality checks. A provider returns an option, not dialogue or
coordinates. The game verifies the exact question, model/checkpoint and bounded
distribution before offering an enum to this gate. It checks current guard and
player eligibility before consuming it.

Requests use the site's existing same-origin session. A Verse player ID alone
does not authenticate a SystemOne call. Guest play, expired sessions, exhausted
quota and unavailable providers leave the original game running. The first
integration is opt-in and per-browser cosmetic behavior; it is not a replicated
or authoritative NPC controller.

The eight transport slots are shared with other page effects. One realm-wide
guard request, low cadence and a local attempt cap protect that shared capacity.
They supplement the API's own account/global limits and never evade them by
creating new identities.

## What must be checked

Pure checks cover every context identity, sequence ownership, exact expiry,
legal-action mask, duplicate offer/take, cancellation, capacity and arithmetic
bounds. Native checks exercise emitted C in debug/release and under sanitizers.
Transport tests deliberately resolve a cancelled fetch after its slot has been
reused; that old result must never reach the successor. Malformed or oversized
successful HTTP bodies remain refusals.

The gate establishes local lifecycle and application ordering. Model quality,
remote availability, multi-player authority and useful guard choices require
their own evidence. The failure drill is simple: disconnect all Pylons and
continue playing. Ordinary guard behavior must remain correct throughout.
