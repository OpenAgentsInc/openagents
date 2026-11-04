# Local Aethyrforge scene commands v1

`aethyrforge-scene show` reads the running desktop's local Everglade document
and catalogue. `aethyrforge-scene apply JSON` accepts the
[typed command](../../proto/aethyrforge/scene-command-v1.schema.json), never
natural language. Coder interprets the user's words and calls this same tool.

`place` names a catalogue item, count (default 1), arrangement (`row` by default
or `stack`) and size percent (default 100). Optional `x_cm`/`z_cm` must appear
together; without them the planner searches open ground near the current player.
Stacks require a catalogue support surface and share one group with their base.
The batch contains 1–16 items (64 total in the document), with size 25–400%.
Stack base elevations must stay within 10 m. A row leaves 1.25 m between
footprints. Nearby placement searches at most 64 candidates; explicit coordinates
try only that position. Coordinates are centimetres within ±16000.
`move` translates every member of `group` by `dx_cm`/`dz_cm`; `remove` removes
that whole group so the bottom of a stack is not removed by itself.

The entire proposed document is checked with Everglade's existing placement
rules before one write. A refused batch changes nothing. The tool reads a base
version and posts that same version through the existing local world API;
HTTP 409 reports a concurrent change rather than overwriting it. A successful
write is read back before reporting success. A lost reply is not retried as an
additional placement. The room's normal edit event updates the page and server
world, including player walls and NPC avoidance fences. This is a local save,
not public publishing. Unsupported items fail explicitly; no asset generation.

The desktop supplies a private `scene-session.json` with schema
`aethyrforge.scene-session.v1`, exact loopback API origin, observed player
centimetres and monotonic observation time. The tool refuses a stale session
or a non-loopback origin. `AETHYRFORGE_SCENE_SESSION` selects that file for an
isolated app/test. The desktop also supplies Coder's context file describing
these commands. All persistent state stays under `~/.openagents/`.
