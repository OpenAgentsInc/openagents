//! Rendering quality tiers: what a device draws, decided once from what its
//! adapter offers and the platform it runs on.
//!
//! The renderer probes the adapter, the tier follows from the probe, and the
//! tier fixes every effect that scales: the sun's cascade count, the shadow
//! filter, screen-space effects, probe resolution, and material detail. An
//! effect that a tier turns off must leave the frame correct, only plainer.
//! The low tier is the WebGL2 and OpenGL ES floor: vertex and fragment work
//! only, with no compute shaders and no storage buffers.

/// A quality tier, ordered from the floor up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// WebGL2, OpenGL ES, and adapters without a floating-point scene target.
    Low,
    /// Phones and WebGPU in a browser.
    Medium,
    /// Desktop GPUs with compute shaders and four-sample multisampling.
    High,
}

/// Where the renderer runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    Desktop,
    /// iOS and Android: tile-based GPUs, where full-screen passes cost
    /// memory bandwidth.
    Mobile,
    /// A browser, through WebGPU or WebGL2.
    Web,
}

impl Platform {
    /// The platform this binary was built for.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_arch = "wasm32") {
            Self::Web
        } else if cfg!(any(target_os = "ios", target_os = "android")) {
            Self::Mobile
        } else {
            Self::Desktop
        }
    }
}

/// What the renderer learned about the adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Probe {
    pub platform: Platform,
    /// Shaders compile to GLSL ES: WebGL2, or Android without Vulkan.
    pub gles: bool,
    /// A floating-point scene target can be rendered, filtered, and blended.
    pub float_target: bool,
    /// Compute shaders and storage buffers are available.
    pub compute: bool,
    /// The scene target's multisample count.
    pub samples: u32,
}

impl Probe {
    /// The highest tier this device can run.
    #[must_use]
    pub fn ceiling(&self) -> Tier {
        if self.gles || !self.float_target {
            return Tier::Low;
        }
        match self.platform {
            Platform::Web | Platform::Mobile => Tier::Medium,
            Platform::Desktop if self.compute && self.samples >= 4 => Tier::High,
            Platform::Desktop => Tier::Medium,
        }
    }

    /// The tier to draw: the device's ceiling, or a lower tier the operator
    /// asked for. A request above the ceiling is held to the ceiling.
    #[must_use]
    pub fn select(&self, requested: Option<Tier>) -> Tier {
        let ceiling = self.ceiling();
        requested.map_or(ceiling, |tier| tier.min(ceiling))
    }
}

/// How the sun's shadow map is filtered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShadowFilter {
    /// Percentage-closer filtering with a penumbra fixed by the light's
    /// angular size, as if every occluder stood 1 m from its receiver. It
    /// needs no blocker search, so it never reads the depth values.
    Fixed,
    /// Percentage-closer soft shadows: a blocker search sets each penumbra.
    Soft,
}

/// Material features a tier keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Materials {
    /// Procedural and mapped surface normals. Without them a surface shades
    /// with its geometric normal.
    pub detail_normals: bool,
    /// Every surface shades fully rough and non-metallic, with no specular
    /// highlight to alias. Reserved for devices below the low tier.
    pub fully_rough: bool,
}

/// Everything a tier fixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Quality {
    pub tier: Tier,
    /// Sun shadow cascades across the view: two on phones and WebGL2, three
    /// on desktops, at most [`crate::lighting::MAX_CASCADES`].
    pub cascades: u32,
    pub shadow_filter: ShadowFilter,
    /// Screen-space ambient occlusion and contact shadows, which need a
    /// readable single-sample depth buffer from a depth prepass.
    pub screen_space: bool,
    /// The most cells along any axis of a baked irradiance probe grid.
    pub probe_cells: u32,
    pub materials: Materials,
}

impl Quality {
    /// Shared admission limits. Timings are targets, not a deadline that cancels a frame.
    pub const fn budget(&self) -> Budget {
        let (effects, memory) = match self.tier {
            Tier::Low => (128, 128 * 1024 * 1024),
            Tier::Medium => (384, 256 * 1024 * 1024),
            Tier::High => (768, 1024 * 1024 * 1024),
        };
        Budget {
            instances: crate::presentation::ResolvedInstances::MAX_INSTANCES,
            optional_effects: effects,
            surfaces: 16_384,
            // A Retina desktop window at the medium tier's multisampling needs
            // about 260 MB of targets, and a full-screen one about 430 MB.
            target_bytes: match self.tier {
                Tier::Medium => 512 * 1024 * 1024,
                _ => memory,
            },
            geometry_bytes: memory,
            texture_bytes: memory,
            buffer_bytes: memory,
            cpu_frame_ms: 1000. / 60.,
            gpu_frame_ms: 1000. / 60.,
        }
    }
    pub const fn sample_ceiling(&self) -> u32 {
        if !matches!(self.tier, Tier::Low) {
            4
        } else {
            1
        }
    }
    /// Cube-shadow views for the chamber; the physical path uses its cascade count.
    pub const fn local_shadow_views(&self) -> u32 {
        match self.tier {
            Tier::Low => 6,
            Tier::Medium => 12,
            Tier::High => 24,
        }
    }
    pub const fn local_shadow_size(&self) -> u32 {
        if matches!(self.tier, Tier::Low) {
            256
        } else {
            512
        }
    }
    /// The sky light's reflection cube: its edge in texels at the sharpest
    /// level, and the GGX samples averaged per texel of each blurrier level.
    /// Every tier builds it on the CPU and samples it with an explicit level
    /// of detail, so the low tier draws it as the others do, only coarser.
    #[must_use]
    pub const fn sky_cube(&self) -> (u32, u32) {
        match self.tier {
            Tier::Low => (16, 32),
            Tier::Medium => (32, 64),
            Tier::High => (64, 64),
        }
    }
}

/// Resource limits shared by rendering adapters; byte counts exclude driver-private storage.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Budget {
    pub instances: usize,
    pub optional_effects: usize,
    pub surfaces: usize,
    pub target_bytes: u64,
    pub geometry_bytes: u64,
    pub texture_bytes: u64,
    pub buffer_bytes: u64,
    pub cpu_frame_ms: f64,
    pub gpu_frame_ms: f64,
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Resources {
    pub target_bytes: u64,
    pub geometry_bytes: u64,
    pub texture_bytes: u64,
    pub buffer_bytes: u64,
    pub retained_source_bytes: u64,
}
impl Budget {
    /// Reject resource excess before an adapter allocates or replaces active targets.
    pub fn admit(&self, value: Resources) -> Result<(), String> {
        for (name, used, limit) in [
            ("target", value.target_bytes, self.target_bytes),
            ("geometry", value.geometry_bytes, self.geometry_bytes),
            ("texture", value.texture_bytes, self.texture_bytes),
            ("buffer", value.buffer_bytes, self.buffer_bytes),
        ] {
            if used > limit {
                return Err(format!(
                    "Renderer {name} bytes exceed the admitted quality budget"
                ));
            }
        }
        Ok(())
    }
}

impl Tier {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// What this tier draws.
    #[must_use]
    pub const fn quality(self) -> Quality {
        match self {
            Self::Low => Quality {
                tier: self,
                cascades: 2,
                shadow_filter: ShadowFilter::Fixed,
                screen_space: false,
                probe_cells: 8,
                materials: Materials {
                    detail_normals: false,
                    fully_rough: false,
                },
            },
            Self::Medium => Quality {
                tier: self,
                cascades: 2,
                shadow_filter: ShadowFilter::Soft,
                screen_space: false,
                probe_cells: 16,
                materials: Materials {
                    detail_normals: true,
                    fully_rough: false,
                },
            },
            Self::High => Quality {
                tier: self,
                cascades: 3,
                shadow_filter: ShadowFilter::Soft,
                screen_space: true,
                probe_cells: 32,
                materials: Materials {
                    detail_normals: true,
                    fully_rough: false,
                },
            },
        }
    }

    /// The tier a name selects: `low`, `medium`, or `high`, in any case.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    /// The tier's lowercase name, as [`Tier::parse`] reads it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resource_excess_is_refused_and_lower_tiers_reduce_optional_work() {
        for tier in Tier::ALL {
            let quality = tier.quality();
            let budget = quality.budget();
            assert!(
                budget
                    .admit(Resources {
                        target_bytes: budget.target_bytes,
                        geometry_bytes: budget.geometry_bytes,
                        texture_bytes: budget.texture_bytes,
                        buffer_bytes: budget.buffer_bytes,
                        retained_source_bytes: 0
                    })
                    .is_ok()
            );
            assert!(
                budget
                    .admit(Resources {
                        target_bytes: budget.target_bytes + 1,
                        ..Default::default()
                    })
                    .is_err()
            );
            assert!(
                budget
                    .admit(Resources {
                        geometry_bytes: budget.geometry_bytes + 1,
                        ..Default::default()
                    })
                    .is_err()
            );
            assert!(
                budget
                    .admit(Resources {
                        texture_bytes: budget.texture_bytes + 1,
                        ..Default::default()
                    })
                    .is_err()
            );
        }
        assert_eq!(Tier::Low.quality().sample_ceiling(), 1);
        assert!(
            Tier::Low.quality().local_shadow_views() < Tier::High.quality().local_shadow_views()
        );
        assert!(
            Tier::Low.quality().budget().optional_effects
                < Tier::High.quality().budget().optional_effects
        );
    }

    fn probe(platform: Platform) -> Probe {
        Probe {
            platform,
            gles: false,
            float_target: true,
            compute: true,
            samples: 4,
        }
    }

    #[test]
    fn webgl2_and_gles_select_the_low_tier() {
        for platform in [Platform::Web, Platform::Mobile, Platform::Desktop] {
            let gles = Probe {
                gles: true,
                compute: false,
                ..probe(platform)
            };
            assert_eq!(gles.select(None), Tier::Low);
            // A WebGL2 context with float render targets is still the floor.
            assert_eq!(gles.select(Some(Tier::High)), Tier::Low);
            let no_float = Probe {
                float_target: false,
                ..probe(platform)
            };
            assert_eq!(no_float.select(None), Tier::Low);
        }
    }

    #[test]
    fn platforms_and_adapters_set_the_ceiling() {
        assert_eq!(probe(Platform::Web).select(None), Tier::Medium);
        assert_eq!(probe(Platform::Mobile).select(None), Tier::Medium);
        assert_eq!(probe(Platform::Desktop).select(None), Tier::High);
        let no_compute = Probe {
            compute: false,
            ..probe(Platform::Desktop)
        };
        assert_eq!(no_compute.select(None), Tier::Medium);
        let single_sample = Probe {
            samples: 1,
            ..probe(Platform::Desktop)
        };
        assert_eq!(single_sample.select(None), Tier::Medium);
    }

    #[test]
    fn a_request_can_lower_the_tier_but_never_raise_it() {
        let desktop = probe(Platform::Desktop);
        for tier in Tier::ALL {
            assert_eq!(desktop.select(Some(tier)), tier);
        }
        let phone = probe(Platform::Mobile);
        assert_eq!(phone.select(Some(Tier::Low)), Tier::Low);
        assert_eq!(phone.select(Some(Tier::High)), Tier::Medium);
    }

    #[test]
    fn tiers_scale_every_effect_monotonically() {
        for pair in Tier::ALL.windows(2) {
            let (lower, higher) = (pair[0].quality(), pair[1].quality());
            assert!(lower.cascades <= higher.cascades);
            assert!(lower.probe_cells <= higher.probe_cells);
            assert!(!lower.screen_space || higher.screen_space);
            assert!(!lower.materials.detail_normals || higher.materials.detail_normals);
            assert!(
                lower.shadow_filter == ShadowFilter::Fixed
                    || higher.shadow_filter == ShadowFilter::Soft
            );
        }
        for tier in Tier::ALL {
            assert_eq!(tier.quality().tier, tier);
            let cascades = tier.quality().cascades as usize;
            assert!((1..=crate::lighting::MAX_CASCADES).contains(&cascades));
        }
        // Two cascades on phones and WebGL2, three on desktops.
        assert_eq!(Tier::Low.quality().cascades, 2);
        assert_eq!(Tier::Medium.quality().cascades, 2);
        assert_eq!(Tier::High.quality().cascades, 3);
        // The floor runs on WebGL2: no blocker search, which reads depth
        // values GLSL ES cannot read from a comparison-sampled texture, and
        // no screen-space pass, which needs a readable depth buffer.
        let low = Tier::Low.quality();
        assert_eq!(low.shadow_filter, ShadowFilter::Fixed);
        assert!(!low.screen_space);
        // Every tier has a sky cube; higher tiers make it no coarser.
        for pair in Tier::ALL.windows(2) {
            let (lower, higher) = (pair[0].quality().sky_cube(), pair[1].quality().sky_cube());
            assert!(lower.0 <= higher.0 && lower.1 <= higher.1);
        }
        for tier in Tier::ALL {
            let (size, samples) = tier.quality().sky_cube();
            assert!(size.is_power_of_two() && size >= 16 && samples > 0);
        }
    }

    #[test]
    fn tier_names_round_trip() {
        for tier in Tier::ALL {
            assert_eq!(Tier::parse(tier.name()), Some(tier));
        }
        assert_eq!(Tier::parse(" HIGH "), Some(Tier::High));
        assert_eq!(Tier::parse("ultra"), None);
    }
}
