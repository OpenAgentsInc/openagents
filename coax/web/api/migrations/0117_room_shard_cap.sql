-- Everglade instances on oa-room shards (#12717): two instances a shard, not three.
-- With the server's town (#12713: townsfolk, the watch, breaks and burns, conditions per player),
-- two full rooms tick at p95 23.6 ms on one shard's 1 vCPU, so a third would pass the 33 ms frame
-- (30 Hz). Six shards then hold 12 instances (192 players). Placement reads rooms_cap each pick,
-- so this takes effect without a room server deploy. Safe to run twice.
UPDATE verse.room_shards SET rooms_cap = 2 WHERE rooms_cap = 3;
