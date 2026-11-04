# External combat control v2

Identity: `starter-warlock-v2`, `openagents.engine.combat.control.v2`.
The published [combat observation v1](external-combat-v1.md) and its SDK
bytes remain unchanged. This profile replaces the initial controller's
search and stopping behavior; quest selection remains outside it.

The controller finds candidate neutral-creature health bars in the captured
RGB image. The detector excludes the SDK strips, player/target HUD, minimap,
quest panel and hotbar; it requires repeated yellow spans and chooses a
central nearby candidate. Pixels permit selection only. Before pulling,
it turns toward a visible candidate, clicks it, then checks the public
target's identity, eligibility, range, and player resources. A missing or
ineligible selected target cannot authorize a spell. A pre-pull approach
invalidates its visual binding and requires another observed selection.
This avoids repeatedly walking toward an off-screen target from a range bit.
Enemy nameplates must be enabled. This first detector qualifies the normal
yellow bars in the native Classic Era profile, not arbitrary UI themes.

If no candidate is visible, the bounded survey visits a 600×600 map-unit
rectangle from the stable qualified home (20300,45900) on map 1426, using
the existing Aethyr-backed waypoint geometry. Its map aspect is 1500/1000.
The absolute X/Y displacement sum from home cannot exceed 1800 units;
restarting a run cannot move that fence. A survey leg must improve distance
by at least 12 units within 12 seconds. Three nearly stationary forward
pulses also mark it blocked. Outside combat, a blocked leg gets a 200 ms
back step and a 300 ms right sidestep, then the next leg is surveyed.
Blocked movement during a fight still refuses. A waypoint does not establish
traversability and no hidden enemy coordinates are read. Large visual turns
use 90 ms; alignment within 400 pixels uses 35 ms. Survey turns use 60 or
100 ms and reobserve actual heading rather than assuming a turn succeeded.

Before an effect, a second capture rechecks player/menu state, target epoch,
NPC/spawn suffix and current action admission. Changed context discards the
proposal. Inputs are at most 300 ms; selection is a 40 ms mouse click.
All input is released before a 500 ms observation interval. Shadow Bolt
has priority when usable; melee is a no-mana fallback. This is a deterministic
combat policy, not a learned combat model. Original local Psionic Elephant
remains the separately qualified waypoint steering mode.

`OUT/stop` remains immediate and can abandon an active fight. It stops the
controller's input, not the external world's enemies or a previously
started game action. `verse-game stop` releases held input immediately.
For development pauses, `OUT/finish` forbids a new pull, finishes an admitted
encounter, waits until the player is out of combat, then records `finished`.
The observation budget and ten-minute wall-clock budget use the same drain
boundary, with up to 100 extra observations for the current bounded fight.
The requested kill count also drains until combat clears before success.
An encounter lasts at most 45 seconds, with at most 20 seconds without
observed damage, allowing an approach and first projectile to resolve.
Immediate stop, process death and physical input still revoke held keys.

The first profile is a level 2–5 warlock on map 1426, with the observed
686/687/6603 spell/attack slots and 159/4604 water/food slots. Missing water
falls back to observed natural regeneration. It refuses danger, unsupported
classes/ranks, UI focus and stale observations; emergency retreat and
automatic corpse recovery are not qualified. No confidence score changes
these bounds. Real runs, including the development-pause death, are in the
[combat journal](../os/verse-combat.md).
