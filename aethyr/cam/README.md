# Camera modes, attention and hand-back (#12994)

The camera reads finished poses only. These modules decide what it looks at and
how it moves between modes; a realm passes its own time, its ground and its
colliders.

| File | What |
| --- | --- |
| `modes.coax` | `CamMode` records (pitch range, ground clearance, aim ahead, press lift, shoulder, near/far, field of view, yaw rate) with parent inheritance by set mask (`cam_modes_resolve`) and volume/input selection (`cam_select`); a world-anchored ground ring (`CamGround`, 0.5 m cells) the realm fills from its own height function; the centred follow (`cam_centred`): the look up past the horizon, the eye held above the land with the view tilting on up, pulled in before body spheres; sight fade, a seeded shake that never moves the listener, and the listener pose |
| `attention.coax` | One answer to "what am I attending to": targets publish capabilities (lock-on, talk, look, action), generation, anchor, radius, validity and priority; a step ranks at most 32 by priority, angle-weighted distance and id inside range, cone, vertical band, filtered line of sight and an optional on-screen flag; a held target keeps through larger retention bands for `hold` ticks and `grace` ticks lost; switch, release, stale generation and despawn say why |
| `frame.coax` | Two-subject framing inside a normalized screen rectangle for a lens, nearest distance by bisection, yaw rate limit, and a declared fallback (the first subject alone at far) |
| `handback.coax` | A cinematic that starts on the live shot and hands back by one outcome (resume the saved orbit, reset behind the subject, retain the track's look), easing to the follow camera's own shot; cancel hands back from where it is; a teleport or lost target cuts with no sweep |

## Adoption

- **Everglade II** (`apps/openagents-web/everglade2.coax`): the follow camera's
  look up, ground clamp, press and trunk pull-in are `cam_centred` with
  Everglade II's mode (`e2_mode`); the former `e2_pitch`, `e2_ray`,
  `e2_ground_room`, `e2_eye_pitch`, `e2_press`, `e2_sphere_room` and `e2_shot`
  bodies are removed. Headless before/after on the same path: identical camera
  probes (room along the ray, eye height) after drags up and down, wasm frame
  p50 1.94 ms (main) and 1.88 ms (this), 60 FPS.
- **Everglade II Character Create** (`everglade2-create.coax`): the run is a
  cinematic (`cam_cut_*`): its end turns the orbit to the run's look (retain)
  and the hand-back eases from the run's framing, still on you, into the follow
  camera over 1.5 s.
- **Everglade Experimental** (`experimental-combat.coax`): a single-target save
  spell's monster is the attention query (action capability, 30 degree cone,
  its range).

`modes.coax` and `handback.coax` load `aethyr/camera.coax` and have no module of
their own, so they join the module that loads them (Everglade's figures load
them beside the orbit).

## Residual and not yet adopted

- Lock-on, talk and look-at consumers still to move onto `attention.coax`;
  `frame.coax` has no playable consumer yet.
- Sight fade and shake are checked here but no realm draws with them yet.

```sh
coax/bin/coax test aethyr/cam/modes.coax --library
coax/bin/coax test aethyr/cam/attention.coax --library
coax/bin/coax test aethyr/cam/frame.coax --library
coax/bin/coax test aethyr/cam/handback.coax --library
coax/bin/coax test --library apps/openagents-web/everglade2.coax
```
