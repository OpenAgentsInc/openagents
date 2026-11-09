# Contents

- [Introduction](README.md)
- [Plan and prerequisites](PLAN.md)

**I. Foundations**

- [1. A frame has a fixed step](ch001-a-frame-has-a-fixed-step.md)
- [2. Spaces and units](ch002-spaces-and-units.md)
- [3. State starts from its defaults](ch003-state-starts-from-its-defaults.md)
- [4. Input becomes typed actions](ch004-input-becomes-typed-actions.md)
- [5. Replay a session exactly](ch005-replay-a-session-exactly.md)

**II. A world**

- [6. Entities are born and die](ch006-entities-are-born-and-die.md)
- [7. Ask the world where things are](ch007-ask-the-world-where-things-are.md)
- [8. Ground and collision](ch008-ground-and-collision.md)
- [9. A character moves](ch009-a-character-moves.md)
- [10. The camera follows](ch010-the-camera-follows.md)

**III. Seeing it**

- [11. The WGSL renderer](ch011-the-wgsl-renderer.md)
- [12. Materials](ch012-materials.md) — partly implemented
- [13. Light and shadow](ch013-light-and-shadow.md) — partly implemented
- [14. Figures and animation](ch014-figures-and-animation.md) — partly implemented
- [15. UI and the HUD](ch015-ui-and-the-hud.md) — partly implemented
- [16. Budgets that refuse](ch016-budgets-that-refuse.md) — partly implemented

**IV. Physics**

- [17. A scene owns the state](ch017-a-scene-owns-the-state.md) — partly implemented
- [18. Rigid bodies and time steps](ch018-rigid-bodies-and-time-steps.md) — partly implemented
- [19. Joints in generalized coordinates](ch019-joints-in-generalized-coordinates.md) — design draft
- [20. Find the pairs that might touch](ch020-find-the-pairs-that-might-touch.md) — partly implemented
- [21. Find where they touch](ch021-find-where-they-touch.md) — partly implemented
- [22. Contact as an optimization](ch022-contact-as-an-optimization.md) — design draft
- [23. Particles and their neighbors](ch023-particles-and-their-neighbors.md) — partly implemented
- [24. Liquids from particles](ch024-liquids-from-particles.md) — partly implemented
- [25. Cloth and rope from constraints](ch025-cloth-and-rope-from-constraints.md) — partly implemented
- [26. Sand, snow and clay on a grid](ch026-sand-snow-and-clay-on-a-grid.md) — partly implemented
- [27. Elastic solids on a mesh](ch027-elastic-solids-on-a-mesh.md) — design draft
- [28. Smoke on a grid](ch028-smoke-on-a-grid.md) — partly implemented
- [29. Materials meet](ch029-materials-meet.md) — design draft
- [30. Contact that holds](ch030-contact-that-holds.md) — design draft
- [31. Many worlds in one step](ch031-many-worlds-in-one-step.md) — design draft
- [32. Differentiate a simulation](ch032-differentiate-a-simulation.md) — design draft
- [33. Sensors read the simulation](ch033-sensors-read-the-simulation.md) — design draft
- [34. Pictures for policies](ch034-pictures-for-policies.md) — design draft
- [35. Bring in robots and meshes](ch035-bring-in-robots-and-meshes.md) — design draft
- [36. Record and replay a world](ch036-record-and-replay-a-world.md) — design draft
- [37. Measure the engine honestly](ch037-measure-the-engine-honestly.md) — design draft

**V. Big worlds**

- [38. Stream a large world](ch038-stream-a-large-world.md) — design draft
- [39. Ephemerides, time scales and frames](ch039-ephemerides-time-scales-and-frames.md) — partly implemented
- [40. Propagate with a stated error](ch040-propagate-with-a-stated-error.md) — design draft
- [41. Burns consume mass](ch041-burns-consume-mass.md) — design draft
- [42. Reach a moving target](ch042-reach-a-moving-target.md) — design draft
- [43. Heat has to go somewhere](ch043-heat-has-to-go-somewhere.md) — design draft
- [44. Inside a turning, accelerating ship](ch044-inside-a-turning-accelerating-ship.md) — design draft
- [45. Sensors that see the past](ch045-sensors-that-see-the-past.md) — design draft
- [46. Time warp that stops for every event](ch046-time-warp-that-stops-for-every-event.md) — design draft
- [47. Hits through layers](ch047-hits-through-layers.md) — design draft
- [48. One world at three fidelities](ch048-one-world-at-three-fidelities.md) — design draft
- [49. Rooms and presence](ch049-rooms-and-presence.md) — partly implemented
- [50. Lockstep for small deterministic games](ch050-shared-state-over-the-network.md) — partly implemented

**VI. Making worlds: Aethyrforge**

- [51. A realm is a document](ch051-a-realm-is-a-document.md) — partly implemented
- [52. Edit a realm in place](ch052-edit-a-realm-in-place.md) — partly implemented
- [53. Bring in assets](ch053-bring-in-assets.md) — partly implemented
- [54. Publish a realm](ch054-publish-a-realm.md) — partly implemented
- [55. Fork a realm](ch055-fork-a-realm.md) — partly implemented

**VII. Realms**

- [56. The Grid](ch056-the-grid.md) — partly implemented
- [57. Made realms](ch057-made-realms.md) — design draft
- [58. Everglade](ch058-everglade.md) — partly implemented
- [59. Dead Gears](ch059-dead-gears.md) — design draft
- [60. Ice Cream Man](ch060-ice-cream-man.md) — design draft
- [61. Hecate's Cemetery](ch061-hecates-cemetery.md) — design draft
- [62. Fairy Forest](ch062-fairy-forest.md) — design draft
- [63. The space games](ch063-the-space-games.md) — design draft
- [64. Every realm runs on Aethyr](ch064-every-realm-runs-on-aethyr.md) — partly implemented

**VIII. Hearing it**

- [65. Sound from numbers: oscillators and envelopes](ch065-sound-from-numbers.md)
- [66. A sequencer on the beat grid](ch066-a-sequencer-on-the-beat-grid.md)
- [67. Scores, mixing and a soundtrack](ch067-scores-mixing-and-a-soundtrack.md)
- [68. Sound in the world: playback, space and reaction](ch068-sound-in-the-world.md) — partly implemented
- [69. Voices in a realm](ch069-voices-in-a-realm.md) — partly implemented

**IX. A living world**

- [70. Rivers, ponds and the sea](ch070-rivers-ponds-and-the-sea.md) — design draft
- [71. Sky, weather and the time of day](ch071-sky-weather-and-the-time-of-day.md) — partly implemented
- [72. Effects made of particles](ch072-effects-made-of-particles.md) — partly implemented
- [73. Things that break](ch073-things-that-break.md) — design draft
- [74. Wheels on the ground](ch074-wheels-on-the-ground.md) — design draft
- [75. Creatures that follow and watch](ch075-creatures-that-follow-and-watch.md) — partly implemented
- [76. Find a way through](ch076-find-a-way-through.md) — partly implemented
- [77. People who talk back](ch077-people-who-talk-back.md) — partly implemented
- [78. Cinematics and close-ups](ch078-cinematics-and-close-ups.md) — partly implemented

**X. Aethyrforge: generating worlds**

- [79. A piece from a sentence](ch079-a-piece-from-a-sentence.md) — design draft
- [80. Terrain and scatter from a seed](ch080-terrain-and-scatter-from-a-seed.md) — partly implemented

**XI. Shipping realms**

- [81. Save and come back](ch081-save-and-come-back.md) — partly implemented
- [82. Change code while it runs](ch082-change-code-while-it-runs.md) — design draft
- [83. Inspect and profile a running realm](ch083-inspect-and-profile-a-running-realm.md) — partly implemented
- [84. One realm on every host](ch084-one-realm-on-every-host.md) — partly implemented
- [85. Everyone can play](ch085-everyone-can-play.md) — partly implemented

**XII. Many players: a multiplayer engine**

- [86. One server owns the scene](ch086-one-server-owns-the-scene.md) — partly implemented
- [87. Server and client share one step and one world](ch087-server-and-client-share-one-step.md) — partly implemented
- [88. A seat joins with a signed ticket](ch088-a-seat-joins-with-a-signed-ticket.md) — partly implemented
- [89. Intents in, acknowledgements out](ch089-intents-in-acknowledgements-out.md) — partly implemented
- [90. Predict your own movement](ch090-predict-your-own-movement.md) — partly implemented
- [91. One record for every replicated entity](ch091-one-record-for-every-replicated-entity.md) — partly implemented
- [92. Priority under a budget](ch092-priority-under-a-budget.md) — partly implemented
- [93. One interpolation buffer for every remote entity](ch093-one-interpolation-buffer.md) — partly implemented
- [94. Effects are state, events are cues](ch094-effects-are-state-events-are-cues.md) — partly implemented
- [95. Offline is a local server](ch095-offline-is-a-local-server.md) — partly implemented
- [96. Casts and combat under latency](ch096-casts-and-combat-under-latency.md) — partly implemented
- [97. Interest without leaks](ch097-interest-without-leaks.md) — partly implemented
- [98. Versions, deploys and reconnect](ch098-versions-deploys-and-reconnect.md) — partly implemented
- [99. Watch the whole action path](ch099-watch-the-whole-action-path.md) — partly implemented
- [100. Prove the room plays like the game](ch100-prove-the-room-plays-like-the-game.md) — partly implemented
- [101. The Everglade room](ch101-the-everglade-room.md) — partly implemented

**XIII. Actions and conditions**

- [102. Ask, act, present](ch102-ask-act-present.md) — partly implemented

**Appendices**

- [A. Everglade rooms against an MMO engine](appendix-mmo-gap-analysis.md)
