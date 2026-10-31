# Initial affine geometry: merged publication

Coax now admits concrete native-C `Point3(Frame,Unit,R)` and
`Vector3(Frame,Unit,R)`. Role, nominal frame/unit and floating carrier remain
distinct; values contain three raw coordinates. This is an initial R12 slice,
with explicit component escapes and no transform/epoch/robot-migration claim.
The [profile](../../coax/SPATIAL.md) states the supported operators and limits.

The frozen compiler proof in [the original record](coax-spatial-2026-10-31.json)
is labelled 0.44: 731 retained check groups, 20 exact procedures, 1,403 killed
mutants and zero survivors. Main subsequently released 0.45 and moved the
physics/spacecraft drafts to Aethyr; our robotics chapter is now 170. We kept
both changes and regenerated the checker seed from the merged authored source.

[The merged record](coax-spatial-merged-2026-10-31.json) binds these results:

- All 24 focused normal-recipe reconstruction commands pass in 93.103 seconds.
  Compiler graph and parser/checker seed C and headers reproduce exactly.
- All 20 frozen procedure bodies match the merged source. The new 0.45 seam
  checks named-capacity `Bounded` point/vector storage, copy/get/set/full refusal,
  mutable live views, generic identity and nominal-frame inference refusal.
- Eight emitted C programs and five headers match previously executed artifacts
  exactly. Native arithmetic, ABI/order, camera and native-verification results
  carry without repeating their numerical executions.
- The independent aggregate oracle has 84,978 observations per mode, including
  20 operand-order traces, eight interrupted calls and alias snapshots.
- The changed camera boundary has two affected source checks, 1,536 native
  coordinates per mode and actual F32-world refusal. D3 and SpaceCam layouts
  remain unchanged. Belt's internal RGB record was renamed `BeltRgb` to free
  the intrinsic name; C syntax and real Wasm object compilation pass.
- Mark2 prechecked source calls retain authored operand traversal. The new
  index probe kills both emitted mutants. Its tiny run was accidentally
  executed twice while collecting the core record; it is counted once.

Debug/release/fail-fast sanitizer numerical profiles retain their exact
non-NaN bits and NaN-class scope. All 30 measured release symbol byte extents
match the raw baseline, but only 28 are relocation-free; equal bytes with
relocations do not establish instruction equivalence. Debug helper overhead
remains. Lowered spatial operator-token swaps are still unqualified, and the
shared compiler-instance budget is 256.

No old full mutation corpus, model, SDK episode or native semantic suite ran.
This is a focused source/seed qualification, not a complete release/cache or
cross-target numerical qualification. The final publish sequence checks the
qualified compiler-input fingerprint after rebase, then book structure and
the diff before pushing.
