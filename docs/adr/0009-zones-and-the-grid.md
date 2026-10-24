# 0009: Zones, and the Grid

Status: Accepted (owner, 2026-10-24: "the grid is definitely going to be an
important zone, and we're going to keep that").

## Decision

- **Apps are shells that open zones.** A zone is a versioned release: a
  manifest whose ID is the SHA-256 of its bytes. It lists scene and asset
  data, compiled shaders, UI screens in the view-tree schema
  ([0005](0005-effect-controls-the-ui.md)), interpreted behavior code, and its
  detail knobs and floors ([0007](0007-capability-and-detail.md)). Releases
  are immutable. Channels such as "current" move by compare-and-swap.
- **A zone never holds the shell's credentials.** It asks the shell for a
  capability, such as "open sign-in" or "show the payment sheet for this
  item", and the shell shows UI the zone cannot draw over. On the web this
  boundary is the `aethyrforge-host` protocol between origins.
- **Zones ship data, shaders, and interpreted code only.** They never ship
  native code. App store rules allow this; native capability lives in the
  shell and updates through the stores.
- **The first zones:** the Haunted House (Kiki's tutorial world), the Verse
  universe and personal planets, and **the Grid**.
- **The Grid** is the zone for coding agents: sessions, computers,
  transcripts, and approvals, the work the OpenAgents mobile and desktop apps
  already do. It is first-party and built into the shell, so it works
  offline and passes store review as an ordinary app feature. It uses the
  same zone interfaces as downloaded zones.

## Consequences

The current Verse tab's bare world is also labeled "The Grid"
([Verse on mobile](../verse/mobile.md)). The name now belongs to the
coding-agent zone. What the bare world is called is listed in the
[merge plan](../aethyr/merge.md#decisions-for-the-owner).
