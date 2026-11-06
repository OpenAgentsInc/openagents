# Slay facial performance and fixed-shot progress — 2026-11-06

Issue #12962 remains open. The owner extended its face-only request with the
Slay trailer shot at 0:57: narrowed eyes, raised left palm, blue orb and sword.
This record qualifies source transport/playback components, not visual parity.

## Skin, face and hair checkpoint

![Target, prior checkpoint and full Slay groom](skin-face-hair.png)

The main Slay groom is **different from Valley's groom**. Native SDK arrays
contain116,850 main-hair curves and2,401,356 points; the eyebrows retain the
same1,489 curves/18,839 points. The private Slay package now contains all
2,301,856 segments, without decimation, replacing the492,907 Valley segments.
The independent oracle checks every original endpoint, converted radius,
curve U and generated index, as well as the actual packed GPU groom bytes.
The main-hair SDK byte digests differ; eyebrow SDK bytes are identical.

Direct hair light now evaluates strand-tangent reflection (R), transmission
(TT), and internal reflection/transmission (TRT) instead of GGX/Lambert on
camera-facing ribbons. Source brown/pepper, roughness, specular and backlight
settings come from Slay's MI_EchoGroom. This is a single-fiber model with an
interim ambient term. It lacks the native groom's volume multiple scattering,
deep opacity, tangent breakup and simulated motion; it is visibly too dark.

The head now uses the source highpass R/G specular modulation and ORM.R mask,
with the native .08 conversion from material Specular to dielectric F0.
The unused Slay MatID texture slot31 now carries T_Head_highpass_S. The8K
source PNG is retained byte-exact in the private package; the existing4K
array upload resizes it through the browser SDK. Valley's legacy binding is
unchanged. Key, fill, wrap and orb evaluate separate half-vectors and GGX
responses; the fill retains its source.5 specular scale. Previously every
light multiplied a single highlight calculated for the key direction.
Skin remains too orange/glossy: regional roughness, proper rectangular-light
integration, skin diffusion, eye refraction and source grading remain open.

The native face audit confirms additive local-space/reference-pose flags and
exactly18 facial tracks, with no omitted head/neck tracks. At body124 these
facial locals already match reference locals within4.51e-7m translation and
8.89e-6 quaternion-component error. The additive flag therefore does not
explain the fixed-frame appearance mismatch. Nearby1125/1132/1137/1142/1147
captures show the source performance advancing; no invented squint or gaze
offset is applied. Exact trailer-frame/camera-shake matching remains open.

The full groom's vertex binding is187,564,308 bytes, beyond WebGPU's default
128MiB storage binding. The opt-in `coaxial_storage_limit_v1` requests that
exact limit through the SDK on initial open and replacement; absent/zero
keeps default limits. Unsupported adapters refuse. The full groom explicitly
raises this lab's vertex-work budget from21m to50m; its4.6GB aggregate budget
still passes the desktop/phone/DPR scalar cases. This is a desktop source
detail profile, not a mobile-device qualification.

Current checks:53 shader/helper checks, independent full source/groom oracle,
13/13 package-gate mutants and1/1 storage-limit mutant. Muted headless Metal
captures of five source frames, return to walking and isolated face pass
with no console errors. Wasm91,934 bytes remains within96,000. See
[the surface receipt](surface-checks.txt). The current full-groom60s diagnostic
recovers in16,145ms with identical state/pixels, then passes ambient walking
and the390x844 viewport. It does not qualify the default10s deadline. The
unchanged599558 indexed module without storage/blend opt-ins also passes on
the new host. The older admission mutation
results below apply to unchanged admission implementations at5d4baa60c1.

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
source packages. The 1,023,634,576-byte private VPK has SHA256
`4799fce88387284363fcceb957a8c31d1efbe3b76dac54ec405f32961cd88ce4`.
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
- Current Wasm is91,934 bytes within96,000. The original80,000 budget was
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
- **The full Slay groom puts default recovery back over the deadline.** The
  combined pack (full groom plus the TF0290 camera template, 1,023,634,576
  bytes, sha256 `5807c40f…dc0c`) fails the default 10 s face and shot
  recovery checks. The labeled 60 s diagnostic recovers in 11,811–14,190 ms
  with identical state at load averages of 6–8. Frames, the legacy host and
  both world viewports pass. Tracked in
  [#12975](https://github.com/OpenAgentsInc/openagents/issues/12975); the
  realm deadline stays at 10 s.

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
its shadow keeps separate stochastic coverage. The new single-fiber R/TT/TRT pass follows after this coverage correction;
deep opacity, multiple scattering and sorted/OIT hair remain unfinished.

The pixel budget bounds the base raster before the explicit3x portrait
supersampling: the1280x800 observation captures3840x2400 physical pixels.
The additive operation currently occurs in the legacy display-mapped target;
full linear-HDR compositing, the source material-time phase,
exact reference frame selection and cinematic background remain open. The
handheld profile imports the exact source camera template; its recovery and
mobile cost limitations are retained in that separate record.

## Skin pass and face view, 2026-11-06

- The exported `T_toksvig_macro` is zero in every sampled texel, in both the
  PNG and a fresh TGA re-export, so it is only a small Toksvig correction. The
  head's roughness is about 0.1–0.2, as in the source. `MF_PerPartRoughnessModulation`
  only scales it by 0.85–1.2 per region (plus 0.25 on the chest). Its region
  masks `T_Head_Skin_001/007/008/009_R` are not exported yet.
- The pinpoint "wet" highlights came from shading TF0290's 64 x 64 cm
  RectLight key, 345.18 cm away, as a point. The key and the orb now widen
  roughness with Unreal's sphere-light `New_a2` term (`SLAY_KEY_SIZE` 0.10462).
- Skin uses subsurface-profile style lighting: two specular lobes at 0.75x
  and 1.30x roughness, mixed 0.85, plus wrapped diffuse that carries red past
  the terminator and bends diffuse normals toward the geometric normal.
- The H and F face views now use the TF0290 hero frame's lights and the Slay
  skin profile, moved to Echo, through a longer lens (4.2 at 1.58 m). Before,
  they used Valley's single generic sun at 0.65 m with a wide lens.
- T or the on-screen REPLAY button replays the shot; playback stops on the last frame.
- Still open: orange albedo balance, eye shading, hair volume scattering, and
  the per-region roughness masks.
