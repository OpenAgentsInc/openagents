# External loot v1

Identity: `external-loot-v1`, `openagents.engine.combat.loot.v1`.
The published combat/navigation strips remain unchanged. A third read-only
strip is 72 bytes in 24 RGB cells, nominal Y=24, with CRC32 over bytes 0–67.
The SDK never targets, interacts, moves or loots. Ordinary game input performs
those effects; automatic loot is the game's own setting.

0..2: 86,76,1; 3 serial u16; 5 flags (valid=1, loot session=2,
autoLootDefault=4, selected dead=8, selected exists=16, CanLootUnit known=32,
selected lootable=64, trainer shown=128). 6 general free slots u16;
8 total slots u16; 10 current loot slots u16; 12/14 opened/closed counters
u16; 16 item-message counter u16; 18 inventory-full counter u16;
20 copper u32; 24/26 UI-error counter/type u16; 28/32 loot NPC/spawn u32;
36 quantity u16; 38 item ID u32; 42 bag-update counter u16;
44 loot-range-error counter u16; 46 other-loot-error counter u16;
48 slots observed on opening u16; 50 free general slots on opening u16;
52 selected target epoch u16; 54 flags (F6 interact=1, F7 last target=2,
autoInteract=4, game menu=8, chat=16, settings=32, merchant=64, dead player=128);
55 source kind (0 unknown, 1 GetLootSourceInfo, 2 selected-dead fallback);
56 event client milliseconds u32; 60 copper at opening u32;
64 known spell mask (172=1,348=2,1454=4,688=8) u32; 68 CRC32 u32.
All integers are big endian and saturate to field width.

A verified XP kill starts a bounded corpse phase. The corpse's NPC/spawn
and target epoch must be rechecked before interaction. F6 is the ordinary
INTERACTTARGET binding, equivalent to interacting with the selected body;
F7 may restore a lost target, which must then independently match the dead
owned creature. Click-to-move must remain off. Approach uses short released
forward pulses and a local displacement/deadline bound. No persistent
autowalk is admitted. Lost/blocked/unmatched corpses are recorded, not
silently counted as looted. New loot-session and item/copper facts verify
results; source kind remains explicit.

No empty general slots is a capacity warning, not proof that an existing
stack cannot accept an item. A real inventory-full event is recorded
separately. Full bags do not authorize deletion or sale of any item. A
failed loot attempt does not erase the verified XP kill. Finish requests
drain the bounded corpse phase before another pull.
