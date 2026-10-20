//! What a run may spend, and what it may promise.
//!
//! [`crate::runtime::Budget`] bounds time and steps; this module
//! bounds money. The two are deliberately separate types: a deadline
//! is a fact the host enforces with a clock, and a spend ceiling is
//! only as real as the prices feeding it — some executors cannot
//! price their work at all.
//!
//! Three rules carry the whole contract:
//!
//! - A metered charge sums against the bound; the charge that would
//!   pass a hard ceiling is refused before the work runs.
//! - An unmetered charge records `unknown` — never zero, never an
//!   estimate — and keeps running under a soft bound.
//! - A hard ceiling is a guarantee: an executor that cannot meter
//!   cannot hold it, so asking for one is refused at the door rather
//!   than discovered at the bill.

/// One lane of a run's spend — where the money went. Every decision
/// call, every review or fallback attempt, every generation, and
/// every delegate is its own lane so the book can say what spent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Lane {
    /// A typed decision call — routing, selection, judgment.
    Decision,
    /// A review or fallback pass over another call's answer.
    Review,
    /// The generation door.
    Generation,
    /// A delegated task's execution.
    Delegate,
}

impl Lane {
    /// The lane's name in reports and ATIF records.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Review => "review",
            Self::Generation => "generation",
            Self::Delegate => "delegate",
        }
    }
}

/// What one charge cost — as reported, never estimated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Price {
    /// A metered amount, in USD micros (10⁻⁶ dollars). An integer so
    /// sums are exact — no float can smudge a ledger.
    Metered(u64),
    /// The executor could not price the call. Recorded as unknown —
    /// a soft bound notes it; a hard guarantee cannot hold it.
    Unknown,
}

/// A lane or run's spend bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    /// A hard ceiling: work stops before the charge that would pass
    /// it. Only a lane that meters can hold this — see
    /// [`Book::open`]'s refusal.
    Hard(u64),
    /// A reporting bound: metered charges sum, unmetered ones record
    /// unknown, and passing it flags the report rather than stopping
    /// work.
    Soft(u64),
    /// No bound stated — metered charges still sum for the record.
    None,
}

impl Bound {
    /// The ceiling the bound states, when it states one — what a
    /// child's declared bound is checked against when it asks for more
    /// than the room that is left.
    #[must_use]
    pub fn ceiling(self) -> Option<u64> {
        match self {
            Self::Hard(micros) | Self::Soft(micros) => Some(micros),
            Self::None => Option::None,
        }
    }
}

/// The book's answer to one charge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Charge {
    /// Recorded; the running totals are the book's reply.
    Recorded {
        /// The metered sum after this charge.
        metered_micros: u64,
        /// How many charges reported unknown.
        unknown: u64,
    },
    /// The charge would pass the bound: refused before the work ran.
    /// The charge itself is not recorded — refusing it is the point.
    OverBound {
        /// The bound that stopped it.
        bound_micros: u64,
        /// The metered sum already held.
        held_micros: u64,
        /// The charge that would have passed it.
        charge_micros: u64,
    },
    /// A soft bound was passed: the charge is recorded and the flag
    /// says the report is over — work continues because nothing was
    /// promised.
    OverSoft {
        /// The soft bound passed.
        bound_micros: u64,
        /// The metered sum now held.
        held_micros: u64,
    },
    /// A hard bound met an unmetered price — the guarantee cannot be
    /// given, so the charge is refused.
    Unguaranteeable,
}

/// Why a book could not open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// A hard bound was asked on a lane that cannot meter — the
    /// guarantee cannot be given, so the book refuses to pretend.
    Unguaranteeable,
    /// A child asked for more than its parent's bound leaves —
    /// children narrow, they never widen.
    Widens,
}

/// One lane's running book: what it was given, what it has spent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Book {
    lane: Lane,
    bound: Bound,
    metered_micros: u64,
    unknown: u64,
    over_soft: bool,
}

impl Book {
    /// Open a lane's book under a bound.
    ///
    /// `metered` says whether the lane's executor prices its calls. A
    /// hard bound on an unmetered lane is refused here: the host
    /// cannot promise a ceiling it cannot observe, and discovering
    /// that at the first charge is how a bill surprises.
    pub fn open(lane: Lane, bound: Bound, metered: bool) -> Result<Self, Refusal> {
        if matches!(bound, Bound::Hard(_)) && !metered {
            return Err(Refusal::Unguaranteeable);
        }
        Ok(Book {
            lane,
            bound,
            metered_micros: 0,
            unknown: 0,
            over_soft: false,
        })
    }

    /// A child's book, scoped under this one's remaining room.
    ///
    /// The child may narrow the bound — a smaller number, a soft
    /// bound under a hard one — and may declare its own metering
    /// (a hard bound still requires it). Asking for more than the
    /// parent has left is refused, never silently clamped: a child
    /// that believes it holds a wider bound than the parent would
    /// spend the parent's promise twice.
    pub fn child(&self, lane: Lane, bound: Bound, metered: bool) -> Result<Self, Refusal> {
        let room = match self.bound {
            Bound::Hard(ceiling) | Bound::Soft(ceiling) => {
                Some(ceiling.saturating_sub(self.metered_micros))
            }
            Bound::None => None,
        };
        if let (Some(room), Bound::Hard(ask) | Bound::Soft(ask)) = (room, bound)
            && ask > room
        {
            return Err(Refusal::Widens);
        }
        Self::open(lane, bound, metered)
    }

    /// Charge one call. The book answers before the work runs — an
    /// `OverBound` or `Unguaranteeable` is a refusal to dispatch, not
    /// an entry.
    pub fn charge(&mut self, price: Price) -> Charge {
        match (self.bound, price) {
            (Bound::Hard(ceiling), Price::Metered(amount))
                if self.metered_micros.saturating_add(amount) > ceiling =>
            {
                Charge::OverBound {
                    bound_micros: ceiling,
                    held_micros: self.metered_micros,
                    charge_micros: amount,
                }
            }
            (Bound::Hard(_), Price::Unknown) => Charge::Unguaranteeable,
            (_, Price::Metered(amount)) => {
                self.metered_micros = self.metered_micros.saturating_add(amount);
                if let Bound::Soft(ceiling) = self.bound
                    && self.metered_micros > ceiling
                {
                    self.over_soft = true;
                    return Charge::OverSoft {
                        bound_micros: ceiling,
                        held_micros: self.metered_micros,
                    };
                }
                Charge::Recorded {
                    metered_micros: self.metered_micros,
                    unknown: self.unknown,
                }
            }
            (_, Price::Unknown) => {
                self.unknown += 1;
                Charge::Recorded {
                    metered_micros: self.metered_micros,
                    unknown: self.unknown,
                }
            }
        }
    }

    /// The lane this book keeps.
    #[must_use]
    pub fn lane(&self) -> Lane {
        self.lane
    }

    /// The metered sum so far.
    #[must_use]
    pub fn metered_micros(&self) -> u64 {
        self.metered_micros
    }

    /// How many charges reported unknown — a count, not a number
    /// pretending to be money.
    #[must_use]
    pub fn unknown_charges(&self) -> u64 {
        self.unknown
    }

    /// Whether a soft bound has been passed — the report's flag, not
    /// a refusal.
    #[must_use]
    pub fn over_soft(&self) -> bool {
        self.over_soft
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_metered_charge_sums_and_the_one_past_the_ceiling_is_refused() {
        let mut book = Book::open(Lane::Decision, Bound::Hard(1_000_000), true).unwrap();
        assert_eq!(
            book.charge(Price::Metered(700_000)),
            Charge::Recorded {
                metered_micros: 700_000,
                unknown: 0
            }
        );
        assert_eq!(
            book.charge(Price::Metered(400_000)),
            Charge::OverBound {
                bound_micros: 1_000_000,
                held_micros: 700_000,
                charge_micros: 400_000
            }
        );
        // The refused charge never entered the book.
        assert_eq!(book.metered_micros(), 700_000);
    }

    #[test]
    fn unknown_cost_is_recorded_never_fabricated() {
        let mut book = Book::open(Lane::Delegate, Bound::Soft(1), true).unwrap();
        assert_eq!(
            book.charge(Price::Unknown),
            Charge::Recorded {
                metered_micros: 0,
                unknown: 1
            }
        );
        assert_eq!(book.unknown_charges(), 1);
        assert_eq!(book.metered_micros(), 0);
    }

    #[test]
    fn a_hard_bound_on_an_unmetered_lane_refuses_at_the_door() {
        assert_eq!(
            Book::open(Lane::Generation, Bound::Hard(5_000_000), false),
            Err(Refusal::Unguaranteeable)
        );
        // Soft and unbounded books open fine — they promise nothing.
        assert!(Book::open(Lane::Generation, Bound::Soft(5_000_000), false).is_ok());
        assert!(Book::open(Lane::Generation, Bound::None, false).is_ok());
    }

    #[test]
    fn a_hard_book_cannot_charge_an_unknown_price() {
        // A lane that declared metering can still return an unknown
        // price on a call — and the guarantee fails honestly.
        let mut book = Book::open(Lane::Review, Bound::Hard(1_000_000), true).unwrap();
        assert_eq!(book.charge(Price::Unknown), Charge::Unguaranteeable);
        assert_eq!(book.unknown_charges(), 0);
    }

    #[test]
    fn a_soft_bound_flags_over_and_keeps_working() {
        let mut book = Book::open(Lane::Generation, Bound::Soft(1_000), true).unwrap();
        assert_eq!(
            book.charge(Price::Metered(1_500)),
            Charge::OverSoft {
                bound_micros: 1_000,
                held_micros: 1_500
            }
        );
        assert!(book.over_soft());
        // Work continues and keeps flagging — nothing was promised,
        // so every charge past the bound still reports it.
        assert_eq!(
            book.charge(Price::Metered(500)),
            Charge::OverSoft {
                bound_micros: 1_000,
                held_micros: 2_000
            }
        );
    }

    #[test]
    fn children_narrow_never_widen() {
        let parent = Book::open(Lane::Delegate, Bound::Hard(1_000_000), true).unwrap();
        // Within the parent's room.
        assert!(
            parent
                .child(Lane::Generation, Bound::Hard(900_000), true)
                .is_ok()
        );
        // Past it — refused, not clamped.
        assert_eq!(
            parent.child(Lane::Generation, Bound::Hard(1_500_000), true),
            Err(Refusal::Widens)
        );
        // An unbounded parent leaves any child bound room.
        let free = Book::open(Lane::Delegate, Bound::None, true).unwrap();
        assert!(
            free.child(Lane::Generation, Bound::Hard(9_000_000), true)
                .is_ok()
        );
        // A child's hard bound still requires its own metering.
        assert_eq!(
            free.child(Lane::Generation, Bound::Hard(100), false),
            Err(Refusal::Unguaranteeable)
        );
    }

    #[test]
    fn a_spent_parent_leaves_less_room() {
        let mut parent = Book::open(Lane::Delegate, Bound::Hard(1_000_000), true).unwrap();
        parent.charge(Price::Metered(800_000));
        assert_eq!(
            parent.child(Lane::Generation, Bound::Hard(300_000), true),
            Err(Refusal::Widens)
        );
        assert!(
            parent
                .child(Lane::Generation, Bound::Hard(200_000), true)
                .is_ok()
        );
    }
}
