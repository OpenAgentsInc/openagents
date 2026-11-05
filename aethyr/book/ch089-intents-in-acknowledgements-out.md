# 89. Intents in, acknowledgements out

> **Status: partly implemented.** A page sends one input frame per 30 Hz
> step, numbered; oa-room queues at most 16 a seat, drops old and repeated
> ones, and acknowledges the last one it stepped in every state frame
> (`room-everglade.coax`, #12731). A seat earns its steps by the server's
> clock since it sat, at most `EG_BANK` (15) owed and `EG_TICK_STEPS` (4) a
> tick, so ticks that run late don't leave its queue standing full
> (commit d14d704b62). Since wire 4 the heading `THETA` is sent in 0.1 mrad
> (`WIRE_FINE`, commit 3370a41ddc). Casts, interactions and rebuilds are
> refused with a reason (`r SEQ REASON`). A cast waits at most 0.5 s for the
> inputs sent before it, by the rules of chapter 96 (#12790).

A client never tells the server what happened. It tells the server what its
player asked for: a move, a cast, a door. The server decides whether and when
it happens. This chapter adds that command pipeline: numbered intents in,
acknowledgements and refusals out.

## What this chapter teaches

- Intents with sequence numbers, and why the server needs them.
- A bounded per-seat queue that refuses old, repeated and excess commands.
- Stepping inputs at the rate time passes, not the rate they arrive.
- Acknowledgements that say which input the state includes.
- Refusals with a reason the page can show.

## The design

The research doc's capability table (section 17) asks a command pipeline to
validate, sequence, expire, acknowledge and deduplicate. Each Everglade
intent is a short text frame:

| Frame | Meaning |
| --- | --- |
| `I SEQ KX KY PX PY THETA B` | One step's input: keys and stick from −100 to 100, the camera's yaw, run, jump and dive bits. Sent every step, idle ones too. |
| `C SEQ SLOT AX AY AZ TGT` | A cast from a bar slot, at an aim and a target code. |
| `E SEQ KIND ID` | An interaction (talk, door, boat, board, end a held spell). |
| `R SEQ` | Ask for the rebuild. |

### The input queue

Each seat keeps its unstepped inputs in order. An input older than the last
one stepped, or no newer than the last one queued, is dropped. Past 16, the
oldest goes. Every field is clamped before it is stored, so a page can't
send a stick of 500.

<!-- source: coax/web/room/room-everglade.coax intent-queue -->
```jai
// I SEQ KX KY PX PY THETA B HEARD: one step's input (THETA in WIRE_FINE units), queued for the tick; and the state tick its page
// heard whole, which is now its page's (wire_seat_ack), and ends its snapshots once it is one of them.
eg_heard_input :: (v: [] RmServer, k: int, seat: int, text: string) {
  if v[0].eg[k].seat[seat].old { eg_old_input(v, k, seat, text); return; }   // (room-oldwire.coax)
  r: [1] WireRd; r[0] = wire_rd(text, 1);
  w := wire_read_in(text, r[..]);
  if !r[0].ok || eg_fields(text) != 9 { return; }
  eg_seat_heard(v, k, seat, w.heard);
  if w.heard > 0 { v[0].eg[k].seat[seat].said_heard = w.heard; v[0].eg[k].seat[seat].said_ms = v[0].mono_ms; }
  st := v[0].eg[k].seat[seat..<seat + 1];
  seq := w.seq;
  n := st[0].q_n;
  if !life_room_takes(seq, st[0].ack, ifx n > 0 then st[0].q[n - 1].seq else st[0].ack) { return; }   // old or repeated (aethyr/session/lifecycle.coax)
  if n >= EG_INPUTS {
    for i: 1..<EG_INPUTS { st[0].q[i - 1] = st[0].q[i]; }
    n = EG_INPUTS - 1;
  }
  st[0].q[n] = EgIn.{seq = seq, kx = eg_clamp(w.kx, -100, 100), ky = eg_clamp(w.ky, -100, 100),
                     px = eg_clamp(w.px, -100, 100), py = eg_clamp(w.py, -100, 100),
                     theta = eg_clamp(eg_round(w.theta * f32(WIRE_FINE)), -40000, 40000), b = eg_clamp(w.b, 0, 31), at = v[0].mono_ms};
  st[0].q_n = n + 1;
}
```

### Step at the rate time passes

A seat earns one step a tick and spends one per input. What it is owed is
kept up to `EG_BANK` 15 steps (half a second), and at most `EG_TICK_STEPS` 4
are stepped in one tick. So a page that stalls and then sends four inputs at
once still moves at the right speed, and a page that sends inputs too fast
can't move faster than the tick over time. When the host itself stalls,
`rm_eg_owe` gives every seat the ticks it missed.

### Acknowledge in the state

Every `D TICK ACK ...` frame names `ACK`, the last input of yours the server
stepped. Your own `p` entry is the state after that input. The page starts
from it and replays the inputs after it (chapter 90). A transport
acknowledgement is not a gameplay acknowledgement (research doc, section 9):
only `ACK` says the server stepped your input.

### Refuse with a reason

A cast is refused for `slot`, `level`, `cooldown` (at least `EG_CAST_GAP` 8
ticks between casts) or `dead`. A rebuild is refused while the town was hit
in the last 10 s and more than once in 30 s. The page shows the reason under
the bar, for example `CAN'T · COOLDOWN`.

## What is left

- A command carries no client time by design; chapter 96 bounds a cast's
  age by its place in the input sequence instead.
- Casts and interactions are refused but not acknowledged when accepted;
  the page learns of an accepted cast from its `e cast` event.

## Acceptance checks

- An input with a repeated or older sequence number moves nothing.
- Seventeen inputs in one frame leave 16 queued, the oldest dropped.
- A page that sends 60 inputs a second moves no faster than one that sends
  30.

## Dependencies

Chapters 4 (typed actions), 86 (the server) and 88 (seats).
