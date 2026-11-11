# Everglade feature parity

This page lists every gameplay and UI feature in each Everglade zone, says
where each one lives, and marks it as shared or as a zone-local copy. It also
describes the feature manifest and the parity check that keep zones from
growing new copies (GitHub #13075).

The owner's rule (2026-11-09): no one-off zone code. A feature is engine-wide
(`aethyr/`) or game-wide (the Everglade layer, `everglade/`), and a zone only
wires it in.

## The zones

| Zone | Its files | Entry units the check follows |
| --- | --- | --- |
| Everglade II (`everglade-ii`) | `apps/openagents-web/everglade2*.coax`, the room's `coax/web/room/room-e2-*.coax` and `room-everglade.coax` | `everglade2.coax`, `room-everglade.coax` |
| Experimental (`experimental`) | `apps/openagents-web/fivelab*.coax`, `charactercreator*.coax`, `experimental-*.coax`, and `verse/experimental-*.coax` | `fivelab.coax` (the module for both `/worlds/everglade-experimental` and the 5E Lab) |
| Everglade v1 (`everglade-v1`) | `apps/openagents-web/everglade.coax` and `everglade-*.coax` | `everglade.coax` |

Some files sit under one zone's paths but serve more than one zone:

- Everglade II loads Experimental's `charactercreator.coax` (as `cc`, for its
  text and font) and `fivelab-bodies.coax` (the figures).
- Everglade II and Experimental both load v1's `everglade-move-types.coax`,
  and Experimental's movement runs on v1's `everglade-step.coax`.

The game layer that the zones already share still lives in
`apps/openagents-web/`, outside every zone's paths: `class-kit.coax` (the class
kits and how each ability aims), `class-look.coax` (looks, colours and the
bar's boxes), `cast-pose.coax` (the cast clip),
`class-menu.coax` (the CLASSES column) and `fivebar.coax` (the hotbar on the
SRD engine). These files move to `everglade/` as the
[book's layout](../../everglade/book/README.md#where-everglades-code-lives)
describes.

## Inventory

Each row says where a zone gets the feature. **Shared** means the zone wires a
module outside its own paths. **Local** means the zone carries its own code;
the burn-down section below lists every such definition. **Absent** means the
zone doesn't have the feature. Paths are under `apps/openagents-web/` unless
they start with `aethyr/`, `verse/`, `coax/` or `everglade/`.

### Bow carry and draw

The shared modules are `aethyr/action/bow.coax` (the draw, the arrows and the
aim), `aethyr/action/archery.coax` (the archer's frame, the sight line, the
flight and what an arrow meets), `everglade/longbow.coax` (the Longbow's hand,
sounds and dust), `aethyr/action/bow-pose.coax` (the keyed draw pose) and
`everglade/held-kit.coax` (the bow in the hand, its string and arrow, and
`held_bow_layer`, the pose over the upper body; moved out of
`charactercreator.coax` for #13079). The Archery Range realm wires the same
three, as Experimental does.

| Zone | Draw and arrows | Carry (the bow in hand or on the back) |
| --- | --- | --- |
| Everglade II | Shared: `E2Mouse` holds a `bow.BowDraw` (everglade2-mouse.coax); `e2m_draw` and `e2m_loose` wire it; `everglade/held-kit.coax` `held_bow_layer` lays the pose (`e2m_figure`). | Shared: `e2_held` places the kit through `held_pieces` (everglade/held-kit.coax). |
| Experimental | Shared: `xc_bow_begin`, `xc_bow_loose` and `xc_bow_step` (everglade/zone-bow.coax) call `bow.coax`; arrows draw through `aethyr/fx/puff.coax` `fxp_arrow_at`. | Shared: `held_weapon`, `bow_string`, `bow_basis` and `bow_matrix` are `everglade/held-kit.coax`'s; `experimental_figure` calls its `held_bow_layer`. No bow on the back. |
| Everglade v1 | Absent for players. NPC bows are local sticks (`rp_person_at` everglade-raid-preview.coax:325, `watch_missiles_draw` everglade-watch.coax:451). | Absent. |

### Damage numbers

No shared module exists yet. Its home is `everglade/hud/`, fed from the shared
presentation queue (`aethyr/action/react.coax`, #13072).

| Zone | Where |
| --- | --- |
| Everglade II | Local, twice: monsters (`E2mnMark` everglade2-monsters.coax:26, `e2mn_mark` :143, drawn in `e2mn_hud` :186) and the guard (`E2gdMark` everglade2-guard.coax:19, drawn in `e2gd_hud` :160). |
| Experimental | Local, three times: the 5E Lab (`HitGroup` fivelab.coax:355, `hit_groups` :360, drawn by `draw_fx` :2610), the combat (`XcFloat` experimental-combat.coax:76, `xc_float_from` :292, throttled by the shared `react.react_present`), and Character Create's pops (`eg_pops` in everglade/hud/screens.coax) drawing `DamagePop` (verse/experimental-hostiles.coax:19). |
| Everglade v1 | Local: `WatchPop` everglade-watch.coax:34, `watch_pop` :617, `watch_pops_draw` :656; condition words through `st_pops_frame` everglade-status.coax:139. |

### Area targeting

The shared modules are `aethyr/action/area-aim.coax` (the SRD's areas as
shapes on the ground: a Sphere's circle, a Cube's square or box, a Cone and a
Line; placing one within range; its outline and guides; which creatures it
holds), `class-kit.coax` (`XC_AIM_AREA`, `xc_aim_for`, `xc_aim_point`, and
`xk_spell_area`, `xk_area`, `xk_zone_area`: the area each ability or battle
zone opens) and `aethyr/rules/srd/area-origin.coax` (whether an area includes
its caster).

| Zone | Where |
| --- | --- |
| Everglade II | Shared: Entangle's square is armed and placed through `area-aim.coax`; Thunderwave's cube shows ahead of you over its slot and where it opened after a cast; monsters inside the area light their plates (`e2k_area_marks`, `e2k_area_holds` everglade2-cast.coax). |
| Experimental | Shared: Thunderwave's cube is `xk_spell_area` drawn by `xc_area_draw` and resolved by `area_aim_contains`; the 5E Lab's `board_areas` outlines each battle zone through `xk_zone_area`. |
| Everglade v1 | Local: `sp_aim` everglade-spells.coax:627, `sp_ring` :1989, `ab_aim_ring` everglade-abilities.coax:1175, and one ring a spell family (`gv_aim_ring` everglade-grove.coax:819, `ci_aim_ring` everglade-circle.coax:1064, `po_aim_ring` everglade-polar.coax:337, `st_aim_ring` everglade-storm.coax:529, `wd_aim_ring` everglade-wild.coax:828). |

### Conditions

The SRD conditions are shared: `aethyr/rules/srd/` (`RS_CONDITIONS`
engine.coax:124, `rs_has`), and so is showing them: `aethyr/rules/srd/condition-display.coax`
(#13082) has their words (`cd_held_text`), their icons over a creature (`cd_icons`) and the
clock of an effect a page counts down (`CdClock`).

| Zone | Where |
| --- | --- |
| Everglade II | Shared rules and display: `e2k_vitals_text` everglade2-cast.coax says them with `cd_held_text`; the room keeps its effects' clocks as `CdClock` (`rm_eg_held_left` room-everglade.coax). |
| Experimental | Shared rules and display: `fx_marks` fivelab.coax takes its conditions' marks from `cd_icons`. |
| Everglade v1 | Local: its own status table (`StEffect` everglade-status.coax:22, `st_cond` :79, `st_tags_draw` :209) and wire bits (`WC_SLOWED`, `WC_BOUND`, `WC_HALTED` everglade-wire.coax:139-142) that mirror `RS_*` without loading the engine. |

### HUD parts

The shared modules are `aethyr/ui/` (HUD atlas boxes, settings, feed),
`verse/hud.coax` and `verse/hud/` (slot geometry and frames), `class-look.coax`
(`xc_slot_at`, the bar's boxes), `class-menu.coax` (the CLASSES column) and
`everglade/hud/`: `parts.coax` (the portrait plate, the bar's tray, the map,
plates, numbers), `class-bar.coax` (the side bar), `windows.coax` (the prompt
plaque, a character's talk, the Golemancer's Workbench, the marks over the bar's
slots and its card, the graphics control, a readout, the wedge toward a blow from
off the screen, the line said while you are down, a number risen off a body),
`create.coax` (Character Create's panel and the world's menu), `screens.coax`
(the animations list, the plain health line, the fallen words, damage pops,
Character Create's boxes, a framed box and prompt) and `lines.coax` (the 5E Lab's
line HUD: buttons, picks, hotbar slots, sliders, roster rows, the log, the frame
rate's corner and where the top buttons, the picker and the hotbar go).

| Zone | Where |
| --- | --- |
| Everglade II | Shared: `e2_overlay` wires `everglade/hud/` (`e2k_bar_draw`, `e2c_draw`, `e2p_overlay`, `e2b_overlay`, `e2mn_wedge`, `e2_gfx_control`, `e2_light_readout`, `e2_floats_draw`, `e2k_down_line`). |
| Experimental | Shared: the 5E Lab (`lab_layout`, `lab_panel_view`, `lab_player_bar`, `lab_top_bar`, `lab_sliders`) draws through `everglade/hud/lines.coax`; Character Create (`creator_layout`, `dev_panel`, `combat_overlay`) and the combat's slot boxes, frames and the ride prompt (`lab_ride_prompt`) through `everglade/hud/screens.coax`. |
| Everglade v1 | Local: `hud` everglade.coax:4084, the spell bar `spells_hud` everglade-spells.coax:2023 and `sp_bar` :2254, the generated icon table everglade-hotbar.coax, party frames `pt_draw` everglade-party.coax:222, and a HUD a subsystem (`watch_hud`, `st_hud`, `ege_hud`, `ega_hud` and more). Shared `verse/hud/frame.coax` for the forge panels only (everglade-forge.coax:20). |

### Class bars

The shared modules are `class-kit.coax` (`xk_bar_at`, `xk_entry_at`) and
`fivebar.coax` (`fb_ready`, `fb_left`, `fb_use`). No zone draws a class
resource bar (Rage, Focus, slots) beyond text.

| Zone | Where |
| --- | --- |
| Everglade II | Shared bar; the room's counts print in a slot's corner (everglade2-cast.coax:261-272). Local client record `E2nKit` everglade2-net.coax:911. |
| Experimental | The combat is shared (`fivebar.coax` through `class-kit.coax`). The 5E Lab copies the per-class lists in `abilities` fivelab.coax:1764 and shows resources as text in `draw_panel` :2802. |
| Everglade v1 | Local: everglade-classes.coax (`EgClass` :39, `cl_cast` :146, `cl_bar` :247): one ability slot a class. |

### Dash

The shared module is `aethyr/rules/srd/dash.coax` (`rs_dash_speed`), reached
through `battle.coax` (`rs_dash`, `rs_cunning_dash`) and `fivebar.coax`
(`FB_DASH`).

| Zone | Where |
| --- | --- |
| Everglade II | Shared: `everglade2-rules.coax:87-95` passes Dash to `fb_use`. Running is the walk toggle (`e2_pace` everglade2-move.coax:17). |
| Experimental | The combat is shared (`fb_seat_use` experimental-combat.coax:1502). The 5E Lab keeps its own kinds and dispatch: `AB_DASH`, `AB_CDASH` fivelab.coax:1756-1757, `player_turn` :1714. |
| Everglade v1 | Shared movement flag only: `eg_step_begin` everglade-step.coax:43 runs forward on link flag 512; the page never sets it. |

### Mounts

Mounts are arriving from another agent. The spec
([mounts-spec.md](../../everglade/docs/mounts-spec.md)) puts the engine in
`aethyr/action/mount.coax` and `aethyr/action/ride-pose.coax`, and the game
layer in `everglade/mounts.coax` (with the rider's keys) and
`everglade/zone-mounts.coax` (mounts standing in a zone: E, the saddle, the
prompt) and `everglade/zone-ride.coax` (a zone's keys handed to them, the
dragon's Fire Breath). Experimental and the Archery Range ride through them
(`bd_ride_posed` fivelab-bodies.coax poses the rider). The manifest
lists the feature as not required yet, and any zone-local mount code (`Mount`,
`mount_up`, `Horse`, a zone's own `ride_pose` and so on) fails the check. A
wrapper of the shared `ride_pose` and a wiring file named for its horse pass
(#13135).

### Name plates

The engine places plates in `aethyr/ui/hud-plates.coax`: a realm adds each
plate it wants a frame (a point over a head, a name, a health share and its own
fade, such as the body's sight share from `aethyr/ui/hud-sight.coax`), and the
module places them from the camera. It fades them with distance, keeps them out
of boxes the realm keeps clear, puts the ones always shown first and the rest
nearest first, and lifts a covered name clear or folds it into a count on the
name over it. A plate can set its own height, so a label of two lines (the
housewright showcase's signs in the 5E Lab) takes its full box. Everglade draws
placed plates in FANTASY with `eg_plates` in `everglade/hud/parts.coax`.

| Zone | Where |
| --- | --- |
| Everglade II | Wired: `e2_heads` everglade2.coax adds the others, the familiars, the monsters (`e2mn_plate_add`) and the guard (`e2gd_plate_add`) and draws them with `eg_plates`. |
| Experimental | Wired: `xc_frame` experimental-combat.coax adds the crates, the monsters and your familiar, and `lab_pack_titles` fivelab.coax the private packs' titles; both draw with `eg_plates`. The 5E Lab (`lab_heads3` fivelab.coax) places with the shared module and draws in its own strokes, because it keeps SCIFI's blend. |
| Everglade v1 | Local: `Label` and `labels` everglade.coax:3734, `name_point` :3817, other players' plates (`remote_plate_at` everglade-rangers.coax:525, `rangers_labels` :543), `room_labels` everglade-room.coax:109, and the moderator's `md_plate` everglade-moderate.coax:86. |

### Spell effects

The shared modules are `aethyr/fx/` (`puff.coax` recipes, `abilities.coax`;
`spell-light.coax`, each spell's colour; `battle.coax`, which effect each SRD
battle event starts; `flat.coax`, a cause drawn from above),
`class-look.coax` (`xc_look_of`, `xc_look_start`: which recipe a kit cast
starts) and `verse/everglade/gpu-fx.coax`.

| Zone | Where |
| --- | --- |
| Everglade II | Shared: `e2k_play` everglade2-cast.coax:84 and `e2k_step` :154 start and emit puffs through `class-look.coax` and `puff.coax`. |
| Experimental | Shared: the combat (`xc_look` experimental-combat.coax) through `class-look.coax`; the 5E Lab, its Siege and its Warlock hand each battle event to `fxe_event` (aethyr/fx/battle.coax) through `lab_effect` fivelab.coax, and the flat view draws the same causes with `fxf_of` (aethyr/fx/flat.coax). |
| Everglade v1 | Local: its own puff system in everglade-spells.coax (`SpPuff` :156, `sp_fx` :1097, `sp_glow` :1199) and net-drawn effects (`net_draw_*` everglade-net.coax:1766-1863). The Warlock uses `puff.coax` (everglade-warlock.coax:27). |

### Camera

The shared modules are `aethyr/camera.coax` (the orbit), `aethyr/cam/` (modes,
mouselook, attention, the reticle, first person, and the zones' follow layer:
`chase.coax`, `studio.coax`, `boom.coax` and `overhead.coax`), `aethyr/shot.coax`
and `aethyr/follow.coax`.

| Zone | Where |
| --- | --- |
| Everglade II | Shared: the follow step, ground ring, trunks and mouselook framing (`aethyr/cam/chase.coax`, wired in `e2_chase_step` everglade2.coax), and Character Create's studio, run and lens (`aethyr/cam/studio.coax`, wired in `e2c_view` and `e2c_view_lens` everglade2-create.coax). |
| Experimental | Shared: the 5E Lab's follow step (`chase.coax`, `lab_chase_step` fivelab.coax) and spectator camera (`aethyr/cam/overhead.coax`, `lab_view_zoom`, `lab_view_home`, `lab_view_pan`); the world's over-the-shoulder boom (`aethyr/cam/boom.coax`, `cc_view` charactercreator.coax), studio and walk (`studio.coax`, `cc_view` and `walk_view` charactercreator-walk.coax). |
| Everglade v1 | Shared orbit (`aethyr/camera.coax`, everglade.coax:62); local framing in `camera_frame` everglade.coax:3547, `cz_frame_shot` everglade-create.coax:500 and `everglade_place_camera` everglade-place.coax:63. |

## The manifest

`everglade/features.coax` is one table for the whole game:

- `EF_FEATURES`: each feature's name, whether every zone must wire it, its
  burn-down issue, the shared modules that own it (`owners`), and the names
  that mark a zone-local copy: definition marks (`marks`) and file-name marks
  (`files`). A mark is a substring of a top-level definition's name; a mark
  that starts with `=` matches the whole name only.
- `EF_ZONES`: each zone's path prefixes, its entry units, and the features it
  declares (`uses`).
- `EF_KNOWN`: the burn-down, one row a zone-local copy that existed when the
  check landed (`feature path symbol`), and the ten that landed on `main`
  before the check gated it (#13135).
- `EF_GAPS`: required features a zone doesn't wire yet (`zone feature`).

## The check

`coax/web/everglade-parity.coax` reads the manifest and every tracked `.coax`
file under the zones' paths. To run it from the repository root:

```sh
coax/web/tool.sh everglade-parity
```

It fails (exit 1) when:

- A zone defines a top-level name that carries a feature's mark, or has a file
  whose name carries one, outside that feature's owners, and `EF_KNOWN` has no
  row for it.
- A zone declares a feature in `uses` and none of the feature's owners is in
  the `#load` closure of its entry units.
- A zone doesn't declare a required feature and `EF_GAPS` has no row for it.

A burn-down row prints one warning line a zone and feature, with its issue, and
the check passes. A row whose copy is gone prints `burned down`, so the row can
be taken off. `coax/web/release.sh` runs the check as a report next to
`tools-check`; run it yourself before you push Everglade changes. To check the
tool's own logic:

```sh
coax/bin/coax test coax/web/everglade-parity.coax --library
```

The check matches names, not behaviour. A wiring procedure whose name happens
to carry a mark fails too; rename it, or narrow the mark in the manifest. Never
add an `EF_KNOWN` row for new code: wire the shared module instead.

### Add a feature

1. Put the implementation in `aethyr/` or `everglade/`.
2. Add its row to `EF_FEATURES` with its owners and the marks a copy would
   carry.
3. Wire it in each zone and add its name to each zone's `uses`. If a zone
   can't wire it yet, add an `EF_GAPS` row and file an issue.

### Burn down a copy

Replace the zone-local code with wiring of the shared module, delete its
`EF_KNOWN` rows, and run the check. When a feature's rows are all gone, close
its issue.

## Burn-down

Each issue below covers one cluster: the same feature implemented more than
once. Everglade v1 is frozen while Everglade is rebuilt; its rows burn down
when v1 wires the shared module or retires.

| Feature | Issue | Copies (II / Experimental / v1) | Gaps |
| --- | --- | --- | --- |
| Bow carry and draw | #13079 | 0 / 0 / 0 | Everglade v1 |
| Damage numbers | #13080 | 7 / 14 / 24 | Everglade II, Experimental, Everglade v1 |
| Area targeting | #13081 | 0 / 0 / 10 | |
| Conditions | #13082 | 0 / 0 / 15 | none |
| HUD parts | #13083 | 0 / 0 / 24 | none |
| Class bars | #13084 | 2 / 1 / 5 | Everglade v1 |
| Dash | #13085 | 0 / 2 / 0 | none |
| Name plates | #13088 | 0 / 0 / 14 | Everglade v1 |
| Spell effects | #13086 | 0 / 0 / 20 | none |
| Camera | #13087 | 8 / 9 / 5 | none |

The rows, by feature (`EF_KNOWN` holds the same list):

### Bow carry and draw (#13079)

No zone-local copies: the held kit moved to `everglade/held-kit.coax`.


### Damage numbers (#13080)

45 zone-local copies.

- Everglade II: `E2GD_MARKS` everglade2-guard.coax:17, `E2GD_MARK_S` everglade2-guard.coax:17, `E2gdMark` everglade2-guard.coax:19, `E2MN_MARKS` everglade2-monsters.coax:17, `E2MN_MARK_S` everglade2-monsters.coax:17, `E2mnMark` everglade2-monsters.coax:26, `e2mn_mark` everglade2-monsters.coax:143.
- Experimental: `XC_FLOATS` experimental-combat.coax:71, `XcFloat` experimental-combat.coax:76, `xc_float` experimental-combat.coax:288, `xc_float_from` experimental-combat.coax:292, `xc_float_int` experimental-combat.coax:304, `xc_hit_mark` experimental-combat.coax:719, `HIT_GROUPS` fivelab.coax:354, `HIT_FOLD` fivelab.coax:354, `HIT_SHOW` fivelab.coax:354, `HitGroup` fivelab.coax:355, `hit_group_of` fivelab.coax:356, `hit_groups` fivelab.coax:360, `hit_shown` fivelab.coax:382, `DamagePop` verse/experimental-hostiles.coax:19.
- Everglade v1: `st_pops_frame` everglade-status.coax:139, `POP_LIFE` everglade-watch.coax:31, `POP_RISE` everglade-watch.coax:31, `WatchPop` everglade-watch.coax:34, `POP_DOWN` everglade-watch.coax:35, `POP_HELD` everglade-watch.coax:35, `POP_ROOTED` everglade-watch.coax:35, `POP_RESTRAINED` everglade-watch.coax:35, `POP_STUNNED` everglade-watch.coax:35, `POP_PRONE` everglade-watch.coax:35, `POP_FRIGHTENED` everglade-watch.coax:36, `POP_SLOWED` everglade-watch.coax:36, `POP_BLINDED` everglade-watch.coax:36, `POP_UP` everglade-watch.coax:36, `POP_HALTED` everglade-watch.coax:36, `POP_WORDS` everglade-watch.coax:37, `watch_pop_slot` everglade-watch.coax:583, `watch_pop_heal` everglade-watch.coax:592, `watch_pop_word` everglade-watch.coax:599, `watch_pop_rgb` everglade-watch.coax:607, `watch_pop` everglade-watch.coax:617, `watch_pop_head` everglade-watch.coax:628, `watch_pops_frame` everglade-watch.coax:634, `watch_pops_draw` everglade-watch.coax:656.

### Area targeting (#13081)

10 zone-local copies, all in the frozen Everglade v1.

- Everglade v1: `ab_aimed` everglade-abilities.coax:82, `ab_aim_ring` everglade-abilities.coax:1175, `ci_aim_ring` everglade-circle.coax:1064, `gv_aim_ring` everglade-grove.coax:819, `po_aim_ring` everglade-polar.coax:337, `sp_ring_r` everglade-spells.coax:620, `sp_aim` everglade-spells.coax:627, `sp_ring` everglade-spells.coax:1989, `st_aim_ring` everglade-storm.coax:529, `wd_aim_ring` everglade-wild.coax:828.

### Conditions (#13082)

15 zone-local copies, all in the frozen Everglade v1. Everglade II and Experimental wire
`aethyr/rules/srd/condition-display.coax`.

- Everglade v1: `ab_rules_cond` everglade-abilities.coax:745, `net_cond` everglade-net.coax:1043, `StEffect` everglade-status.coax:22, `st_add` everglade-status.coax:26, `st_cond` everglade-status.coax:79, `st_word` everglade-status.coax:102, `st_tags_draw` everglade-status.coax:209, `st_bodies_draw` everglade-status.coax:248, `st_draw_on` everglade-status.coax:261, `everglade_room_folk_cond` everglade-watch.coax:1000, `everglade_room_cond_left` everglade-watch.coax:1457, `everglade_room_cond` everglade-watch.coax:1463, `WC_SLOWED` everglade-wire.coax:139, `WC_BOUND` everglade-wire.coax:140, `WC_HALTED` everglade-wire.coax:142.

### HUD parts (#13083)

24 zone-local copies, all in the frozen Everglade v1. Everglade II and Experimental wire
`everglade/hud/`.

- Everglade v1: `ega_hud` everglade-access.coax:398, `ega_test_hud` everglade-access.coax:708, `cz_hud` everglade-create.coax:328, `everglade_edit_hud` everglade-edit.coax:348, `ege_hud` everglade-events.coax:553, `forge_hud` everglade-forge.coax:179, `fork_hud` everglade-fork.coax:327, `everglade_guide_hud` everglade-guide.coax:106, `eg_first_hud` everglade-guide.coax:187, `eg_raid_arrival_hud` everglade-guide.coax:245, `everglade-hotbar.coax` (file), `join_hud` everglade-join.coax:311, `pt_draw` everglade-party.coax:222, `eg_quality_hud` everglade-quality.coax:62, `rp_hud_off` everglade-raid-preview.coax:140, `egs_hud` everglade-sound.coax:507, `spells_hud` everglade-spells.coax:2023, `sp_bar` everglade-spells.coax:2254, `sp_bar_desk` everglade-spells.coax:2313, `sp_bar_card` everglade-spells.coax:2348, `st_hud` everglade-storm.coax:549, `everglade_voice_hud` everglade-voice.coax:1032, `watch_hud` everglade-watch.coax:692, `hud` everglade.coax:4084.

### Class bars (#13084)

8 zone-local copies.

- Everglade II: `E2nKit` everglade2-net.coax:911, `e2n_kit` everglade2-net.coax:912.
- Experimental: `abilities` fivelab.coax:1764.
- Everglade v1: `everglade-classes.coax` (file), `CL_ABILITY` everglade-classes.coax:30, `EgClass` everglade-classes.coax:39, `cl_cast` everglade-classes.coax:146, `cl_bar` everglade-classes.coax:247.

### Dash (#13085)

2 zone-local copies.

- Experimental: `AB_DASH` fivelab.coax:1756, `AB_CDASH` fivelab.coax:1757.

### Name plates (#13088)

14 zone-local copies.

- Everglade v1: `md_plates_clear` everglade-moderate.coax:84, `md_plate` everglade-moderate.coax:86, `rp_labels` everglade-raid-preview.coax:541, `REMOTE_PLATE_NEAR` everglade-rangers.coax:53, `REMOTE_PLATE_FADE` everglade-rangers.coax:54, `remote_plate_at` everglade-rangers.coax:525, `remote_plate_for` everglade-rangers.coax:532, `remote_plate_shown` everglade-rangers.coax:536, `rangers_labels` everglade-rangers.coax:543, `room_labels` everglade-room.coax:109, `Label` everglade.coax:3733, `labels` everglade.coax:3734, `name_point` everglade.coax:3817, `name_hidden` everglade.coax:3827.

### Spell effects (#13086)

20 zone-local copies, all in the frozen Everglade v1.

- Everglade v1: `net_draw_pack` everglade-net.coax:1767, `net_draw_stone` everglade-net.coax:1788, `net_draw_tentacles` everglade-net.coax:1808, `net_draw_thorns` everglade-net.coax:1833, `net_draw_zone` everglade-net.coax:1846, `net_draw_line` everglade-net.coax:1864, `FX_FIRE` everglade-spells.coax:151, `SpMeteor` everglade-spells.coax:154, `SpPuff` everglade-spells.coax:157, `sp_bolt_jag` everglade-spells.coax:810, `sp_fx` everglade-spells.coax:1143, `sp_fx_key` everglade-spells.coax:1153, `sp_fx_keyed` everglade-spells.coax:1157, `sp_fx_id` everglade-spells.coax:1171, `sp_fx_close` everglade-spells.coax:1173, `sp_fx_thin` everglade-spells.coax:1201, `sp_fx_bucket` everglade-spells.coax:1241, `sp_glow` everglade-spells.coax:1245, `sp_shards` everglade-spells.coax:1616, `wl_draw` everglade-warlock.coax:409.

### Camera (#13087)

5 zone-local copies, all in Everglade v1, which is frozen. Everglade II and
Experimental wire the shared modules.

- Everglade v1: `cz_frame_shot` everglade-create.coax:500, `everglade_place_camera` everglade-place.coax:63, `LOOK_DRAG` everglade.coax:374, `CAM_TRUNK_CLEAR` everglade.coax:3445, `camera_frame` everglade.coax:3547.
