//! Audio-versus-picture offset and drift, measured from events.
//!
//! Each timeline is reduced to event times: a picture brightens (a flash,
//! a clap board, a cut to white) or the sound gets loud (a beep, a clap).
//! Events of the measured timeline are paired with events of an
//! independent reference timeline and a straight line is fitted through
//! the pairs with a consensus search, so a stray event cannot pull it:
//!
//! `measured = offset + (1 + drift) × reference`
//!
//! Nothing here reads the recorder's own idea of sync (container start
//! times are only used to place each stream's first sample). The sign
//! convention everywhere: a positive offset means the audio comes later
//! than the picture.

/// Values sampled at a fixed rate; sample `i` is at `start + i / rate`.
#[derive(Clone, Debug)]
pub struct Series {
    pub start: f64,
    pub rate: f64,
    pub values: Vec<f32>,
}

impl Series {
    fn time(&self, index: f64) -> f64 {
        self.start + index / self.rate
    }
}

/// The smallest brightness jump (0 to 255) that counts as a picture event.
const MIN_PICTURE_STEP: f32 = 8.0;
/// The smallest loudness rise (mean absolute 16-bit sample) that counts.
const MIN_AUDIO_RISE: f32 = 200.0;

/// Times when the picture brightens sharply: frames whose brightness rises
/// by at least half the largest rise in the clip. Each frame's time is its
/// presentation start.
#[must_use]
pub fn picture_events(series: &Series, refractory: f64) -> Vec<f64> {
    let steps: Vec<f32> = series
        .values
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    let largest = steps.iter().copied().fold(0.0_f32, f32::max);
    if largest < MIN_PICTURE_STEP {
        return Vec::new();
    }
    let threshold = largest * 0.5;
    let mut events: Vec<f64> = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        if *step >= threshold {
            let time = series.time((index + 1) as f64);
            if events.last().is_none_or(|last| time - last >= refractory) {
                events.push(time);
            }
        }
    }
    events
}

/// The loudness of 16-bit mono samples: the mean absolute sample of each
/// `window` samples, placed at the window's middle.
#[must_use]
pub fn envelope(samples: &[i16], sample_rate: f64, window: usize, start: f64) -> Series {
    let window = window.max(1);
    let values = samples
        .chunks(window)
        .map(|chunk| {
            chunk
                .iter()
                .map(|sample| f32::from(*sample).abs())
                .sum::<f32>()
                / chunk.len() as f32
        })
        .collect();
    let rate = sample_rate / window as f64;
    Series {
        start: start + 0.5 / rate,
        rate,
        values,
    }
}

/// Times when the sound turns loud: rising crossings of the level halfway
/// between the clip's median and its 99.9th percentile, interpolated
/// between envelope samples.
#[must_use]
pub fn audio_events(series: &Series, refractory: f64) -> Vec<f64> {
    if series.values.len() < 2 {
        return Vec::new();
    }
    let mut sorted = series.values.clone();
    sorted.sort_by(f32::total_cmp);
    let at = |fraction: f64| sorted[((sorted.len() - 1) as f64 * fraction).round() as usize];
    let floor = at(0.5);
    let peak = at(0.999);
    if peak - floor < MIN_AUDIO_RISE {
        return Vec::new();
    }
    let threshold = floor + (peak - floor) * 0.5;
    let mut events: Vec<f64> = Vec::new();
    for index in 1..series.values.len() {
        let (before, after) = (series.values[index - 1], series.values[index]);
        if before < threshold && after >= threshold {
            let fraction = f64::from((threshold - before) / (after - before));
            let time = series.time((index - 1) as f64 + fraction);
            if events.last().is_none_or(|last| time - last >= refractory) {
                events.push(time);
            }
        }
    }
    events
}

/// A line through paired events: `measured = offset + (1 + drift) × reference`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub offset: f64,
    pub drift: f64,
    /// Pairs on the line.
    pub pairs: usize,
    /// Root-mean-square distance of those pairs from the line, seconds.
    pub rms: f64,
    /// The first and last reference times on the line.
    pub span: (f64, f64),
}

impl Fit {
    #[must_use]
    pub fn at(&self, reference: f64) -> f64 {
        self.offset + (1.0 + self.drift) * reference
    }
}

/// Why no line was found.
#[derive(Debug, PartialEq)]
pub enum NoFit {
    /// Fewer than two events matched within `max_offset`.
    TooFewPairs(usize),
}

/// Pair `measured` events with `reference` events no more than
/// `max_offset` apart and fit the line that the most pairs agree with
/// within `tolerance` seconds, then refine it by least squares over them.
///
/// # Errors
/// [`NoFit::TooFewPairs`] when fewer than two pairs agree.
pub fn fit(
    reference: &[f64],
    measured: &[f64],
    max_offset: f64,
    tolerance: f64,
) -> Result<Fit, NoFit> {
    let mut candidates = Vec::new();
    for x in reference {
        for y in measured {
            if (y - x).abs() <= max_offset {
                candidates.push((*x, *y));
            }
        }
    }
    // Bound the pair-of-pairs search on long, busy clips.
    let stride = candidates.len().div_ceil(400).max(1);
    let seeds: Vec<(f64, f64)> = candidates.iter().copied().step_by(stride).collect();
    let mut best: Option<(usize, f64, f64, f64)> = None;
    for (index, first) in seeds.iter().enumerate() {
        for second in &seeds[index + 1..] {
            let span = second.0 - first.0;
            if span.abs() < 0.25 {
                continue;
            }
            let slope = (second.1 - first.1) / span;
            // A real clock is never more than 5% off.
            if !(0.95..=1.05).contains(&slope) {
                continue;
            }
            let intercept = first.1 - slope * first.0;
            let (count, error) = agreeing(&candidates, intercept, slope, tolerance);
            let better = best.is_none_or(|(best_count, best_error, ..)| {
                count > best_count || (count == best_count && error < best_error)
            });
            if better {
                best = Some((count, error, intercept, slope));
            }
        }
    }
    let Some((count, _, intercept, slope)) = best else {
        return Err(NoFit::TooFewPairs(candidates.len().min(1)));
    };
    if count < 2 {
        return Err(NoFit::TooFewPairs(count));
    }
    let inliers: Vec<(f64, f64)> = unique_by_reference(
        candidates
            .iter()
            .copied()
            .filter(|(x, y)| (y - (intercept + slope * x)).abs() <= tolerance),
        intercept,
        slope,
    );
    let (intercept, slope) = least_squares(&inliers).unwrap_or((intercept, slope));
    let rms = (inliers
        .iter()
        .map(|(x, y)| (y - (intercept + slope * x)).powi(2))
        .sum::<f64>()
        / inliers.len() as f64)
        .sqrt();
    let first = inliers
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::INFINITY, f64::min);
    let last = inliers
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    Ok(Fit {
        offset: intercept,
        drift: slope - 1.0,
        pairs: inliers.len(),
        rms,
        span: (first, last),
    })
}

/// How many candidate pairs lie within `tolerance` of the line, counting
/// each reference event once, and their summed distance.
fn agreeing(candidates: &[(f64, f64)], intercept: f64, slope: f64, tolerance: f64) -> (usize, f64) {
    let close = unique_by_reference(
        candidates
            .iter()
            .copied()
            .filter(|(x, y)| (y - (intercept + slope * x)).abs() <= tolerance),
        intercept,
        slope,
    );
    let error: f64 = close
        .iter()
        .map(|(x, y)| (y - (intercept + slope * x)).abs())
        .sum();
    (close.len(), error)
}

/// Keep, for each reference event, only its pair closest to the line.
fn unique_by_reference(
    pairs: impl Iterator<Item = (f64, f64)>,
    intercept: f64,
    slope: f64,
) -> Vec<(f64, f64)> {
    let mut kept: Vec<(f64, f64)> = Vec::new();
    for (x, y) in pairs {
        let distance = (y - (intercept + slope * x)).abs();
        match kept.iter_mut().find(|(kept_x, _)| *kept_x == x) {
            Some(existing) => {
                if distance < (existing.1 - (intercept + slope * x)).abs() {
                    *existing = (x, y);
                }
            }
            None => kept.push((x, y)),
        }
    }
    kept
}

fn least_squares(pairs: &[(f64, f64)]) -> Option<(f64, f64)> {
    let count = pairs.len() as f64;
    if pairs.len() < 2 {
        return None;
    }
    let mean_x = pairs.iter().map(|(x, _)| x).sum::<f64>() / count;
    let mean_y = pairs.iter().map(|(_, y)| y).sum::<f64>() / count;
    let spread: f64 = pairs.iter().map(|(x, _)| (x - mean_x).powi(2)).sum();
    if spread <= f64::EPSILON {
        return None;
    }
    let slope = pairs
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>()
        / spread;
    Some((mean_y - slope * mean_x, slope))
}

/// Audio against picture as a function of picture time:
/// `audio_time - picture_time = offset + drift × picture_time`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvOffset {
    pub offset: f64,
    pub drift: f64,
}

impl AvOffset {
    /// From a direct fit of audio events (measured) against picture events
    /// (reference) of the same clip.
    #[must_use]
    pub fn direct(fit: &Fit) -> Self {
        Self {
            offset: fit.offset,
            drift: fit.drift,
        }
    }

    /// From the picture and the audio each fitted against a separate
    /// reference timeline. Without a picture fit the picture is taken to
    /// run on the reference timeline itself.
    #[must_use]
    pub fn via_reference(picture: Option<&Fit>, audio: &Fit) -> Self {
        let (p0, p1) = picture.map_or((0.0, 1.0), |fit| (fit.offset, 1.0 + fit.drift));
        let (a0, a1) = (audio.offset, 1.0 + audio.drift);
        Self {
            offset: a0 - a1 * p0 / p1,
            drift: a1 / p1 - 1.0,
        }
    }

    #[must_use]
    pub fn at(&self, picture_time: f64) -> f64 {
        self.offset + self.drift * picture_time
    }

    /// One sentence a person reads, with the signed numbers.
    #[must_use]
    pub fn summary(&self, duration: f64) -> String {
        fn side(seconds: f64) -> String {
            if seconds.abs() < 0.0005 {
                "in sync with the picture".to_owned()
            } else if seconds > 0.0 {
                format!("{:.3} s behind the picture (late)", seconds)
            } else {
                format!("{:.3} s ahead of the picture (early)", -seconds)
            }
        }
        let start = side(self.at(0.0));
        let mut text = format!("At the start the audio is {start}");
        if self.drift.abs() >= 0.000_005 {
            let direction = if self.drift > 0.0 { "later" } else { "earlier" };
            text.push_str(&format!(
                "; it drifts {direction} by {:.3}% ({:.1} ms per minute), so at {:.1} s it is {}",
                self.drift.abs() * 100.0,
                self.drift.abs() * 60_000.0,
                duration,
                side(self.at(duration))
            ));
        }
        text.push('.');
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clip with a flash every 2 s and a beep that starts 0.4 s late and
    /// runs 0.14% slow, as the picture series and the audio envelope the
    /// media tool decodes.
    fn drifting_clip(duration: f64) -> (Series, Series) {
        let fps = 30.0;
        let frames = (duration * fps) as usize;
        let picture = Series {
            start: 0.0,
            rate: fps,
            values: (0..frames)
                .map(|frame| {
                    let t = frame as f64 / fps;
                    if t % 2.0 < 0.05 { 235.0 } else { 16.0 }
                })
                .collect(),
        };
        let rate = 8000.0;
        let samples: Vec<i16> = (0..(duration * rate) as usize)
            .map(|index| {
                let t = index as f64 / rate;
                let local = (t - 0.4) / 1.0014;
                if t >= 0.4 && local % 2.0 < 0.05 {
                    (12_000.0 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin()) as i16
                } else {
                    // A little hiss, so the floor is not silence.
                    ((index * 7919 % 61) as i16) - 30
                }
            })
            .collect();
        (picture, envelope(&samples, rate, 8, 0.0))
    }

    #[test]
    fn a_known_late_and_slow_audio_track_is_measured_with_its_sign() {
        let duration = 60.0;
        let (picture, audio) = drifting_clip(duration);
        let flashes = picture_events(&picture, 0.25);
        let beeps = audio_events(&audio, 0.25);
        assert!(flashes.len() >= 28, "{flashes:?}");
        assert!(beeps.len() >= 28, "{beeps:?}");
        let fit = fit(&flashes, &beeps, 1.0, 0.04).unwrap();
        let av = AvOffset::direct(&fit);
        // Acceptance (#11172): sign and size within 20 ms.
        assert!((av.at(0.0) - 0.4).abs() < 0.020, "{av:?}");
        assert!(
            (av.at(duration) - (0.4 + 0.0014 * duration)).abs() < 0.020,
            "{av:?}"
        );
        assert!((av.drift - 0.0014).abs() < 0.0002, "{av:?}");
        let summary = av.summary(duration);
        assert!(summary.contains("behind the picture"), "{summary}");
        assert!(summary.contains("later by 0.14"), "{summary}");
    }

    #[test]
    fn early_audio_reads_negative() {
        let reference: Vec<f64> = (1..20).map(|k| f64::from(k) * 1.5).collect();
        let measured: Vec<f64> = reference.iter().map(|t| t - 0.12).collect();
        let fit = fit(&reference, &measured, 1.0, 0.03).unwrap();
        let av = AvOffset::direct(&fit);
        assert!((av.offset + 0.12).abs() < 1e-9, "{av:?}");
        assert!(av.drift.abs() < 1e-9);
        assert!(av.summary(30.0).contains("ahead of the picture"));
    }

    #[test]
    fn stray_events_do_not_pull_the_line() {
        let reference: Vec<f64> = (0..15).map(|k| f64::from(k) * 2.0 + 1.0).collect();
        let mut measured: Vec<f64> = reference.iter().map(|t| 0.25 + t * 1.001).collect();
        measured.extend([3.6, 9.9, 17.2]);
        measured.sort_by(f64::total_cmp);
        let fit = fit(&reference, &measured, 1.0, 0.02).unwrap();
        assert_eq!(fit.pairs, 15);
        assert!((fit.offset - 0.25).abs() < 1e-6, "{fit:?}");
        assert!((fit.drift - 0.001).abs() < 1e-6, "{fit:?}");
    }

    #[test]
    fn too_few_events_is_said_plainly() {
        assert!(matches!(
            fit(&[1.0], &[1.2], 1.0, 0.02),
            Err(NoFit::TooFewPairs(_))
        ));
        assert!(fit(&[], &[], 1.0, 0.02).is_err());
    }

    #[test]
    fn a_reference_timeline_cancels_a_shared_shift() {
        // Picture and audio both start 1 s into the reference, and the
        // audio a further 0.2 s late: only the 0.2 s is A/V offset.
        let picture = Fit {
            offset: 1.0,
            drift: 0.0,
            pairs: 10,
            rms: 0.0,
            span: (0.0, 10.0),
        };
        let audio = Fit {
            offset: 1.2,
            ..picture
        };
        let av = AvOffset::via_reference(Some(&picture), &audio);
        assert!((av.offset - 0.2).abs() < 1e-9, "{av:?}");
        assert!(av.drift.abs() < 1e-9);
        // With no picture fit the picture is the reference timeline.
        assert!((AvOffset::via_reference(None, &audio).offset - 1.2).abs() < 1e-9);
    }

    #[test]
    fn quiet_or_still_clips_have_no_events() {
        let flat = Series {
            start: 0.0,
            rate: 30.0,
            values: vec![100.0; 90],
        };
        assert!(picture_events(&flat, 0.25).is_empty());
        let hiss = envelope(&[20_i16; 8000], 8000.0, 8, 0.0);
        assert!(audio_events(&hiss, 0.25).is_empty());
    }
}
