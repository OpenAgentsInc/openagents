# 21c. A guard asks a Pylon

[Chapter 21b](ch021b-a-decision-arrives-while-play-continues.md) built a local
request ticket. This chapter gives it a real game consumer: Bram, Everglade's
first gate guard. The first implementation is **shadow only**. It can ask a
Pylon for a harmless option and count whether that proposal passes the current
local gate. It does not display a line or change any game behavior.

The [delivery plan](../../docs/verse/pylon-guard-decisions.md) places this before
other game controllers. A courtesy line can fail quietly while we measure the
service. Walking, patrol, alarms, spells, wanted state, combat and rewards keep
their existing local rules. Belt is a later consumer.

## Three authored options

The fixed options are `quiet`, `greeting` and `weather`. The result proposes
an enum. It supplies no dialogue, coordinates, code, player identity or new
permission. The request contains a short authored statement about the currently
calm living guard, the nearby peaceful living player, and bounded time/weather
categories. It contains no conversation, private history or hidden information.

<!-- source: verse/everglade/guard-decision.coax request -->
```jai
ga_request :: (out:[]u8,o:GaObservation)->int {
  days:=[4]string.["night","morning","afternoon","evening"];weather:=[4]string.["clear","cloudy","rain","storm"];
  n:=json_raw(out,0,"{\"model\":\"elephant-1\",\"state\":\"A calm healthy town guard and a nearby peaceful living player. Time: ");
  n=json_raw(out,n,days[o.day]);n=json_raw(out,n,". Weather: ");n=json_raw(out,n,weather[o.weather]);
  return json_raw(out,n,".\",\"questions\":{\"ambient\":{\"type\":\"choice\",\"instructions\":\"Choose one harmless ambient action. Never affect movement, crime, combat or rewards.\",\"criteria\":{\"quiet\":\"Remain quiet\",\"greeting\":\"Offer an authored greeting\",\"weather\":\"Make an authored weather remark\"}}}}");
}
```

The caller uses the existing same-origin `/api/v1/systemone` edge and HttpOnly
session. A Verse player key is not sent as authentication. A guest's 401 and an
account's 429 disable guard attempts for that visit. A missing provider, bad
body or exhausted browser slot leaves ordinary play running.

Only the pinned Elephant checkpoint is accepted. The decoder requires the
exact model/service identity and `psionic-pylon-v1` service version, a unique `ambient` choice question and exactly
these three probability keys. Each probability and confidence independently
lies in `[0,1]`; the distribution sums to one within one part per million.
Choice and confidence must agree with the lexical maximum. Its numeric profile
is ordinary decimal notation with up to nine fractional digits, matching the
current service serializer. Other encodings refuse. Unknown additional fields
grant no authority. The maximum answer body is 8192 bytes.

This establishes a response contract, not model calibration or useful choices.
The provider/door fields are declarations carried by the authenticated edge;
matching a checkpoint string is not remote attestation of GPU execution.

## Observe after the game has stepped

The page computes eligibility at the end of the full game frame, after harm,
spells, weather and voice edits can change the world. Bram must be alive, calm,
present outdoors and within eight metres. The player must be alive and peaceful,
with no dialogue, map/editor or armed/casting spell. The page must be playable,
visible and focused, with no pending warp. Golden and clip views cannot dispatch.

A reply callback stages an enum. Only a later simulation boundary may consume
it against current context and eligibility. Merely receiving a valid HTTP200
cannot count an application in that callback.

<!-- source: verse/everglade/guard-decision.coax frame -->
```jai
ga_frame :: (s:[]GaSession,o:GaObservation,generation:int,revision:int,now_ms:int) {
  c:=GdContext.{epoch=s[0].epoch,actor=0,generation=generation,revision=revision*16+o.day*4+o.weather,policy=1,options=1};
  if s[0].ticket[0].phase!=0 && (s[0].mode!=1 || s[0].disabled || !ga_eligible(o) || s[0].suspended || !gd_same_context(c,s[0].context) || now_ms>=s[0].ticket[0].ticket.expires_ms) {ga_invalidate(s);c.epoch=s[0].epoch;}
  s[0].context=c;
  if s[0].ticket[0].phase==GD_READY {
    a:=gd_take(s[0].ticket[..],s[0].ticket[0].sequence,c,now_ms,7);
    if a.code==GdCode.ok {s[0].accepted+=1;s[0].last_action=a.action;}else {ga_invalidate(s);}
  }
  if s[0].mode!=1 || s[0].disabled || s[0].suspended || !ga_eligible(o) || s[0].ticket[0].phase!=0 || s[0].attempts>=6 || now_ms<s[0].due_ms {return;}
  b:=gd_begin(s[0].ticket[..],c,now_ms,GA_TIMEOUT);if b.code!=GdCode.ok {return;}
  s[0].attempts+=1;s[0].due_ms=now_ms+GA_CADENCE;s[0].body_n=ga_request(s[0].body[..],o);
  // All sixteen admitted category pairs fit this fixed buffer (checked below).
  s[0].send=true;
}
```

Semantic context binds the page epoch, Bram's identity/generation, the local
edit revision, time/weather categories, and the fixed policy/options version.
An ordinary frame or movement within the allowed proximity does not invalidate
a request. Blur, hidden/leave, a warp/hash change, dialogue entry, edits,
watch rebuild and respawn revoke it. A dead, hostile, missing or now-distant
guard fails current eligibility. The guard's automatic revival takes forty
seconds, beyond the eight-second request lease.

## Keep policy in a separate Coax library

[`guard-decision.coax`](../../verse/everglade/guard-decision.coax) is an
independently checked `#authority` library. The page adapter exports only
observations and local lifecycle metadata through
[`guard-types.coax`](../../verse/everglade/guard-types.coax). The C face forwards
that data to the policy, then forwards the policy's URL/body/flags/remaining
milliseconds to the generic HTTP effect. A fixed C-owned policy arena holds
state; C does not choose the action or admission rule.

This follows Everglade's existing separately linked Player, Grid and River
libraries. Loading the policy's authority directive into the entire world
would make a normal page build execute all imported world checks under the
compiler's small authority-test budget. Separate compilation keeps the policy
qualified without changing the compiler or dropping its checks.

The shadow policy can mutate only its own ticket/counters. There is no output
hook that applies its option to the world, dialogue or render state. Authored
ambient application is a separate, still-disabled milestone.

## Try and check the slice

A normal visit makes no guard request. `?guard-ai=shadow` explicitly enables
the probe. `?guard-ai=ambient` remains off. The first implementation permits one
pending request, at least sixty seconds between starts, and six attempts per
visit. The monotonic lease is eight seconds. A timeout bounds local acceptance;
it does not promise a remote answer within that time or cancel remote GPU work.

The browser's `coaxial.guardProbe(field)` exposes only scalar diagnostics:

| Field | Value |
| --- | --- |
| 0 | mode: 0 off, 1 shadow |
| 1 | attempted requests |
| 2 | ticket phase: 0 idle, 1 waiting, 2 staged |
| 3 | admitted shadow proposals |
| 4 | refused/revoked proposals |
| 5 | last admitted enum: 0 quiet, 1 greeting, 2 weather; -1 none |
| 6 | requests disabled for this visit |

Focused checks avoid real models and the unchanged world/crypto suites:

```shell
coax/bin/coax test verse/everglade/guard-decision.coax --library
coax/bin/coax mutate verse/everglade/guard-decision.coax --root-only --jobs 4
verse/test_guard_ai.sh
coax/web/wasm.sh apps/openagents-web/everglade.coax apps/openagents-web/everglade-face.c /tmp/everglade.wasm verse/player.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-decision.coax
EVERGLADE_GUARD_WASM=/tmp/everglade.wasm bun test coax/web/everglade-guard.test.ts
```

The browser harness uses the actual realm Wasm, host and assets with a local
synthetic API. It needs headless Chrome/WebGPU and stays muted. This proves
integration behavior without claiming a real provider or learned-model quality
result. The [verification record](../../docs/verification/pylon-guard-decisions-2026-10-30.md)
separates those checks from the remaining live-shadow and held-out evaluation
work. Useful real proposals and measured latency come before enabling lines.
