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
    /// Sun shadow cascades across the view.
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

impl Tier {
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// What this tier draws.
    #[must_use]
    pub const fn quality(self) -> Quality {
        match self {
            Self::Low => Quality {
                tier: self,
                cascades: 1,
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
                cascades: 4,
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
            assert!(tier.quality().cascades >= 1);
        }
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
