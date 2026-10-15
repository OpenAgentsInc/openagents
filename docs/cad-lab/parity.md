# Parity with ClassCAD's documented functionality (#11628)

Status on 2026-10-14. Our own engine (`crates/cad`, on SolidCraft's Rust
crates on Truck, pinned), built clean-room from ClassCAD's public docs as
summarised in [classcad.md](../research/cad/classcad.md): our own names,
documents and docs; none of their code, text or software. Each line says
where it lives and the test that proves it. Every capability is a `cad_*`
tool (chat, `openagents cad`, and MCP through `cad-lab mcp`) checked on
every edit: the document, the rebuild, closed solids, interference and mass
properties.

| ClassCAD (documented) | Ours | Where / proof |
| --- | --- | --- |
| **Part API**: a feature history that replays; `update*` twins | `part.v1` features replayed from the top; `cad_edit_feature` changes, suppresses or removes any feature and later ones rebuild | `crates/cad/src/build.rs`; `cad-lab` tests |
| Expressions and parameters linked to features and dimensions | `parameters` (assembly-wide), expressions in any length, angle or sketch dimension | `sketch_constraints_and_dimensions_drive_the_profile`, `a_direct_block_repeats_steps...` |
| Stable face and edge references across rebuilds | Faces named `feature:role` (`body:top`, `body:side:l3`, `boss:+z`), edges `{between: [...]}` / `{of: face}`, from the kernel's persistent naming; `cad_faces` lists them | `crates/cad/src/refs.rs`; `fillets_and_shells_name_their_edges_and_faces_and_survive_a_resize` |
| Rollback to a feature | `cad_faces after`, `cad_add_feature after`, suppress; every version kept | `crates/cad-lab/src/lab.rs` |
| **Solid API**: direct operations without history | `cad_solid` (no document, nothing saved) and `cad::solid` | `a_direct_block_repeats_steps...`, `assemblies_come_from_json...` |
| **EntityInjection**: direct code inside a part, not yet recomputed in theirs | `op: "block"`: any modelling operation, declared `inputs`, `repeat` over a range with `{var}` in params; recomputed with the history | same test: a parameter change moves the block's holes |
| Primitives, sketches, extrude, revolve, sweep, loft, booleans, fillet, chamfer, shell, draft, patterns, mirror | All, plus pipe, coil, hole, rib, web, emboss, thread, split, offset face | `sweep_loft_draft_and_mirror_build_from_documents`, `a_filleted_plate...`, `patterned_and_mirrored...` |
| Sketch constraints and named dimensions | `constraints` (horizontal, vertical, coincident, tangent, parallel, perpendicular, equal, concentric, collinear, fix, midpoint, symmetry, curvature, polygon) and `dimensions` (parameters allowed); degrees of freedom in the report | `sketch_constraints_and_dimensions_drive_the_profile` |
| **Assemblies**: templates and instances, nested; assembly from JSON with templates pointing at files | `parts` (part templates) and `assemblies` (subassembly templates), instances of either, paths (`bogie_l.wheel`); `cad_from_json`; templates copied from another document's version (`from`, `source` kept) | `subassembly_templates_place_copies...`, `assemblies_come_from_json_with_templates_from_other_documents` |
| Constraints: fastened, revolute (limits), slider (limits), cylindrical, planar, spherical, gear | All seven, with values and limits; gears follow a driver joint (rack and pinion off a slider), loops of gears refused | `planar_spherical_and_gear_joints_move_their_children` |
| Closed loops and `moveUnderConstraints`: a mechanism driven by one joint, the others following within their limits; parallel and fastened-to-origin relations | Joints with `closes: true` (fastened, revolute, slider, cylindrical, planar, spherical, parallel; no parent = the origin) and gear couplings between joints; a Levenberg–Marquardt solver over the joint values (`crates/cad/src/solve.rs`) with limits, `drive`, and per-loop residual, mobility, degrees of freedom left and redundant equations; a loop that can't close is refused; `cad_assemble drive`, `cad_motion` (#11646) | `a_four_bar_closes_and_traces_its_analytic_coupler_curve` (residual under 10⁻⁹ over a full turn), `a_slider_crank_follows_its_crank`, `over_constrained_loops_are_reported_not_forced`, `a_four_bar_of_parts_closes_and_follows_its_driven_crank`, `a_linkage_is_driven_and_moved_through_its_range` |
| Mates on geometry (constraints between faces and edges) | A joint's frames from named faces (`{"face": "base:+z"}`: centre and normal, a round face's axis) and edges (`{"edge": [face, face]}`), kept with the built part (#11646) | `mates_take_joint_frames_from_named_faces_and_edges` |
| (Not documented by ClassCAD) interference | Exact on the triangles: flux of the part of each surface inside the other, BVHs, Green's theorem on crossed triangles; B-rep intersection for flat-faced pairs; touching faces are contact; each pair remembered by its meshes, so an edit re-measures only what moved (rover: 53 pairs in 10 ms, an edit 7 ms) (#11648) | `a_thin_sliver_the_grid_missed_is_caught`, `a_thin_overlap_is_refused_and_resting_faces_are_contact`, `faceted_cylinders_overlap_by_their_polygons` |
| Linear and circular patterns in assemblies | Instance `pattern`: row, grid or ring, of parts or whole subassemblies | `instance_patterns_place_rows_grids_and_rings` |
| Mass properties of a solid, part, subassembly or the whole | Volume, mass, centre of mass and inertia tensor per instance, per subassembly and for the whole; SMLib exact cross-check for flagged parts | `flagged_parts_cross_check_their_mass_against_smlib` (a 60 mm wheel: 0.14% volume, 0.28% inertia) |
| Polylines with bulges, DXF in and out | `sketch.polyline` (bulged points), `cad_import_dxf` (touching lines and arcs become polylines), `cad_export_dxf` (a sketch, or a section) | `crates/cad/src/dxf.rs`; `polylines_and_dxf_drawings_make_solids_and_go_back_out`, `dxf_drawings_come_in...` |
| STEP import (AP203/214/242) | `solid.import_step` features from the Lab's file store; `cad_import_step` | `step_and_3mf_keep_the_assembly_tree_and_step_comes_back_as_a_feature` (AP242 and AP214) |
| STEP AP242 export with the assembly | Products per part and per placed subassembly, an occurrence per instance; parts built afresh so their exact solids write cleanly | same test: 7 occurrences, 5 products, reads back to the same volume and box |
| glTF, STL, 3MF | glb (ours), STL (kernel), 3MF with components (ours) | `exports_step_3mf_and_glb_and_reads_step_back`, the tree test |
| WebAssembly and native builds of one API | `crates/cad-wasm` (wasm32-unknown-unknown, wasm-bindgen) and `cad-lab serve` | `scripts/cad-wasm-test.sh`: check, faces, Solid API, DXF and all exports under Node |
| Agent tooling: typed tools, an MCP server, checks | 19 `cad_*` tools; `cad-lab mcp` (stdio JSON-RPC); checks on every edit | `an_mcp_client_lists_and_calls_the_cad_tools`, `every_tool_is_declared...` |
| URDF | #11643 (another agent) | |

## Not covered, and why

Each is its own issue; none blocks the parity map above.

- **2D drawings** (ClassCAD's drawing2d: projected views with hidden lines,
  dimensions) (#11647). The kernel gives sections and sketch DXF, which we
  expose; projected hidden-line views are new work.
- **The wasm engine on openagents.com** (#11649). The build runs the tools;
  wiring a page to it is product work.
- **OFB, IWP and SCG files** are ClassCAD's own formats; we read and write
  open ones (STEP, DXF, 3MF, STL, glTF) and our JSON documents instead.
