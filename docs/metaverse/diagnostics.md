# Typed diagnostics: `why ID`

Issue [#13429](https://github.com/OpenAgentsInc/openagents/issues/13429).
An agent that edits a world can't tell from a picture why a thing is missing.
"It is in the state but not drawn" and "its system is paused" need different
fixes. The engine answers `why ID` with the first link that failed in one
chain, in this order:

| Link | The question |
| --- | --- |
| state | Does the realm's state or content have a thing with this id? |
| spawned | Is it an entity in the World now? |
| view | Is it inside what the camera sees? |
| drawn | Did this frame's draw list include it? |
| asset | Is its mesh, texture or pack loaded? |
| collider | Does it have a collider, and does the collider block now? |
| system | Did the system that moves it run? |

The answer names the code, the field that decided, what the engine expected
there, what it found, and whether asking again later can help.

```sh
coax/web/tool.sh drive web apps/openagents-web /worlds/splice "tree" "why you" "why piece-3" "why caps"
```

```text
> why piece-3
why piece-3: draw-dropped
code: engine.diagnostic.draw-dropped.v1
link: drawn (4 of 7)
field: draw.records
expected: at most 34
actual: 35
retry: no
passed: state spawned view
```

`retry: yes` means the engine is still getting there (a commit, a frame, a
fetch): step the clock and ask again. `retry: no` means something has to
change: an edit, the camera, a mode or a limit. A link that doesn't apply to
the thing (scenery has no system) is listed under `not applicable`.

`why caps` lists the limits the engine applied this run, a line each: a full
World, a full draw store, a held frame. A cap that dropped nothing isn't
listed.

## The codes

The codes are the protocol's: each is a row of `proto/registry.json` (kind
`diagnostic`) and of `proto/verse/engine-diagnostics-v1.json`. The table is
in [proto/README.md](../../proto/README.md#diagnostic-codes). A number is
never changed or used again. The one codec is
`metaverse-engine/agent/why.coax`; check both against each other with
`coax/web/tool.sh metaverse-engine/tools/why-profile.coax proto/verse/engine-diagnostics-v1.json`.

## Answering `why` in a realm

A realm walks the chain for one id from its own state. Splice is the first
(`metaverse/splice-bots/why.coax`): `you`, `piece-N`, `foe-N`, `rubble` and
`boss-wall`.

1. `why_begin(w, id)`.
2. For each link in order: `why_pass`, `why_skip` (the link doesn't apply) or
   `why_fail(w, CODE, field, expected, actual)`. `why_fail_i` and
   `why_fail_m` write whole numbers and metres. Only the first failure is
   kept.
3. `why_answer(w, out)` is the text.

For caps, write a line for each limit with `why_cap(out, at, WHY_BUDGET_HIT,
field, limit, dropped)` and finish with `why_caps_answer`.

Read facts the realm already keeps. Don't add a second copy of a counter for
the diagnostic, and never change the game from a `why`.

## Where it shows

- `coax drive why ID` (`coax/web/drive.coax`), today.
- `world why`, the editor's inspector and the result of `world apply` print
  the same `why_answer` text when the world tools land (#13415).
- Alerts (#13297) and crash buckets (#13379) name a code from this set.

A realm that uses the engine's driver but hasn't written its walk answers
`why` with the driver's "unknown operation" line. Everglade is next.
