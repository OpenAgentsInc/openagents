# 21f. A guard gives directions

[Chapter 21e](ch021e-directions-on-request.md) defined a small Directions
contract and a strong local reference rule. Now a player can use that rule in
Everglade: approach Bram, press **G** or touch the **Directions** button, and
receive an authored navigation tip on the first eligible frame. The local game
provides the tip immediately. An optional Elephant request runs beside it as a
shadow observation.

This distinction matters. We already know the route from the current objective.
Waiting for a model would add latency and failure to a five-state task with a
perfect deterministic answer. The integration exercises real request ownership,
current-world checks and measurement while keeping useful help available.
Elephant's choice cannot replace the displayed line, advance a quest, open
conversation, move a guard, or affect combat, crime or rewards.

## Ask for a current need

Directions is a separate interaction from `talk_to`. It neither opens character
chat nor submits player speech. The
[page](../../apps/openagents-web/everglade-guide.coax) observes the single visible
followed objective and its current stage. The
[projection policy](../../verse/everglade/guard-guide-game.coax) maps those facts
to the existing authored routes:

| Visible objective | Current stage | Local tip |
| --- | --- | --- |
| Waystone journey | Outbound or continuing | East Gate toward the Deep Wood and waystone |
| Ruins journey | Outbound or continuing | North Trail toward the ruins |
| River journey | Outbound or continuing | West Gate toward the river crossing |
| Any of those journeys | Return to board | Commons quest board |
| Explicit Find Work request | No followed objective | Commons quest board |

An empty tracker is not a request. With no followed objective, the player may
choose the separately labeled **Find Work** interaction. Only that explicit
intent supplies the `find_work` goal. Saved quest arrays and completed histories
are not model context. The offer to ask for help is separate from permission to
send a request; it does not silently create a help intent.

The observation must describe a living, healthy, calm, visible nearby Bram and
a peaceful living player in foreground gameplay. Conversation, the quest board,
combat, editing, casting, the map and a pending warp block it. These are current
facts, checked after gameplay has advanced, rather than a promise made when the
button was drawn.

A route must also be known in the current world. Both the outer successful world
response and the page's document parser must agree that the world was read.
Unknown, malformed or failed loads refuse. This slice accepts only the canonical
unmodified route: placed edits, deleted/hidden objects, broken geometry and live
spell-created walls or stones all refuse directions. A nonzero document version
alone is not evidence of an edit, and seeing a destination object alone does not
prove a reachable route.

## Put the local line before the network

The [session policy](../../verse/everglade/guard-guide-session.coax) composes the
existing Gg semantic contract with the Gd request ticket. Its
[data-only contract](../../verse/everglade/guard-guide-session-types.coax) returns
a `GgsView` for drawing and a separate `GgsEffect` for HTTP. The displayed action
always comes from `gg_baseline`, never from a remote callback.

The [runnable example](../examples/guard-gives-directions.coax) uses authored
facts, a synthetic East Gate response and fixed monotonic timestamps. Its
[expected transcript](../fixtures/guard-gives-directions.out) starts with a local
line before any response is supplied:

<!-- source: psionic/examples/guard-gives-directions.coax local -->
```jai
    ggs_step(session[..],input,100);
    local:=ggs_view(session[..],100);
    print("local before reply: %\n",local.local_action);
    print("local line: %\n",gg_line(local.local_action));
    effect:=ggs_effect(session[..],100);
    print("shadow effect is bounded: %\n",effect.sequence==1 && effect.flags==5 && effect.timeout_ms==8000);
```

Creating an effect does not perform HTTP. The game face copies the bounded body
to the generic browser transport and reports whether dispatch started. A full
slot table, authentication failure, absent model or network error cannot delay
the local tip. Normal play and guests can use local Directions without enabling
any model request.

| Bound | Meaning |
| --- | --- |
| Four seconds | A newly shown local line remains visible |
| Six seconds | Cooldown before another local line |
| Eight seconds | Shadow lease; the exact deadline is already expired |
| Ten seconds | Minimum interval between shadow starts |
| Six starts | Maximum shadow attempts during one engine visit |

Each new explicit intent consumes one opportunity. If cooldown, cadence,
eligibility or capacity refuses it, there is no queued retry when conditions
later improve. A fresh local tip can succeed while shadow cadence refuses.
Leaving and returning does not reset the visit's remote budget.

## A callback stages; the current frame decides

The opt-in query is literal:

```shell
# Open the existing game with shadow measurement explicitly selected:
# /realms/everglade?guard-ai=directions-shadow
```

The request still needs an explicit help intent and a confirmed normal viewer
hint from the cookie-backed session reader. That hint is not authentication
proof. The real endpoint requires its ordinary HttpOnly cookie and normal
quotas; the browser does not hold a special model key. A 401 or 429 disables
further shadow attempts for the visit and keeps local help available. An owned
401/429 retains that classification even if it arrives after the lease; other
expired callbacks close as timeouts. No expired answer can be admitted.

The session validates the existing pinned checkpoint, service identity, complete
option distribution and current legal route through Gg. Arrival only stages a
bounded enum through Gd. It cannot modify a page view:

<!-- source: psionic/examples/guard-gives-directions.coax shadow -->
```jai
    ggs_sent(session[..],effect.sequence,true);
    ggs_receive(session[..],effect.sequence,110,response);
    print("callback stages without admission: %\n",ggs_probe(session[..],212)==GD_READY && ggs_probe(session[..],205)==0);
    input.observation.hint_visible=local.visible;input.hint_intent=local.local_intent;
    ggs_step(session[..],input,111);
    print("same-intent own tip admits shadow: %\n",ggs_probe(session[..],205)==1);
    print("display remains the local rule: %\n",ggs_view(session[..],111).local_action==2);
```

The immediate local tip needs an ownership label. Otherwise its newly visible
hint would make its own pending request ineligible. Only the same intent's own
local tip receives this exemption, including during its cooldown after visual
expiry. An external hint or a tip belonging to another intent still blocks
admission.

Every new intent gets a monotonically increasing serial, including a repeated
request for the same goal. The queued gesture captures its input epoch; a map,
world or lifecycle invalidation before its frame cannot mint it as a new intent
in the changed context. Epoch, actor/generation, world revision, current
objective revision, actual goal and intent all remain current before a staged
shadow can be counted. A replacement invalidates the old owner even when
cooldown refuses the replacement request:

<!-- source: psionic/examples/guard-gives-directions.coax replacement -->
```jai
    input.observation.hint_visible=false;input.intent=ggp_next_intent(input.intent);
    ggs_step(session[..],input,10100);next:=ggs_effect(session[..],10100);
    ggs_sent(session[..],next.sequence,true);
    input.hint_intent=ggs_view(session[..],10100).local_intent;input.observation.hint_visible=true;
    input.intent=ggp_next_intent(input.intent);ggs_step(session[..],input,10101);
    print("replacement during cooldown cancels old work: %\n",!ggs_pending(session[..],next.sequence) && ggs_probe(session[..],203)==2);
    ggs_receive(session[..],next.sequence,10102,response);
    print("old callback changes no admission: %\n",ggs_probe(session[..],205)==1 && ggs_probe(session[..],211)==1);
```

The bridge retains the HTTP slot together with its sequence and cancels it as
soon as the session no longer owns that request. Generic transport cancellation
also fences queued completions, slot reuse and engine replacement. Unknown and
duplicate callbacks cannot consume a new ticket. There is one terminal outcome
per started request. The example also stages a valid answer before a world edit
and supplies another exactly at expiry; neither becomes an admitted shadow or
a new displayed line.

## Finish gameplay, then draw the current view

The page's ordinary Coax execution remains independent of network effects.
The real Wasm face uses two phases within the same frame:

1. Coax advances the world and records current projection facts, deferring HUD
   output.
2. The C face passes those facts to Coax Ggp/Ggs, performs the returned HTTP and
   clock effects, and copies the resulting local view back.
3. Coax finishes the HUD from that current view before the frame returns.

C does not decide the route or interpret model probabilities. The
[aggregate policy](../../verse/everglade/guard-guide-policy.coax) loads old ambient
policy, Gg/Gd/Gm and the new projection/session once, so their shared exports have
one linked owner. The large page imports only data types; its pure VM checks do
not pull every authority policy into a game-rendering check.

Blur, hidden/background state, world or actor replacement, respawn, changed
objectives, map/conversation entry, edits and spell geometry revoke obsolete
work. Current-frame invalidation happens before the HUD is drawn. Golden shots,
clips and blocked/replay contexts cannot start shadow requests. The normal URL
keeps model behavior off; ambient model behavior is unchanged and disabled.

## Qualify the slice without overstating the evidence

Run this example in both modes:

```shell
coax/bin/coax run psionic/examples/guard-gives-directions.coax
coax/bin/coax run psionic/examples/guard-gives-directions.coax --release
```

Its transcript is synthetic and makes no model or game request. The focused
[session checks](../tests/guard-guide-session/README.md) cover ownership, deadlines,
all current context identities, own/external tips, visit limits and emitted C
ABI behavior. The [projection checks](../tests/guard-guide/GAME-PROJECTION.md)
cover the current game-fact mapping and strict world/viewer hints. The frozen
five-context real API evidence from chapter 21e remains a separate result; it
does not qualify this page consumer or establish a player benefit.

<!-- root: replace this paragraph with the exact actual-Wasm/browser record. -->
Actual-Wasm browser qualification is pending in this chapter. Its intended scope
is the real two-phase page with controlled synthetic client responses: local
help before a reply, safe stale/late handling and current gameplay invalidation.
It must identify its artifact and distinguish those checks from real neural
network calls. No new live model result or default model rollout is claimed here.

The next promotion question is whether shadow measurement reveals anything
worth the network cost beyond the perfect local rule. Matching that rule does
not answer it. Remote choice remains shadow-only until separate evidence
justifies a more useful task and an explicitly reviewed display policy.
