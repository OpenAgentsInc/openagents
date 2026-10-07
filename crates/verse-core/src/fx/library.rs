//! The effect library: every effect under `assets/verse/fx/effects/`,
//! compiled into the binary so the web and phone builds have them too.
//!
//! On the desktop, `VERSE_FX_RELOAD=1` rereads the directory at most once a
//! second when a file's modification time changes, so an effect can be
//! tuned while the game runs. A file that fails to parse keeps the last
//! good library and logs why.

use std::sync::{Arc, Mutex, OnceLock};

use super::def::Effect;

macro_rules! effect {
    ($name:literal) => {
        (
            $name,
            include_str!(concat!(
                "../../../../assets/verse/fx/effects/",
                $name,
                ".toml"
            )),
        )
    };
}

/// The effect files, by name. A new effect is a new TOML file and a line
/// here.
pub const SOURCES: [(&str, &str); 75] = [
    effect!("water_sleet"),
    effect!("water_entry_splash"),
    effect!("water_droplets"),
    effect!("water_drips"),
    effect!("water_crest_spray"),
    effect!("water_steam_puff"),
    effect!("water_splash"),
    effect!("water_wade"),
    effect!("water_falls_spray"),
    effect!("water_bubbles"),
    effect!("water_steam"),
    effect!("water_rain"),
    effect!("water_mist"),
    effect!("meteor_head"),
    effect!("meteor_trail"),
    effect!("meteor_explosion"),
    effect!("cast_embers"),
    effect!("scorch_embers"),
    effect!("debris_dust"),
    effect!("chimney_smoke"),
    effect!("butterflies"),
    effect!("greco_candle_glow"),
    effect!("grove_flame_bolt"),
    effect!("grove_flame_hit"),
    effect!("grove_star_mote"),
    effect!("grove_star_hit"),
    effect!("grove_poison_puff"),
    effect!("grove_vines"),
    effect!("grove_faerie"),
    effect!("grove_ice_shatter"),
    effect!("grove_heal"),
    effect!("grove_moonbeam"),
    effect!("grove_thorns"),
    effect!("grove_lightning"),
    effect!("grove_storm_cloud"),
    effect!("grove_hail"),
    effect!("grove_fire_wall"),
    effect!("grove_sunbeam"),
    effect!("grove_sunburst"),
    effect!("grove_necrotic"),
    effect!("grove_acid"),
    effect!("grove_cast_burst"),
    effect!("grove_cold_cone"),
    effect!("grove_fire_cone"),
    effect!("grove_lightning_line"),
    effect!("grove_stink_cloud"),
    effect!("grove_swarm"),
    effect!("grove_fog"),
    effect!("grove_sleet"),
    effect!("crypt_steam"),
    effect!("cauldron_bubbles_green"),
    effect!("cauldron_bubbles_red"),
    effect!("cauldron_bubbles_amber"),
    effect!("crypt_dust"),
    effect!("crypt_fog"),
    effect!("candle_glow"),
    effect!("brazier_fire"),
    effect!("grove_shapechange_rune"),
    effect!("grove_shapechange_vortex"),
    effect!("grove_shapechange_flash"),
    effect!("grove_dragon_breath"),
    effect!("grove_dragon_roar"),
    effect!("grove_tail_sweep"),
    effect!("grove_wing_buffet"),
    effect!("grove_burning"),
    effect!("grove_area_ring"),
    effect!("grove_fireflies"),
    effect!("grove_campfire"),
    effect!("grove_torch"),
    effect!("grove_brazier"),
    effect!("grove_lantern_glow"),
    effect!("grove_rune_glow"),
    effect!("grove_thorn_wall"),
    effect!("thunderbolt_strike"),
    effect!("tower_crash"),
];

/// Parsed effects, in [`SOURCES`] order.
#[derive(Debug)]
pub struct Library {
    pub effects: Vec<Effect>,
}

impl Library {
    /// Parses `sources`.
    ///
    /// # Errors
    ///
    /// Returns the first effect that doesn't parse or check.
    pub fn parse<'a>(
        sources: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Result<Self, String> {
        let effects = sources
            .into_iter()
            .map(|(name, source)| Effect::parse(name, source))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { effects })
    }

    /// The compiled-in effects.
    ///
    /// # Panics
    ///
    /// Panics if a compiled-in effect is invalid, which the tests rule out.
    #[must_use]
    pub fn builtin() -> Self {
        Self::parse(SOURCES).expect("compiled-in effects parse")
    }

    #[must_use]
    pub fn index(&self, name: &str) -> Option<usize> {
        self.effects.iter().position(|e| e.name == name)
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Effect> {
        self.effects.iter().find(|e| e.name == name)
    }

    /// The process's library and its generation, which a reload bumps.
    #[must_use]
    pub fn shared() -> (Arc<Self>, u64) {
        let state = shared_state().lock().unwrap_or_else(|e| e.into_inner());
        (state.library.clone(), state.generation)
    }

    /// The library when it is newer than `generation`, reloading first
    /// when hot reload is on and its files changed.
    #[must_use]
    pub fn shared_if_newer(generation: u64) -> (Option<Arc<Self>>, u64) {
        let mut state = shared_state().lock().unwrap_or_else(|e| e.into_inner());
        reload::poll(&mut state);
        if state.generation > generation {
            (Some(state.library.clone()), state.generation)
        } else {
            (None, state.generation)
        }
    }
}

struct Shared {
    library: Arc<Library>,
    generation: u64,
    #[allow(dead_code)]
    watch: reload::Watch,
}

fn shared_state() -> &'static Mutex<Shared> {
    static SHARED: OnceLock<Mutex<Shared>> = OnceLock::new();
    SHARED.get_or_init(|| {
        Mutex::new(Shared {
            library: Arc::new(Library::builtin()),
            generation: 0,
            watch: reload::Watch::new(),
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
mod reload {
    use std::path::PathBuf;
    use std::time::{Instant, SystemTime};

    use super::{Library, SOURCES, Shared};

    pub struct Watch {
        dir: Option<PathBuf>,
        checked: Option<Instant>,
        stamps: Vec<Option<SystemTime>>,
    }

    impl Watch {
        pub fn new() -> Self {
            let on = std::env::var("VERSE_FX_RELOAD").is_ok_and(|v| v == "1");
            let dir = on.then(|| {
                std::env::var_os("VERSE_FX_DIR").map_or_else(
                    || {
                        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../assets/verse/fx/effects")
                    },
                    PathBuf::from,
                )
            });
            let mut watch = Self {
                dir,
                checked: None,
                stamps: Vec::new(),
            };
            watch.stamps = watch.read_stamps();
            watch
        }

        fn read_stamps(&self) -> Vec<Option<SystemTime>> {
            let Some(dir) = &self.dir else {
                return Vec::new();
            };
            SOURCES
                .iter()
                .map(|(name, _)| {
                    std::fs::metadata(dir.join(format!("{name}.toml")))
                        .and_then(|m| m.modified())
                        .ok()
                })
                .collect()
        }
    }

    pub fn poll(state: &mut Shared) {
        let Some(dir) = state.watch.dir.clone() else {
            return;
        };
        if state
            .watch
            .checked
            .is_some_and(|t| t.elapsed().as_secs_f32() < 1.0)
        {
            return;
        }
        state.watch.checked = Some(Instant::now());
        let stamps = state.watch.read_stamps();
        if stamps == state.watch.stamps {
            return;
        }
        state.watch.stamps = stamps;
        let sources: Result<Vec<(&str, String)>, String> = SOURCES
            .iter()
            .map(|(name, _)| {
                std::fs::read_to_string(dir.join(format!("{name}.toml")))
                    .map(|s| (*name, s))
                    .map_err(|e| format!("{name}: {e}"))
            })
            .collect();
        let parsed = sources
            .and_then(|sources| Library::parse(sources.iter().map(|(n, s)| (*n, s.as_str()))));
        match parsed {
            Ok(library) => {
                state.library = std::sync::Arc::new(library);
                state.generation += 1;
                eprintln!("verse: reloaded particle effects from {}", dir.display());
            }
            Err(error) => eprintln!("verse: kept the last particle effects: {error}"),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod reload {
    use super::Shared;

    pub struct Watch;

    impl Watch {
        pub fn new() -> Self {
            Self
        }
    }

    pub fn poll(_: &mut Shared) {}
}
