# Camera modes, attention and hand-back (#12994)

The camera reads finished poses only. These modules decide what it looks at and
how it moves between modes; a realm passes its own time, its ground and its
colliders.

| File | What |
| --- | --- |
| `modes.coax` | `CamMode` records (pitch range, ground clearance, aim ahead, press lift, shoulder, near/far, field of view, yaw rate) with parent inheritance by set mask (`cam_modes_resolve`) and volume/input selection (`cam_select`); a world-anchored ground ring (`CamGround`, 0.5 m cells) the realm fills from its own height function; the centred follow (`cam_centred`): the look up past the horizon, the eye held above the land with the view tilting on up, pulled in before body spheres; sight fade, a seeded shake that never moves the listener, and the listener pose |
| `attention.coax` | One answer to "what am I attending to": targets publish capabilities (lock-on, talk, look, action), generation, anchor, radius, validity and priority; a step ranks at most 32 by priority, angle-weighted distance and id inside range, cone, vertical band, filtered line of sight and an optional on-screen flag; a held target keeps through larger retention bands for `hold` ticks and `grace` ticks lost; switch, release, stale generation and despawn say why |
| `frame.coax` | Two-subject framing inside a normalized screen rectangle for a lens, nearest distance by bisection, yaw rate limit, and a declared fallback (the first subject alone at far) |
| `filters.coax` | The query classes each purpose selects from one admitted world (`aethyr/query-scene.coax`): solid blocks walking, the camera and sight; camera-only walls block only the camera; sight-only obstruction (hedges, reeds) only sight; water, decorative debris and the receiver's own colliders block none. `cam_filter`/`cam_select_world` give each purpose its colliders, `cam_wall_room` the camera's room along its line (the swept eye's first camera-class hit, added to `cam_centred`'s room), and attention takes the sight selection |
| `parity.coax` | Frame-rate parity: one recorded input played through `aethyr/loop.coax` at 30, 60 and 144 FPS and over uneven frames with a stall gives the same actor and attention state tick for tick, and evaluating the camera on each frame leaves the actor simulation digest unchanged |
| `handback.coax` | A cinematic that starts on the live shot and hands back by one outcome (resume the saved orbit, reset behind the subject, retain the track's look), easing to the follow camera's own shot; cancel hands back from where it is; a teleport or lost target cuts with no sweep |
| `eye.coax` | Checked head-to-eye sphere sweep over a semantic selection: body categories, roles and ignored self identities come from the owner; stale selections and query failures report their exact status, and an embedded start cannot claim a safe view |
| `semantic.coax` | Source-part adapters `cam_centred_selected` and `cam_shake_selected` for the existing follow/shot owner; collision tests the displaced shake segment, and the accepted eye displacement moves the aim while the listener retains the unshaken base |
| `track.coax` | A camera track any zone can load from a stage (#13107): keys of time, eye, look and easing as table rows (`cam_track_load`), flown through `cinematic.coax`'s Catmull-Rom at a clock the zone owns (`cam_track_at`, whole game steps by `cam_track_clock`), so a recording gives the same frames every run; a sky clock (`cam_track_day`) sweeps the day over the shot |

`attn_step_selected` consumes a sight selection from `query-scene.coax`. It
rechecks its epoch and referenced entities before running the same bounded
attention policy. `AttnState.query_status` distinguishes a query refusal from
ordinary hidden-target grace. A refused ray discards any prefix ranking and
clears the held target and runner; a later successful step clears the status.
The legacy `attn_step` still accepts already-selected `Colliders`.

`cam_eye_clip` permits a segment up to 200 m and a radius in (0, 2] m. It
sweeps with an additional 1 mm radius for rounding clearance. `safe=false`
means no view has qualified, including a head inside the clearance skin; a
consumer must cut or hold a separately qualified shot. An unsafe previous
shot is never implicitly blessed. Both adapters read finished world poses and
leave the selected geometry untouched. Callers retain immutable geometry and
selection scratch, choose body/sight filters independently and supply valid
owned mode/ground records. These typed APIs are not untrusted wire decoders.

The [#13026 query record](../../docs/verification/camera-13026-query/README.md)
qualifies this boundary in a numerical fixture: 264 production mutants killed,
native/Wasm and sanitized replay at four frame cadences, and an independent
state/clearance oracle. This does not yet establish a new playable consumer,
whole-mode/cinematic mutation coverage or complete camera frame-rate parity.

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
  its range). Your target (its Hit Points over the bar and its gold name) is a
  lock-on held by `attn_step` (`xc_lock_step`): set only by a strike, never
  acquired for you (acquire 0), kept within 24 m all round you for at least the
  hold, kept through a second of grace when it is out of reach, and dropped at
  once when it falls.

`modes.coax` and `handback.coax` load `aethyr/camera.coax` and have no module of
their own, so they join the module that loads them (Everglade's figures load
them beside the orbit).

## Residual and not yet adopted

- Talk and look-at consumers still to move onto `attention.coax` (no playable
  world has an NPC you talk to through a prompt yet); `frame.coax` has no
  playable consumer (Experimental's camera does not frame you with your target).
- No realm tags its colliders with the filter classes yet: Everglade II's camera
  room is its ground ring and trunk spheres, Experimental's meadow has no walls.
- Sight fade and shake are checked here but no realm draws with them yet.

```sh
coax/bin/coax test aethyr/cam/modes.coax --library
coax/bin/coax test aethyr/cam/attention.coax --library
coax/bin/coax test aethyr/cam/frame.coax --library
coax/bin/coax test aethyr/cam/handback.coax --library
coax/bin/coax test aethyr/cam/filters.coax --library
coax/bin/coax test aethyr/cam/parity.coax --library
coax/bin/coax test aethyr/cam/track.coax --library
coax/bin/coax test --library apps/openagents-web/everglade2.coax
```
