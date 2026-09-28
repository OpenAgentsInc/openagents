//! Which connector drives which CRTC, and what changed since the last scan.
//!
//! The hardware backend reads the device's connectors when it opens the
//! device and again each time `udev` says the device changed, which is what
//! a monitor plugged in or pulled out looks like. This module holds the
//! decision and no device: it takes the connectors a scan read and answers
//! the screens to drop and the screens to start, and it gives each new
//! screen a CRTC that its encoders can reach and no other screen holds.
//!
//! A scan drops before it starts, so a monitor moved from one port to
//! another in one change frees its CRTC for the port it moved to.

/// One connector as a scan read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    /// The connector's handle, as the device numbers it.
    pub connector: u32,
    /// The name a screen on it carries, such as `DP-2`.
    pub name: String,
    /// Whether a monitor is plugged into it.
    pub connected: bool,
    /// Every CRTC one of its encoders can drive, in the device's order.
    pub crtcs: Vec<u32>,
}

/// One screen a scan starts or drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// A monitor arrived: start a screen on this connector and CRTC.
    Connected {
        /// The connector's handle.
        connector: u32,
        /// The CRTC the screen drives.
        crtc: u32,
        /// The screen's name.
        name: String,
    },
    /// A monitor left: drop the screen on this connector and CRTC.
    Disconnected {
        /// The connector's handle.
        connector: u32,
        /// The CRTC the screen drove.
        crtc: u32,
        /// The screen's name.
        name: String,
    },
    /// A monitor arrived and every CRTC it can reach drives another screen.
    NoCrtc {
        /// The screen's name.
        name: String,
    },
}

/// Whether a card's connectors are read: a card opened on the active
/// terminal reads them at once, and one opened from another terminal
/// reads them when the seat arrives. logind and seatd give DRM master to
/// the session on the active terminal alone, so a compositor started from
/// another terminal cannot reset or drive a connector until its terminal
/// is switched to; it holds the card open and waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    /// The seat is on another terminal: the card is open, and its
    /// connectors wait for the seat.
    Waiting,
    /// The card's connectors are read, and the seat drives them.
    Driving,
}

impl Readiness {
    /// The readiness of a card opened while the seat is, or is not, on
    /// this terminal.
    pub fn at_open(seat_active: bool) -> Readiness {
        if seat_active {
            Readiness::Driving
        } else {
            Readiness::Waiting
        }
    }

    /// The seat arrived on this terminal. Answers whether the card's
    /// connectors are read for the first time, which happens once: a card
    /// that was driving keeps its screens and answers no.
    pub fn seat_arrived(&mut self) -> bool {
        let first = *self == Readiness::Waiting;
        *self = Readiness::Driving;
        first
    }
}

/// The connectors that drive a screen now, each with its CRTC.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tracker {
    bound: Vec<(u32, u32, String)>,
}

impl Tracker {
    /// Every connector that drives a screen, as `(connector, crtc, name)`.
    #[cfg(test)]
    pub fn bound(&self) -> &[(u32, u32, String)] {
        &self.bound
    }

    /// The changes one scan makes, applied to the tracker.
    pub fn scan(&mut self, seen: &[Seen]) -> Vec<Change> {
        let mut changes = Vec::new();
        let gone: Vec<(u32, u32, String)> = self
            .bound
            .iter()
            .filter(|(connector, _, _)| {
                !seen
                    .iter()
                    .any(|held| held.connector == *connector && held.connected)
            })
            .cloned()
            .collect();
        for (connector, crtc, name) in gone {
            self.bound.retain(|held| held.0 != connector);
            changes.push(Change::Disconnected {
                connector,
                crtc,
                name,
            });
        }
        for connector in seen.iter().filter(|held| held.connected) {
            if self.bound.iter().any(|held| held.0 == connector.connector) {
                continue;
            }
            let free = connector
                .crtcs
                .iter()
                .copied()
                .find(|crtc| !self.bound.iter().any(|held| held.1 == *crtc));
            match free {
                Some(crtc) => {
                    self.bound
                        .push((connector.connector, crtc, connector.name.clone()));
                    changes.push(Change::Connected {
                        connector: connector.connector,
                        crtc,
                        name: connector.name.clone(),
                    });
                }
                None => changes.push(Change::NoCrtc {
                    name: connector.name.clone(),
                }),
            }
        }
        changes
    }

    /// Forgets every connector, which is what removing the device does, and
    /// answers the screens to drop.
    pub fn clear(&mut self) -> Vec<Change> {
        std::mem::take(&mut self.bound)
            .into_iter()
            .map(|(connector, crtc, name)| Change::Disconnected {
                connector,
                crtc,
                name,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(connector: u32, name: &str, connected: bool, crtcs: &[u32]) -> Seen {
        Seen {
            connector,
            name: name.to_string(),
            connected,
            crtcs: crtcs.to_vec(),
        }
    }

    /// The NVIDIA card on `coderos-4080` on 2026-09-16: four connectors,
    /// one monitor on `DP-2`, and four CRTCs any connector can reach.
    fn this_host(dp3: bool) -> Vec<Seen> {
        vec![
            port(100, "DP-2", true, &[40, 41, 42, 43]),
            port(101, "DP-3", dp3, &[40, 41, 42, 43]),
            port(102, "DP-4", false, &[40, 41, 42, 43]),
            port(103, "HDMI-A-3", false, &[40, 41, 42, 43]),
        ]
    }

    #[test]
    fn the_first_scan_starts_a_screen_for_each_monitor() {
        let mut tracker = Tracker::default();
        let changes = tracker.scan(&this_host(false));
        assert_eq!(
            changes,
            vec![Change::Connected {
                connector: 100,
                crtc: 40,
                name: "DP-2".to_string()
            }]
        );
    }

    #[test]
    fn a_scan_that_finds_nothing_new_changes_nothing() {
        let mut tracker = Tracker::default();
        tracker.scan(&this_host(false));
        assert!(tracker.scan(&this_host(false)).is_empty());
    }

    #[test]
    fn a_second_monitor_takes_a_crtc_the_first_does_not_hold() {
        let mut tracker = Tracker::default();
        tracker.scan(&this_host(false));
        let changes = tracker.scan(&this_host(true));
        assert_eq!(
            changes,
            vec![Change::Connected {
                connector: 101,
                crtc: 41,
                name: "DP-3".to_string()
            }]
        );
        assert_eq!(tracker.bound().len(), 2);
    }

    #[test]
    fn a_pulled_monitor_drops_its_screen_and_frees_its_crtc() {
        let mut tracker = Tracker::default();
        tracker.scan(&this_host(true));
        let changes = tracker.scan(&this_host(false));
        assert_eq!(
            changes,
            vec![Change::Disconnected {
                connector: 101,
                crtc: 41,
                name: "DP-3".to_string()
            }]
        );
        let back = tracker.scan(&this_host(true));
        assert_eq!(
            back,
            vec![Change::Connected {
                connector: 101,
                crtc: 41,
                name: "DP-3".to_string()
            }]
        );
    }

    #[test]
    fn a_monitor_moved_between_ports_in_one_change_keeps_a_crtc() {
        let mut tracker = Tracker::default();
        tracker.scan(&[port(1, "DP-1", true, &[9]), port(2, "DP-2", false, &[9])]);
        let changes = tracker.scan(&[port(1, "DP-1", false, &[9]), port(2, "DP-2", true, &[9])]);
        assert_eq!(
            changes,
            vec![
                Change::Disconnected {
                    connector: 1,
                    crtc: 9,
                    name: "DP-1".to_string()
                },
                Change::Connected {
                    connector: 2,
                    crtc: 9,
                    name: "DP-2".to_string()
                },
            ]
        );
    }

    #[test]
    fn a_connector_that_vanished_from_the_list_is_dropped() {
        let mut tracker = Tracker::default();
        tracker.scan(&this_host(true));
        let changes = tracker.scan(&[port(100, "DP-2", true, &[40, 41])]);
        assert_eq!(changes.len(), 1);
        assert!(matches!(&changes[0], Change::Disconnected { name, .. } if name == "DP-3"));
    }

    #[test]
    fn a_monitor_with_no_free_crtc_is_named_and_not_started() {
        let mut tracker = Tracker::default();
        let changes = tracker.scan(&[port(1, "DP-1", true, &[7]), port(2, "DP-2", true, &[7])]);
        assert_eq!(changes.len(), 2);
        assert_eq!(
            changes[1],
            Change::NoCrtc {
                name: "DP-2".to_string()
            }
        );
        assert_eq!(tracker.bound().len(), 1);
    }

    #[test]
    fn removing_the_device_drops_every_screen() {
        let mut tracker = Tracker::default();
        tracker.scan(&this_host(true));
        let changes = tracker.clear();
        assert_eq!(changes.len(), 2);
        assert!(tracker.bound().is_empty());
    }

    #[test]
    fn a_card_opened_on_the_active_terminal_drives_at_once() {
        let mut readiness = Readiness::at_open(true);
        assert_eq!(readiness, Readiness::Driving);
        assert!(
            !readiness.seat_arrived(),
            "a switch back to a driving card reads its connectors again for nothing"
        );
        assert_eq!(readiness, Readiness::Driving);
    }

    #[test]
    fn a_card_opened_from_another_terminal_waits_for_the_seat_and_reads_once() {
        // The parity run of 2026-09-17: the compositor started on tty2 while tty1
        // was active, and the switch to tty2 was the first time the card
        // could be reset and read.
        let mut readiness = Readiness::at_open(false);
        assert_eq!(readiness, Readiness::Waiting);
        assert!(
            readiness.seat_arrived(),
            "the first arrival reads the connectors"
        );
        assert_eq!(readiness, Readiness::Driving);
        assert!(
            !readiness.seat_arrived(),
            "a later switch back to this terminal is a resume, and reads nothing new"
        );
    }
}
