# The old set

These pages are not part of what is live. The default build skips them (#13269);
a release builds them (`COAXIAL_PAGES=all`), and `coax/remote/check.sh module NAME`
builds one.

- `v1/`: v1 Everglade (`everglade`), 5E MMO (`fivemmo`) and Everglade 5E (`everglade5e`).
  The entries `everglade.coax` and `everglade5e.coax` and their faces sit one folder up.
- `experimental/`: Everglade Experimental and the 5E Lab (`fivelab`).
- `labs/`: the lab pages and two engine fixtures (`cullstream`, `state-lab`).
  `waterlab.coax` and its face sit one folder up. The Nostr Lab and Anime Lab are still there too.

## What only v1 loads

Of every page module, only v1's three (`everglade`, `fivemmo`, `everglade5e`) reach these
115 files. No live page, no other old page and nothing in the room or the client loads them.
They are deleted when v1 is (the AAA audit's "What to delete", after P2). Some have checks or tools
of their own that load them; those go with them or move first.

- `apps/openagents-web/`: `everglade-forge-npcs.coax`, `everglade-forge.coax`, `everglade.coax`, `everglade5e.coax`
- `apps/openagents-web/old/v1/`: `everglade-abilities.coax`, `everglade-access.coax`, `everglade-arrange.coax`, `everglade-audit.coax`, `everglade-cast-rules.coax`, `everglade-cast-sight.coax`, `everglade-character.coax`, `everglade-circle.coax`, `everglade-classes.coax`, `everglade-create.coax`, `everglade-doors.coax`, `everglade-dress.coax`, `everglade-edit.coax`, `everglade-events.coax`, `everglade-fire.coax`, `everglade-forest.coax`, `everglade-fork.coax`, `everglade-frost.coax`, `everglade-glade.coax`, `everglade-grove.coax`, `everglade-guard-ai.coax`, `everglade-guide.coax`, `everglade-hotbar.coax`, `everglade-interiors.coax`, `everglade-items.coax`, `everglade-join.coax`, `everglade-link.coax`, `everglade-lod.coax`, `everglade-marks.coax`, `everglade-moderate.coax`, `everglade-move.coax`, `everglade-net.coax`, `everglade-party.coax`, `everglade-place.coax`, `everglade-placed-fire.coax`, `everglade-polar.coax`, `everglade-quality.coax`, `everglade-raid-preview.coax`, `everglade-raid-scale.coax`, `everglade-raid-tactics.coax`, `everglade-rangers.coax`, `everglade-repair.coax`, `everglade-rest.coax`, `everglade-room-abilities.coax`, `everglade-room-effects.coax`, `everglade-room.coax`, `everglade-rubble.coax`, `everglade-sky.coax`, `everglade-sound.coax`, `everglade-spells.coax`, `everglade-stand.coax`, `everglade-status.coax`, `everglade-storm.coax`, `everglade-tide.coax`, `everglade-track.coax`, `everglade-voice.coax`, `everglade-warlock.coax`, `everglade-watch.coax`, `everglade-wild.coax`, `everglade5e-beasts.coax`, `everglade5e-rules.coax`, `fivemmo-battle.coax`, `fivemmo.coax`
- `everglade/legacy/v1/`: `building-catalogue.coax`, `captain.coax`, `folk.coax`, `glade-zone.coax`, `people.coax`, `person-names.coax`, `plan.coax`, `quests.coax`, `ranger.coax`, `river.coax`, `routines.coax`, `rumours.coax`, `srd5-combat.coax`, `srd5.coax`, `watch.coax`, `water-spells.coax`
- `everglade/npc/`: `guard-decision.coax`, `guard-fixtures.coax`, `guard-guide-check-fixtures.coax`, `guard-guide-game-check-fixtures.coax`, `guard-guide-game-types.coax`, `guard-guide-game.coax`, `guard-guide-policy.coax`, `guard-guide-session-check-fixtures.coax`, `guard-guide-session-types.coax`, `guard-guide-session.coax`, `guard-guide-types.coax`, `guard-guide.coax`, `guard-metrics.coax`, `guard-replay.coax`, `guard-types.coax`, `guide-route.coax`, `people-words.coax`
- `everglade/world/`: `edits.coax`
- `metaverse-engine/anim/`: `flock.coax`
- `metaverse-engine/audio/`: `space.coax`
- `metaverse-engine/cam/`: `flyover.coax`
- `metaverse-engine/core/`: `decision-trace.coax`, `replay-wire.coax`
- `metaverse-engine/render/`: `grass-root.coax`
- `metaverse-engine/render/tests/`: `grass-root-oracle.coax`
- `metaverse-engine/session/`: `effect-reservation.coax`
- `metaverse-engine/ui/`: `settings.coax`
- `metaverse/`: `demolish.coax`, `water-look.coax`, `water-wade.coax`
- `metaverse/valley/`: `echo-orbit.coax`
- `psionic/lib/game/`: `decision.coax`
