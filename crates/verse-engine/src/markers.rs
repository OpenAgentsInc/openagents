//! Presentation marker delivery independent of gameplay authority.
use crate::core::LifeId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub id: u32,
    pub seconds: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub duration: f64,
    pub markers: Vec<Marker>,
}
impl Track {
    pub fn validate(&self) -> Result<(), String> {
        if !self.duration.is_finite()
            || self.duration <= 0.
            || self.duration > 86400.
            || self.markers.len() > 256
        {
            return Err("Invalid animation marker duration or capacity".into());
        }
        let mut previous = None;
        for marker in &self.markers {
            if !marker.seconds.is_finite() || marker.seconds < 0. || marker.seconds >= self.duration
            {
                return Err("Animation marker lies outside its clip".into());
            }
            let key = (marker.seconds, marker.id);
            if previous.is_some_and(|p| p >= key) {
                return Err("Animation markers must have unique ordered time and ID pairs".into());
            }
            previous = Some(key);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipTrack {
    pub clip: u16,
    pub track: Track,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub life: LifeId,
    pub selection_epoch: u64,
    pub marker: u32,
    pub playback_seconds: f64,
}
#[derive(Default)]
pub struct Cursor {
    previous: Option<(LifeId, u64, Track, bool, f64)>,
}
impl Cursor {
    /// Delivers crossings in `(previous, time]`. Initial sampling, a new life or
    /// selection epoch, changed tracks, and backward seeks establish a baseline.
    /// Refusals leave the cursor unchanged; callers must explicitly seek or retry.
    pub fn advance(
        &mut self,
        track: &Track,
        life: LifeId,
        epoch: u64,
        time: f64,
        looping: bool,
    ) -> Result<Vec<Event>, String> {
        track.validate()?;
        if !time.is_finite() || !(0. ..=1_000_000_000.).contains(&time) {
            return Err("Invalid animation marker playback time".into());
        }
        let previous = self.previous.as_ref().filter(
            |(old_life, old_epoch, old_track, old_loop, old_time)| {
                *old_life == life
                    && *old_epoch == epoch
                    && old_track == track
                    && *old_loop == looping
                    && *old_time <= time
            },
        );
        let mut events = Vec::new();
        if let Some((_, _, _, _, previous)) = previous {
            for marker in &track.markers {
                if looping {
                    let first =
                        (((previous - marker.seconds) / track.duration).floor() + 1.).max(0.);
                    let last = ((time - marker.seconds) / track.duration).floor();
                    let count = (last - first + 1.).max(0.);
                    if count > (256 - events.len()) as f64
                        || first > u64::MAX as f64
                        || last > u64::MAX as f64
                    {
                        return Err("Animation marker crossings exceed the event budget".into());
                    }
                    for offset in 0..count as u64 {
                        let seconds = (first + offset as f64) * track.duration + marker.seconds;
                        events.push(Event {
                            life,
                            selection_epoch: epoch,
                            marker: marker.id,
                            playback_seconds: seconds,
                        });
                    }
                } else if marker.seconds > *previous && marker.seconds <= time {
                    events.push(Event {
                        life,
                        selection_epoch: epoch,
                        marker: marker.id,
                        playback_seconds: marker.seconds,
                    });
                }
            }
            events.sort_by(|a, b| {
                a.playback_seconds
                    .total_cmp(&b.playback_seconds)
                    .then(a.marker.cmp(&b.marker))
            });
        }
        self.previous = Some((life, epoch, track.clone(), looping, time));
        Ok(events)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn life() -> LifeId {
        LifeId {
            instance: 1,
            actor: 2,
            generation: 0,
        }
    }
    fn track() -> Track {
        Track {
            duration: 1.,
            markers: vec![
                Marker { id: 1, seconds: 0. },
                Marker {
                    id: 2,
                    seconds: 0.5,
                },
            ],
        }
    }
    #[test]
    fn loop_crossings_are_ordered_and_never_repeated() {
        let mut c = Cursor::default();
        let t = track();
        assert!(c.advance(&t, life(), 0, 0., true).unwrap().is_empty());
        let events = c.advance(&t, life(), 0, 2., true).unwrap();
        assert_eq!(
            events
                .iter()
                .map(|e| (e.marker, e.playback_seconds))
                .collect::<Vec<_>>(),
            vec![(2, 0.5), (1, 1.), (2, 1.5), (1, 2.)]
        );
        assert!(c.advance(&t, life(), 0, 2., true).unwrap().is_empty());
    }
    #[test]
    fn life_selection_and_backward_seek_reset_without_catchup() {
        let mut c = Cursor::default();
        let t = track();
        c.advance(&t, life(), 0, 0., true).unwrap();
        assert!(
            c.advance(&t, life().next().unwrap(), 0, 10., true)
                .unwrap()
                .is_empty()
        );
        assert!(c.advance(&t, life(), 1, 10., true).unwrap().is_empty());
        assert!(c.advance(&t, life(), 1, 0., true).unwrap().is_empty());
    }
    #[test]
    fn held_clips_deliver_once_and_budget_refusals_are_atomic() {
        let mut c = Cursor::default();
        let t = track();
        c.advance(&t, life(), 0, 0., false).unwrap();
        assert_eq!(c.advance(&t, life(), 0, 20., false).unwrap().len(), 1);
        assert!(c.advance(&t, life(), 0, 21., false).unwrap().is_empty());
        c.advance(&t, life(), 1, 0., true).unwrap();
        assert!(c.advance(&t, life(), 1, 500., true).is_err());
        assert_eq!(c.advance(&t, life(), 1, 1., true).unwrap().len(), 2);
    }
    #[test]
    fn invalid_tracks_and_times_do_not_change_delivery() {
        let mut c = Cursor::default();
        let t = track();
        c.advance(&t, life(), 0, 0., true).unwrap();
        let mut bad = t.clone();
        bad.markers[1].seconds = 1.;
        assert!(c.advance(&bad, life(), 0, 0.5, true).is_err());
        assert!(c.advance(&t, life(), 0, f64::NAN, true).is_err());
        assert_eq!(c.advance(&t, life(), 0, 0.5, true).unwrap().len(), 1);
    }
}
