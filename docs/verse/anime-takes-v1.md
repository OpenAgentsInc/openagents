# Anime recorded takes: JSON v1

This document defines the `anime.v1` JSON recorded-take profile for
[#12633](https://github.com/OpenAgentsInc/openagents/issues/12633). The authority
is [`verse/anime/take.coax`](../../verse/anime/take.coax); the page is
[`animelab.coax`](../../apps/openagents-web/animelab.coax). The words, recorded
times, and held catalogue answers belong to the take. Loading one performs no
speech recognition, selector call, or asset generation.

Source, native API and headless media qualification is recorded in
[`docs/verification/anime-take-2026-11-02.json`](../verification/anime-take-2026-11-02.json).
It includes deterministic still/frame comparisons, full-size transport and
independent MP4 decoding. Deployment is a separate operation.

## Document

```json
{
  "schema": "anime.v1",
  "stage": "courtyard",
  "auto": false,
  "length": 8,
  "lines": [
    {
      "said": 0,
      "done": 0.2,
      "words": "Kai casts a fireball at the gate.",
      "answers": [
        {"kind": "cast", "who": "hero", "spell": "fireball", "place": "gate", "p": 0.97}
      ]
    },
    {
      "said": 0.3,
      "done": 0.5,
      "words": "Low angle on Kai.",
      "answers": [
        {"kind": "shot", "who": "hero", "framing": "low-angle", "move": "hold", "p": 0.99}
      ]
    },
    {
      "said": 0.7,
      "done": 0.9,
      "words": "A paper whale floats overhead.",
      "answers": [
        {
          "kind": "make", "what": "paper whale", "like": "creature", "place": "sky", "p": 0.99,
          "stock": false, "render_supported": false, "decision": "heard"
        }
      ]
    }
  ]
}
```

The root MUST be an object with `schema:"anime.v1"` and a `lines` array.
Only the fields below are permitted. Unknown fields, wrong types, duplicate
decoded object keys, malformed JSON, or an invalid answer refuse the complete
document. Duplicate-key checking also applies inside retained metadata.

| Root field | Contract |
| --- | --- |
| `schema` | Required string, exactly `anime.v1`. |
| `lines` | Required array of 0–16 line records, in recorded time order. |
| `stage` | Optional string, exactly `courtyard`; omission uses that stage. |
| `auto` | Optional boolean, default `true`; permits the existing timeline grammar to insert framing shots. |
| `length` | Optional numeric seconds, greater than zero and at most 600. It MUST cover every line's `done` time and every scheduled execution event's start. |

When `length` is omitted, it is the later of the final words' completion and
the final execution event's start, plus three seconds. The derived value MUST
also fit the 600-second limit. Explicit length can leave additional time for
effects and captions to finish.

Each line MUST contain exactly `said`, `done`, `words`, and `answers`:

| Line field | Contract |
| --- | --- |
| `said` | Numeric seconds at which the recorded words begin, in 0–600. |
| `done` | Numeric seconds at which they finish; `done >= said`, in 0–600. |
| `words` | A string of 0–600 decoded UTF-8 bytes. Tab, LF, and CR are allowed; other C0 controls are refused. |
| `answers` | An array of catalogue answer objects, in their recorded order. There may be zero answers. |

A line's `said` MUST be at least the preceding line's `done`; overlapping or
reversed intervals are refused. Times pass through the bounded finite F64
reader before comparison, then become F32 timeline values. No arbitrary
precision time guarantee is made.

All limits apply together:

| Resource | Maximum |
| --- | --- |
| Complete decoded JSON source, including whitespace and metadata | 32,768 bytes |
| Lines in the document | 16 |
| Answers across **all** lines | 16 |
| Decoded `words` bytes in each line | 600 |
| Recorded times and total take length | 600 seconds |
| JSON value nodes, including retained metadata | 4,096 |
| JSON container nesting | 64 |
| Decoded object-key bytes | 64; keys must be nonempty and free of C0 controls |
| Scheduled events, including automatic framing | 32 |

The source limit is a joint budget: a take with sixteen long lines, answers,
and metadata must still fit 32,768 bytes and 4,096 nodes. These are decoded
JSON-source bytes, not the length of the URL's base64url representation.

## Answers and confidence

Answers use the flat catalogue fields understood by `anime_read`. The
[director API catalogue](2026-11-02-anime-director-api.md#direction-catalogue)
defines their finite vocabulary. No cast-name rebinding or custom cast table
is accepted in this profile; `who`, `to`, `archetype`, and `like` use the
catalogue archetypes.

| Kind | Required fields besides `kind` and `p` | Optional direction fields |
| --- | --- | --- |
| enter | archetype | how, count, place |
| exit | who | how |
| act | who, action | — |
| emote | who, emotion | — |
| cast | spell; exactly one of to or place | who, size |
| fx | effect | place, size |
| shot | framing; exactly one of who or place | move |
| cut | to | transition |
| beat | beat | — |
| make | what, like | place |

`set` and `say` are outside this answer profile. A missing shot `move`
defaults to hold, an entrance `how` to walks, `count` to one, and a cut
`transition` to cut. `count` MUST be an integer from one through three.
`what` MUST be 1–40 decoded UTF-8 bytes that the existing flat reader can
preserve: no quote, backslash, or C0 control characters.

Every answer MUST supply numeric `p` in 0–1 on the exact 0.001 grid. Decimal
or exponent spellings that denote a grid value are accepted; non-grid values
such as `0.8501` are refused without rounding. The holder stores an integer
`milli` from 0 through 1,000 and preserves the recorded probability in
captions, including heard-only answers.

Optional metadata is held as follows:

| Field | Contract |
| --- | --- |
| `timing` | If present, exactly `now`, `after`, or `with`. It is retained intent, not an instruction to reschedule recorded times. |
| `stock` | If present, MUST equal the catalogue fact: false for make, true for other admitted kinds. |
| `render_supported` | If present, MUST equal the current shared static profile. Omission derives that value; a caller cannot override it. |
| `decision` | `heard` may conservatively suppress any answer. `play` requires stock, support, and `p > 0.85`; `soft` requires stock, support, and `0.5 <= p <= 0.85`. Other values or incompatible dispositions are refused. |
| `confidence` | If present, MUST be an object. It is retained bounded JSON metadata. The take loader does not recompute vectors, normalization, argmax, or a minimum from it. |

Without an explicit conservative `heard`, supported in-stock answers with
`p >= 0.5` may execute. Exactly 0.85 is soft; 0.851 is above the play
threshold. A low probability, out-of-stock answer, or unsupported profile
always remains heard-only. Metadata cannot promote it into execution.

An API-produced answer can retain its complete confidence maps. Their API
validation and provenance must be established separately; importing an
object-shaped `confidence` field is not proof of those properties or empirical
calibration. Only root direction fields enter the compatibility reader, so
keys nested in confidence metadata cannot shadow a selected field or `p`.

## Static support and asset boundaries

[`render-profile.coax`](../../verse/anime/render-profile.coax) is the shared
authority used by the API and take holder. It distinguishes existing distinct
poses, emotions, fireball, supported entrance/camera/effect combinations,
cut/flash/smash transitions, and hold/speed-lines beats from catalogue options
that the renderer cannot faithfully perform. Stock membership and render
support are separate facts. Static support does not establish current subject
visibility or successful rasterization.

The current body drawer draws slots 0–5. A creature entrance, a creature
subject for act/emote/shot, a cut to a creature, or a cast from/to a creature
is unsupported. A static fallback camera or substituted spell does not make
the requested option supported.

Every arbitrary `make` is caption-only. Its description and probability remain
visible in director presentation with a heard-only/out-of-stock verdict.
It MUST NOT execute the fixed demonstration dragon, display drawing progress,
claim MADE, or claim the requested asset was generated. A take may validly
have recorded answers and no execution events.

## Recorded timing and ownership

The existing `anime_timeline` grammar schedules admitted execution events.
An answer's caption time is its line's `done`, plus 0.35 seconds when
`said < done`, plus 0.14 seconds for each preceding answer in that line.
Camera events can wait for a 0.6-second close-up hold or a 0.4-second cast
anticipation; feelings after a flash cut wait for its three white frames.
`auto:true` permits the grammar's own framing shots. The supplied `timing`
intent does not implement a live now/after/with queue in this recorded profile.

Admission is atomic. Source bytes, decoded words, answer ranges, numeric
directions, and scheduled events are owned by `AnimeTake`; no retained string
view borrows the transient host message or caller buffer. A failed replacement
leaves the accepted document and events unchanged. Accessors copy words and
answer JSON out; temporary caption description views last only for the paint
call. Captions and line ticks use the loaded words, times, and all recorded
answers, including those that never enter the execution timeline.

## Page transport and presentation

The page accepts one `take64` query value containing **canonical unpadded
base64url** of the UTF-8 JSON bytes. The alphabet uses `-` and `_`; padding,
whitespace, the standard `+`/`/` alphabet, nonzero unused tail bits, duplicate
`take64` values, or an empty value are refused. The decoded result MUST satisfy
all document bounds. The maximum 32,768-byte input occupies 43,691 unpadded
base64url characters.

An explicit invalid initial take clears the default execution timeline and
shows **TAKE REFUSED · NO TAKE IS PLAYING**, including in film/card capture.
It MUST NOT silently capture the built-in demo as the requested take. A later
invalid replacement preserves the accepted take, playback time, and events,
with **REPLACEMENT REFUSED · KEEPING LOADED TAKE**. A valid replacement resets
playback to zero and clears the prior pin/refusal.

| Control | Behavior |
| --- | --- |
| `coaxial.send(701, JSON_TEXT)` | Load or replace a raw JSON take through the same atomic holder. |
| `coaxial.send(700, "5.5")` | Pin a moment in seconds. |
| `coaxial.send(700, "-1")` | Clear the pin and reset playback to zero, allowing it to run. |
| `?t=5.5` | Initial pinned moment. |
| `?card=1` | Hide ordinary caption overlay; an initial/replacement refusal remains visible. |
| `?stage=direct` / `?stage=film` | Director captions or film presentation. A custom take keeps its own timeline at normal playback speed in either presentation. |
| F | Toggle a custom take's presentation while preserving its time, pin, and events. |
| H | Toggle the ordinary caption overlay. |

Without `take64`, the existing built-in demo remains the default. Its film
presentation remains the built-in short. Those recorded simulations include
the fixed ink-dragon draw-in; that demonstration does not establish an asset
maker for arbitrary imported takes.

For automation, `coaxial.probe()` reports take status: 0 built-in, 1 accepted
custom, -1 initial refusal, -2 rejected replacement with a custom take retained.
`coaxial.guardProbe(field)` exposes the following integer observations:

| Field | Observation |
| --- | --- |
| 0 | Take status |
| 1 | Recorded line count |
| 2 | Recorded answer count |
| 3 | Execution event count, including automatic framing |
| 4 | Take length in milliseconds |
| 5 | Heard-only answer count |
| 6 | Admitted custom source bytes, including JSON whitespace; zero for built-in |
| 7 | Playback time in milliseconds |

Unknown fields return zero. A retained replacement keeps the previous custom
take's counts and bytes. Initial refusal reports zero line/answer/event counts.

## Existing shot and clip commands

Replace `TAKE64` below with the canonical encoding of the complete document.
These commands use the existing `labs shot` and `labs clip` URL interfaces.
They run headless and muted. Serve the local page before using the local URLs.

```sh
openagents labs shot "http://127.0.0.1:8800/realms/anime-lab?stage=direct&take64=TAKE64&t=5.5" take.png --size 1280x720 --fresh
```

For a clean still, add `&card=1`. A clip should clear any initial pin and reset
the page before fixed-step recording; `--eval` runs before the skip/capture:

```sh
openagents labs clip "http://127.0.0.1:8800/realms/anime-lab?stage=film&take64=TAKE64&card=1" take.mp4 --size 1280x720 --seconds 8 --fps 30 --skip 0 --eval "coaxial.send(700, '-1')" --frames take-frames --fresh
```

The URL carries the take. `--seconds` chooses capture duration independently
of its declared length. This interface does not add the future `.anime` text
parser or a new `labs anime render` command from the design sketch.
