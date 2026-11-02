# A declared shared-edge geometry scope

This oracle declares which boundary packets remain **unqualified** in one
finite geometry fixture. It establishes no GPU error allowance, legal
alternative vertex/depth result, or visible-edge/depth-winner proof. The
ordinary complete-image comparison and its failures remain recorded.

```sh
coax/web/tool.sh aethyr/render/tests/derive-shared-edge-envelope.coax \
  --geometry vertices.bin --rounding nearest-even --out EMPTY_DIRECTORY
```

The Coax entry requires the pinned Dead Gears nominal vertex packet SHA256
`62823c676088a5296d1ca46bc265aafe93b1946dec1f60268ee05818474d3302`,
1,155,168 bytes, 6,876 triangles, and 14 little-endian f32 words per vertex.
The viewport is 320×180. Only the measured `nearest-even` and
`nearest-positive` eight-bit profiles with guard region 65,536 are accepted.
The half-cell radius is fixed at **1/512 pixel**; no CLI band or tolerance
parameter exists. A nonempty output or unsupported profile refuses before
oracle execution.

Coax owns process deadlines, the selected compiler/image/native/Python/source
fences, profile/input admission, artifacts and a second complete mask check.
`shared_edge_envelope_oracle.py` is a reason-2 independent mathematical
oracle. It reads raw geometry and the declared profile, never an image,
fragment result, failed-pixel list or GPU depth buffer.

Every input word must be finite, including attributes unused by the geometry
calculation. Every original triangle and all three edge occurrences enter
the census. Endpoint keys are the exact seven clip/world f32 bit patterns;
projected XY coincidence does not establish a shared edge. An eligible
triangle has nonzero exact world area, positive W, all three original clip
depths in `[0,W]`, projected coordinates inside the declared guard, and
nonzero snapped screen area. Other triangles remain strict. The oracle does
not invent clipped edges or project through a zero/negative denominator.

For each exact shared group with at least two eligible owners, exact world
triple products distinguish coplanar faces from noncoplanar folds. Every
eligible fold group is processed, including groups currently occluded by
other geometry. Coplanar triangulation edges, unmatched edges and degenerate
segments produce no boundary exclusion. This is a conservative geometry
scope, so a marked pixel need not be on a visible boundary.

The stored f32 clip values are decoded as exact rationals. Perspective
division and viewport mapping are rational; only then is the declared
eight-bit grid rounding applied. A snapped endpoint's square of radius
1/512 describes a declared quantization-cell geometry region, **not** legal
post-snap hardware motion. The WebGPU
[rasterization specification](https://www.w3.org/TR/2026/CRD-webgpu-20260915/#rasterization)
does not establish this as a universal numerical error bound.

Let the snapped endpoints be A and B and let Q be the axis-aligned square
`[-h,h]²`. The union of finite segments joining endpoints in `A+Q` and
`B+Q` is exactly `segment(A,B)+Q`: convex combinations of two points in Q
remain in Q, and choosing the same offset at both endpoints gives the
reverse inclusion. The Python oracle tests pixel center P by intersecting
`t ∈ [0,1]` with each exact rational constraint
`P[j]-h ≤ A[j]+t(B[j]-A[j]) ≤ P[j]+h`.

The separate Coax check uses integer units of 1/512 pixel. It intersects the
finite endpoint AABB with
`abs(cross(B-A,P-A)) ≤ h*(abs(dx)+abs(dy))`. It reconstructs the complete
mask from every emitted eligible edge, then verifies every witness's two
rational interval endpoints using bounded exact integer products. Its
coordinate cap keeps products within signed 64-bit range.

Both measured profiles produce the same 159-pixel mask for this fixture:
SHA256 `fe53de8772f0ea5d9ebcf5343247bf385a1691781b19c4de0807af10f951198c`.
The complete census contains 20,628 edge occurrences, 9,276 exact shared
groups, 7,471 eligible fold groups, and 1,598 eligible coplanar groups. The
6,086 eligible triangles exclude 736 world degenerates, 46 nonpositive-W
triangles and eight snapped screen degenerates. The W count includes fully
behind geometry and the previously identified crossing triangles. These
exclusions are scope limits, not successful validation of those triangles.
Triangle categories record the first ineligibility reason in that order;
they are mutually exclusive rather than independent counts of every trait.

The output contains `facts.json`, a binary `mask.bin`, complete hashed
`edges.jsonl` and `witnesses.jsonl`, a 62-pixel exact-centerline control, and
a separate all-zero mask. All 159 mask pixels have witnesses; there are 160
edge/pixel witness records because one pixel belongs to two eligible edges.
The two previously inspected points are regression checks, never selectors
in the global generation algorithm. Their witnesses do not prove that a
different depth winner is legal.

Facts carry `admitted: false`. A consumer must separately join geometry,
shape, resource/profile/source identities and all artifact hashes before
reading images. It must preserve the full raw strict result, fail every
nonfinite value even at a masked pixel, report exact excluded pixels and
channels, and label whole RGBA packets at those pixels **UNQUALIFIED**.
Only the remaining interior has the ordinary finite comparison claim.
