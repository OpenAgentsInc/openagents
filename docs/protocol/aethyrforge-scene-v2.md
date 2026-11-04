# Local Aethyrforge scene commands v2

V2 retains every [v1 command](aethyrforge-scene-v1.md) and adds individual
object resizing through the same `aethyrforge-scene apply JSON` command.
The [v2 schema](../../proto/aethyrforge/scene-command-v2.schema.json) keeps
the published v1 schema unchanged.

Read `show` to get the current `version` and each object's `index`, position,
size and group. To triple just object 13 from version 23:

```json
{"op":"resize","index":13,"version":23,"scale_percent":300}
```

`scale_percent` multiplies the current size, rounded to the nearest percent.
Alternatively `size_percent:300` sets the size to three times the catalogue
default. Supply exactly one size field. The final size must be 25..400 percent;
an out-of-range result refuses without clamping or changing anything.

The index is zero-based and bound to the exact version read by the agent.
A stale version refuses before planning, so deletion/reordering cannot silently
resize another object. The existing POST base check also fences concurrent
edits between reading and saving. Read back confirms the whole saved document.

Resize changes only the selected object's size. Its position, yaw, lift, tint
and group stay intact, as do all other records and document flags. It never
recreates the group. The complete proposal passes the same Everglade geometry
checks as placement before saving. The page, player collision and NPC fences
receive the resized object through the existing world update path. As in the
manual editor, another object's transform does not follow a size edit.
