# Haunted House scene-readiness evidence

The 2026-10-29 release of `8402fa4267` passed the staging walk, but its first
production landing-to-house visit drew 0.3% lit pixels against the unchanged
3% minimum. The later galaxy-to-house visit drew 17.0%. Production was rolled
back. Five later direct visits to the rolled-back production site all drew
17.0%; no screenshot or instrumentation from the failing frame established
its cause. This change fixes an independently observed readiness gap, not a
proven diagnosis of that failure.

The host's existing `coaxial.ready` means the engine loaded. It can become
true before WebGPU setup, asynchronous asset decoding, scene binding or a
scene frame. `coaxial.sceneReady` is a separate observation: a nonempty indexed
scene frame for the current installed engine was submitted and the GPU queue
completed, after all active shader loads settled. Engine requests and shader
load starts and completions invalidate it. It does not prove shader request
ordering, GPU validation success or visible pixels. The Haunted House walk
requires this observation within a bounded 20 seconds and still performs its
existing error and 3% pixel checks without retrying or lowering the
threshold; its network-quiet behavior is unchanged.

A real host/Wasm/WebGPU browser fixture passed seven checks covering delayed
assets and GPU work, missing assets, empty/zero-index/zero-instance draws,
engine replacement, concurrent shader loads and a delayed particle build.
The affected storage fixture passed six tests after its local server was
updated to serve the host's existing module imports and isolate storage replies
from viewer messages. The visual invariant check passed. A local-only proxy
served this host alongside the actual staging Haunted House page, Wasm and
assets: the scene observation succeeded and the unchanged pixel check measured
17.0%. This was not a production deployment or a test of the earlier failed
frame.

An independent source review found no concrete stale-engine or unsettled-shader
path satisfying the observation under this scope. It did not rerun tests and
did not approve deployment. Shader install order and target GPU behavior
remain separate questions.
