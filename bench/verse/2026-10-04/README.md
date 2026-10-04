# Spell playgrounds

These native 2560×1440, 30 FPS recordings cover #10452–#10460. Each video is
at most 25 seconds and ends with a 0.25× replay restored from the live
checkpoint. The adjacent JSON records saves, damage, physical state, replay
identity, and linear and angular momentum residuals. The PNG shows a replay
frame.

The videos use the shared game-icons artwork, 4× multisampling, and checkpoint
revision v18. Each JSON file comes from the same captured run.

| Spell | Video | Evidence |
| --- | --- | --- |
| Telekinesis | [Video](spell-telekinesis.mp4) | [JSON](spell-telekinesis.json) |
| Wall of Stone | [Video](spell-wall-of-stone.mp4) | [JSON](spell-wall-of-stone.json) |
| Levitate | [Video](spell-levitate.mp4) | [JSON](spell-levitate.json) |
| Feather Fall | [Video](spell-feather-fall.mp4) | [JSON](spell-feather-fall.json) |
| Gust of Wind | [Video](spell-gust-of-wind.mp4) | [JSON](spell-gust-of-wind.json) |
| Wind Wall | [Video](spell-wind-wall.mp4) | [JSON](spell-wind-wall.json) |
| Black Tentacles | [Video](spell-black-tentacles.mp4) | [JSON](spell-black-tentacles.json) |
| Meteor Swarm | [Video](spell-meteor-swarm.mp4) | [JSON](spell-meteor-swarm.json) |
| Reverse Gravity | [Video](spell-reverse-gravity.mp4) | [JSON](spell-reverse-gravity.json) |

In the chamber, Shift+1 through Shift+9 cast these spells in table order.
Click a creature or loose prop to select it. T steers the Telekinesis hand
along the camera ray; R releases it. Page Up and Page Down change Levitate
altitude. G re-aims Gust of Wind, X attempts a tentacle escape, and C ends
concentration. Feather Fall shows a reaction prompt when a visible creature
falls within range. Meteor Swarm requires open sky and is refused indoors.

To record one scene, run `verse_play --original --spell-playground SPELL OUT.mp4`.
Use `all OUT_DIR` or a comma-separated spell list and `OUT_DIR` to record a set.
The renderer uses the procedural original pack and retained licensed character
assets; it reads no Blizzard installation.

Regenerate the JSON without GPU assets with
`cargo run -p verse-world --example spell_evidence -- OUT_DIR`.
