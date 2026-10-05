//! The Agent Studio in a hosted Everglade instance: where each seat of a
//! NIP-HOST studio snapshot (`coder_access::studio`) stands, the world
//! authority that walks the seats for every viewer, and the studio rights a
//! viewer's panels need (`docs/verse/networking.md`, "Shared Everglade").
//!
//! The NIP-HOST `world` right admits a viewer to walk the instance and see
//! its seats. Reading a board or a log needs `observe`, acting on a task or
//! a seat needs `operate`, and deciding a merge needs `review`, the same
//! rights the host checks for each studio operation.

use super::everglade::{DESK_SEATS, HALL, STATIONS, Station as Place};
use super::seats::{SeatPlan, SeatPose, Seats};
use crate::social::controller::Footprint;
use coder_access::studio::{Activity, Snapshot, Station, View};
use coder_access::{Code, Error, Operation, Right};
use glam::Vec3;

/// How far a seat stands from its desk's point toward the middle of the
/// hall, m, so it works beside its bench rather than in the aisle.
pub const DESK_ASIDE: f32 = 0.7;
/// Spacing of seats sharing one station, m.
pub const SLOT: f32 = 0.9;

/// The layout ID of the place a snapshot station is drawn at. A seat's
/// desk is its own desk, not the desks station.
#[must_use]
pub fn place_id(station: Station) -> &'static str {
    match station {
        Station::Desk => "desks",
        Station::Library => "library",
        Station::Workbench => "workbench",
        Station::ProvingGround => "proving",
        Station::Oracle => "oracle",
        Station::Podium => "podium",
        Station::Lounge => "lounge",
        Station::TaskWall => "task_wall",
    }
}

fn place(id: &str) -> &'static Place {
    STATIONS
        .iter()
        .find(|station| station.id == id)
        .unwrap_or(&STATIONS[2])
}

/// Where a seat works at the desk whose standing point is `seat`:
/// [`DESK_ASIDE`] toward the middle of the hall, at the same depth.
#[must_use]
pub fn at_desk(seat: [f32; 2]) -> [f32; 2] {
    let [x, z] = seat;
    [x - DESK_ASIDE * (x - HALL.0[0]).signum(), z]
}

/// Where a seat stands for `station`: beside its own desk ([`at_desk`])
/// for the desk, else the station's point moved sideways to `slot` of
/// `count` seats there. Returns the point on the ground and the heading to
/// face.
#[must_use]
pub fn standing(station: Station, desk: u32, slot: usize, count: usize) -> ([f32; 2], f32) {
    if station == Station::Desk
        && let Some(seat) = usize::try_from(desk).ok().and_then(|i| DESK_SEATS.get(i))
    {
        return (at_desk(*seat), 0.0);
    }
    let place = place(place_id(station));
    // Sideways to the heading: forward is (sin, cos), so right is
    // (cos, -sin).
    let shift = (slot as f32 - (count.max(1) - 1) as f32 / 2.0) * SLOT;
    let (sin, cos) = place.facing.sin_cos();
    (
        [place.at[0] + cos * shift, place.at[1] - sin * shift],
        place.facing,
    )
}

/// Which of the seats at the same place as seat `index` it is, and how many
/// stand there. Each desk holds only its own seat.
#[must_use]
pub fn sharing(view: &View, index: usize) -> (usize, usize) {
    let seat = &view.seats[index];
    let at_own_desk = |s: &coder_access::studio::Seat| {
        s.station == Station::Desk && (s.desk as usize) < DESK_SEATS.len()
    };
    if at_own_desk(seat) {
        return (0, 1);
    }
    let same = |s: &coder_access::studio::Seat| {
        !at_own_desk(s) && place_id(s.station) == place_id(seat.station)
    };
    let slot = view.seats[..index].iter().filter(|s| same(s)).count();
    let count = view.seats.iter().filter(|s| same(s)).count();
    (slot, count)
}

/// Where every seat of `view` belongs, in the view's order.
#[must_use]
pub fn plans(view: &View) -> Vec<SeatPlan> {
    view.seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            let (slot, count) = sharing(view, index);
            let (home, facing) = standing(seat.station, seat.desk, slot, count);
            SeatPlan {
                name: seat.seat.clone(),
                home,
                facing,
                waits: seat.activity == Activity::Waiting && seat.station == Station::Podium,
            }
        })
        .collect()
}

/// What a viewer may do with the studio's panels, from the NIP-HOST
/// rights its grant holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelAccess {
    /// Open a panel and read boards, logs, and memory: `observe`.
    pub read: bool,
    /// Act on tasks and seats: `operate`.
    pub act: bool,
    /// Decide a merge: `review`.
    pub merge: bool,
}

impl PanelAccess {
    /// The access `rights` give. `world` alone gives none: it admits
    /// walking only.
    #[must_use]
    pub fn of(rights: &[Right]) -> Self {
        Self {
            read: rights.contains(&Right::Observe),
            act: rights.contains(&Right::Operate),
            merge: rights.contains(&Right::Review),
        }
    }

    /// Whether this access holds `right`'s part of the panels.
    #[must_use]
    pub fn holds(self, right: Right) -> bool {
        match right {
            Right::Observe => self.read,
            Right::Operate => self.act,
            Right::Review => self.merge,
            _ => false,
        }
    }
}

/// The refusal for a viewer whose rights lack `right`.
#[must_use]
pub fn missing(right: Right) -> Error {
    let mut refused = Error::new(
        Code::MissingRight,
        format!("this connection lacks the `{}` right", right.as_str()),
    );
    refused.missing = Some(right);
    refused
}

/// Checks that `rights` may open a studio panel: reading needs `observe`.
///
/// # Errors
/// `missing_right` naming `observe`.
pub fn admit_read(rights: &[Right]) -> Result<(), Error> {
    if PanelAccess::of(rights).read {
        Ok(())
    } else {
        Err(missing(Right::Observe))
    }
}

/// Checks that `rights` may send `operation` from a panel: the right the
/// host requires for it, and `observe` to have the panel open at all.
///
/// # Errors
/// `missing_right` naming the right `rights` lack.
pub fn admit(rights: &[Right], operation: &Operation) -> Result<(), Error> {
    admit_read(rights)?;
    match operation.required() {
        Some(right) if !rights.contains(&right) => Err(missing(right)),
        _ => Ok(()),
    }
}

/// Where the world host reads the studio from: the Coder host's
/// coordinator beside it, a host connection under an `observe` grant, or
/// a test's fixture.
pub trait SnapshotSource: Send {
    /// The studio now, when it changed since the last poll.
    fn poll(&mut self) -> Option<Snapshot>;
}

/// The studio half of a hosted Everglade instance: reads snapshots from a
/// [`SnapshotSource`], owns the seat actors every viewer draws, and serves
/// studio data only to viewers whose rights allow it.
pub struct StudioHost {
    source: Box<dyn SnapshotSource>,
    snapshot: Option<Snapshot>,
    seats: Seats,
}

impl StudioHost {
    /// Reads `source`; seats route around `blockers`.
    #[must_use]
    pub fn new(source: Box<dyn SnapshotSource>, blockers: Vec<Footprint>) -> Self {
        Self {
            source,
            snapshot: None,
            seats: Seats::new(blockers),
        }
    }

    /// Polls the source, takes a changed snapshot, and walks every seat
    /// `dt` seconds. Waiting seats meet the nearest of `players`.
    pub fn tick(&mut self, dt: f32, players: &[Vec3]) {
        if let Some(snapshot) = self.source.poll()
            && self.snapshot.as_ref() != Some(&snapshot)
        {
            self.seats.apply(&plans(&snapshot.view));
            self.snapshot = Some(snapshot);
        }
        self.seats.tick(dt, players);
    }

    /// Where every seat stands now. Seats are part of the world: any
    /// admitted viewer sees them.
    #[must_use]
    pub fn seats(&self) -> Vec<SeatPose> {
        self.seats.poses()
    }

    /// The studio's view for a viewer holding `rights`, while one is loaded.
    ///
    /// # Errors
    /// `missing_right` without `observe`, and `unavailable` before the
    /// first snapshot.
    pub fn view(&self, rights: &[Right]) -> Result<&View, Error> {
        admit_read(rights)?;
        self.snapshot
            .as_ref()
            .map(|snapshot| &snapshot.view)
            .ok_or_else(|| Error::new(Code::Unavailable, "the studio has not loaded yet"))
    }

    /// Checks that a viewer holding `rights` may send `operation`.
    ///
    /// # Errors
    /// `missing_right` naming the right `rights` lack.
    pub fn admit(&self, rights: &[Right], operation: &Operation) -> Result<(), Error> {
        admit(rights, operation)
    }
}
