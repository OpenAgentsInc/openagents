# Valley Lab: first real-rock parent

The first nonplanar star parent runs on the original rock in Coax, through
manual selection, conservative screen error and real HTTP paging. Qualified
on Apple Metal WebGPU in muted headless Chrome, desktop 1280×800 and phone
390×844. Implementation began from `04d2d01521`.

The supported patch is an ordered six-edge boundary with one interior vertex,
extracted from cube face +X. Source triangles are matched cyclically to the
original table. The parent replaces six triangles with four and preserves all
six boundary positions and segments exactly. Every other source triangle is
submitted from the original shared vertex/index table. This is a small
qualification fixture, not an arbitrary nonplanar cluster reducer.

The analytic certificate maps the interior apex onto boundary vertex zero.
Every triangle maps to one output triangle or a retained edge. Conversely,
every output triangle is the image of an input triangle. Linear barycentric
motion is at most the apex displacement, bounded conservatively by L1.
The f64 sum plus child error rounds upward to the smallest enclosing f32.
This conservative bound is intentionally loose; measured samples never
replace it. Strict convexity is checked in the supplied projection. The
qualification covers the three pinned generated profiles, not general robust
predicates for ill-conditioned imported geometry.

`oracle.json` records independent Python f64 point-to-triangle distances,
3,366 source and 2,244 parent samples per profile, boundary byte comparisons,
source triangle IDs, original hashes and actual byte counts. Source files
were emitted by the native Coax tool into temporary files; the oracle parses
VRO1/VGP1 independently. The original art files, generator and hashes do not
change. Reproduce native page/proof files with:

```
coax/web/tool.sh aethyr/tools/valley-rock-pages.coax OUTPUT_DIRECTORY PROOF_DIRECTORY
```

Each patch drops 7→6 vertices, 6→4 triangles, 172→136 encoded bytes and
328→256 encoded-plus-decoded resident bytes. Whole-rock triangles become
766 / 3,070 / 12,286. Full canonical assets remain retained, so this does not
claim whole-asset download savings or reduced physical cache allocation.

Validation records:

- `mutation.txt`: star authority root gate, 90 killed, zero survivors/equivalents.
- `tests.txt`: 478 passing checks from the app suite plus the corrected-budget
  real-rock check; 479 distinct passing checks. The initial new check exceeded
  its compile-time instruction allowance, which was raised to 1.5 billion;
  its successful run takes about nine seconds. No other passing gate was rerun
  solely to confirm it.
- `browser.txt`: actual desktop/phone input, selection, HTTP hold/corruption,
  full-detail fallback, exact byte-budget boundary, GPU readback comparison,
  and exact state/pixels after two device losses per profile. Each loss logs
  one expected diagnostic, consumed explicitly; other console errors fail.
- `pages.txt`: native real-rock page SHA-256, error and byte measurements.
- `artifact.txt`: release Wasm bytes/hash and Coax tool policy check.

The release uses the previously qualified Coax 0.50 final compiler. Its
source fingerprint and path match the earlier rock/three-stage receipts.
Frame buffers remain under the 6 MB arena, 64,000 submitted-vertex limit,
16,384 geometry vertices and 65,536 indices for all three profiles. No host,
shader, compiler or published contract changes are needed.

CPU pins end after copying into the frame buffer. GPU page allocation,
shared-renderer retirement, arbitrary parent groups, large hierarchy cuts,
occlusion, indirect draws, streamed world cells and native rendering remain
open. No book chapter status changes.
