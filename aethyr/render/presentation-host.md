# Host presentation selection

`render_host_presentation(version,profile,width,height,format,maximum)` returns
`RenderHostPresentation {enabled:bool, plan:RenderPresentation}`. Only host
contract 1 and the qualified Everglade profile 2 enable the shared quantized
presentation. Its single version/profile decision is `hrm_presentation` in
`aethyr/host/render-mode.coax`; the small web service companion can project
that scalar without rebuilding a realm or importing a second renderer.

When enabled, the result is the existing checked
`render_presentation_quantized_v1` descriptor: complete graph into RGBA16Float,
then existing media entry 13 into the actual RGBA8/BGRA8 display order. The
descriptor supplies source, bindings, sampler, state, extent and uniforms.
An invalid eligible request remains `enabled=true` with a refused plan.
The adapter must fail it explicitly; it must not choose a legacy fallback.
Unsupported profiles and pre-contract faces return disabled with an empty
plan and retain their existing host path. Contract-version admission remains
the host's separate existing gate.

Checks precede implementation. The scalar decision kills 5/5 root mutants;
the final typed forwarding gate kills 2/2, without equivalents. Actual SDK
adoption, resource affordability/recovery and ordinary frame capture remain
separate from this policy. Merely loading this file changes no host selector,
realm opt-in or desktop default.
