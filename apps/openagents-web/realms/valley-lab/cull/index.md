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
  `residency.coax`, with a shadow view's requests weighted lower and each
  texture's wanted mip after the page entries,
- pages fetched by HTTP Range from each scene's VPK3 file, admitted by their
  SHA-256 and copied into GPU slots on install (GitHub #13051).

The page prints its report. `coax/web/tool.sh coax/web/cull-check.coax URL`
runs it in headless Chrome and checks it.
