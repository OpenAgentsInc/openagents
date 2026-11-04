# Sources of Aethyr's sounds

Every sound Aethyr plays in a realm is listed here with its source and licence.
Add a line when you add a sound.

## Everglade's effects (GitHub #12751)

Source: synthesized by Aethyr from the recipes in
[`sfx.coax`](sfx.coax) (`sfx_recipe`), with the oscillators, noise, envelopes
and filters of [`synth.coax`](synth.coax). No recording or sample library is
used. The page renders them when its audio starts, so none is downloaded.

Licence: CC0 1.0 (made by OpenAgents for this repository).

| Name | What it is | Played by |
| --- | --- | --- |
| `fire-cast`, `fire-travel`, `fire-hit` | Fire Bolt, Fireball's bead, Meteor Swarm, Produce Flame and the druid's fire spells | `everglade-sound.coax` `egs_wl_look`, `egs_sp_look`, `egs_ab_look` |
| `fireball` | Fireball's and each meteor's burst | the same |
| `frost-cast`, `frost-travel`, `frost-hit`, `ice` | Ray of Frost, Ice Knife, Ice Storm, Sleet Storm, Cone of Cold | the same |
| `eldritch-cast`, `eldritch-travel`, `eldritch-hit` | Eldritch Blast, a hit for each beam | the same |
| `necro-cast`, `necro-travel`, `necro-hit` | Chill Touch, Finger of Death, Circle of Death | the same |
| `bolt-cast`, `bolt-travel`, `thunder` | Lightning Bolt, the druid's bolts, Call Lightning | the same |
| `cone`, `hex`, `hold`, `rebuke`, `blight`, `teleport` | Burning Hands, Hex, Hold Person, Hellish Rebuke, Blight, Dimension Door | the same |
| `shield`, `heal`, `thunderwave`, `missile`, `levitate`, `splash` | wards, healing, Thunderwave, darts, Levitate and Feather Fall, the Water Orb and water spells | the same |
| `arcane-cast`, `arcane-hit`, `radiant` | every other spell's cast and impact | the same |
| `swing`, `hit`, `hit-heavy`, `death`, `stand` | a weapon's swing, a blow, a big blow, a fall, standing back up | `egs_swung`, `egs_blow`, `egs_watch`, `egs_others` |
| `horn`, `wall-hit`, `tower-hit`, `raid-won`, `raid-lost` | the Cultist Raid's start and climax, a broken wall or tower, its end | `egs_raid`, `egs_break` |
| `step-1`, `step-2`, `step-3` | footfalls on packed earth | `egs_frame`, `egs_others` |
| `ambient` | the town's bed: wind in the leaves and two birds, looped | `everglade_sound_lines` |

## Earlier sounds

- The haunted house's `haunt-*.mp3` sounds: CC0, listed in
  `apps/openagents-ios/OpenAgents/Resources/Haunt-NOTICE.txt`.
- Aethyr's `promo-epic` score ([`epic.coax`](epic.coax)): synthesized, CC0.
