# Slay facial performance and fixed-shot progress — 2026-11-06

Issue #12962 remains open. The owner extended its face-only request with the
Slay trailer shot at 0:57: narrowed eyes, raised left palm, blue orb and sword.
This record qualifies source transport/playback components, not visual parity.

![Owner reference left; current Aethyr shot right](comparison.png)

The local view is `http://127.0.0.1:8894/worlds/valley-canyon?pack=valley-slay`.
It starts paused at TF0290 sequence frame1125: body124, face4. P plays/pauses,
comma/period step, J returns to the shot; F selects isolated face performance,
brackets select clips, T restarts, B restores neutral and R returns to walking.
The camera is the source 40mm/24.889999mm filmback, 2.4 aspect. Source handheld
camera shake is now available in the [handheld source profile](../slay-camera-2026-11-06/README.md),
using `?pack=valley-slay-handheld`. The source actor transform track is disabled;
applying its serialized keys again would double-transform the animation root.

## Source transport

SDK reads use the clean private SlayData project and the installed UE5.8 SDK.
They load data-only packages and do not run source Blueprint events or save
source packages. The 794,477,568-byte private VPK has SHA256
`9c285e9ffa59b850e93337c4089be86a39e7c54f52cda51419fe86eac7ef4074`.
ECP3 binds the unchanged rig, full EFC1 morph/face source and SHS1 v2 fixed-shot
samples. CPU state retains a checked face projection and the admitted shot;
GPU storage retains sparse position/normal deltas. No licensed source payloads
are committed. The comparison image is a QA observation, not training data.

The full independent SDK/glTF comparison checks all564,903 sparse deltas and
GPU CSR records bit-for-bit, all143,750 curve samples bit-for-bit, all18,400
facial local transforms and27,000 shot body locals as exact float32 values.
Signed-zero differences are ignored for local-transform numeric comparison.
See [the receipt](source-oracle.json). The oracle is
`aethyr/tools/slay_asset_oracle.py`; source packing/playback is Coax.

Twenty source clips contain131 raw curves each.125 bind actual mesh morphs.
`blendParent1`, `tongueOut`, `jawFwd`, `CTRL_rigLogic_OnOff`,
`mouth_cornerPull_cor` and `mouth_stretch_cor` are named in SDK receipts but
have no mesh morph target; no invented mapping is applied.16 FACIAL tracks
bind both mesh reference skeletons. `facial_c_skull` and `facial_c_chin` are
unweighted source skeleton leaves absent from both mesh reference skeletons.
Body poses are retained separately, so face layering never accumulates on
itself or changes the locomotion/collision decision. Ambient source segments
are TF0130[4..22], TF0145[3..21], TF0250[19..40], every8s with.15s fades.

## Checks and limitations

- EFC1 admission:402/402 mutants killed; SHS1 v2:190/190;
  ECP3/legacy rig admission:246/246; whole-package SHA:13/13. No equivalents.
- Source frame round trip produces identical image hashes; the neighboring
  frame differs. Body/face/camera playback advances on the same source clock.
- Current Canyon checks:10/10, including walking, jump taps, non-inverted
  mouse/touch/arrow look, conservative collision and desktop/phone/DPR budgets.
- Ambient timing and non-face local-pose preservation checks pass.
- Current Wasm is91,877 bytes within96,000. The original80,000 budget was
  raised explicitly for shared facial playback and the source-shot view.
- **Earlier default GPU recovery failure, now superseded:** both new shot and face observations exceeded
  the legacy host's10-second replacement deadline under the observed machine
  load. Earlier face-only recovery passed before the shot extension; that
  result does not qualify this package. [The shot failure is retained](shot-gpu-red.txt).
  A separately labeled60s diagnostic recovered in9,697ms with identical
  pixels/state, then passed ambient locomotion and the390x844 phone viewport.
  [Its receipt](diagnostic-gpu.txt) does not qualify the unchanged default10s fence.
  The [retained-source fix and visible-capture record](../valley-recovery-2026-11-06/README.md)
  now qualify the unchanged default deadline with Coax 0.52: face recovery in
  9,032 ms and source-shot recovery in 8,289 ms. The latter makes the private
  source unavailable after its first request and verifies no second request.
  Both retain exact producer state and measured GPU pixels. These observations
  replace the recovery limitation only; the cinematic differences below remain.

The source project disables bloom. The source orb mesh and its original texture layers now use Coax panning,
normal distortion, source Fresnel/tint/HDR/desaturation arithmetic and an
additive colour draw. Its halo/flare remains analytic; source curl-noise
vertex displacement and the original Niagara flare are not yet ported. Key/fill/wrap samples drive an interim direct-light
model. Proper area-light integration, regional skin scattering, eyes/refraction,
strand coverage/shading, source orb texture/flare and temple scenery
remain. These differences are visible in the comparison and prevent claiming
an exact shot recreation or closing the issue.

Slay render source extension:101,672 UV pairs,25,547 sword vertices,
134,418 sword indices and35 bundled textures pass the independent source-byte
oracle. The sword follows the source `prop_01` track through the existing
`groom.prop_01` palette entry135; its mesh inverse bind is the same. Slay's
head instance uses Desat=.3 and Tint=(.921582,1,.871367), unlike Valley's .2
and(.9,1,.949020). Its actual textures replace Valley counterparts in this
private package.35 mipmapped4K RGBA8 layers reserve approximately3.13GB of
GPU texture storage; the lab's explicit aggregate budget is4.6GB. A phone
viewport observation on this Mac does not qualify mobile hardware memory
or performance. A separate mobile/source-detail profile remains necessary.

The orb extension adds559 exact source vertices,2,880 indices and1,114,112
original RGBA texels, including independent checks of the full packed GPU
streams. Its world hand quaternion matches independent double precision over
all200 frames within1.93e-7. [SHS1 v2](private-shot-format.md) explicitly versions
that new quaternion metadata and validates its unit length before publication.
Distortion remains linear and normal-mapped; the energy texture decodes sRGB
before bilinear filtering, matching the native SDK texture flags.

The opt-in legacy browser `coaxial_draw_blend_v1` keeps ordinary geometry
opaque, selects straight-alpha additive colour for the orb and source-over
coverage for strand colour. Translucent colour depth-tests without depth
writes; shadow coverage remains shader-owned. The unchanged599558 indexed
module without this export and the parent lab pass on the new host. The
actual orb GPU comparison adds colour to171,054 supersampled pixels and
darkens none; the scene remains ready on return to ordinary walking. Hair
colour uses continuous coverage instead of stochastic opaque colour samples;
its shadow keeps separate stochastic coverage. This improves the visible
noise but is not Marschner scattering, deep opacity or sorted/OIT hair.

The pixel budget bounds the base raster before the explicit3x portrait
supersampling: the1280x800 observation captures3840x2400 physical pixels.
The additive operation currently occurs in the legacy display-mapped target;
full linear-HDR compositing, the source material-time phase,
exact reference frame selection and cinematic background remain open. The
handheld profile imports the exact source camera template; its recovery and
mobile cost limitations are retained in that separate record.
