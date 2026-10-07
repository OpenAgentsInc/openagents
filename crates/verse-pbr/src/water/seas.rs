//! Sea states: the wave controls of a spectral sea, from
//! `assets/verse/water/seas/*.toml`. A file sets the wind speed, fetch,
//! peak wavelength (0 for JONSWAP's own), amplitude, choppiness,
//! directional spread, standing ratio, time scale, and tile (0 for eight
//! peak wavelengths); the zone sets the wind's direction, the seed, the
//! depth, and the clock ([`SeaState::spectrum`]). The values come from the
//! JONSWAP fetch laws for the named wind, not from any other product.

use std::sync::OnceLock;

use physics::water::Spectrum;
use serde::Deserialize;

/// One named sea state.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeaState {
    #[serde(skip)]
    pub name: String,
    pub description: String,
    pub wind_speed: f64,
    pub fetch: f64,
    pub peak_wavelength: f64,
    pub amplitude: f64,
    pub choppiness: f64,
    pub spread: f64,
    pub standing: f64,
    pub time_scale: f64,
    pub patch: f64,
}

/// The sea states built into this crate, calmest first.
const FILES: [(&str, &str); 3] = [
    (
        "calm",
        include_str!("../../../../assets/verse/water/seas/calm.toml"),
    ),
    (
        "moderate",
        include_str!("../../../../assets/verse/water/seas/moderate.toml"),
    ),
    (
        "storm",
        include_str!("../../../../assets/verse/water/seas/storm.toml"),
    ),
];

impl SeaState {
    /// Parses a sea state file named `name`.
    ///
    /// # Errors
    /// The text is not a sea state, or the spectrum it makes is out of
    /// range.
    pub fn parse(name: &str, text: &str) -> Result<Self, String> {
        let mut state: Self = toml::from_str(text).map_err(|e| format!("Sea state {name}: {e}"))?;
        state.name = name.into();
        state
            .spectrum(0.0, 1, 50.0)
            .validate()
            .map_err(|e| format!("Sea state {name}: {e}"))?;
        Ok(state)
    }

    /// Every built-in sea state, calmest first.
    ///
    /// # Panics
    /// When a built-in file does not parse, which a test rules out.
    #[must_use]
    pub fn all() -> &'static [SeaState] {
        static ALL: OnceLock<Vec<SeaState>> = OnceLock::new();
        ALL.get_or_init(|| {
            FILES
                .iter()
                .map(|(name, text)| Self::parse(name, text).expect("a built-in sea state parses"))
                .collect()
        })
    }

    /// The built-in sea state `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<&'static SeaState> {
        Self::all().iter().find(|s| s.name == name)
    }

    /// This sea with waves traveling toward `wind` (rad about +Y, 0 is +z)
    /// from `seed`, over water `depth` m deep, on a 120 Hz clock that loops
    /// every 256 s.
    #[must_use]
    pub fn spectrum(&self, wind: f64, seed: u64, depth: f64) -> Spectrum {
        Spectrum {
            seed,
            wind_speed: self.wind_speed,
            wind,
            fetch: self.fetch,
            peak_wavelength: self.peak_wavelength,
            amplitude: self.amplitude,
            choppiness: self.choppiness,
            spread: self.spread,
            standing: self.standing,
            time_scale: self.time_scale,
            depth,
            patch: self.patch,
            ..Spectrum::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file parses, and the seas rise from calm to storm.
    #[test]
    fn the_sea_states_parse_and_rise() {
        let all = SeaState::all();
        assert_eq!(all.len(), FILES.len());
        let heights: Vec<f64> = all
            .iter()
            .map(|s| s.spectrum(0.0, 1, 200.0).significant_height())
            .collect();
        assert!(heights.windows(2).all(|w| w[0] < w[1]), "{heights:?}");
        assert!(heights[0] < 0.4 && heights[2] > 3.0, "{heights:?}");
        assert!(SeaState::named("moderate").is_some());
    }
}
