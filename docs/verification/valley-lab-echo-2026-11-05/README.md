# Echo source port qualification

This record qualifies source import and the explicitly selected Aethyr walking
profile. It does not establish Unreal gameplay or image parity.

`source-oracle.json` records full-buffer source comparisons from the independent
`aethyr/tools/echo_asset_oracle.py`. All 313,371 vertices, 1,470,318 indices,
334 bind/rest transforms, 2,696 source frames, 492,907 strand segments and
32 original PNG files are checked. The combined pack retains the Canyon
geometry prefix and original placements. Source data lives exclusively in
`~/.openagents/aethyrforge/valley-lab/`; no licensed payload is in this record.

The current SDK is installed UE 5.8. Source Valley's native game modules target
an incompatible build, so a clean data-only export project reads asset defaults.
The SDK plugin is our own editor-only reader and optional offscreen oracle.
The product is Coax/Wasm/WebGPU; it never embeds an Unreal renderer.

The source velocity, locomotion, walking, collision, rig and pack-policy mutation
runs killed 98, 25, 598, 33, 165 and 13 mutants respectively, with no survivors
or equivalent annotations. Logs were `/tmp/valley-echo-source-motion-mutation.log`,
`/tmp/valley-echo-locomotion-mutation.log`,
`/tmp/valley-echo-walk-mutation-final.log`,
`/tmp/valley-echo-collision-mutation.log`,
`/tmp/valley-echo-rig-mutation.log` and
`/tmp/valley-echo-pack-gate-final.log`. These unchanged results are not rerun.
The independent velocity oracle has 1,024 strict float32 cases. Film mapping
has 97 independent input vectors; retained logs also include 427 app checks
and 39 shader checks. The film approximation is not the complete Unreal LUT.

Remaining visual gaps include eye refraction/occlusion, regional skin scattering,
strand shading/coverage, and unplayed facial morphs. Native Idle itself has
nonzero eye-look morph curves; skinning alone cannot reproduce its face.
The optional native reference groom is visibly broken and does not qualify
hair parity. Full AnimBP/camera/ability graphs, vaulting, cloth and hair
simulation remain separate work. #12962 adds Slay facial playback, and the
owner's specific trailer frame additionally needs its body pose and orb.
