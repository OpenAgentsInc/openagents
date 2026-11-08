# Valley Lab GPU Cull

An unlisted check page at `/worlds/valley-lab/cull` for Valley Lab step 6
(GitHub #13008). It runs cluster culling on the GPU in WGSL that `coax wgsl`
writes from `aethyr/render/geometry/cull.coax`:

- the cluster DAG's draw rule over installed pages (`dag.coax`),
- frustum culling,
- two-phase Hi-Z occlusion: last frame's depth pyramid first, then a retest
  against this frame's,
- indirect draws, one instance a cluster,
- a request array the GPU fills each frame, read back to drive
  `residency.coax`.

The page prints its report. `coax/web/tool.sh coax/web/cull-check.coax URL`
runs it in headless Chrome and checks it.
