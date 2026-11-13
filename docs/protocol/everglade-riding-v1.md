# Everglade riding, version 1

Riding in a room, in wire 7 ([#13268](https://github.com/OpenAgentsInc/openagents/issues/13268)).
The room decides who rides which mount and where each mount is. A page asks; the room answers and
writes each mount as a row.

- Profile: `proto/verse/everglade2-riding-v1.json`
- Codec, loaded by the room and the page: `everglade/shared/mount-wire.coax`
- Check: `coax/web/tool.sh coax/web/room/riding-profile.coax proto/verse/everglade2-riding-v1.json`
- Rules: `everglade/server/mounts.coax` (who may ride), `everglade/shared/mount-land.coax` (the step)

## The row

A mount is a row of the session (`everglade-session-v7`), kind 16 (`WK_MOUNT`).

| Field | Meaning |
| --- | --- |
| id | 1170300 + the mount's number (0 to 3) |
| slot | 477 + the mount's number |
| x, y, z, yaw | where it stands and faces; `y` is its feet's height over the ground (a flier's, a jump's) |
| vx, vy, vz | its speed along its heading (against it when it backs) and its feet's speed up |
| G | its state: 0 free, 1 being mounted, 2 ridden, 3 being dismounted |
| CLIP | its gait: 0 idle, 1 walk, 2 gallop |
| T0 | the tick its mounting or dismounting began |
| OWNER | its rider as a target code (1000 + seat), -1 nobody |
| P1 | its kind: 0 a riding horse, 1 a red dragon |
| P2 | its feet's height in centimetres |

A ridden mount rides on its rider's seat in interest management: its rider always hears it, and a
page that blocks its rider never does. A page on wire 6 is sent no mount row.

## Asks

An ask is the session's use frame, `E SEQ KIND ID`. `SEQ` rises with each ask, above 0.

| Ask | Frame | Meaning |
| --- | --- | --- |
| Mount | `E SEQ 20 MOUNT` | mount the mount `MOUNT`, standing within its reach |
| Get down | `E SEQ 21 0` | get down from the mount you ride |
| Land | `E SEQ 22 0` | bring your flier down; it lands unless you fly on |

## The answer

`e ride SEQ CODE MOUNT`, an entry of `D` told to the asker alone on the reliable class. `MOUNT` is the
mount the ask was about, -1 none.

| CODE | Meaning |
| --- | --- |
| 0 | done |
| 1 | no such mount |
| 2 | your conditions don't let you act or move |
| 3 | you are already on a mount |
| 4 | someone else has it |
| 5 | out of its reach |
| 6 | you ride nothing |
| 7 | still mounting or already getting down |
| 8 | nobody gets off in the air: land first |
| 9 | no room to step down on either side |
| 10 | land: it is not in the air |

An ask that is no riding ask at all is refused with the session's `r SEQ ride`.

## The step

One input of the rider's is one step of its mount, 1/30 s, as it is one step of a walker. The
rider's page predicts its mount with the same step (`e2ml_step`) on the same inputs and reconciles
from the row. A rider's own row is where its mount seats it. A mount nobody rides steps once a tick
while it still moves. A rider who leaves the room or falls is off the mount at once.
