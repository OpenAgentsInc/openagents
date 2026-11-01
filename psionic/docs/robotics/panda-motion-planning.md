# From Panda queries to motion planning

This is a design sequence, not a collision or path qualification. Position/pose
IK, the isolated PD model and the short owned-dynamics prefix have their own
records. A posture projection still needs a valid motion, collision evidence and
timing before a controller can use it.

## Start from the geometry we actually have

Aethyr's general query, broad phase, narrow phase and robot import chapters
remain design work. Existing F64 Verse contact primitives are useful references,
but do not provide an owned robot collision pipeline. The renderer's mesh BVH
thins and rescales geometry; it cannot establish collision containment.

The pinned LIBERO scene has 66 robot geoms: 53 explicitly visual meshes and
13 collision-role shapes. The latter are eleven mesh instances using ten binary
STL assets, plus two finger-pad boxes. The mesh instances account for 14,580 triangles.
Use every original vertex when constructing an enclosure. The hand's local
rotation and right finger's local half-turn belong to the geometry transform;
body and inertia frames cannot replace them. The robot world is Z-up, requiring
an explicit basis at a Y-up game boundary.

The source XML SHA is
`95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f`.
The [retained model metadata](../../fixtures/robotics/panda-actuation/sdk-model.json)
is a starting point. It does not bind all collision masks, welded-body IDs,
mesh transforms, margins and exclusions needed for source-equivalent pairs.

Both directional type/affinity intersections participate in MuJoCo's pair
filter. A finger mesh with type zero can still contact through its affinity.
Weld and parent rules also matter, including the world-weld exception. Bind
those facts explicitly using the [pinned collision driver](https://raw.githubusercontent.com/google-deepmind/mujoco/a209134315dda126a714529bfa80f7478902644d/src/engine/engine_collision_driver.c).
The constraint/contact-off Euler profile supplies no empty-pair oracle.

## Deliver four small stages

| Stage | Concrete result | Required evidence |
| --- | --- | --- |
| Geometry and filters | Thirteen authenticated enclosing local boxes, exact pad boxes, geometry/body transforms and a fixed source-pair policy | Every original STL vertex contained; original asset hashes and unsupported transform/default refusals; compiled-mask/weld facts acquired once or independently proven |
| Snapshot clearance | F64 oriented-box separation over fifteen axes, plane support and deterministic pair enumeration | Independent corner/support checks, near-touch/parallel cases, all-pair truth tables, typed frames/units, admission and atomicity |
| Whole-edge clearance | Conservative point-displacement bounds over a linear joint edge, with bounded subdivision | Complete edge identity and numeric enclosure; typed `uncertified` on work exhaustion; dense sampling as an additional oracle |
| Planning and timing | Seeded RRT-Connect, shortcutting through the same edge query, then a separate timing pass | Retained endpoint/edge evidence, reproducible paths, independent final recheck, declared velocity/acceleration limits |

Each stage is a separate task with checks before code and a push within two
hours. Use only new small geometry contexts. Carry unchanged FK/dynamics proofs
by source and emitted-artifact identity; repeat no whole-model or old episode.

## Give an enclosure a precise meaning

A box containing every source vertex contains its triangles and convex hull.
This needs no watertight-mesh assumption. An overlap between two enclosing boxes
does not establish contact between their underlying meshes. The useful answer
is therefore `clear` or `uncertified`, with a limiting pair and declared reserve.
The planner treats an uncertified edge as unavailable, without calling it global
unreachability.

Begin with fixed finger coordinates, all thirteen source collision-role shapes,
at most eight owned static boxes and an optional Z-up plane. This is a fixed
Panda clearance profile. A complete LIBERO scene, moving objects, payloads and
changing gripper closure are later profiles. Test that useful source poses have
a nonempty certified region before calling this geometry ready for RRT. If
the enclosing boxes reject everything, preserve that outcome and add tighter
convex support; do not silently discard inconvenient pairs.

## Certify the motion between samples

For a joint edge, bound each enclosed point's displacement from chain lever
arms and angle changes. Around the midpoint, a conservative geometric bound
is half the sum of each ancestor lever bound times its joint-angle change.
Moving self pairs require both bodies' bounds. A separating-axis lower bound
larger than those motions and reserves can cover the entire subedge.
Subdivide unresolved intervals with an explicit depth/work cap.

The numerical enclosure is its own requirement. State outward rounding or a
qualified padded arithmetic profile; random checks and an arbitrary epsilon do
not establish an all-F64 certificate. A ten-times-denser recheck is useful
evidence, while finite sampling alone cannot prove clearance between samples.

Use that same edge contract for RRT extension, final verification and
shortcutting. A geometric path still needs timing, a tracking-error reserve,
runtime supervision and separately authorized hardware integration. It does
not establish contact dynamics or compiler R14 bounded control.
