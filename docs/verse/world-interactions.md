# Maps, companions, and doors

This ordered implementation extends the shared Verse world in Coder. The
release remains held until map navigation [#9720](https://github.com/OpenAgentsInc/openagents/issues/9720),
companion interaction [#9721](https://github.com/OpenAgentsInc/openagents/issues/9721),
and door demos [#9722](https://github.com/OpenAgentsInc/openagents/issues/9722)
are checked together with the mobile connection corrections in
[#9718](https://github.com/OpenAgentsInc/openagents/issues/9718).

## Map navigation

The compact map sits at the top right, below the system status area. Tap it to
expand. The expanded map shows the seeded city's building footprints and named
wards, your position, destinations, and your remaining route. A slow scanning
line and breathing destination marker show activity without another status
banner. Landmark shortcuts target clear approaches to the Computer, Gym,
Oracle, Library, proving ground, and plaza.

Tap a clear map position or landmark to walk there. The map folds away while
the character walks. Reopen it to inspect the route or select **Stop walk**.
On desktop, `M` toggles the map and `Esc` closes it or cancels the route. Manual
movement or jumping cancels automatic walking; camera-only input can continue.
Opening an application panel or backgrounding stops the route. A blocked
position produces a map error and stops an earlier route instead of silently
substituting a different destination.

Navigation plans a bounded route around the same inflated collision footprints
as the player controller, including the Gym's real entrance. The character
walks through that controller at ordinary speed. Clicking the map never
teleports the player through a building. Destination coordinates are local
world positions, not relay addresses, URLs, commands, or grants.

The map and its animation draw through Verse's existing Rust GPU overlay. Rust
also owns click capture, coordinate projection, route state, and bounds. Native
adapters supply safe areas, pointer contacts, and accessibility callbacks.
Product styling remains in `coder-ui`; Rust Native remains a generic drawing
surface foundation.

## What the existing doors mean

The oracle arch marks where replayed agents ask typed decision questions. Its
shape visualizes a model *door*, an endpoint with a declared contract. A replay
visiting that arch does not call a model. The Gym doorway is a physical opening
in collision geometry. Entering the Gym starts observation only under an
existing, separate Gym grant.

Neither structure currently defines general cross-world travel. The new demo
will add an explicit, inspectable routing layer instead of treating decorative
geometry as permission to execute work. Local demo routes to the Gym stop
outside its observation boundary. A visual key is an item in the demo, not a
signing key, pairing invitation, or authorization credential.

## Ordered delivery

| Issue | Deliverable | Acceptance |
| --- | --- | --- |
| #9720 | Shared navigation, expandable map, and native input adapters | Select a real map position, follow a collision-safe route, cancel, and preserve camera/gesture behavior. |
| #9721 | Companion tap and playful bounded emote | A visible tap animates the companion; drags and occluded targets do not. |
| #9722 | Reactive doors, held demo items, destination memory, and portal specification | Inspect and change destinations, see distinct bounded effects, and exercise local routes without service execution. |

The simulator establishes rendering and adapter behavior. It cannot establish
physical-device sensor quality or two-thumb comfort. No model or benchmark
runs are part of this work. A single combined TestFlight build follows the
ordered acceptance checks.
