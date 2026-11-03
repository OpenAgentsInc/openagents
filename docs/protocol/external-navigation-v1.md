# External navigation v1

Identity: `external-navigation-v1`, `openagents.engine.navigation.v1`.
The semantic schema starts in `proto/openagents/engine/navigation/v1/navigation.proto`.
This is a local JSONL observation/action/outcome journal over desk-control-v2.
It does not change the desk contract or replay the external game's simulation.

One iteration captures an image and sensed facts, chooses one bounded action,
rechecks the target/context, performs the action, releases input, waits for
the game to render, then captures and checks the outcome. Never issue an
unbounded held input, auto-run, or the next action before observing the last.

Each observation binds the desk session, handle/lifetime revision, capture
sequence, sensor sequence, monotonic time, source and image. Map coordinates
are normalized east/south coordinates in the named map, not Aethyr metres.
Heading is radians counterclockwise from north. Arrival uses the goal's
declared map-coordinate radius. A map change or missing position refuses.
The first CLI profile is Dun Morogh, map 1426. Its journal/arguments use
quantized `*_u16` coordinates (divide by 65535 for the semantic normalized
coordinates) and a declared east/north map aspect of 1500/1000. The radius
is in the Y coordinate's units; X distances are scaled by that aspect.
The forward calibration observed a 12-by-23 coordinate change at heading
39648/65535 turns, consistent with this aspect. Other maps are refused by
the initial CLI until their geometry is qualified.

Aethyr supplies the shared `Intent` vocabulary and controller-level external
event record. Psionic's existing `GdContext`/ticket gates asynchronous
decisions: epoch is the desk session, actor/generation the window lifetime,
revision the observation, policy/options fixed profile identifiers. A model
enum is staged and consumed once under the same context before its deadline.
No model score can extend action bounds or legal input. Old, duplicate and
expired answers are refused before effects.

Initial actions: stop, forward, left, right, back, jump. Turn actions mean
turning in the game's control profile; they do not assert a known physical
angle. Maximum one action is 500 ms, followed by at least 300 ms settling.
Unavailable/dead/combat observations stop. A stale sensor, focus loss, failed
control, no progress or an exhausted iteration budget stops the loop. A
separate `verse-game stop` can cancel held input immediately.
Create `OUT/stop` to end the controller loop as well. It checks that file
before model work, after the answer, and at the next iteration. No input
is held during inference; an action already running ends within its pulse
bound. Killing the controller closes its connection and releases its input.

The current waypoint selector offers stop/forward/left/right. Back/jump
exist in the shared action adapter for explicit recovery, but are not model
options. Forward pulses are 40–300 ms and turns 20–100 ms, shortened near
the goal/alignment. Geometry admits stop plus only the currently meaningful
steering direction; a model proposal outside that mask refuses, without
substituting a rule. Two forward pulses with less than four coordinate
units of movement stop as no progress. Every run also has an action cap
and takes a final observation after its last permitted action.

Elephant consumes typed textual facts from a named sensor. It does not read
the screenshot. Preserve the full probability map and pinned model identity
in the journal, alongside observed progress; confidence is not measured
navigation quality. A deterministic controller is a separately named mode,
never an undisclosed fallback from a failed local-model call.
The qualified learned request is a compact semantic bearing (waypoint
left/right/ahead); full sensor/goal geometry stays in the journal. Numeric
prompt trials were less reliable and are retained as failed experiments.

## WoW public-API sensor profile

The optional `os/wow/VerseNavigation` addon is SDK glue. It samples the
public Classic map-position/facing/life/combat APIs and draws a visible
64×8 nominal-pixel strip at the top-left of the game. It has no input effects and
does not inspect process memory. Source API definitions:
[map API](https://github.com/Gethe/wow-ui-source/blob/classic_era/Interface/AddOns/Blizzard_APIDocumentationGenerated/MapDocumentation.lua),
[player API](https://github.com/Gethe/wow-ui-source/blob/classic_era/Interface/AddOns/Blizzard_APIDocumentationGenerated/PlayerScriptDocumentation.lua).

Eight 8×8 cells carry 24 RGB bytes. Samples use each cell's center, in
game pixels; the decoder resolves WoW's UI-scale cell pitch from the complete
magic/checksum (4–32 physical pixels per cell). Bytes 0–2 are 86,78,1;
3–4 the big-endian rolling sample counter; 5 flags (valid=1, alive=2,
combat=4, menu/settings/chat blocked=8); 6–7 map ID; 9–10 map X;
12–13 map Y; 15–16 facing as a fraction of one turn. Positions/facing are
rounded to 0–65535. Bytes 8,11,14,17–22 are zero. Byte 23 is the sum of
0–22 modulo 256. Samples update at most every 0.1 seconds.

Wrong magic/checksum/reserved bytes, unavailable/dead/combat flags, missing
maps and a sensor counter that fails to advance refuse navigation. These
are UI observations, not authenticated facts. A window/image change still
invalidates the model ticket. Install the addon before launching the client;
a UI reload does not discover a newly created addon folder.
