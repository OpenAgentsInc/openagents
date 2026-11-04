# External combat control v4

Identity: `starter-warlock-v4`, `openagents.engine.combat.control.v4`.
Composes unchanged combat observation v1 with loot v1 and evidence v2.
The attack policy and qualified level 2–5 spell layout remain those of v3.

After an XP-verified player kill, retain its NPC/spawn identity and complete
the bounded loot phase before selecting another creature. F6 interacts with
the selected corpse; F7 can restore the previous target once, followed by a
fresh identity/dead-state check. Autoloot must be enabled, F6/F7 must have
the expected ordinary bindings, and persistent `autoInteract` must be off.
No protected actions run in the sensor addon.

Approach pulses are 200 ms, taps 40 ms, with fresh public-state rechecks.
The corpse phase ends within 12 seconds, 24 effects or 400 normalized map
units from its start. A matching newly opened/closed loot session plus an
item event or increased money is a receipt. Empty loot, full inventory,
timeout, lost identity and refusal are separate outcomes. Zero free slots
is a warning; it does not imply stacks or coins cannot be collected.
No items are deleted, sold or mailed. An owned loot window may be closed
with Escape; unrelated menus stop admission.

`--min-level N` rejects lower-level targets before a pull. `--home-x X
--home-y Y` explicitly declares a new local survey anchor on map 1426;
the same 600-unit circuit and 1800-unit fence apply. A requested waypoint
does not assert a traversable route. `--until-level 6` stops new pulls at
the observed level and drains the pending corpse. It does not extend the
combat profile to level 6 or assume newly learned spells.
`--hostile-only` restricts visual acquisition to red nameplates, avoiding
repeated selection of neutral low-level animals in a hostile hunting patch.
It never bypasses eligibility or defensive-combat admission.
Ambient combat can precede WoW's automatic target by one sample. V4 allows
up to three seconds of observation-only waiting while alive, healthy and
without a target; once the attacker is selected, normal defensive admission
applies. It does not start a blind attack.

Each kill records measured XP, an XP≤20 flag, an easy-kill flag (at least
three levels below, fight≤10 seconds, no net health loss), and a new-area
suggestion when every kill in a sample of at least three is low XP.
These are recorded heuristics, not optimal leveling claims. Minimum level,
home, goal and budgets are explicit run configuration, not model decisions.
Survey steering still reuses Aethyr navigation; original local Elephant
remains available through the separate qualified waypoint controller.
