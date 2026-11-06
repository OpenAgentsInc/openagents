# Private SHS1 source shot, version 2

This private payload is admitted only inside the pinned ECP3/VPK source bundle.
Version2 replaces the earlier experimental orb-colour metadata with the
animated world quaternion. Version1 is refused by this reader.

All integers are little-endian u32; scalar fields are finite IEEE binary32.
The64-byte header contains magic SHS1, version2, whole byte count, frame
count, bone count, rate(f32), name-table offset64, data offset,
frame stride, first source sequence frame, facial clip index, facial section
start and facial section end. Each68-byte bone record contains the canonical
64-byte name and its admitted rig index. Each frame contains one seven-float
local transform per named bone, then44 metadata floats. The quaternion in
metadata30–33 must be unit length within the same admitted tolerance as locals.

| Metadata | Meaning |
| --- | --- |
| 0–2 / 3–5 / 6–8 / 9–11 | Camera eye / right / up / forward, engine coordinates |
| 12 / 13 | Horizontal projection multiplier / source filmback aspect |
| 14–16 / 17 / 18 | Warm wrap direction / reserved1 / wrap intensity |
| 19–21 / 22 / 23 | Orb world centre / radius in metres / point-light intensity |
| 24–26 / 27–29 | Key direction / key linear RGB |
| 30–33 | Orb world rotation, quaternion xyzw from the source hand chain |
| 34 / 35 | Focus distance in metres / f-stop |
| 36–38 / 39–41 | Head world centre / fill direction |
| 42 / 43 | Key intensity / fill intensity |

The source actor transform track is disabled and is not applied a second time.
The body contains134 mesh joints plus the source prop track bound by name to
`groom.prop_01` at palette135. Its parent already follows the body root, so
that root transform is not duplicated. Shader time is clip-relative at24fps;
the source renderer's absolute material-time phase is not inferred from video.
