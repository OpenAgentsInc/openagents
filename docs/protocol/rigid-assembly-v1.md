# Rigid assembly v1

Issue #13639. Owner: Metaverse Engine. This is a visual rigid-link assembly,
not an articulated dynamics solver or permission to operate hardware.

The shared glTF importer retains each mesh-bearing source node's identity,
parent, world rest transform, vertex/index range and bounds. The existing
48-byte vertices stay in source world-rest coordinates. A separately pinned
mechanical description binds each part to a named joint. Joint axes and pivots
are expressed in that same world-rest frame, in metres and radians, Y up.
Parents precede children. Pose values are absolute joint coordinates; motion
uses the difference from the recorded stand coordinate.

For joint j, form a rigid rotation about its rest pivot, then compose it with
its parent's delta using the existing `pose_chain`. The resulting delta acts
on the already transformed source vertices. This moves descendants' axes and
pivots with their parent without reapplying the source rest pose. Presentation
explosion offsets are added after mechanical posing and never change angles.

## RBA1 record

Registry identity: `metaverse.engine.rigid-assembly.v1`. One codec:
`metaverse-engine/anim/assembly-wire.coax`. All integers and IEEE binary32
words are little endian; the three SHA-256 identities are raw big-endian
digest bytes. Reads stage a complete candidate and publish only after full
admission. Writes require the exact complete size before touching output.

Header (124 bytes): `RBA1`, total byte count, joint count, part count, vertex
count, index count, source node count (seven u32 words including magic), then
32-byte source GLB, mechanical-description and topology-file SHA-256 values.
Joints: 1–32. Parts: 1–96. Source nodes: 1–256. Geometry is limited to two
million vertices and six million triangle indices. All three identities are
nonzero and source identity is compared with the expected source before read.

Each joint is 168 bytes: 128-byte name, signed parent index, three axis
floats, three pivot floats, low/high/stand floats. Names are nonempty printable
ASCII, at most 127 bytes, NUL-terminated and zero padded; duplicate names refuse.
Axes are unit length within 0.001. All numbers are finite and bounded. Limits
are ordered and contain stand. A parent is -1 or an earlier joint.

Each part is 368 bytes: 128-byte stable source ID, 128-byte display label;
seven words (signed joint, source node, signed source parent, first vertex,
vertex count, first index, index count); six bounds floats (low xyz/high xyz),
three explosion-offset floats and twelve source world-rest matrix floats
(three rows of four). Referenced joints/nodes and nonempty contiguous geometry
ranges must be valid. Part IDs/source node IDs are unique. Source parent links
between represented nodes must be acyclic; unrepresented transform-only source
parents remain identified by node index in the pinned original GLB.

The exact size is `124 + 168*joints + 368*parts`, at most 40,828 bytes.
No trailing bytes or noncanonical name padding is accepted. The record can be
carried in a VPK1 catalogue as one `RBA1 BASE64` line; this codec owns that
carrier too. Existing pack readers ignore the additional catalogue line.
The whole-pack digest gate still admits the complete pack before any GPU use.

## Microduck source binding

Keep the existing pinned workshop GLB. The matching workshop revision also
contains `components.json` (joints' rest-world axes/pivots, limits, stand pose,
part IDs/body ownership and explosion offsets) and its referenced MJCF file.
Their hashes and body/joint topology are verified independently before packing.
The MJCF is source data, never executed. The beak has no simulated joint in
this description and must not acquire an invented mechanical control.

WebGPU and Metal use one draw per part and a bounded uniform palette indexed
by base instance. The shared shader applies the same rigid delta to positions
and normals. Picking transforms the ray into the part's rest frame and uses
the shared ray/bounds query. Direct selection is a bounds query; a named picker
provides an unambiguous route to internal parts. Model controls and inspection
are separate from any world-editor or actuator authority.
