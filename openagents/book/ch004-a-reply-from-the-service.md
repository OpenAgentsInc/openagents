# 4. A reply from the service

> **Status: partly implemented.** The effect outbox
> ([`apps/shared/app/core/effects.coax`](../../apps/shared/app/core/effects.coax)),
> `#authority` URL admission policy
> ([`apps/shared/app/core/url-policy.coax`](../../apps/shared/app/core/url-policy.coax)),
> controlled-clock streamed reply and Stop fence
> ([`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax)),
> and runnable example [`openagents/examples/service-reply-stop.coax`](../examples/service-reply-stop.coax)
> pass on `cpu` in debug and release. Remaining for full chapter completion:
> registering `openagents.chat.v1` as a `.proto` schema and qualifying the live
> native iOS/desktop HTTP streaming host.

## Premise

Chapter 3 draws your messages, and nothing answers. This chapter sends a
message to our service and draws the reply as it streams in. It adds
effects: typed requests the app asks the host to run, answers that come
back as actions, Stop, a fake service for tests and a clock the tests
control.

## Draft

An effect is a typed request with an operation ID, a declared capability
and a bounded payload
([Coax Native, "Services and native modules"](../../coax/native.md#services-and-native-modules)).
`composer.send` now commits two things together: your message in the state
and a `chat.send` request in the effect outbox. The host takes the request
from the outbox after the commit and runs it with its native HTTP service.
The answer comes back later as an ordinary action. A component never calls
the host; only the outbox does.

Taking a request from the outbox means the host accepted or refused it, not
that it succeeded. State says what an effect is doing: pending, done,
failed or unknown. Unknown is real: a send can reach the service and its
answer can be lost. An unknown outcome never retries on its own. The send
carries its operation ID, and the service answers a repeated ID with the
first result, so a deliberate retry stores the message once.

The reply streams. The service answers with parts, and each part arrives as
a `chat.part` action that appends to the reply in the session record and
marks the transcript dirty. While a reply comes, Send becomes Stop.

Stop is a cancel, and cancellation has one rule on every host. Pressing
Stop ends the pending turn at once. A part that arrives after it is
dropped, because the turn it belongs to is closed, and the transcript ends
with `Stopped`. A late answer can never change the screen of a turn you
stopped, a screen you left or a session you logged out of.

The requests the app makes follow two rules, the minimum of the protocol
that chapters 62 to 67 complete:

- **One schema.** A record that crosses the wire or is stored has one
  definition in [`proto/`](../../proto/README.md), a row in
  [`proto/registry.json`](../../proto/registry.json) and one codec file.
  `coax/web/tool.sh wire-check` fails a shape with neither
  ([how to register one](../../proto/README.md#register-a-wire-shape)).
- **The decision is Coax.** A route's rule on the service is an
  `#authority` file, `*-policy.coax`, checked with `coax mutate` and zero
  survivors. The C route moves facts in and the answer out
  ([COAX.md](../../coax/web/api/COAX.md)).

The API is versioned. An installed iPhone app can be weeks older than the
service, because every change ships in a store build. A route keeps
answering the app versions still installed, and a change to a record is a
new version, never an edit in place.

Tests never reach the real service. A fake service answers with recorded
parts, in a declared order, each held until the test releases it, so a run
is the same every time. The clock is controlled too: `coax drive clock
+MS` advances the app's time and `clock data release NAME` lets one held
answer through. The pinned test environment is seed 13096,
2026-01-01T00:00:00Z, en-US and UTC, the values the engine's agent pin uses
([`metaverse-engine/agent/pin.coax`](../../metaverse-engine/agent/pin.coax)).
The clock never fast-forwards a real network timeout into a success. A
check against the live service is a separate record that names its real
nondeterminism.

The reply in this chapter comes from whatever answers the chat route today.
Chapter 12 puts the first loop of our own behind it, and this chapter's case
still passes.

## What we build

A plan for what this chapter ships once the draft is approved:

- `chat.send` as an effect with pending, done, failed and unknown states, and
  its operation ID.
- The host's HTTP service on iOS, running requests from the outbox and
  delivering streamed parts as actions.
- Stop, with the late-part fence.
- A fake chat service with recorded parts and held answers, and the
  controlled clock.
- The chat record registered in `proto/`, with its codec, before the app
  sends it.

It builds on what exists today:

- [`coax/native.md`](../../coax/native.md)
- [`coax/EFFECTS.md`](../../coax/EFFECTS.md)
- [`coax/lib/agent/session/stream.coax`](../../coax/lib/agent/session/stream.coax)
- [`coax/lib/gateway/sse.coax`](../../coax/lib/gateway/sse.coax)
- [`coax/web/api/COAX.md`](../../coax/web/api/COAX.md)
- [`proto/README.md`](../../proto/README.md)
- [`docs/metaverse/agent-clock.md`](../../docs/metaverse/agent-clock.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A message and its reply | `openagents.chat.v1` `ConversationRequest`, `ConversationResult` ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1)) | HTTP stream | Spec sketch; no `.proto` file. Today's `/chat` routes write their own JSON ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), steps C5 and C9) |
| The session record the screen draws | Coder's transcript log, id `openagents.session.v1` on disk; planned package `openagents.session.log.v1alpha1` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C1) | Not sent | Planned. Written by hand in `coax/lib/agent/session/log.coax` |

### The chapter's check

- **Source:** [`apps/shared/app/core/effects.coax`](../../apps/shared/app/core/effects.coax),
  [`apps/shared/app/core/url-policy.coax`](../../apps/shared/app/core/url-policy.coax),
  [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax),
  and [`openagents/examples/service-reply-stop.coax`](../examples/service-reply-stop.coax).
- **Trace:** committed debug/release stdout in [`openagents/examples/service-reply-stop.out`](../examples/service-reply-stop.out):
  `fx_request` admits `POST /api/chat/submit/v2alpha1`, `FXO_UNKNOWN` transitions to
  `FX_UNKNOWN` with `queued=0` (never auto-retried), and a controlled-clock send streams
  three `chat.part` chunks plus `chat.done` into `you: hello` / `reply: Hello! This is a fixture reply.`.
- **Refusal:** URL policy rejects cross-origin (`https://evil/api/chat/x`) and privileged
  (`/api/wallet/spend`) paths with `0`; duplicate completions return `FXC_NONE`; `fx_cancel`
  advances the owner generation and drops late completions (`FXC_DROPPED`, `dropped_count=1`);
  `tap composer.stop` after one part drops every remaining `chat.part` and `chat.done` with
  `late part dropped` and appends `notice: Stopped`; and `clock data lose chat.send` followed
  by retry deduplicates on the same operation ID (`op-3 already stored; first result`).
- **Independent check:** `coax/bin/coax test apps/shared/app/core/effects.coax --library`,
  `coax/bin/coax test apps/shared/app/core/url-policy.coax --library`,
  `coax/bin/coax test apps/shared/openagents-app.coax`, and the session record's checks
  (`coax/bin/coax test coax/lib/agent/session/session.coax`).

## Source listing & runnable example

Run [`openagents/examples/service-reply-stop.coax`](../examples/service-reply-stop.coax) (committed transcript [`openagents/examples/service-reply-stop.out`](../examples/service-reply-stop.out)):

<!-- source: openagents/examples/service-reply-stop.coax outbox-and-stream-stop -->
```jai
main :: () {
  // Part 1: Effect outbox rules (apps/shared/app/core/effects.coax).
  o: [1] FxOutbox;
  fx_init(o[..]);
  // Refusal: policy rejects cross-origin and non-chat privileged paths.
  bad_url := fx_request(o[..], 0, FX_HTTP, 1, UP_POST, "https://evil/api/chat/x", "", "{\"m\":\"hi\"}", 4096, 5000);
  bad_wallet := fx_request(o[..], 0, FX_HTTP, 1, UP_POST, "/api/wallet/spend", "", "{\"sats\":10}", 4096, 5000);
  print("outbox_policy_refuse bad_url=% bad_wallet=%\n", bad_url, bad_wallet);

  // Admitted POST -> pending -> unknown outcome (never auto-retried) -> duplicate completion ignored.
  op1 := fx_request(o[..], 0, FX_HTTP, 1, UP_POST, "/api/chat/submit/v2alpha1", "", "{\"text\":\"hello\"}", 4096, 10000);
  gen1 := fx_generation(o[..], 0);
  taken1 := fx_take(o[..], op1);
  c1 := fx_complete(o[..], op1, gen1, FXO_UNKNOWN);
  dup1 := fx_complete(o[..], op1, gen1, FXO_DONE);
  print("outbox_unknown op=% taken=% deliver=% state=% queued=% dup=%\n",
    op1, taken1, c1 == FXC_DELIVER, fx_state(o[..], op1) == FX_UNKNOWN, fx_queued(o[..]), dup1 == FXC_NONE);

  // Cancel bumps owner generation so a late completion is dropped (FXC_DROPPED).
  op2 := fx_request(o[..], 0, FX_HTTP, 1, UP_POST, "/api/chat/submit/v2alpha1", "", "{\"text\":\"again\"}", 4096, 10000);
  _ := fx_take(o[..], op2);
  _ = fx_cancel(o[..], 0);
  cancel_notify := fx_cancel_next(o[..]);
  late2 := fx_complete(o[..], op2, gen1, FXO_DONE);
  print("outbox_cancel op=% notify=% dropped=% gen_after=% dropped_count=%\n",
    op2, cancel_notify, late2 == FXC_DROPPED, fx_generation(o[..], 0), o[0].dropped);

  // Part 2: Controlled-clock streaming, Stop fence, and retry dedupe (apps/shared/openagents-app.coax).
  a: [1] OapApp;
  tr: [1] OapTrace;
  oap_play(a[..], OAP_T_LOGIN, tr[..]);
  _ = oap_drive(a[..], "tap shell.new");
  _ = oap_drive(a[..], "type composer.input hello");
  _ = oap_drive(a[..], "tap composer.send");
  print("turn_pending=%\n", oap_drive(a[..], "state turn"));
  print("send_release=%\n", oap_drive(a[..], "clock data release chat.send"));
  _ = oap_drive(a[..], "clock data release chat.part");
  _ = oap_drive(a[..], "clock data release chat.part");
  _ = oap_drive(a[..], "clock data release chat.part");
  _ = oap_drive(a[..], "clock data release chat.done");
  print("stream_done turn=%\n", oap_drive(a[..], "state turn"));
  print("transcript_1:\n%\n", oap_drive(a[..], "state transcript"));

  // Second turn: Stop after one part -> held late parts are dropped and notice: Stopped is recorded.
  _ = oap_drive(a[..], "type composer.input again");
  _ = oap_drive(a[..], "tap composer.send");
  _ = oap_drive(a[..], "clock data release chat.send");
  _ = oap_drive(a[..], "clock data release chat.part");
  print("stop=%\n", oap_drive(a[..], "tap composer.stop"));
  print("late_part_1=%\n", oap_drive(a[..], "clock data release chat.part"));
  print("late_part_2=%\n", oap_drive(a[..], "clock data release chat.part"));
  print("late_done=%\n", oap_drive(a[..], "clock data release chat.done"));
  print("transcript_stopped:\n%\n", oap_drive(a[..], "state transcript"));

  // Third turn: lost connection replays the same operation ID -> stored once.
  _ = oap_drive(a[..], "type composer.input retry");
  _ = oap_drive(a[..], "tap composer.send");
  print("lost_send=%\n", oap_drive(a[..], "clock data lose chat.send"));
  print("retry_release=%\n", oap_drive(a[..], "clock data release chat.send"));
  print("service_messages=%\n", oap_drive(a[..], "state service.messages"));
}
```

```shell
coax/bin/coax run openagents/examples/service-reply-stop.coax
```

Expected output:

```text
outbox_policy_refuse bad_url=0 bad_wallet=0
outbox_unknown op=1 taken=true deliver=true state=true queued=0 dup=true
outbox_cancel op=2 notify=2 dropped=true gen_after=2 dropped_count=1
turn_pending=live
send_release=released chat.send: stored op-1
stream_done turn=idle
transcript_1:
you: hello
reply: Hello! This is a fixture reply.
stop=tapped composer.stop (chat)
late_part_1=released chat.part: late part dropped
late_part_2=released chat.part: late part dropped
late_done=released chat.done: late part dropped
transcript_stopped:
you: hello
reply: Hello! This is a fixture reply.
you: again
reply: Hello! 
notice: Stopped
lost_send=lost chat.send: sent again as op-3
retry_release=released chat.send: op-3 already stored; first result
service_messages=3
```

Planned paths: none new — CPU rules and example run in [`apps/shared/app/core/effects.coax`](../../apps/shared/app/core/effects.coax), [`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax), and [`openagents/examples/service-reply-stop.coax`](../examples/service-reply-stop.coax). Service routes go in [`coax/web/api/`](../../coax/web/api) and their schemas in [`proto/`](../../proto).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, chat with the same thing everywhere
- **Also:**
  - [Coax Native, "Time and lifecycle"](../../coax/native.md#time-and-lifecycle)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 8 and 10
  - [The protocol review](../../docs/protocol/2026-11-11-protocol-review.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P12, P13

## Open questions for the owner

1. Which service answers in this chapter, before chapter 12: the existing
   `/chat` route with its live models, or only the fake service until our
   own loop exists?
