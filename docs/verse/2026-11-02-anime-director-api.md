# Anime Lab director API, 2026-11-02

This is the public v1 contract implemented for
[#12632](https://github.com/OpenAgentsInc/openagents/issues/12632): words and
stage context become bounded catalogue directions. It follows
[the director design](anime-lab.md#2-the-director-words-to-directions-with-a-decision-model)
and preserves the flat direction fields that `anime_read` understands.
This document describes the source contract and does not assert deployment.
Qualification profiles and their limits are recorded in
[the verification record](../verification/anime-direct-api-2026-11-02.json).
The public contract below is independent of the private provider envelope.

## Request

`POST /api/playtest/labs/anime/direct`

Send `Content-Type: application/json`. The two accepted UTF-8 charset forms are
`application/json; charset=utf-8` and `application/json;charset=utf-8`.
The complete body must contain 1 through 16,384 bytes of valid UTF-8 JSON.

```json
{
  "v": 1,
  "words": "Hold this shot.",
  "state": {
    "cast": [
      {
        "slot": 1,
        "name": "Kai",
        "archetype": "hero",
        "on": true,
        "place": "courtyard"
      }
    ],
    "shot": {"framing": "medium", "who": "hero", "move": "hold"},
    "last": []
  }
}
```

Only the documented properties are accepted. Duplicate object keys are rejected,
including differently escaped spellings of the same decoded key. `v` must be
the integer `1`; `words`, `state`, and each of `cast`, `shot`, and `last` are
required. Unknown fields, malformed JSON, or an invalid field fail the whole
request before a model reservation.

| Field | Bound and meaning |
| --- | --- |
| `words` | 1–600 decoded UTF-8 bytes, containing something besides space, tab, LF, or CR. Those whitespace characters are permitted; other C0 controls are refused. |
| `state.cast` | At most seven records with unique integer `slot` values 0–6. Each record requires `slot`, `name`, `archetype`, `on`, and `place`; optional `action` and `emotion` use the catalogue below. |
| Cast `name` | 1–40 decoded UTF-8 bytes without C0 controls. It is model context, not an executable identifier. |
| Cast `archetype` | Must match the fixed slot: 0 heroine, 1 hero, 2–4 hooded, 5 dark-lord, 6 creature. |
| Cast `on` | A JSON boolean describing whether the supplied cast member is on stage. |
| `state.shot` | Requires `framing` and at least one of `who` or `place`; optional `move`. Both subject and place may be supplied as state context. |
| `state.last` | At most eight previously held directions. Existing scalar-`p` directions are accepted; vectors may also be supplied. If both are present, scalar `p` must equal their derived minimum. |

Stage state is supplied context. The route does not persist a take, mutate a
stage, or establish that a requested subject is currently visible.

## Authentication and Origin

A caller uses either a valid site session cookie or a verified NIP-98
`Authorization: Nostr …` request. `X-Verse-Player` alone does not authorize this
route. An explicit invalid Authorization header fails with 401 even when a
valid cookie accompanies it.

A signed POST must bind the exact URL, method, and body with its payload hash,
and its signature must be unused. Native signed callers may omit Origin.
Cookie callers must send the configured site's matching Origin. An explicit
foreign Origin is refused with 403 before identity verification can consume a
signature's first use. A signed request is single-use even if later input
validation, quota admission, or model execution fails; sign a fresh retry.

## Response and confidence

A successful response has HTTP 200 and this shape. The numbers here are an
illustrative valid answer, not a recorded model result.

```json
{
  "v": 1,
  "heard": "Hold this shot.",
  "answers": [{
    "kind": "beat",
    "beat": "hold",
    "p": 0.900,
    "timing": "now",
    "confidence": {
      "kind": {
        "enter": 0, "exit": 0, "act": 0, "emote": 0, "cast": 0,
        "fx": 0, "shot": 0.1, "cut": 0, "beat": 0.9, "make": 0
      },
      "beat": {
        "hold": 0.97, "slow-motion": 0.01,
        "freeze-frame": 0.01, "speed-lines": 0.01
      },
      "timing": {"now": 1, "after": 0, "with": 0}
    },
    "stock": true,
    "render_supported": true,
    "decision": "play"
  }]
}
```

`heard` preserves the decoded words. `answers` contains zero through eight
ordered directions; an unmatched request can return an empty array. Each
answer is checked against the catalogue and again through the existing
`anime_read`/`anime_direction_ok` compatibility gate. One invalid answer fails
the complete response; no accepted prefix is published. Output is bounded to
65,536 bytes.

For every selected finite field, including `kind` and `timing`, `confidence`
contains a complete map naming every option in that field's catalogue order.
Every probability is in 0–1 on the exact 0.001 grid; each map sums exactly to
1.000. The selected option must have the largest probability. A tie chooses
the first option in the catalogue order below. Unused maps are refused.
The server derives `p` as the minimum selected-option probability across all
present finite fields. The opaque description `what` has no probability map.

These are model-reported probability maps. Applying the design's thresholds
does not demonstrate empirical calibration or measured semantic accuracy.
The server derives `decision`; the caller applies it:

| Conditions | `decision` | Intended caller behavior |
| --- | --- | --- |
| In stock, render supported, `p > 0.85` | `play` | Play the direction. |
| In stock, render supported, `0.5 <= p <= 0.85` | `soft` | Play with the design's “did you mean” affordance. |
| `p < 0.5`, or out of stock, or render unsupported | `heard` | Preserve the heard direction without executing it. |

Exactly 0.85 is soft; 0.851 is play when stock and support permit it.
`timing` is `now`, `after`, or `with`; the route returns timing intent and
does not schedule a timeline itself.

## Direction catalogue

The following order also defines probability-map tie breaking. Integer `count`
is selected from 1, 2, 3; its map uses string keys `"1"`, `"2"`, `"3"`.

| Field | Options, in order |
| --- | --- |
| `kind` | enter, exit, act, emote, cast, fx, shot, cut, beat, make |
| `who`, `to`, `archetype`, `like` | heroine, hero, hooded, dark-lord, creature |
| `action` | raise-wand, cast, block, run, fall, kneel, turn, point, hesitate, recoil, shout |
| `emotion` | calm, sad, angry, afraid, determined, smug, shocked, crying, laughing, hesitant |
| `spell` | bolt, fireball, shield, frost-ring, guardian, light |
| `effect` | explosion, rubble-rain, flash, smoke, embers, wind, lightning, petals, shockwave |
| `framing` | extreme-close-up, close-up, medium, wide, establishing, over-the-shoulder, low-angle, high-angle |
| `move` | hold, push-in, pull-out, orbit, shake, whip-pan |
| `transition` | cut, flash, smash, speed-line-wipe, match |
| `beat` | hold, slow-motion, freeze-frame, speed-lines |
| `how` | walks, runs, appears, lands |
| `place` | gate, courtyard, wall, sky, left, right, behind, above |
| `size` | small, large, huge |
| `count` | 1, 2, 3 |
| `timing` | now, after, with |

| Kind | Required direction fields | Optional direction fields |
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

Every new answer also requires timing and its confidence maps. A missing shot
move defaults to hold, an entrance how to walks, a count to one, and a cut
transition to cut in the existing engine projection. `what` is 1–40 decoded
UTF-8 bytes of plain text without quotes, backslashes, or C0 control escapes,
because the existing flat reader must preserve it. `set`, `say`, and
destructive commands are outside this route's answer profile.

## Stock and render support

`stock` is a catalogue fact: `make` is false; other admitted kinds are true.
An out-of-stock ask retains its description and nearest archetype as a `make`
answer with `stock:false`, `render_supported:false`, and `decision:"heard"`.
This route does not generate the requested asset or substitute the fixed demo
creature as a completed generation.

`render_supported` describes the current static stage profile. It is separate
from stock and confidence. For example, a stock `petals` effect or `orbit`
camera move remains typed catalogue data but is unsupported and heard-only.
Support covers the implemented distinct poses and emotions, fireball, selected
entrances and camera combinations, cut/flash/smash transitions, the implemented
effect/place/size combinations, and hold/speed-lines beats. The exact finite
profile is checked in
[`anime-director-render-checks.coax`](../../coax/web/api/anime-director-render-checks.coax).
It does not certify current cast visibility, renderer availability, a rendered
frame, or a newly generated asset. Changing that profile requires corresponding
policy and checks; it does not change the finite vocabulary above.

## Provider boundary

The host uses the existing Gemini selector transport. Coax prepares a typed
private envelope: each answer contains `selections`, with one
`{field,value,probabilities}` row per selected field. The field tag is a finite
enum; the value is a string, and probabilities are the complete vector in that
field's catalogue order. Integer counts use string values on this private wire;
`what` uses an empty vector. Coax checks and converts these rows into the public
flat fields and named maps, then applies the unchanged catalogue holder.

The generated provider schema omits numeric, string and array bounds. Including
them in the full nested profile produced `400 INVALID_ARGUMENT` in the retained
live controls; Coax enforces every documented bound before any public answer.
The compact row profile also avoids the incomplete maps and long completion
observed with the nested profile. These are observations of this qualified model
profile, not a claim that every bounded schema is unsupported. Google documents
[complex-schema rejection and simplification](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/capabilities/control-generated-output).

The model is `OA_ANIME_MODEL`, falling back to `OA_NPC_MODEL` and then
`gemini-3.8-flash`. `OA_ANIME_URL` overrides the normal Vertex endpoint for
isolated qualification. Provider output, including a syntactically valid output,
is untrusted until the Coax holder accepts the whole response.

## Admission, errors, and retention

Quota is reserved atomically before calling the selector. Limits are 12 admitted
attempts per identity in the preceding minute, 120 per identity per UTC day,
1,200 globally per UTC day, one active request per identity, and four globally.
Refusals do not insert a call or invoke the model. Admitted attempts count even
when the model later fails. Each reservation has a 60-second lease; completion
ends activity earlier, and an expired lease releases active capacity after a
worker disappears. A lease does not refund minute or daily usage.

The selector timeout defaults to 8,000 ms per try. Deployment configuration
may set a digits-only integer from 1 through 15,000 ms. Transport retries an
unreached, 429, or 5xx selector once; this is a per-try timeout, not a promise
about total HTTP latency. Authentication-token and database work are separate.
The upstream body accepted by the policy is at most 65,536 bytes.

Failures use a complete versioned JSON body such as
`{"v":1,"error":"quota_exceeded"}`. They do not echo the prompt or publish
partial answers.

| HTTP status | Error words used by this route |
| --- | --- |
| 400 | invalid_input |
| 401 | authentication_required |
| 403 | cross_site |
| 405 | method_not_allowed |
| 413 | input_limit |
| 415 | application_json_required |
| 429 | quota_exceeded |
| 502 | invalid_model: malformed/unsupported selector output or an upstream status other than 200, 0, or 429 |
| 503 | director_unavailable: unreachable/429 upstream or unavailable local admission; output_capacity if bounded serialization cannot fit |

Responses are non-cacheable and vary on Cookie and Authorization. The route's
`anime.calls` ledger stores only an ID, authenticated identity, start time,
lease expiry, finished flag, and final status. It stores no words, stage state,
model prompt, or model response. NIP-98 first-use tracking separately retains
the event ID and time through the existing identity helper. Request context
and the provider prompt exist in memory and are sent to the selector; this
local retention contract makes no claim about an external provider's retention.

The executable authorities are
[`anime-director-policy.coax`](../../coax/web/api/anime-director-policy.coax)
and [`anime-route-policy.coax`](../../coax/web/api/anime-route-policy.coax).
[`anime.c`](../../coax/web/api/anime.c) performs HTTP, authentication, database,
and selector effects;
[`0103_anime_director.sql`](../../coax/web/api/migrations/0103_anime_director.sql)
defines the ledger. Provider schema compatibility and final integration
verification must be recorded separately before claiming the real route works.
