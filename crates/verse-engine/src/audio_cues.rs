//! Original synthesized scene cues. No recordings or imported sound assets.
use crate::audio::Clip;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cue {
    Footstep,
    FireLaunch,
    Impact,
    Shield,
    RitualAmbience,
}

/// Synthesizes admitted mono PCM at the requested device rate. The seed controls
/// noise variation; callers retain clips and never synthesize in a device callback.
pub fn synthesize(cue: Cue, rate: u32, seed: u64) -> Result<Clip, String> {
    if !(8000..=192000).contains(&rate) {
        return Err("Invalid procedural audio sample rate".into());
    }
    let duration = match cue {
        Cue::Footstep => 0.18,
        Cue::FireLaunch => 0.8,
        Cue::Impact => 0.35,
        Cue::Shield => 0.7,
        Cue::RitualAmbience => 4.,
    };
    let count = (duration * f64::from(rate)).round() as usize;
    let mut state = seed;
    let mut filtered = 0.;
    let mut phase = 0.;
    let mut samples = Vec::with_capacity(count);
    for index in 0..count {
        let time = index as f64 / f64::from(rate);
        let progress = time / duration;
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let noise = ((state >> 32) as u32 as f64 / f64::from(u32::MAX)) * 2. - 1.;
        let cutoff = match cue {
            Cue::Footstep => 900.,
            Cue::FireLaunch => 1800.,
            Cue::Impact => 3000.,
            _ => 600.,
        };
        let smoothing = 1. - (-std::f64::consts::TAU * cutoff / f64::from(rate)).exp();
        filtered += smoothing * (noise - filtered);
        let attack = (time / 0.008).min(1.);
        let tail = ((duration - time) / 0.03).clamp(0., 1.);
        let sample = match cue {
            Cue::Footstep => {
                let thump = (std::f64::consts::TAU * 95. * time).sin() * (-time * 35.).exp();
                (filtered * 0.45 * (-time * 24.).exp() + thump * 0.45) * attack * tail
            }
            Cue::FireLaunch => {
                phase += std::f64::consts::TAU * (90. + 440. * progress) / f64::from(rate);
                (filtered * 0.65 + phase.sin() * 0.15)
                    * (std::f64::consts::PI * progress).sin().powf(0.8)
                    * attack
                    * tail
            }
            Cue::Impact => {
                (filtered * 0.65 + (std::f64::consts::TAU * 60. * time).sin() * 0.25)
                    * (-time * 18.).exp()
                    * attack
                    * tail
            }
            Cue::Shield => {
                let ring = [420., 630., 1050.]
                    .into_iter()
                    .map(|frequency| (std::f64::consts::TAU * frequency * time).sin())
                    .sum::<f64>()
                    / 3.;
                ring * 0.55 * (-time * 4.).exp() * attack * tail
            }
            // Integer cycle counts over four seconds make the loop boundary continuous.
            Cue::RitualAmbience => [55., 82.5, 110.]
                .into_iter()
                .enumerate()
                .map(|(i, frequency)| {
                    (std::f64::consts::TAU * frequency * time).sin() * (0.12 / (i + 1) as f64)
                })
                .sum(),
        };
        samples.push(sample as f32);
    }
    Clip::new(samples, rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_original_bounded_reproducible_and_audible() {
        for cue in [
            Cue::Footstep,
            Cue::FireLaunch,
            Cue::Impact,
            Cue::Shield,
            Cue::RitualAmbience,
        ] {
            let clip = synthesize(cue, 48000, 17).unwrap();
            assert_eq!(
                clip.samples(),
                synthesize(cue, 48000, 17).unwrap().samples()
            );
            assert!(clip.samples().iter().all(|s| s.is_finite() && s.abs() < 1.));
            let energy = clip
                .samples()
                .iter()
                .map(|s| f64::from(*s).powi(2))
                .sum::<f64>()
                / clip.samples().len() as f64;
            assert!(energy > 0.0001, "{cue:?} is silent");
            if cue == Cue::RitualAmbience {
                assert!(clip.samples()[0].abs() < 1e-6);
                assert!(clip.samples().last().unwrap().abs() < 0.005);
            } else {
                assert_eq!(clip.samples()[0], 0.);
                assert!(clip.samples().last().unwrap().abs() < 0.001);
            }
        }
        assert!(synthesize(Cue::Footstep, 0, 0).is_err());
    }
    #[test]
    fn noise_variants_change_without_relabeling_cue_or_rate() {
        assert_ne!(
            synthesize(Cue::Footstep, 8000, 1).unwrap().samples(),
            synthesize(Cue::Footstep, 8000, 2).unwrap().samples()
        );
        assert_eq!(synthesize(Cue::Shield, 16000, 1).unwrap().rate(), 16000);
    }
}
