//! The sound a chat's Coder work makes when it needs the person: one cue
//! when a task finishes, asks for something, or fails.
//!
//! This decides *which* cue and *when*; a platform adapter plays the bytes
//! [`wav`] returns. A cue follows a change of [`Activity`], the attention
//! indicator's input before the seen marker applies, so a task that
//! finishes in the open chat still chimes. A chat seen for the first time
//! is only recorded, so opening the app on finished work plays nothing,
//! and an activity that does not change plays nothing again.
//!
//! The three cues follow Zeron's done, request, and attention sounds
//! (public MIT zeronsh/zeron at `9e1a1115`, `crates/ui/src/sound.rs` and
//! `docs/sound-design/README.md`). The sounds are original: [`wav`]
//! synthesizes them here, and no Zeron audio file is copied.

use crate::attention::Activity;
use std::collections::BTreeMap;

/// Which sound to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cue {
    /// A task finished with a result: a settling pair.
    Done,
    /// A task waits for an answer or an approval: a rising pair.
    Request,
    /// A task failed: a low, falling pair.
    Attention,
}

impl Cue {
    /// The cue for a task that has just come to `activity`, `None` for an
    /// activity that asks nothing of the person.
    #[must_use]
    pub fn of(activity: Activity) -> Option<Cue> {
        match activity {
            Activity::Completed => Some(Cue::Done),
            Activity::AwaitingInput => Some(Cue::Request),
            Activity::Failed => Some(Cue::Attention),
            Activity::Working | Activity::Idle => None,
        }
    }

    /// Where the cue sorts when several arrive at once: lower needs the
    /// person sooner.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Cue::Request => 0,
            Cue::Attention => 1,
            Cue::Done => 2,
        }
    }

    /// The one cue to play for `cues` that arrived together, the most
    /// urgent, so a burst of changes plays one sound rather than a chord.
    #[must_use]
    pub fn most_urgent(cues: impl IntoIterator<Item = Cue>) -> Option<Cue> {
        cues.into_iter().min_by_key(|cue| cue.rank())
    }

    /// The notes this cue plays: (frequency in hertz, start in seconds).
    fn notes(self) -> [(f32, f32); 2] {
        match self {
            // G5 settling to C5.
            Cue::Done => [(783.99, 0.0), (523.25, 0.11)],
            // D5 rising to A5.
            Cue::Request => [(587.33, 0.0), (880.0, 0.11)],
            // B4 falling to G4, slower.
            Cue::Attention => [(493.88, 0.0), (392.0, 0.16)],
        }
    }
}

/// The cue for a chat whose activity was `before` and is now `after`:
/// `None` the first time a chat is seen (`before` is `None`) and when
/// nothing changed.
#[must_use]
pub fn transition(before: Option<Activity>, after: Activity) -> Option<Cue> {
    let before = before?;
    if before == after {
        return None;
    }
    Cue::of(after)
}

/// The last activity seen for each chat.
#[derive(Debug, Default)]
pub struct Cues {
    seen: BTreeMap<String, Activity>,
}

impl Cues {
    /// Records each chat's `(id, activity)` and returns the cue each chat
    /// that changed calls for, in the order given. Chats no longer listed
    /// are forgotten, and come back unannounced.
    pub fn observe(
        &mut self,
        chats: impl IntoIterator<Item = (String, Activity)>,
    ) -> Vec<(String, Cue)> {
        let mut cues = vec![];
        let mut seen = BTreeMap::new();
        for (id, activity) in chats {
            if let Some(cue) = transition(self.seen.get(&id).copied(), activity) {
                cues.push((id.clone(), cue));
            }
            seen.insert(id, activity);
        }
        self.seen = seen;
        cues
    }
}

/// The sample rate [`wav`] writes.
pub const SAMPLE_RATE: u32 = 22_050;

/// How long each note rings, in seconds.
const RING: f32 = 0.42;

/// The softest peak of the mix, as a fraction of full scale.
const LEVEL: f32 = 0.32;

/// `cue` as a mono 16-bit PCM WAV file, about half a second long.
///
/// Each note is a sine with a quieter octave above it, a 6-millisecond
/// attack so it starts without a click, and an exponential decay that
/// reaches silence by [`RING`]. Every platform's system player decodes
/// it, down to bare ALSA `aplay`.
#[must_use]
pub fn wav(cue: Cue) -> Vec<u8> {
    let notes = cue.notes();
    let length = notes
        .iter()
        .map(|(_, start)| start)
        .fold(0.0_f32, |a, b| a.max(*b))
        + RING;
    let count = (length * SAMPLE_RATE as f32).ceil() as usize;
    let rate = SAMPLE_RATE as f32;
    let attack = 0.006;
    let mut samples = Vec::with_capacity(count);
    for index in 0..count {
        let t = index as f32 / rate;
        let mut value = 0.0;
        for (frequency, start) in notes {
            let local = t - start;
            if !(0.0..RING).contains(&local) {
                continue;
            }
            let rise = (local / attack).min(1.0);
            // Decays to about 0.2% by the end of the ring, then a short
            // linear tail closes it to exactly zero.
            let fall = (-local * 15.0).exp() * ((RING - local) / 0.02).min(1.0);
            let phase = std::f32::consts::TAU * frequency * local;
            value += rise * fall * (phase.sin() + 0.25 * (2.0 * phase).sin());
        }
        samples.push((value * LEVEL / 1.25 * f32::from(i16::MAX)).round() as i16);
    }
    encode(&samples)
}

/// `samples` in a RIFF WAVE container: PCM, one channel, 16 bits.
fn encode(samples: &[i16]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes a second
    out.extend_from_slice(&2_u16.to_le_bytes()); // bytes a frame
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Activity; 5] = [
        Activity::Working,
        Activity::AwaitingInput,
        Activity::Failed,
        Activity::Completed,
        Activity::Idle,
    ];

    fn chat(activity: Activity) -> Vec<(String, Activity)> {
        vec![("c1".into(), activity)]
    }

    #[test]
    fn each_transition_maps_to_its_cue() {
        assert_eq!(
            transition(Some(Activity::Working), Activity::Completed),
            Some(Cue::Done)
        );
        assert_eq!(
            transition(Some(Activity::Working), Activity::AwaitingInput),
            Some(Cue::Request)
        );
        assert_eq!(
            transition(Some(Activity::Working), Activity::Failed),
            Some(Cue::Attention)
        );
        // An answered question that ends straight away still chimes done.
        assert_eq!(
            transition(Some(Activity::AwaitingInput), Activity::Completed),
            Some(Cue::Done)
        );
        for before in ALL {
            assert_eq!(transition(Some(before), Activity::Working), None);
            assert_eq!(transition(Some(before), Activity::Idle), None);
        }
    }

    #[test]
    fn an_unchanged_activity_or_a_first_sighting_plays_nothing() {
        for activity in ALL {
            assert_eq!(transition(Some(activity), activity), None, "{activity:?}");
            assert_eq!(transition(None, activity), None, "{activity:?}");
        }
    }

    #[test]
    fn a_chat_cues_once_per_change() {
        let mut cues = Cues::default();
        // Opening the app on finished work.
        assert!(cues.observe(chat(Activity::Completed)).is_empty());
        assert!(cues.observe(chat(Activity::Working)).is_empty());
        assert_eq!(
            cues.observe(chat(Activity::AwaitingInput)),
            [("c1".to_owned(), Cue::Request)]
        );
        // Asked again, still waiting: nothing.
        assert!(cues.observe(chat(Activity::AwaitingInput)).is_empty());
        assert!(cues.observe(chat(Activity::AwaitingInput)).is_empty());
        assert!(cues.observe(chat(Activity::Working)).is_empty());
        assert_eq!(
            cues.observe(chat(Activity::Completed)),
            [("c1".to_owned(), Cue::Done)]
        );
        assert!(cues.observe(chat(Activity::Completed)).is_empty());
        assert!(cues.observe(chat(Activity::Working)).is_empty());
        assert_eq!(
            cues.observe(chat(Activity::Failed)),
            [("c1".to_owned(), Cue::Attention)]
        );
        assert!(cues.observe(chat(Activity::Failed)).is_empty());
        // A chat that goes away is forgotten, and comes back unannounced.
        assert!(cues.observe(vec![]).is_empty());
        assert!(cues.observe(chat(Activity::AwaitingInput)).is_empty());
    }

    #[test]
    fn several_chats_at_once_play_the_most_urgent_cue() {
        let mut cues = Cues::default();
        let all = |a, b, c| {
            vec![
                ("a".to_owned(), a),
                ("b".to_owned(), b),
                ("c".to_owned(), c),
            ]
        };
        use Activity::{AwaitingInput, Completed, Failed, Working};
        assert!(cues.observe(all(Working, Working, Working)).is_empty());
        let changed = cues.observe(all(Completed, Failed, AwaitingInput));
        assert_eq!(changed.len(), 3);
        assert_eq!(
            Cue::most_urgent(changed.into_iter().map(|(_, cue)| cue)),
            Some(Cue::Request)
        );
        assert_eq!(
            Cue::most_urgent([Cue::Done, Cue::Attention]),
            Some(Cue::Attention)
        );
        assert_eq!(Cue::most_urgent(std::iter::empty()), None);
    }

    #[test]
    fn each_cue_is_a_short_quiet_wav_that_starts_and_ends_silent() {
        let mut seen = vec![];
        for cue in [Cue::Done, Cue::Request, Cue::Attention] {
            let bytes = wav(cue);
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..16], b"WAVEfmt ");
            assert_eq!(&bytes[36..40], b"data");
            let riff = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
            let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
            assert_eq!(riff, bytes.len() - 8);
            assert_eq!(data, bytes.len() - 44);
            let samples: Vec<i16> = bytes[44..]
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
            assert!((0.4..0.7).contains(&seconds), "{cue:?} {seconds}");
            assert_eq!(samples[0], 0, "{cue:?} starts silent");
            assert_eq!(*samples.last().unwrap(), 0, "{cue:?} ends silent");
            let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
            assert!(
                peak > i16::MAX as u16 / 8 && peak < i16::MAX as u16 / 2,
                "{cue:?} {peak}"
            );
            seen.push(bytes);
        }
        assert_ne!(seen[0], seen[1]);
        assert_ne!(seen[1], seen[2]);
        assert_eq!(wav(Cue::Done), seen[0], "synthesis is deterministic");
    }
}
