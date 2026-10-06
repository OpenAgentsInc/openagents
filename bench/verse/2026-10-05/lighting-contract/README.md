# Shared lighting contract references

These captures exercise the authored chamber, physical daylight stage, and
physical space-sky paths at low, medium, and high quality. They use one admitted
original procedural character, a gray card with linear base color 0.18, and a
metal card with base color 0.65, metallic 1, and perceptual roughness 0.2. The
backdrop and geometry are controlled references, rather than full forest or
station assets. They are engineering comparisons, not approved production art.

| Reference | Point units | Base exposure | Output grade | Active shadow views, low/medium/high |
| --- | --- | --- | --- | --- |
| Torch | Authored relative intensity | 1 | Chamber, +0.77 stops | 6 / 12 / 24 |
| Spell | Authored relative intensity | 1 | Chamber, +0.77 stops | 6 / 12 / 24 |
| Forest light | Candela; directional/ambient lux | EV100 12 | Neutral | 2 / 2 / 3 |
| Station light | Directional lux, physical space sky | EV100 15 | Neutral, fixed camera | 1 / 1 / 1 |

Each JSON file records the generated content digest, exposure, selected points,
shadow sources and size, ambient profile, quality, adapter, and measured
character-region difference from an otherwise identical frame without the
character. The comparison requires more than 100 changed pixels and p95 RGB
code-value difference above 12. Images and region bounds make this metric
reviewable. It establishes visibility in this profile, rather than perceptual
quality, correct tone reproduction on a physical display, or zone-wide readability.

Physical stage lamp selection ranks visible contribution. The source in slot 31
survives all 8/16/32-lamp budgets. Chamber local shadows select it at every tier;
the chamber retains its bounded 32-source shading loop. Both native shaders
expand one attenuation and coverage contract. The profiles preserve distinct
ambient behavior and light units. Recent dusk and split-toning settings remain
available; these controlled references use the grades recorded above.

Shadow-view counts include cached maps. Chamber shadow sizes are 256 pixels at
low and 512 at medium/high; physical sun maps are 2048 pixels. These limits bound
active map work and logical texels; they do not prove a device frame deadline.
Chamber timing fields include capture/readback work, and GPU timestamp queries
are disabled. No additional GI, reflection, shadow, or temporal pass is added.

The pinned-toolchain checks pass 155 engine tests, 80 renderer tests with seven
explicit GPU helpers ignored, and ten GLES/Metal translation checks in the Verse
consumer. Each selected Vulkan capture run passes separately. Source,
commands, logs, and artifact SHA-256 values are retained. The focused consumer
check uses `--no-default-features`; captures use the imported-surface renderer
and physical renderer on an NVIDIA GeForce RTX 4080 at 640 × 360.

The shared disk filled during a consumer dependency build. The existing warm
Cargo target stays in use; the retained compiler wrapper redirects outputs to
private RAM and preserves earlier archives behind their logical paths. It does
not alter the declared compiler optimization/debug settings. Some behavior
checks use explicit package-specific zero-debug/zero-optimization settings.
The relocation remains temporary until persistent capacity is available.

Full-zone art review, exposure-adaptation traversal, broader content materials,
and actual phone/browser measurements remain outside this controlled profile.

| Scene | Low | Medium | High |
| --- | --- | --- | --- |
| Torch | [Capture](torch-low.png) | [Capture](torch-medium.png) | [Capture](torch-high.png) |
| Spell | [Capture](spell-low.png) | [Capture](spell-medium.png) | [Capture](spell-high.png) |
| Forest light | [Capture](forest-low.png) | [Capture](forest-medium.png) | [Capture](forest-high.png) |
| Station light | [Capture](station-low.png) | [Capture](station-medium.png) | [Capture](station-high.png) |
