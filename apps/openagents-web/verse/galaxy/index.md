# Galaxy

The Verse playtest's galaxy at https://openagents.com/verse/galaxy, after the landing ([/verse](https://openagents.com/verse)).

The playtest's worlds turn on their orbits around OpenAgents. The **Haunted House** (the orange one, with its CLICK TO ENTER tag) goes to [/realms/haunted-house](https://openagents.com/realms/haunted-house); The Grid, Everglade, Dead Gears, the Sandbox and Ice Cream Man open their realms too. Coding is coming later, and Blue Rush and Big Brain are locked.

Players' own realms circle on the outer orbit, the newest eight, tinted by their kind (green forest, gold village, violet cemetery). Each one opens its own world: /realms/haunted-house#planet-… (see the realm page for how its owner publishes edits and accepts or declines changes others offer).

**Make your realm**, under GALAXY, sets one up: a name, its kind, look, view and time of day, then **Create** (or Enter). A browser with no player key gets one first and keeps it; the realm is made on the server (`POST /api/playtest/planets`) and its world opens. Back on the galaxy the button reads **Your realm** and reopens it.

- Computer: drag to turn the galaxy; point at a world to see its name; click the Haunted House, or its tag, to enter.
- Phone: drag to turn it; the first tap on a world names it, the second opens it.

What you pick is recorded with an anonymous id (`map`, `planet-click` with the world and whether it opened, `planet-setup` and `planet-create` with the choices).

Drawn by Coax with WebGPU (`apps/openagents-web/verse.coax`, page 1).
