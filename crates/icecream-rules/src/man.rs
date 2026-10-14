//! The Ice Cream Man's state machine (`docs/verse/games/the-ice-cream-man.md`,
//! The Ice Cream Man): *wander* rooms, *investigate* noises, *chase* on
//! sight, *search* the last place he saw you, then wander again. Hot fudge
//! slows him and, after enough of it, freezes him solid; a fart cloud
//! shoves him away and leaves him *confused*, with no idea where you are.

use crate::DT;
use crate::geom::{V2, first_hit, slide, v};
use crate::layout::{House, cell_of, centre};
use crate::rng::Rng;
use crate::tuning::*;

/// What he is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ManState {
    Wander,
    Investigate,
    Chase,
    Search,
    /// Frozen solid in hardened fudge.
    Frozen,
    /// Lost in a fart cloud.
    Confused,
}

/// What he did this step, for the game to carry out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// A sprinkle volley toward this direction.
    Volley(V2),
    Grab,
    /// He opened the closet he saw you go into.
    PullOut,
    Spotted,
    Lost,
    Froze,
}

/// What he can sense of the player.
#[derive(Clone, Copy, Debug)]
pub struct Senses<'a> {
    pub player: V2,
    pub hidden: bool,
    /// The closet's spot, while the player hides in one.
    pub closet: Option<V2>,
    /// Noises this step: where, and how far they carry.
    pub noises: &'a [(V2, f32)],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Man {
    pub pos: V2,
    pub facing: V2,
    pub state: ManState,
    /// Waypoints to [`Man::goal`].
    pub path: Vec<V2>,
    pub goal: Option<V2>,
    pub last_seen: Option<V2>,
    /// Time left in Search, Frozen or Confused.
    pub timer: f32,
    /// Time left slowed by fudge.
    pub slow: f32,
    pub fudge_hits: u32,
    fudge_memory: f32,
    pub shove: V2,
    volley_wait: f32,
    grab_wait: f32,
    /// Time left standing still, pleased with himself, after a grab.
    pub gloat: f32,
    sniff: f32,
    /// He saw the player get into the closet they are in.
    pub saw_hide: bool,
    /// Whether he moved this step (for his bob).
    pub moving: bool,
    stuck: u32,
    replan: f32,
    /// In Search: whether he reached the spot and is looking around.
    looking: bool,
    rng: Rng,
    /// The numbers he plays by.
    pub tuning: Tuning,
    /// His health against the earned weapons; at none he melts.
    pub health: f32,
}

impl Man {
    #[must_use]
    pub fn new(at: V2, seed: u64) -> Self {
        Self::tuned(at, seed, Tuning::default())
    }

    /// He plays by `tuning`'s numbers.
    #[must_use]
    pub fn tuned(at: V2, seed: u64, tuning: Tuning) -> Self {
        Self {
            pos: at,
            facing: v(0.0, 1.0),
            state: ManState::Wander,
            path: Vec::new(),
            goal: None,
            last_seen: None,
            timer: 0.0,
            slow: 0.0,
            fudge_hits: 0,
            fudge_memory: 0.0,
            shove: V2::ZERO,
            volley_wait: tuning.volley_wait,
            grab_wait: 0.0,
            gloat: 0.0,
            sniff: tuning.sniff_every,
            saw_hide: false,
            moving: false,
            stuck: 0,
            replan: 0.0,
            looking: false,
            rng: Rng::new(seed.wrapping_mul(31).wrapping_add(17)),
            health: tuning.man_health,
            tuning,
        }
    }

    /// Whether he can see a player standing at `player`: in range, nothing
    /// tall between, and in front of him unless he is close or chasing.
    #[must_use]
    pub fn can_see(&self, house: &House, player: V2) -> bool {
        if matches!(self.state, ManState::Frozen | ManState::Confused) {
            return false;
        }
        let to = player - self.pos;
        let distance = to.length();
        if distance > self.tuning.sight {
            return false;
        }
        let ahead = distance <= self.tuning.notice
            || matches!(self.state, ManState::Chase)
            || self.facing.dot(to.normalized()) >= self.tuning.sight_cos();
        ahead && first_hit(&house.blocks, self.pos, player, 0.0, 1.5).is_none()
    }

    /// A squirt of hot fudge landed. Returns whether it froze him.
    pub fn fudge(&mut self) -> bool {
        if self.state == ManState::Frozen {
            return false;
        }
        self.fudge_hits += 1;
        self.fudge_memory = self.tuning.fudge_memory;
        self.slow = self.tuning.fudge_slow;
        if self.fudge_hits >= self.tuning.freeze_hits {
            self.fudge_hits = 0;
            self.state = ManState::Frozen;
            self.timer = self.tuning.frozen;
            self.path.clear();
            self.shove = V2::ZERO;
            return true;
        }
        false
    }

    /// An earned weapon or one of your minions hurt him. Returns whether
    /// that melted him.
    pub fn hurt(&mut self, damage: f32) -> bool {
        if self.health <= 0.0 {
            return false;
        }
        self.health = (self.health - damage).max(0.0);
        self.health <= 0.0
    }

    /// A fart cloud went off at `at` within reach: shoved away, and he
    /// forgets where you are.
    pub fn fart(&mut self, at: V2) {
        let away = (self.pos - at).normalized();
        let away = if away == V2::ZERO { v(1.0, 0.0) } else { away };
        self.shove = away * self.tuning.fart_shove;
        if self.state != ManState::Frozen {
            self.state = ManState::Confused;
            self.timer = self.tuning.confused;
        }
        self.last_seen = None;
        self.saw_hide = false;
        self.path.clear();
        self.goal = None;
        self.facing = away;
    }

    fn head_to(&mut self, house: &House, goal: V2) {
        self.goal = Some(goal);
        self.path = house.route(self.pos, goal, MAN_RADIUS);
        self.replan = 0.5;
    }

    /// Walks along the path. Returns whether he has arrived.
    fn follow(&mut self, house: &House, speed: f32) -> bool {
        while let Some(&next) = self.path.first() {
            if self.pos.distance(next) < 0.3 {
                self.path.remove(0);
            } else {
                break;
            }
        }
        let Some(&next) = self.path.first() else {
            return true;
        };
        self.walk(house, next, speed);
        false
    }

    fn walk(&mut self, house: &House, toward: V2, speed: f32) {
        let to = toward - self.pos;
        let dir = to.normalized();
        let step = (speed * DT).min(to.length());
        let before = self.pos;
        self.pos = slide(&house.blocks, self.pos, dir * step, MAN_RADIUS, 0.0);
        self.facing = if dir == V2::ZERO { self.facing } else { dir };
        self.moving = true;
        if self.pos.distance(before) < step * 0.2 {
            self.stuck += 1;
        } else {
            self.stuck = 0;
        }
        // Wedged on furniture: go by the room's middle.
        if self.stuck > 20 {
            self.stuck = 0;
            if let Some(goal) = self.goal {
                let c = centre(cell_of(self.pos));
                let mut path = vec![c];
                path.extend(house.route(c, goal, MAN_RADIUS));
                self.path = path;
            }
        }
    }

    fn speed(&self, base: f32) -> f32 {
        if self.slow > 0.0 {
            base * self.tuning.slowed
        } else {
            base
        }
    }

    /// One step.
    pub fn step(&mut self, house: &House, senses: Senses<'_>) -> Vec<Act> {
        let mut acts = Vec::new();
        self.moving = false;
        for timer in [
            &mut self.volley_wait,
            &mut self.grab_wait,
            &mut self.gloat,
            &mut self.slow,
            &mut self.replan,
        ] {
            *timer = (*timer - DT).max(0.0);
        }
        if self.fudge_memory > 0.0 {
            self.fudge_memory -= DT;
            if self.fudge_memory <= 0.0 {
                self.fudge_hits = 0;
            }
        }
        if self.shove.length() > 0.05 {
            self.pos = slide(&house.blocks, self.pos, self.shove * DT, MAN_RADIUS, 0.0);
            self.shove = self.shove * (1.0 - SHOVE_FADE * DT).max(0.0);
        }

        match self.state {
            ManState::Frozen => {
                self.timer -= DT;
                if self.timer <= 0.0 {
                    let goal = self.last_seen.unwrap_or(senses.player);
                    self.state = ManState::Search;
                    self.timer = self.tuning.search_time;
                    self.looking = false;
                    self.head_to(house, goal);
                }
                return acts;
            }
            ManState::Confused => {
                self.timer -= DT;
                // Stumbles about, turning at walls.
                if self.rng.below(30) == 0 || self.facing == V2::ZERO {
                    let a = self.rng.unit() * std::f32::consts::TAU;
                    self.facing = v(a.cos(), a.sin());
                }
                let before = self.pos;
                let facing = self.facing;
                self.pos = slide(
                    &house.blocks,
                    self.pos,
                    facing * self.tuning.man_wander * 0.6 * DT,
                    MAN_RADIUS,
                    0.0,
                );
                self.moving = true;
                if self.pos.distance(before) < 0.01 {
                    self.facing = facing.rotated(0.0, 1.0);
                }
                if self.timer <= 0.0 {
                    self.state = ManState::Wander;
                    self.path.clear();
                    self.goal = None;
                }
                return acts;
            }
            _ => {}
        }

        let sees = !senses.hidden && self.can_see(house, senses.player);
        if sees {
            if self.state != ManState::Chase {
                acts.push(Act::Spotted);
                self.state = ManState::Chase;
            }
            self.last_seen = Some(senses.player);
        } else if self.state == ManState::Chase && !(senses.hidden && self.saw_hide) {
            acts.push(Act::Lost);
            let goal = self.last_seen.unwrap_or(senses.player);
            self.state = ManState::Search;
            self.timer = self.tuning.search_time;
            self.looking = false;
            self.head_to(house, goal);
        }

        if matches!(
            self.state,
            ManState::Wander | ManState::Investigate | ManState::Search
        ) {
            if let Some((at, _)) = senses
                .noises
                .iter()
                .rev()
                .find(|(at, radius)| at.distance(self.pos) <= *radius)
            {
                self.state = ManState::Investigate;
                if self.goal.is_none_or(|g| g.distance(*at) > 1.0) || self.replan <= 0.0 {
                    self.head_to(house, *at);
                }
            }
        }
        if self.state == ManState::Wander {
            self.sniff -= DT;
            if self.sniff <= 0.0 {
                self.sniff = self.tuning.sniff_every;
                self.state = ManState::Investigate;
                self.head_to(house, centre(cell_of(senses.player)));
            }
        }

        match self.state {
            ManState::Chase => {
                if self.gloat > 0.0 {
                    return acts;
                }
                let target = match (senses.hidden && self.saw_hide, senses.closet) {
                    (true, Some(closet)) => closet,
                    _ => senses.player,
                };
                let speed = self.speed(self.tuning.man_chase);
                let direct = first_hit(&house.blocks, self.pos, target, MAN_RADIUS * 0.8, 0.0)
                    .is_none_or(|t| t >= 0.999);
                if direct && (sees || senses.hidden) {
                    self.path.clear();
                    self.goal = Some(target);
                    self.walk(house, target, speed);
                } else {
                    if self.replan <= 0.0 || self.path.is_empty() {
                        self.head_to(house, target);
                    }
                    self.follow(house, speed);
                }
                let distance = self.pos.distance(target);
                if sees {
                    self.facing = (senses.player - self.pos).normalized();
                    if distance <= self.tuning.grab_reach && self.grab_wait <= 0.0 {
                        acts.push(Act::Grab);
                        self.grab_wait = self.tuning.grab_wait;
                        self.gloat = self.tuning.gloat;
                    } else if (self.tuning.volley_min..=self.tuning.volley_max).contains(&distance)
                        && self.volley_wait <= 0.0
                    {
                        acts.push(Act::Volley(self.facing));
                        self.volley_wait = self.tuning.volley_wait;
                    }
                } else if senses.hidden
                    && self.saw_hide
                    && distance <= self.tuning.grab_reach + 0.4
                    && self.grab_wait <= 0.0
                {
                    acts.push(Act::PullOut);
                    self.saw_hide = false;
                    self.grab_wait = self.tuning.grab_wait;
                    self.gloat = self.tuning.gloat;
                }
            }
            ManState::Investigate => {
                let speed = self.speed(self.tuning.man_investigate);
                if self.follow(house, speed) {
                    self.state = ManState::Search;
                    self.timer = self.tuning.search_time;
                    self.looking = true;
                }
            }
            ManState::Search => {
                let speed = self.speed(self.tuning.man_search);
                let arrived = self.follow(house, speed);
                self.looking |= arrived;
                if self.looking {
                    self.timer -= DT;
                }
                if arrived {
                    if self.timer <= 0.0 {
                        self.state = ManState::Wander;
                        self.last_seen = None;
                        self.goal = None;
                    } else {
                        // Looks around the room: a nearby spot in its lanes.
                        let c = centre(cell_of(self.pos));
                        let spots = [
                            v(0.0, 0.0),
                            v(3.0, 0.0),
                            v(-3.0, 0.0),
                            v(0.0, 3.0),
                            v(0.0, -3.0),
                        ];
                        let spot = c + spots[self.rng.below(spots.len())];
                        self.goal = Some(spot);
                        self.path = vec![c, spot];
                    }
                }
            }
            ManState::Wander => {
                let speed = self.speed(self.tuning.man_wander);
                if self.follow(house, speed) {
                    let here = cell_of(self.pos);
                    let next = house.neighbours(here);
                    let pick = if next.is_empty() {
                        here
                    } else {
                        next[self.rng.below(next.len())]
                    };
                    self.head_to(house, centre(pick));
                }
            }
            ManState::Frozen | ManState::Confused => {}
        }
        acts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn senses(player: V2) -> Senses<'static> {
        Senses {
            player,
            hidden: false,
            closet: None,
            noises: &[],
        }
    }

    /// A house and a spot in the man's own room, in his lane, in view.
    fn setup() -> (House, Man) {
        let house = House::new(3);
        let man = Man::new(house.man_start, 3);
        (house, man)
    }

    #[test]
    fn he_wanders_from_room_to_room() {
        let (house, mut man) = setup();
        let far = v(-100.0, -100.0);
        let start = cell_of(man.pos);
        let mut rooms = vec![start];
        for _ in 0..30 * 40 {
            man.step(&house, senses(far));
            assert_ne!(man.state, ManState::Chase);
            let here = cell_of(man.pos);
            if rooms.last() != Some(&here) {
                rooms.push(here);
            }
        }
        assert!(rooms.len() >= 3, "{rooms:?}");
    }

    #[test]
    fn he_investigates_a_noise_then_searches() {
        let (house, mut man) = setup();
        let far = v(-100.0, -100.0);
        let noise = [(centre(cell_of(man.pos)) + v(3.0, 0.0), 30.0)];
        let acts = man.step(
            &house,
            Senses {
                noises: &noise,
                ..senses(far)
            },
        );
        assert!(acts.is_empty());
        assert_eq!(man.state, ManState::Investigate);
        for _ in 0..30 * 5 {
            man.step(&house, senses(far));
        }
        assert_eq!(man.state, ManState::Search);
        for _ in 0..30 * 12 {
            man.step(&house, senses(far));
        }
        assert_eq!(man.state, ManState::Wander);
    }

    #[test]
    fn he_chases_on_sight_and_volleys_at_range_then_grabs_up_close() {
        let (house, mut man) = setup();
        let player = man.pos + v(0.0, 4.0);
        man.facing = v(0.0, 1.0);
        let mut acts = Vec::new();
        for _ in 0..30 * 3 {
            acts.extend(man.step(&house, senses(player)));
        }
        assert_eq!(acts.first(), Some(&Act::Spotted));
        assert_eq!(man.state, ManState::Chase);
        assert!(acts.contains(&Act::Grab), "{acts:?}");

        let (house, mut man) = setup();
        man.facing = v(0.0, 1.0);
        let player = man.pos + v(0.0, 3.6);
        man.volley_wait = 0.0;
        let acts = man.step(&house, senses(player));
        assert!(
            matches!(acts[..], [Act::Spotted, Act::Volley(_)]),
            "{acts:?}"
        );
    }

    #[test]
    fn he_does_not_see_behind_himself_or_through_walls() {
        let (house, mut man) = setup();
        man.facing = v(0.0, -1.0);
        assert!(!man.can_see(&house, man.pos + v(0.0, 4.5)));
        assert!(man.can_see(&house, man.pos + v(0.0, -4.5)));
        assert!(
            man.can_see(&house, man.pos + v(0.0, 3.0)),
            "close enough to notice"
        );
        // Through the wall beside the doorway.
        man.facing = v(8.0, 3.0).normalized();
        assert!(!man.can_see(&house, man.pos + v(8.0, 3.0)));
    }

    #[test]
    fn losing_sight_sends_him_to_search_the_last_seen_spot() {
        let (house, mut man) = setup();
        man.state = ManState::Chase;
        let seen = man.pos + v(0.0, 3.0);
        man.last_seen = Some(seen);
        let acts = man.step(
            &house,
            Senses {
                hidden: true,
                ..senses(seen)
            },
        );
        assert_eq!(acts, vec![Act::Lost]);
        assert_eq!(man.state, ManState::Search);
        assert_eq!(man.goal, Some(seen));
    }

    #[test]
    fn enough_fudge_freezes_him_and_he_thaws() {
        let (house, mut man) = setup();
        for hit in 1..FREEZE_HITS {
            assert!(!man.fudge(), "hit {hit}");
            assert!(man.slow > 0.0);
        }
        assert!(man.fudge());
        assert_eq!(man.state, ManState::Frozen);
        let at = man.pos;
        let player = man.pos + v(0.0, 2.0);
        for _ in 0..(FROZEN * 30.0) as usize - 2 {
            let acts = man.step(&house, senses(player));
            assert!(acts.is_empty());
            assert_eq!(man.pos, at, "frozen solid");
        }
        for _ in 0..4 {
            man.step(&house, senses(player));
        }
        assert_ne!(man.state, ManState::Frozen);
    }

    #[test]
    fn fudge_slows_him() {
        let (house, mut man) = setup();
        let player = man.pos + v(0.0, 4.4);
        man.facing = v(0.0, 1.0);
        man.grab_wait = 10.0;
        let mut fast = man.clone();
        man.fudge();
        let (a, b) = (man.pos, fast.pos);
        man.step(&house, senses(player));
        fast.step(&house, senses(player));
        let slow_step = man.pos.distance(a);
        let fast_step = fast.pos.distance(b);
        assert!(
            (slow_step - fast_step * SLOWED).abs() < 1e-3,
            "{slow_step} {fast_step}"
        );
        // Hits fade if they stop coming.
        for _ in 0..(FUDGE_MEMORY * 30.0) as usize + 2 {
            man.step(&house, senses(v(-100.0, -100.0)));
        }
        assert_eq!(man.fudge_hits, 0);
    }

    #[test]
    fn a_fart_shoves_him_away_and_he_forgets_you() {
        let (house, mut man) = setup();
        man.state = ManState::Chase;
        man.last_seen = Some(man.pos);
        let start = man.pos;
        man.fart(man.pos + v(0.0, -1.0));
        assert_eq!(man.state, ManState::Confused);
        assert_eq!(man.last_seen, None);
        let player = man.pos + v(0.0, 3.0);
        for _ in 0..10 {
            let acts = man.step(&house, senses(player));
            assert!(acts.is_empty(), "he can't see while confused");
        }
        assert!(man.pos.z > start.z + 0.5, "shoved away: {:?}", man.pos);
        for _ in 0..(CONFUSED * 30.0) as usize {
            man.step(&house, senses(v(-100.0, -100.0)));
        }
        assert_eq!(man.state, ManState::Wander);
    }

    #[test]
    fn he_opens_a_closet_only_if_he_saw_you_get_in() {
        let (house, mut man) = setup();
        let closet = man.pos + v(0.0, 2.0);
        let hiding = |_saw: bool| Senses {
            player: closet,
            hidden: true,
            closet: Some(closet),
            noises: &[],
        };
        man.state = ManState::Chase;
        man.saw_hide = true;
        let mut acts = Vec::new();
        for _ in 0..60 {
            acts.extend(man.step(&house, hiding(true)));
        }
        assert!(acts.contains(&Act::PullOut), "{acts:?}");

        let (house, mut man) = setup();
        man.state = ManState::Chase;
        man.saw_hide = false;
        let mut acts = Vec::new();
        for _ in 0..60 {
            acts.extend(man.step(&house, hiding(false)));
        }
        assert!(!acts.contains(&Act::PullOut));
        assert!(acts.contains(&Act::Lost));
    }
}
