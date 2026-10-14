//! A run: the player, their tools, the shots in the air, the Ice Cream
//! Man, keys, fridges, closets, the front door, and the run card.

use crate::geom::{V2, first_hit, segment_distance, slide, v};
use crate::layout::{House, Pickup};
use crate::man::{Act, Man, ManState, Senses};
use crate::tuning::*;
use crate::{DT, HZ};

/// The hot bar's tools, keys 1 to 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Fudge,
    Fart,
    /// The key ring: shows the keys; a click jingles them, which he hears.
    Keys,
}

impl Tool {
    pub const ALL: [Self; 3] = [Self::Fudge, Self::Fart, Self::Keys];
}

/// What the player does this step. Directions are on the floor, in world
/// space; the host turns keys and the camera into them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// Where to walk, up to length 1.
    pub walk: V2,
    pub sprint: bool,
    /// Held: a jump starts when it is pressed on the ground.
    pub jump: bool,
    /// Held: the tool fires as fast as it can.
    pub fire: bool,
    /// Where tools point.
    pub aim: V2,
    /// Held: E. Pressing it hides, comes out, opens fridges; holding it
    /// unlocks the front door.
    pub interact: bool,
    pub select: Option<Tool>,
}

/// Choices for tests and captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Whether the Ice Cream Man is in the house (off: captures and tests).
    pub man: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { man: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Playing,
    Escaped,
    /// Health ran out: he plopped you into a cone.
    Scooped,
}

/// What the interact key would do here, for the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prompt {
    Hide,
    Leave,
    Fridge,
    /// All keys: hold to unlock.
    Unlock,
    /// Not all keys yet.
    Locked,
}

/// Something that happened this step, for sounds and effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    FudgeFired,
    FudgeHit,
    /// A squirt hit a wall here.
    FudgeSplat(V2),
    Frozen,
    /// A fart cloud went off here.
    Fart(V2),
    FartHit,
    Jingle,
    /// The tool is empty.
    Empty(Tool),
    Volley,
    SprinkleHit,
    Grabbed,
    PulledOut,
    Spotted,
    Lost,
    /// A key was picked up; the count so far.
    Key(u32),
    Pickup(Pickup),
    Hid,
    Unhid,
    DoorLocked,
    Jumped,
    Escaped,
    Scooped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
    Fudge,
    /// A sprinkle; its colour, 0 to 4.
    Sprinkle(u8),
}

/// Something flying.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub kind: ShotKind,
    pub pos: V2,
    pub height: f32,
    pub vel: V2,
    /// Metres left before it falls.
    pub left: f32,
}

/// A fart cloud, hanging about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cloud {
    pub at: V2,
    pub age: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub pos: V2,
    /// Feet above the floor.
    pub height: f32,
    vy: f32,
    pub facing: V2,
    pub health: f32,
    pub tool: Tool,
    pub fudge: u32,
    pub farts: u32,
    pub keys: u32,
    /// The closet hidden in.
    pub hidden: Option<usize>,
    pub shove: V2,
    /// Seconds since the last hit.
    pub since_hit: f32,
    fire_wait: f32,
    /// Seconds E has been held at the front door with every key.
    pub unlock: f32,
    pub moving: bool,
    pub sprinting: bool,
}

/// What the run card shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub hits_taken: u32,
    pub fudge_shots: u32,
    pub fudge_hits: u32,
    pub farts: u32,
}

/// The run card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunCard {
    pub escaped: bool,
    pub seconds: u32,
    pub keys: u32,
    pub hits_taken: u32,
    /// Fudge that hit him, of every squirt fired, in percent; `None`
    /// without a squirt.
    pub accuracy: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Game {
    pub house: House,
    pub player: Player,
    pub man: Man,
    pub options: Options,
    /// The numbers this run plays by.
    pub tuning: Tuning,
    pub shots: Vec<Shot>,
    pub clouds: Vec<Cloud>,
    /// Which of the house's keys were picked up.
    pub taken: Vec<bool>,
    /// Which fridges were opened.
    pub opened: Vec<bool>,
    pub status: Status,
    pub tick: u64,
    /// This step's events.
    pub events: Vec<Event>,
    pub stats: Stats,
    noises: Vec<(V2, f32)>,
    was_interact: bool,
    was_jump: bool,
    sprinkle: u8,
}

/// The fan of a volley: each pellet's turn, as cosine and sine.
fn fan(spread: f32) -> [(f32, f32); VOLLEY_PELLETS] {
    let mut turns = [(1.0, 0.0); VOLLEY_PELLETS];
    let middle = (VOLLEY_PELLETS / 2) as f32;
    for (i, turn) in turns.iter_mut().enumerate() {
        let degrees = (i as f32 - middle) * spread;
        let radians = degrees.to_radians();
        *turn = (radians.cos(), radians.sin());
    }
    turns
}

impl Game {
    #[must_use]
    pub fn new(seed: u64, options: Options) -> Self {
        Self::tuned(seed, options, Tuning::default())
    }

    /// A run with its numbers from `tuning` (Game Lab's versions).
    #[must_use]
    pub fn tuned(seed: u64, options: Options, tuning: Tuning) -> Self {
        let house = House::tuned(seed, tuning.extra_closets as usize);
        let man = Man::tuned(house.man_start, seed, tuning);
        let player = Player {
            pos: house.spawn,
            height: 0.0,
            vy: 0.0,
            facing: v(0.0, -1.0),
            health: tuning.max_health,
            tool: Tool::Fudge,
            fudge: tuning.fudge_start,
            farts: tuning.farts_start,
            keys: 0,
            hidden: None,
            shove: V2::ZERO,
            since_hit: tuning.regen_after,
            fire_wait: 0.0,
            unlock: 0.0,
            moving: false,
            sprinting: false,
        };
        Self {
            taken: vec![false; house.keys.len()],
            opened: vec![false; house.fridges.len()],
            house,
            player,
            man,
            options,
            tuning,
            shots: Vec::new(),
            clouds: Vec::new(),
            status: Status::Playing,
            tick: 0,
            events: Vec::new(),
            stats: Stats::default(),
            noises: Vec::new(),
            was_interact: false,
            was_jump: false,
            sprinkle: 0,
        }
    }

    /// Seconds played.
    #[must_use]
    pub fn seconds(&self) -> f32 {
        self.tick as f32 / HZ as f32
    }

    #[must_use]
    pub fn run_card(&self) -> RunCard {
        RunCard {
            escaped: self.status == Status::Escaped,
            seconds: (self.tick / u64::from(HZ)) as u32,
            keys: self.player.keys,
            hits_taken: self.stats.hits_taken,
            accuracy: (self.stats.fudge_shots > 0)
                .then(|| self.stats.fudge_hits * 100 / self.stats.fudge_shots),
        }
    }

    /// Whether the Ice Cream Man is in the house and active.
    #[must_use]
    pub fn man_here(&self) -> bool {
        self.options.man
    }

    /// The closet's spot while hiding.
    #[must_use]
    pub fn closet_spot(&self) -> Option<V2> {
        self.player.hidden.map(|i| self.house.closets[i].at)
    }

    /// What E would do here.
    #[must_use]
    pub fn prompt(&self) -> Option<Prompt> {
        self.target().map(|(prompt, _)| prompt)
    }

    /// The interactable in reach: the front door first, then the nearest
    /// unopened fridge or closet.
    fn target(&self) -> Option<(Prompt, usize)> {
        if self.player.hidden.is_some() {
            return Some((Prompt::Leave, 0));
        }
        let p = self.player.pos;
        if p.distance(self.house.front.stand) <= self.tuning.reach {
            let prompt = if self.player.keys as usize >= self.house.keys.len() {
                Prompt::Unlock
            } else {
                Prompt::Locked
            };
            return Some((prompt, 0));
        }
        let mut best: Option<(Prompt, usize, f32)> = None;
        for (i, (spot, _)) in self.house.fridges.iter().enumerate() {
            let d = p.distance(spot.at);
            if !self.opened[i] && d <= self.tuning.reach && best.is_none_or(|b| d < b.2) {
                best = Some((Prompt::Fridge, i, d));
            }
        }
        for (i, spot) in self.house.closets.iter().enumerate() {
            let d = p.distance(spot.at);
            if d <= self.tuning.reach && best.is_none_or(|b| d < b.2) {
                best = Some((Prompt::Hide, i, d));
            }
        }
        best.map(|(prompt, i, _)| (prompt, i))
    }

    fn hurt(&mut self, damage: f32) {
        self.player.health = (self.player.health - damage).max(0.0);
        self.player.since_hit = 0.0;
        self.stats.hits_taken += 1;
        if self.player.health <= 0.0 && self.status == Status::Playing {
            self.status = Status::Scooped;
            self.events.push(Event::Scooped);
        }
    }

    /// One step of [`DT`] seconds.
    pub fn step(&mut self, input: Input) {
        self.events.clear();
        if self.status != Status::Playing {
            return;
        }
        self.tick += 1;
        self.noises.clear();
        let pressed = input.interact && !self.was_interact;
        let jumped = input.jump && !self.was_jump;
        self.was_interact = input.interact;
        self.was_jump = input.jump;
        if let Some(tool) = input.select {
            self.player.tool = tool;
        }

        self.move_player(input, jumped);
        self.fire(input);
        self.interact(input, pressed);
        self.pick_up_keys();
        self.fly();
        if self.options.man {
            self.step_man();
        }
        self.clouds.iter_mut().for_each(|c| c.age += DT);
        self.clouds.retain(|c| c.age < self.tuning.cloud_life);

        let player = &mut self.player;
        player.since_hit += DT;
        if player.since_hit >= self.tuning.regen_after {
            player.health =
                (player.health + self.tuning.regen_rate * DT).min(self.tuning.max_health);
        }
    }

    fn move_player(&mut self, input: Input, jumped: bool) {
        let blocks = &self.house.blocks;
        let player = &mut self.player;
        player.fire_wait = (player.fire_wait - DT).max(0.0);
        if player.hidden.is_some() {
            player.moving = false;
            player.sprinting = false;
            return;
        }
        let walk = input.walk.clamped(1.0);
        player.moving = walk.length() > 0.1;
        player.sprinting = player.moving && input.sprint;
        let speed = if player.sprinting {
            self.tuning.sprint
        } else {
            self.tuning.walk
        };
        let delta = walk * (speed * DT) + player.shove * DT;
        player.pos = slide(blocks, player.pos, delta, PLAYER_RADIUS, player.height);
        player.shove = player.shove * (1.0 - SHOVE_FADE * DT).max(0.0);
        if player.moving {
            player.facing = walk.normalized();
        }
        if player.sprinting {
            self.noises.push((player.pos, self.tuning.sprint_noise));
        }
        if jumped && player.height <= 0.0 {
            player.vy = self.tuning.jump_speed;
            self.events.push(Event::Jumped);
        }
        player.height += player.vy * DT;
        player.vy -= self.tuning.gravity * DT;
        if player.height <= 0.0 {
            player.height = 0.0;
            player.vy = 0.0;
        }
    }

    fn fire(&mut self, input: Input) {
        let player = &mut self.player;
        if !input.fire || player.fire_wait > 0.0 || player.hidden.is_some() {
            return;
        }
        let aim = match input.aim.normalized() {
            V2::ZERO => player.facing,
            aim => aim,
        };
        match player.tool {
            Tool::Fudge if player.fudge > 0 => {
                player.fudge -= 1;
                player.fire_wait = 1.0 / self.tuning.fudge_rate;
                self.shots.push(Shot {
                    kind: ShotKind::Fudge,
                    pos: player.pos + aim * 0.5,
                    height: 1.1,
                    vel: aim * self.tuning.fudge_speed,
                    left: self.tuning.fudge_range,
                });
                self.stats.fudge_shots += 1;
                self.noises.push((player.pos, self.tuning.fudge_noise));
                self.events.push(Event::FudgeFired);
            }
            Tool::Fart if player.farts > 0 => {
                player.farts -= 1;
                player.fire_wait = self.tuning.fart_wait;
                let from = player.pos;
                let to = from + aim * self.tuning.fart_range;
                let at = match first_hit(&self.house.blocks, from, to, 0.3, 1.0) {
                    Some(t) => from + (to - from) * (t * 0.9),
                    None => to,
                };
                self.clouds.push(Cloud { at, age: 0.0 });
                self.stats.farts += 1;
                self.events.push(Event::Fart(at));
                if self.options.man && self.man.pos.distance(at) <= self.tuning.fart_radius {
                    self.man.fart(at);
                    self.events.push(Event::FartHit);
                }
            }
            Tool::Keys => {
                player.fire_wait = self.tuning.jingle_wait;
                self.noises.push((player.pos, self.tuning.jingle_noise));
                self.events.push(Event::Jingle);
            }
            tool => {
                player.fire_wait = 0.4;
                self.events.push(Event::Empty(tool));
            }
        }
    }

    fn interact(&mut self, input: Input, pressed: bool) {
        let target = self.target();
        if !matches!(target, Some((Prompt::Unlock, _))) || !input.interact {
            self.player.unlock = 0.0;
        }
        let Some((prompt, i)) = target else {
            return;
        };
        match prompt {
            Prompt::Leave if pressed => {
                self.player.hidden = None;
                self.events.push(Event::Unhid);
            }
            Prompt::Hide if pressed => {
                let spot = self.house.closets[i].at;
                // He knows only if he watched you get in.
                self.man.saw_hide = self.options.man
                    && self.man.state == ManState::Chase
                    && self.man.can_see(&self.house, self.player.pos);
                self.player.hidden = Some(i);
                self.player.pos = spot;
                self.player.height = 0.0;
                self.events.push(Event::Hid);
            }
            Prompt::Fridge if pressed => {
                self.opened[i] = true;
                let pickup = self.house.fridges[i].1;
                match pickup {
                    Pickup::Fudge => {
                        self.player.fudge = (self.player.fudge + self.tuning.fudge_pickup)
                            .min(self.tuning.fudge_max);
                    }
                    Pickup::Fart => {
                        self.player.farts = (self.player.farts + 1).min(self.tuning.farts_max)
                    }
                }
                self.noises
                    .push((self.player.pos, self.tuning.fridge_noise));
                self.events.push(Event::Pickup(pickup));
            }
            Prompt::Locked if pressed => {
                self.noises.push((self.player.pos, self.tuning.door_noise));
                self.events.push(Event::DoorLocked);
            }
            Prompt::Unlock if input.interact => {
                self.player.unlock += DT;
                if self.tick.is_multiple_of(u64::from(HZ / 2)) {
                    self.noises.push((self.player.pos, self.tuning.door_noise));
                }
                if self.player.unlock >= self.tuning.unlock - 1e-4 {
                    self.status = Status::Escaped;
                    self.events.push(Event::Escaped);
                }
            }
            _ => {}
        }
    }

    fn pick_up_keys(&mut self) {
        if self.player.hidden.is_some() {
            return;
        }
        for (i, key) in self.house.keys.iter().enumerate() {
            if !self.taken[i] && key.distance(self.player.pos) <= self.tuning.key_reach {
                self.taken[i] = true;
                self.player.keys += 1;
                self.events.push(Event::Key(self.player.keys));
            }
        }
    }

    fn fly(&mut self) {
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|shot| {
            let from = shot.pos;
            let to = from + shot.vel * DT;
            let travel = shot.vel.length() * DT;
            if let Some(t) = first_hit(&self.house.blocks, from, to, 0.0, shot.height) {
                if shot.kind == ShotKind::Fudge {
                    self.events.push(Event::FudgeSplat(from + (to - from) * t));
                }
                return false;
            }
            match shot.kind {
                ShotKind::Fudge => {
                    if self.options.man
                        && segment_distance(from, to, self.man.pos) <= MAN_RADIUS + 0.25
                    {
                        self.stats.fudge_hits += 1;
                        self.events.push(Event::FudgeHit);
                        if self.man.fudge() {
                            self.events.push(Event::Frozen);
                        }
                        return false;
                    }
                }
                ShotKind::Sprinkle(_) => {
                    if self.player.hidden.is_none()
                        && self.player.height < self.tuning.pellet_dodge
                        && segment_distance(from, to, self.player.pos) <= PLAYER_RADIUS + 0.1
                    {
                        self.events.push(Event::SprinkleHit);
                        self.hurt(self.tuning.sprinkle_damage);
                        return false;
                    }
                }
            }
            shot.pos = to;
            shot.left -= travel;
            shot.left > 0.0
        });
        self.shots = shots;
    }

    fn step_man(&mut self) {
        let senses = Senses {
            player: self.player.pos,
            hidden: self.player.hidden.is_some(),
            closet: self.closet_spot(),
            noises: &self.noises,
        };
        let acts = self.man.step(&self.house, senses);
        for act in acts {
            match act {
                Act::Volley(dir) => {
                    for (cos, sin) in fan(self.tuning.volley_spread) {
                        let vel = dir.rotated(cos, sin) * self.tuning.pellet_speed;
                        self.sprinkle = (self.sprinkle + 1) % 5;
                        self.shots.push(Shot {
                            kind: ShotKind::Sprinkle(self.sprinkle),
                            pos: self.man.pos + dir * (MAN_RADIUS + 0.2),
                            height: 1.2,
                            vel,
                            left: self.tuning.volley_max + 3.0,
                        });
                    }
                    self.events.push(Event::Volley);
                }
                Act::Grab => {
                    let away = (self.player.pos - self.man.pos).normalized();
                    self.player.shove = away * self.tuning.grab_shove;
                    self.events.push(Event::Grabbed);
                    self.hurt(self.tuning.grab_damage);
                }
                Act::PullOut => {
                    self.player.hidden = None;
                    let away = (self.player.pos - self.man.pos).normalized();
                    self.player.shove = away * self.tuning.grab_shove;
                    self.events.push(Event::PulledOut);
                    self.hurt(self.tuning.grab_damage);
                }
                Act::Spotted => self.events.push(Event::Spotted),
                Act::Lost => self.events.push(Event::Lost),
                Act::Froze => self.events.push(Event::Frozen),
            }
        }
    }

    /// A short digest of the whole state, for determinism checks.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bits: u64| {
            hash ^= bits;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        };
        let p = &self.player;
        for f in [
            p.pos.x,
            p.pos.z,
            p.height,
            p.health,
            self.man.pos.x,
            self.man.pos.z,
            self.man.timer,
        ] {
            eat(u64::from(f.to_bits()));
        }
        eat(u64::from(p.fudge) << 32 | u64::from(p.farts) << 16 | u64::from(p.keys));
        eat(self.tick);
        eat(self.man.state as u64);
        eat(self.status as u64);
        eat(self.shots.len() as u64);
        for shot in &self.shots {
            eat(u64::from(shot.pos.x.to_bits()));
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::centre;

    fn calm(seed: u64) -> Game {
        Game::new(seed, Options { man: false })
    }

    fn walk_to(game: &mut Game, to: V2) {
        for _ in 0..30 * 20 {
            let d = to - game.player.pos;
            if d.length() < 0.1 {
                break;
            }
            let walk = (d * (1.0 / (WALK * DT))).clamped(1.0);
            game.step(Input {
                walk,
                ..Input::default()
            });
        }
    }

    /// Walks a route through the house.
    fn travel(game: &mut Game, to: V2) {
        for point in game.house.route(game.player.pos, to, PLAYER_RADIUS) {
            walk_to(game, point);
        }
    }

    #[test]
    fn a_run_starts_full_in_the_bedroom() {
        let game = Game::new(5, Options::default());
        assert_eq!(game.player.health, MAX_HEALTH);
        assert_eq!(game.player.fudge, FUDGE_START);
        assert_eq!(game.player.farts, FARTS_START);
        assert_eq!(game.player.keys, 0);
        assert_eq!(game.player.tool, Tool::Fudge);
        assert_eq!(game.status, Status::Playing);
        assert_eq!(game.man.state, ManState::Wander);
    }

    #[test]
    fn walls_stop_the_player() {
        let mut game = calm(1);
        for _ in 0..30 * 6 {
            game.step(Input {
                walk: v(0.0, 1.0),
                sprint: true,
                ..Input::default()
            });
        }
        let c = centre(crate::layout::cell_of(game.house.spawn));
        assert!(game.player.pos.z < c.z + 5.0 - 0.15 - PLAYER_RADIUS + 0.01);
    }

    #[test]
    fn a_jump_goes_up_and_comes_down() {
        let mut game = calm(1);
        game.step(Input {
            jump: true,
            ..Input::default()
        });
        assert_eq!(game.events, vec![Event::Jumped]);
        let mut top: f32 = 0.0;
        for _ in 0..30 {
            game.step(Input {
                jump: true,
                ..Input::default()
            });
            top = top.max(game.player.height);
        }
        assert!(top > 0.8 && top < 1.2, "{top}");
        assert_eq!(game.player.height, 0.0);
    }

    #[test]
    fn fudge_uses_ammo_hits_him_and_freezes_him() {
        let mut game = Game::new(3, Options::default());
        // Stand in his room's lane, facing him.
        let at = game.man.pos + v(0.0, 3.0);
        game.player.pos = at;
        game.man.state = ManState::Frozen;
        game.man.timer = 0.0;
        game.man.state = ManState::Wander;
        let aim = v(0.0, -1.0);
        let mut hits = 0;
        let mut froze = false;
        for _ in 0..30 * 2 {
            game.man.pos = at + v(0.0, -3.0);
            game.step(Input {
                fire: true,
                aim,
                ..Input::default()
            });
            hits += game
                .events
                .iter()
                .filter(|e| **e == Event::FudgeHit)
                .count();
            froze |= game.events.contains(&Event::Frozen);
            if froze {
                break;
            }
        }
        assert!(froze, "{hits} hits");
        assert_eq!(game.man.state, ManState::Frozen);
        assert_eq!(game.stats.fudge_hits as usize, hits);
        assert!(game.player.fudge < FUDGE_START);
        assert!(game.run_card().accuracy.is_some());
    }

    #[test]
    fn an_empty_blaster_says_so() {
        let mut game = calm(2);
        game.player.fudge = 0;
        game.step(Input {
            fire: true,
            ..Input::default()
        });
        assert_eq!(game.events, vec![Event::Empty(Tool::Fudge)]);
    }

    #[test]
    fn the_fart_launcher_shoves_and_confuses_him() {
        let mut game = Game::new(3, Options::default());
        game.player.pos = game.man.pos + v(0.0, 2.5);
        game.man.state = ManState::Chase;
        game.step(Input {
            select: Some(Tool::Fart),
            fire: true,
            aim: v(0.0, -1.0),
            ..Input::default()
        });
        assert!(game.events.contains(&Event::FartHit), "{:?}", game.events);
        assert_eq!(game.player.farts, FARTS_START - 1);
        assert_eq!(game.man.state, ManState::Confused);
        assert_eq!(game.clouds.len(), 1);
    }

    #[test]
    fn the_key_ring_jingles_and_he_comes_to_look() {
        let mut game = Game::new(3, Options::default());
        game.player.pos = game.man.pos + v(10.0, 0.0);
        game.man.facing = v(-1.0, 0.0);
        game.step(Input {
            select: Some(Tool::Keys),
            fire: true,
            ..Input::default()
        });
        assert!(game.events.contains(&Event::Jingle));
        assert!(matches!(
            game.man.state,
            ManState::Investigate | ManState::Chase
        ));
    }

    #[test]
    fn sprinkles_hurt_and_a_jump_dodges_them() {
        let pellet = |game: &Game| Shot {
            kind: ShotKind::Sprinkle(0),
            pos: game.player.pos + v(-2.0, 0.0),
            height: 1.2,
            vel: v(PELLET_SPEED, 0.0),
            left: 10.0,
        };
        let mut game = calm(4);
        game.shots.push(pellet(&game));
        for _ in 0..10 {
            game.step(Input::default());
        }
        assert_eq!(game.player.health, MAX_HEALTH - SPRINKLE_DAMAGE);
        assert_eq!(game.stats.hits_taken, 1);

        let mut game = calm(4);
        game.step(Input {
            jump: true,
            ..Input::default()
        });
        for _ in 0..4 {
            game.step(Input::default());
        }
        game.shots.push(pellet(&game));
        for _ in 0..6 {
            game.step(Input::default());
        }
        assert_eq!(game.player.health, MAX_HEALTH, "jumped over it");
    }

    #[test]
    fn a_grab_hurts_more_and_shoves() {
        let mut game = Game::new(3, Options::default());
        game.player.pos = game.man.pos + v(0.0, 1.0);
        game.man.facing = v(0.0, 1.0);
        let mut grabbed = false;
        for _ in 0..30 {
            game.step(Input::default());
            grabbed |= game.events.contains(&Event::Grabbed);
        }
        assert!(grabbed);
        assert!(game.player.health <= MAX_HEALTH - GRAB_DAMAGE);
    }

    #[test]
    fn health_comes_back_after_ten_quiet_seconds() {
        let mut game = calm(4);
        game.hurt(30.0);
        for _ in 0..(REGEN_AFTER * 30.0) as usize - 5 {
            game.step(Input::default());
        }
        assert_eq!(game.player.health, MAX_HEALTH - 30.0);
        for _ in 0..30 * 3 {
            game.step(Input::default());
        }
        assert!(game.player.health > MAX_HEALTH - 30.0 + 8.0);
        for _ in 0..30 * 30 {
            game.step(Input::default());
        }
        assert_eq!(game.player.health, MAX_HEALTH);
    }

    #[test]
    fn running_out_of_health_gets_you_scooped() {
        let mut game = calm(4);
        game.hurt(95.0);
        game.hurt(5.0);
        assert_eq!(game.status, Status::Scooped);
        assert_eq!(game.events, vec![Event::Scooped]);
        let card = game.run_card();
        assert!(!card.escaped);
        assert_eq!(card.hits_taken, 2);
        let before = game.fingerprint();
        game.step(Input::default());
        assert_eq!(game.fingerprint(), before, "a finished run stays put");
    }

    #[test]
    fn hiding_in_a_closet_and_coming_out() {
        let mut game = calm(6);
        let spot = game.house.closets[0].at;
        travel(&mut game, spot);
        assert_eq!(game.prompt(), Some(Prompt::Hide));
        game.step(Input {
            interact: true,
            ..Input::default()
        });
        assert_eq!(game.events, vec![Event::Hid]);
        assert_eq!(game.prompt(), Some(Prompt::Leave));
        let at = game.player.pos;
        game.step(Input {
            walk: v(1.0, 0.0),
            interact: true,
            ..Input::default()
        });
        assert_eq!(game.player.pos, at, "no walking out of a closet");
        assert!(game.events.is_empty(), "a held key is one press");
        game.step(Input::default());
        game.step(Input {
            interact: true,
            ..Input::default()
        });
        assert_eq!(game.events, vec![Event::Unhid]);
    }

    #[test]
    fn a_hidden_player_is_not_seen() {
        let mut game = Game::new(3, Options::default());
        let closet = game.house.closets[0];
        game.player.hidden = Some(0);
        game.player.pos = closet.at;
        game.man.pos = closet.at + v(0.0, 0.0);
        game.man.facing = v(1.0, 0.0);
        for _ in 0..30 {
            game.step(Input::default());
            assert_ne!(game.man.state, ManState::Chase);
        }
    }

    #[test]
    fn fridges_give_ammo_once() {
        let seed = (0..50)
            .find(|s| !House::new(*s).fridges.is_empty())
            .unwrap();
        let mut game = calm(seed);
        game.player.fudge = 0;
        game.player.farts = 0;
        let spot = game.house.fridges[0].0.at;
        travel(&mut game, spot);
        assert_eq!(game.prompt(), Some(Prompt::Fridge));
        game.step(Input {
            interact: true,
            ..Input::default()
        });
        assert!(matches!(game.events[..], [Event::Pickup(_)]));
        assert!(game.player.fudge == FUDGE_PICKUP || game.player.farts == 1);
        assert_ne!(game.prompt(), Some(Prompt::Fridge));
    }

    #[test]
    fn the_front_door_needs_three_keys_and_a_two_second_hold() {
        let mut game = calm(8);
        let stand = game.house.front.stand;
        travel(&mut game, stand);
        assert_eq!(game.prompt(), Some(Prompt::Locked));
        game.step(Input {
            interact: true,
            ..Input::default()
        });
        assert_eq!(game.events, vec![Event::DoorLocked]);
        for key in game.house.keys.clone() {
            travel(&mut game, key);
        }
        assert_eq!(game.player.keys, 3);
        travel(&mut game, stand);
        assert_eq!(game.prompt(), Some(Prompt::Unlock));
        let hold = Input {
            interact: true,
            ..Input::default()
        };
        for _ in 0..30 {
            game.step(hold);
        }
        // Letting go starts it over.
        game.step(Input::default());
        assert_eq!(game.player.unlock, 0.0);
        for _ in 0..(UNLOCK * 30.0) as usize - 1 {
            game.step(hold);
            assert_eq!(game.status, Status::Playing);
        }
        game.step(hold);
        assert_eq!(game.status, Status::Escaped);
        assert!(game.events.contains(&Event::Escaped));
        let card = game.run_card();
        assert!(card.escaped);
        assert_eq!(card.keys, 3);
        assert_eq!(card.accuracy, None);
        assert!(card.seconds > 5);
    }

    #[test]
    fn walking_over_a_key_picks_it_up() {
        let mut game = calm(9);
        let key = game.house.keys[1];
        travel(&mut game, key);
        assert_eq!(game.player.keys, 1);
        assert!(game.taken[1]);
    }
}
