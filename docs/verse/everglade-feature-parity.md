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
bar's boxes), `cast-pose.coax` (the cast clip and the bow's aim pose),
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
aim), `aethyr/action/bow-pose.coax` (the keyed draw pose) and `cast-pose.coax`
(`cp_bow_layer`, the pose over the upper body).

| Zone | Draw and arrows | Carry (the bow in hand or on the back) |
| --- | --- | --- |
| Everglade II | Shared: `E2Mouse` holds a `bow.BowDraw` (everglade2-mouse.coax:9); `e2m_draw` :26 and `e2m_loose` :36 wire it; `cast-pose.coax` `cp_bow_layer` is used at everglade2-mouse.coax:75. | Absent: no weapon is drawn on the body (cast-pose.coax:10). |
| Experimental | Shared: `xc_bow_begin` experimental-combat.coax:949, `xc_bow_loose` :1004 and `xc_bow_step` :1040 call `bow.coax`; arrows draw through `aethyr/fx/puff.coax` `fxp_arrow_at` (:1089). | Local: `held_weapon` charactercreator.coax:409, `bow_string` :441, `bow_basis` :503, `bow_matrix` :520. The draw pose inside `experimental_figure` (charactercreator.coax:368-373) repeats `cp_bow_layer`. No bow on the back. |
| Everglade v1 | Absent for players. NPC bows are local sticks (`rp_person_at` everglade-raid-preview.coax:325, `watch_missiles_draw` everglade-watch.coax:451). | Absent. |

### Damage numbers

No shared module exists yet. Its home is `everglade/hud/`, fed from the shared
presentation queue (`aethyr/action/react.coax`, #13072).

| Zone | Where |
| --- | --- |
| Everglade II | Local, twice: monsters (`E2mnMark` everglade2-monsters.coax:26, `e2mn_mark` :143, drawn in `e2mn_hud` :186) and the guard (`E2gdMark` everglade2-guard.coax:19, drawn in `e2gd_hud` :160). |
| Experimental | Local, three times: the 5E Lab (`HitGroup` fivelab.coax:355, `hit_groups` :360, drawn by `draw_fx` :2610), the combat (`XcFloat` experimental-combat.coax:76, `xc_float_from` :292, throttled by the shared `react.react_present`), and Character Create's `combat_hud` (charactercreator.coax:302) drawing `DamagePop` (verse/experimental-hostiles.coax:19). |
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
engine.coax:124, `rs_has`). How a zone shows them is not.

| Zone | Where |
| --- | --- |
| Everglade II | Shared rules (through `class-kit.coax` and `battle.coax`); local display: condition names as text in `e2k_health_text` everglade2-cast.coax:423, `E2K_CONDITION` :450, `E2K_EFFECT` :455. The room mirrors them for v1 in `rm_eg_cond` room-everglade.coax:1367. |
| Experimental | Shared rules; local display: `fx_lasting` fivelab.coax:654 maps conditions to its effect flags. |
| Everglade v1 | Local: its own status table (`StEffect` everglade-status.coax:22, `st_cond` :79, `st_tags_draw` :209) and wire bits (`WC_SLOWED`, `WC_BOUND`, `WC_HALTED` everglade-wire.coax:139-142) that mirror `RS_*` without loading the engine. |

### HUD parts

The shared modules are `aethyr/ui/` (HUD atlas boxes, settings, feed),
`verse/hud.coax` and `verse/hud/` (slot geometry and frames), `class-look.coax`
(`xc_slot_at`, the bar's boxes) and `class-menu.coax` (the CLASSES column).
`everglade/hud/` is arriving from the HUD work.

| Zone | Where |
| --- | --- |
| Everglade II | Local: the hotbar and health line `e2k_hud` everglade2-cast.coax:259, monster and guard plates `e2mn_hud` everglade2-monsters.coax:186 and `e2gd_hud` everglade2-guard.coax:160, `e2c_hud` everglade2-create.coax:417, `e2_quality_hud` everglade2.coax:785, `e2_light_hud` :1062. Shared slot layout and art. |
| Experimental | Local: the 5E Lab's `layout` fivelab.coax:2427, `draw_panel` :2802, `draw_player` :2971, `draw_top` :3069, `draw_sliders` :3409; the combat's `xc_slot_box` experimental-combat.coax:1686 and `xc_frame_box` :1992; Character Create's `dev_hud` charactercreator.coax:277, `combat_hud` :302 and `layout` :921. |
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
layer in `everglade/mounts.coax`. No zone has mounts today. The manifest lists
the feature as not required yet, so a zone may wire it when it lands, and any
zone-local mount code (`Mount`, `mount_up`, `Horse` and so on) fails the check.

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
mouselook, attention, the reticle), `aethyr/shot.coax` and
`aethyr/follow.coax`.

| Zone | Where |
| --- | --- |
| Everglade II | Shared orbit, modes and mouselook; a local follow layer on top: `e2_follow` everglade2.coax:668, `e2_shot` :623, `e2_ml_shot` :584, `e2_cam_ground` :617, and Character Create's `e2c_shot` everglade2-create.coax:671. |
| Experimental | Shared orbit in the 5E Lab (`follow_cam` fivelab.coax:2216 over `verse_orbit_*`), with local `cam_zoom` :3700, `cam_home` :3704, `cam_pan` :3714; the Experimental world computes its own orbit and boom in `camera` charactercreator.coax:1068 and `cc_camera_boom` :1054, and the walk in `walk_camera` charactercreator-walk.coax:209. |
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
  check landed (`feature path symbol`).
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
| Bow carry and draw | #13079 | 0 / 4 / 0 | Everglade v1 |
| Damage numbers | #13080 | 7 / 14 / 24 | Everglade II, Experimental, Everglade v1 |
| Area targeting | #13081 | 0 / 0 / 10 | |
| Conditions | #13082 | 7 / 1 / 15 | none |
| HUD parts | #13083 | 6 / 10 / 24 | none |
| Class bars | #13084 | 2 / 1 / 5 | Everglade v1 |
| Dash | #13085 | 0 / 2 / 0 | none |
| Name plates | #13088 | 0 / 0 / 14 | Everglade v1 |
| Spell effects | #13086 | 0 / 0 / 20 | none |
| Camera | #13087 | 8 / 9 / 5 | none |

The rows, by feature (`EF_KNOWN` holds the same list):

### Bow carry and draw (#13079)

4 zone-local copies.

- Experimental: `held_weapon` charactercreator.coax:409, `bow_string` charactercreator.coax:441, `bow_basis` charactercreator.coax:503, `bow_matrix` charactercreator.coax:520.

### Damage numbers (#13080)

45 zone-local copies.

- Everglade II: `E2GD_MARKS` everglade2-guard.coax:17, `E2GD_MARK_S` everglade2-guard.coax:17, `E2gdMark` everglade2-guard.coax:19, `E2MN_MARKS` everglade2-monsters.coax:17, `E2MN_MARK_S` everglade2-monsters.coax:17, `E2mnMark` everglade2-monsters.coax:26, `e2mn_mark` everglade2-monsters.coax:143.
- Experimental: `XC_FLOATS` experimental-combat.coax:71, `XcFloat` experimental-combat.coax:76, `xc_float` experimental-combat.coax:288, `xc_float_from` experimental-combat.coax:292, `xc_float_int` experimental-combat.coax:304, `xc_hit_mark` experimental-combat.coax:719, `HIT_GROUPS` fivelab.coax:354, `HIT_FOLD` fivelab.coax:354, `HIT_SHOW` fivelab.coax:354, `HitGroup` fivelab.coax:355, `hit_group_of` fivelab.coax:356, `hit_groups` fivelab.coax:360, `hit_shown` fivelab.coax:382, `DamagePop` verse/experimental-hostiles.coax:19.
- Everglade v1: `st_pops_frame` everglade-status.coax:139, `POP_LIFE` everglade-watch.coax:31, `POP_RISE` everglade-watch.coax:31, `WatchPop` everglade-watch.coax:34, `POP_DOWN` everglade-watch.coax:35, `POP_HELD` everglade-watch.coax:35, `POP_ROOTED` everglade-watch.coax:35, `POP_RESTRAINED` everglade-watch.coax:35, `POP_STUNNED` everglade-watch.coax:35, `POP_PRONE` everglade-watch.coax:35, `POP_FRIGHTENED` everglade-watch.coax:36, `POP_SLOWED` everglade-watch.coax:36, `POP_BLINDED` everglade-watch.coax:36, `POP_UP` everglade-watch.coax:36, `POP_HALTED` everglade-watch.coax:36, `POP_WORDS` everglade-watch.coax:37, `watch_pop_slot` everglade-watch.coax:583, `watch_pop_heal` everglade-watch.coax:592, `watch_pop_word` everglade-watch.coax:599, `watch_pop_rgb` everglade-watch.coax:607, `watch_pop` everglade-watch.coax:617, `watch_pop_head` everglade-watch.coax:628, `watch_pops_frame` everglade-watch.coax:634, `watch_pops_draw` everglade-watch.coax:656.

### Area targeting (#13081)

10 zone-local copies, all in the frozen Everglade v1.

- Everglade v1: `ab_aimed` everglade-abilities.coax:82, `ab_aim_ring` everglade-abilities.coax:1175, `ci_aim_ring` everglade-circle.coax:1064, `gv_aim_ring` everglade-grove.coax:819, `po_aim_ring` everglade-polar.coax:337, `sp_ring_r` everglade-spells.coax:620, `sp_aim` everglade-spells.coax:627, `sp_ring` everglade-spells.coax:1989, `st_aim_ring` everglade-storm.coax:529, `wd_aim_ring` everglade-wild.coax:828.

### Conditions (#13082)

23 zone-local copies.

- Everglade II: `e2k_health_text` everglade2-cast.coax:423, `E2K_CONDITION` everglade2-cast.coax:450, `E2K_EFFECTS` everglade2-cast.coax:453, `E2K_EFFECT_BIT` everglade2-cast.coax:454, `E2K_EFFECT` everglade2-cast.coax:455, `rm_eg_cond` coax/web/room/room-everglade.coax:1367, `rm_eg_cond_left` coax/web/room/room-everglade.coax:1370.
- Experimental: `fx_lasting` fivelab.coax:654.
- Everglade v1: `ab_rules_cond` everglade-abilities.coax:745, `net_cond` everglade-net.coax:1043, `StEffect` everglade-status.coax:22, `st_add` everglade-status.coax:26, `st_cond` everglade-status.coax:79, `st_word` everglade-status.coax:102, `st_tags_draw` everglade-status.coax:209, `st_bodies_draw` everglade-status.coax:248, `st_draw_on` everglade-status.coax:261, `everglade_room_folk_cond` everglade-watch.coax:1000, `everglade_room_cond_left` everglade-watch.coax:1457, `everglade_room_cond` everglade-watch.coax:1463, `WC_SLOWED` everglade-wire.coax:139, `WC_BOUND` everglade-wire.coax:140, `WC_HALTED` everglade-wire.coax:142.

### HUD parts (#13083)

40 zone-local copies.

- Everglade II: `e2k_hud` everglade2-cast.coax:259, `e2c_hud` everglade2-create.coax:417, `e2gd_hud` everglade2-guard.coax:160, `e2mn_hud` everglade2-monsters.coax:186, `e2_quality_hud` everglade2.coax:785, `e2_light_hud` everglade2.coax:1062.
- Experimental: `dev_hud` charactercreator.coax:277, `combat_hud` charactercreator.coax:302, `layout` charactercreator.coax:921, `xc_slot_box` experimental-combat.coax:1686, `xc_frame_box` experimental-combat.coax:1992, `layout` fivelab.coax:2427, `draw_panel` fivelab.coax:2802, `draw_player` fivelab.coax:2971, `draw_top` fivelab.coax:3069, `draw_sliders` fivelab.coax:3409.
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

22 zone-local copies.

- Everglade II: `e2c_shot` everglade2-create.coax:671, `e2c_lens` everglade2-create.coax:699, `E2_CAM_TRUNKS` everglade2.coax:56, `E2_LOOK_DRAG` everglade2.coax:57, `e2_ml_shot` everglade2.coax:584, `e2_cam_ground` everglade2.coax:617, `e2_shot` everglade2.coax:623, `e2_follow` everglade2.coax:668.
- Experimental: `walk_camera` charactercreator-walk.coax:209, `cc_camera_boom` charactercreator.coax:1054, `cc_camera_terrain_room` charactercreator.coax:1060, `camera` charactercreator.coax:1068, `LOOK_DRAG` fivelab.coax:1972, `follow_cam` fivelab.coax:2216, `cam_zoom` fivelab.coax:3700, `cam_home` fivelab.coax:3704, `cam_pan` fivelab.coax:3714.
- Everglade v1: `cz_frame_shot` everglade-create.coax:500, `everglade_place_camera` everglade-place.coax:63, `LOOK_DRAG` everglade.coax:374, `CAM_TRUNK_CLEAR` everglade.coax:3445, `camera_frame` everglade.coax:3547.
