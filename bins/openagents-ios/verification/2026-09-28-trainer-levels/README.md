# Trainer levels in the Grid and on the Account card

Captured 2026-09-28 on a cloned iPhone 17 Pro simulator, debug Rust
library, for [#9847](https://github.com/OpenAgentsInc/openagents/issues/9847).

| Capture | Launch arguments | What it shows |
| --- | --- | --- |
| `grid-level-tag-preview.png` | `--tab verse --xp-preview` | The Grid's name tag with the level, `c25458d5 · lv 3`, from the labeled fixture: six tutorial reproductions of 50 XP, derived and re-checked by `verse::xp::snapshot`. The world stays offline. |
| `trainer-card-preview.png` | `--tab account --account-route trainer --xp-preview` | The trainer card on the same fixture: level 3, 300 XP, 220 XP to level 4 under `trainer-curve-v1`, and the counted awards, with the preview labeled as not real awards. |
| `trainer-card-live.png` | `--tab account --account-route trainer` | The live card, read from `wss://relay.openagents.com` under the OpenAgents referee: level 1, no awards yet, and 17 open quests (11 TB4 `kb-transfer` quests and the 6 tutorial `reproduce` quests). |

No award exists on the relay yet, so the live captures show level 1; a level
over a head in the live Grid needs the first accepted reproduction.

## Trainer profile opt-in (#9895)

- `profile-shown-preview.png`: the labeled preview, whose fixture includes a
  shown profile: **Level over your head** says it's shown and offers
  **Hide my level**.
- `profile-not-shown-live.png`: a fresh world key on the live relay: no
  profile yet, so the Grid tag is the prefix alone and the screen offers
  **Show my level** (published only after a confirmation).
