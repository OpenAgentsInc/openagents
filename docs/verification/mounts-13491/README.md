# Mount follow-ups, #13491

Measured 2026-11-13. Changes extend the shared mount engine, Everglade's
shared driver diagnostics and the existing asset bake. The room and the page
predict with the same engine change; their release remains a paired release.

## Production horse

The production page named
`/build/realms/everglade2/assets/everglade2.0f0039ca2bdc.vpk3`.
Its complete 4,133,890-byte catalog was read through the VPK3 manifest and
DEFLATE pages: **zero `C glade/horse` rows**. The tracked pack also had zero.
Thus this was a production asset omission, not just a local readiness fault.

The tracked pack now takes all 36 horse frames from the shared horse packer's
`--append-horse-e2` mode. Everglade II's shader needs model kind 9; the legacy
Experimental kind 8 is foliage here and recolours the horse orange. The web
build runs `mount-pack-check.coax` over every frame's actual indexed vertices,
checking ranges and kind 9. The checker refused the kind-8 pack before the
corrected pack was generated.
Blender simplifies the bind mesh once at ratio 0.25 before evaluating the
ordinary animations. The original source and full Experimental bake remain
in the shared library. The 36 saddle rows are byte-identical to the full
bake's `saddle.txt`; clip timing and scale are unchanged.

The pack reader's 64 MiB arena includes its heap base, not just the decoded
image. The measured reactor heap base is 1,263,344 bytes, leaving at most
65,845,520 bytes for the image. A pack that merely passes the header's
67,108,864-byte budget can therefore still fail in the browser. The web bake
leaves room for both the reader and the final rig/cell image: the sealed tracked
pack is 63,139,480 bytes, and the final rig/cell image is 65,379,852 bytes
(32,622,484 bytes stored as VPK3).

## Collision and the village

`mount_stop` identifies the first stopping box/post. An existing overlap may
shrink or slide along its boundary, but may not deepen. The gap uses the
whole body's separating axes (and the segment-to-post distance), so samples
crossing a thick wall cannot trap a body backing out. New checks cover the
0.4199 m touch on both sides, escape, and a round post; earlier pivot, slide,
backing, jump and flight checks pass.

The exact headless zone route was:

```text
mount horse-0
clock +1500
walk-to gremlin-9
why horse-0
walk-to dragon-1
why horse-0
state w[0].creator[0].props[0].n
state w[0].creator[0].mounts[0].mounts[0]
```

The first stop names `box-6 [-4.6 -0.0 -23.3 0.7 1.1 -21.3]`.
The horse then comes out to `(6.450, -15.586)`, facing `-5.962`, and its next
forward probe names `box-0 [4.9 -0.5 -14.3 6.6 4.9 -12.0]`.
The added props count is **0**. This is a real village collider on the
straight route to the dragon. A permanent zone regression pins that answer;
another regression supplies an actual added prop and checks that it is named.
The driver steers straight at its target; this change adds no pathfinder.

## Checks

- Shared engine mount: 42 checks passed.
- Shared mount diagnostics and dependencies: 69 checks passed.
- Everglade II client: all 2,096 checks passed after rebasing onto main.
- Zone suite passed, including the new prop diagnostic checks. The added
  village regression passed and the final page build checked its complete
  current source closure; the zone suite passed again after rebasing.
- Mount-land: all 239 checks passed.
- Release module builds passed their budgets: Everglade II 1,253,237 bytes;
  fivelab 1,648,976 bytes in this public checkout. Fivelab's budget is now
  3,200,000 to cover the issue's full-world 3,073,464-byte measurement plus
  diagnostics/headroom.
- Horse pack binary-layout regression passed; the build checker accepts all
  36 corrected kind-9 frames and refused the old kind-8 pack.
- `tools-check`, `wire-check` and `layer-check` passed.
- Zone browser route: headless, muted Chrome, Apple Metal hardware WebGPU.

No mutation result is claimed. The optional root mutation attempt stopped
before mutation: the existing 600-step backing check exceeds the interpreter's
20,000,000-step baseline budget. Ordinary native checks pass; `mutate` does
not accept `test`'s `--steps` override. No authority or budget was weakened.

## Everglade II browser acceptance

With the rebuilt native room, its matching revision-2 content tables, the
final VPK3 and the release page, headless muted Chrome on Apple Metal reports:

```text
net[0].state = 2
rv[0].eg[0].frames = 36
mount horse-0: done, pressed E (mount)
horse: on true, x 9.000, z 9.000, state 2, rider 0
walk-to dragon-1: done, there
horse after riding and stopping: x -14.140, z 5.210, speed 0, state 2, blocked false
why horse-0: ok
```

The idle and ridden screenshots were inspected: the white horse, rider and
saddle render correctly. The room was restarted before this acceptance run
so earlier rides could not leave the horse somewhere else. The native test room ran on a build host through a loopback SSH
forward; its content table must be beside the executable (or supplied through
the existing content path setting). Starting it elsewhere without that table
correctly refuses the page's content pin; it does not exercise riding.
