//! A run: the player, their tools, the shots in the air, the Ice Cream
//! Man, keys, fridges, closets, the front door, and the run card.

use crate::geom::{V2, first_hit, segment_distance, slide, v};
use crate::layout::{BUTTONS, House, Pickup, cell_of, centre};
use crate::man::{Act, Man, ManState, Senses};
use crate::minion::Minion;
use crate::rng::Rng;
use crate::tuning::*;
use crate::{DT, HZ};

/// The hot bar's tools. A run starts with the first three; the rest are
/// earned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Fudge,
    Fart,
    /// The key ring: shows the keys; a click jingles them, which he hears.
    Keys,
    /// Melee: a quick wide swing. Dropped by a melted minion.
    Sword,
    /// Melee with reach: a stretched-out waffle cone. Behind the button
    /// puzzle.
    Spear,
    /// Fires cones. For freezing him with hot fudge.
    Pistol,
    /// Throws sugar that bursts. For melting minions.
    Sugar,
    /// One super long sprinkle that makes your own minions. For winning.
    Staff,
}

impl Tool {
    pub const ALL: [Self; 8] = [
        Self::Fudge,
        Self::Fart,
        Self::Keys,
        Self::Sword,
        Self::Spear,
        Self::Pistol,
        Self::Sugar,
        Self::Staff,
    ];
    /// The tools a run starts with.
    pub const START: [Self; 3] = [Self::Fudge, Self::Fart, Self::Keys];
    /// The four weapons earned in a run.
    pub const EARNED: [Self; 4] = [Self::Sword, Self::Spear, Self::Pistol, Self::Sugar];
}

/// Something lying on the floor to walk over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropKind {
    /// An edible ice cream cone: extra health for a while.
    Cone,
    /// An earned weapon.
    Weapon(Tool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    pub at: V2,
    pub kind: DropKind,
    pub age: f32,
}

/// How long a dropped cone lies about before it melts away, in seconds.
pub const CONE_LIFE: f32 = 45.0;
/// The most of your own minions at once.
pub const MAX_FRIENDS: usize = 24;

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
    /// The earned weapons melted him: a win too.
    Melted,
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
    /// A sprinkle button of the spear puzzle.
    Button,
    /// The cone cabinet, still locked.
    Cabinet,
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
    /// A sword or spear swing.
    Swing(Tool),
    ConeFired,
    SugarFired,
    /// Sugar burst here.
    SugarBurst(V2),
    /// The sprinkle staff's long sprinkle, from and to.
    Staff(V2, V2),
    /// Your own minions came, this many.
    FriendsCame(u32),
    /// An earned weapon hit him.
    ManHit,
    /// He melted.
    Melted,
    /// A minion came, here.
    MinionCame(V2),
    MinionHit(V2),
    MinionMelted(V2),
    /// A minion bit you.
    MinionBite,
    /// An earned weapon is yours.
    Earned(Tool),
    /// A weapon fell on the floor here, to pick up.
    WeaponDropped(Tool, V2),
    ConeDropped(V2),
    ConeEaten,
    /// The cone's extra health wore off.
    BoostOver,
    /// A puzzle button lit: how many are lit now.
    Button(u32),
    /// A button out of order: they all went dark.
    PuzzleWrong,
    /// The cone cabinet opened.
    PuzzleSolved,
    CabinetLocked,
    /// Free roam began.
    FreeRoam,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotKind {
    Fudge,
    /// A cone from the cone pistol.
    Cone,
    /// Sugar from the sugar launcher: it bursts.
    Sugar,
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
    /// Cones for the cone pistol, and sugar for the launcher.
    pub cones: u32,
    pub sugar: u32,
    /// The tools in the hot bar, in [`Tool::ALL`] order.
    pub owned: Vec<Tool>,
    /// Seconds left of an eaten cone's extra health.
    pub boost: f32,
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
    /// Times fudge froze him.
    pub freezes: u32,
    pub minions_melted: u32,
    pub cones_eaten: u32,
    /// Damage the earned weapons and your minions did him.
    pub man_damage: u32,
}

/// The run card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunCard {
    pub escaped: bool,
    /// He melted (a win, or in free roam).
    pub melted: bool,
    pub minions_melted: u32,
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
    /// Minions, his and yours.
    pub minions: Vec<Minion>,
    /// Cones and weapons on the floor.
    pub drops: Vec<Drop>,
    /// Puzzle buttons lit, in order.
    pub lit: u32,
    /// The cone cabinet is open.
    pub solved: bool,
    /// He melted.
    pub man_melted: bool,
    /// Playing on after a win, with the sprinkle staff.
    pub free_roam: bool,
    /// How the run was won, kept through free roam.
    pub won: Option<Status>,
    noises: Vec<(V2, f32)>,
    was_interact: bool,
    was_jump: bool,
    sprinkle: u8,
    minion_wait: f32,
    sword_dropped: bool,
    rng: Rng,
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
        let house = House::of(seed, &tuning);
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
            cones: 0,
            sugar: 0,
            owned: Tool::START.to_vec(),
            boost: 0.0,
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
            minions: Vec::new(),
            drops: Vec::new(),
            lit: 0,
            solved: false,
            man_melted: false,
            free_roam: false,
            won: None,
            noises: Vec::new(),
            was_interact: false,
            was_jump: false,
            sprinkle: 0,
            minion_wait: tuning.minion_first,
            sword_dropped: false,
            rng: Rng::new(seed ^ 0x3141_5926_5358_9793),
        }
    }

    /// Whether the earned weapons, the puzzle and his melting are in this
    /// version.
    #[must_use]
    pub fn weapons_on(&self) -> bool {
        self.tuning.earned_weapons == 1
    }

    /// Whether the player has `tool`.
    #[must_use]
    pub fn has(&self, tool: Tool) -> bool {
        self.player.owned.contains(&tool)
    }

    /// Full health now: more while an eaten cone lasts.
    #[must_use]
    pub fn max_health(&self) -> f32 {
        let boost = if self.player.boost > 0.0 {
            self.tuning.cone_boost
        } else {
            0.0
        };
        self.tuning.max_health * (1.0 + boost)
    }

    /// A tool is the player's: in the hot bar (in its place) and in hand.
    fn earn(&mut self, tool: Tool) {
        if self.has(tool) {
            return;
        }
        let place = |t: &Tool| Tool::ALL.iter().position(|x| x == t).unwrap_or(0);
        self.player.owned.push(tool);
        self.player.owned.sort_by_key(place);
        match tool {
            Tool::Pistol => self.player.cones = self.tuning.pistol_ammo,
            Tool::Sugar => self.player.sugar = self.tuning.sugar_ammo,
            _ => {}
        }
        self.player.tool = tool;
        self.events.push(Event::Earned(tool));
    }

    /// Every earned weapon at once (captures and tests).
    pub fn arm(&mut self) {
        for tool in Tool::EARNED {
            self.earn(tool);
        }
        self.player.tool = Tool::Spear;
        self.events.clear();
    }

    /// After a win, plays on in free roam with the sprinkle staff, when
    /// this version allows it. Returns whether it did.
    pub fn keep_playing(&mut self) -> bool {
        if self.tuning.free_roam != 1 || !matches!(self.status, Status::Escaped | Status::Melted) {
            return false;
        }
        self.status = Status::Playing;
        self.free_roam = true;
        self.player.unlock = 0.0;
        self.player.health = self.max_health();
        self.events.clear();
        self.earn(Tool::Staff);
        self.events.push(Event::FreeRoam);
        true
    }

    /// Seconds played.
    #[must_use]
    pub fn seconds(&self) -> f32 {
        self.tick as f32 / HZ as f32
    }

    #[must_use]
    pub fn run_card(&self) -> RunCard {
        RunCard {
            escaped: self.won.unwrap_or(self.status) == Status::Escaped,
            melted: self.man_melted,
            minions_melted: self.stats.minions_melted,
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
        self.options.man && !self.man_melted
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
        if !self.free_roam && p.distance(self.house.front.stand) <= self.tuning.reach {
            let prompt = if self.player.keys as usize >= self.house.keys.len() {
                Prompt::Unlock
            } else {
                Prompt::Locked
            };
            return Some((prompt, 0));
        }
        let mut best: Option<(Prompt, usize, f32)> = None;
        if self.weapons_on() && !self.solved {
            for (i, at) in self.house.buttons.iter().enumerate() {
                let d = p.distance(*at);
                if d <= self.tuning.reach && best.is_none_or(|b| d < b.2) {
                    best = Some((Prompt::Button, i, d));
                }
            }
            let d = p.distance(self.house.cabinet);
            if d <= self.tuning.reach && best.is_none_or(|b| d < b.2) {
                best = Some((Prompt::Cabinet, 0, d));
            }
        }
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
        // In free roam nobody gets scooped.
        let floor = if self.free_roam { 1.0 } else { 0.0 };
        self.player.health = (self.player.health - damage).max(floor);
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

        if let Some(tool) = input.select
            && self.has(tool)
        {
            self.player.tool = tool;
        }

        self.move_player(input, jumped);
        self.fire(input);
        self.interact(input, pressed);
        self.pick_up_keys();
        self.pick_up_drops();
        self.fly();
        if self.man_here() {
            self.step_man();
        }
        if self.options.man {
            self.step_minions();
            self.reap();
            self.spawn_minions();
        }
        self.clouds.iter_mut().for_each(|c| c.age += DT);
        self.clouds.retain(|c| c.age < self.tuning.cloud_life);

        if self.player.boost > 0.0 {
            self.player.boost -= DT;
            if self.player.boost <= 0.0 {
                self.player.boost = 0.0;
                self.player.health = self.player.health.min(self.tuning.max_health);
                self.events.push(Event::BoostOver);
            }
        }
        let max = self.max_health();
        let player = &mut self.player;
        player.since_hit += DT;
        if player.since_hit >= self.tuning.regen_after {
            player.health =
                (player.health + self.tuning.regen_rate * DT).min(max.max(player.health));
            player.health = player.health.min(max);
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
                if self.man_here() && self.man.pos.distance(at) <= self.tuning.fart_radius {
                    self.man.fart(at);
                    self.events.push(Event::FartHit);
                }
                let radius = self.tuning.fart_radius;
                let damage = self.tuning.fart_minion_damage;
                for i in 0..self.minions.len() {
                    let m = &mut self.minions[i];
                    if !m.friendly && m.pos.distance(at) <= radius {
                        let away = (m.pos - at).normalized();
                        m.pos = slide(&self.house.blocks, m.pos, away * 1.2, MINION_RADIUS, 0.0);
                        self.damage_minion(i, damage);
                    }
                }
            }
            Tool::Sword | Tool::Spear => {
                let t = &self.tuning;
                let (reach, wait, damage, cos) = if player.tool == Tool::Sword {
                    (t.sword_reach, t.sword_wait, t.sword_damage, 0.3)
                } else {
                    (t.spear_reach, t.spear_wait, t.spear_damage, 0.88)
                };
                player.fire_wait = wait;
                let from = player.pos;
                let tool = player.tool;
                self.events.push(Event::Swing(tool));
                self.melee(from, aim, reach, cos, damage);
            }
            Tool::Pistol if player.cones > 0 => {
                player.cones -= 1;
                player.fire_wait = 1.0 / self.tuning.pistol_rate;
                self.shots.push(Shot {
                    kind: ShotKind::Cone,
                    pos: player.pos + aim * 0.5,
                    height: 1.1,
                    vel: aim * self.tuning.pistol_speed,
                    left: self.tuning.pistol_range,
                });
                self.events.push(Event::ConeFired);
            }
            Tool::Sugar if player.sugar > 0 => {
                player.sugar -= 1;
                player.fire_wait = self.tuning.sugar_wait;
                self.shots.push(Shot {
                    kind: ShotKind::Sugar,
                    pos: player.pos + aim * 0.5,
                    height: 1.2,
                    vel: aim * self.tuning.sugar_speed,
                    left: self.tuning.sugar_range,
                });
                self.events.push(Event::SugarFired);
            }
            Tool::Staff => {
                player.fire_wait = self.tuning.staff_wait;
                let from = player.pos;
                self.staff(from, aim);
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
                if self.has(Tool::Pistol) {
                    self.player.cones =
                        (self.player.cones + self.tuning.pistol_pickup).min(self.tuning.pistol_max);
                }
                if self.has(Tool::Sugar) {
                    self.player.sugar =
                        (self.player.sugar + self.tuning.sugar_pickup).min(self.tuning.sugar_max);
                }
                self.noises
                    .push((self.player.pos, self.tuning.fridge_noise));
                self.events.push(Event::Pickup(pickup));
            }
            Prompt::Button if pressed => {
                if i as u32 == self.lit {
                    self.lit += 1;
                    self.events.push(Event::Button(self.lit));
                    if self.lit as usize >= BUTTONS {
                        self.solved = true;
                        self.drops.push(Drop {
                            at: self.house.cabinet,
                            kind: DropKind::Weapon(Tool::Spear),
                            age: 0.0,
                        });
                        self.events.push(Event::PuzzleSolved);
                    }
                } else if i as u32 > self.lit {
                    self.lit = 0;
                    self.events.push(Event::PuzzleWrong);
                }
            }
            Prompt::Cabinet if pressed => self.events.push(Event::CabinetLocked),
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
                    self.won = Some(Status::Escaped);
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
        let mut bursts = Vec::new();
        let mut shots = std::mem::take(&mut self.shots);
        shots.retain_mut(|shot| {
            let from = shot.pos;
            let to = from + shot.vel * DT;
            let travel = shot.vel.length() * DT;
            if let Some(t) = first_hit(&self.house.blocks, from, to, 0.0, shot.height) {
                let at = from + (to - from) * t;
                match shot.kind {
                    ShotKind::Fudge => self.events.push(Event::FudgeSplat(at)),
                    ShotKind::Sugar => bursts.push(from + (to - from) * (t * 0.9)),
                    _ => {}
                }
                return false;
            }
            let player_shot = !matches!(shot.kind, ShotKind::Sprinkle(_));
            if player_shot
                && let Some(i) = self.minions.iter().position(|m| {
                    !m.friendly
                        && m.health > 0.0
                        && segment_distance(from, to, m.pos) <= MINION_RADIUS + 0.25
                })
            {
                match shot.kind {
                    ShotKind::Fudge => {
                        self.stats.fudge_hits += 1;
                        self.damage_minion(i, self.tuning.fudge_minion_damage);
                    }
                    ShotKind::Cone => self.damage_minion(i, self.tuning.pistol_damage),
                    _ => bursts.push(self.minions[i].pos),
                }
                return false;
            }
            match shot.kind {
                ShotKind::Fudge => {
                    if self.man_here()
                        && segment_distance(from, to, self.man.pos) <= MAN_RADIUS + 0.25
                    {
                        self.stats.fudge_hits += 1;
                        self.events.push(Event::FudgeHit);
                        if self.man.fudge() {
                            self.events.push(Event::Frozen);
                            self.stats.freezes += 1;
                            if self.weapons_on() && self.stats.freezes >= self.tuning.pistol_task {
                                self.earn(Tool::Pistol);
                            }
                        }
                        return false;
                    }
                }
                ShotKind::Cone => {
                    if self.man_here()
                        && segment_distance(from, to, self.man.pos) <= MAN_RADIUS + 0.25
                    {
                        self.damage_man(self.tuning.pistol_damage);
                        return false;
                    }
                }
                ShotKind::Sugar => {
                    if self.man_here()
                        && segment_distance(from, to, self.man.pos) <= MAN_RADIUS + 0.3
                    {
                        bursts.push(self.man.pos);
                        return false;
                    }
                    if shot.left - travel <= 0.0 {
                        bursts.push(to);
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
        // Shots fired while these flew (none yet) stay too.
        shots.append(&mut self.shots);
        self.shots = shots;
        for at in bursts {
            self.burst(at);
        }
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

    /// Sugar bursts at `at`: it hurts him and every minion of his in it.
    fn burst(&mut self, at: V2) {
        self.events.push(Event::SugarBurst(at));
        let radius = self.tuning.sugar_radius;
        let damage = self.tuning.sugar_damage;
        if self.man_here() && self.man.pos.distance(at) <= radius + MAN_RADIUS {
            self.damage_man(damage);
        }
        for i in 0..self.minions.len() {
            if !self.minions[i].friendly
                && self.minions[i].pos.distance(at) <= radius + MINION_RADIUS
            {
                self.damage_minion(i, damage);
            }
        }
    }

    /// Whether nothing tall stands between two points.
    fn clear(&self, a: V2, b: V2) -> bool {
        first_hit(&self.house.blocks, a, b, 0.0, 1.0).is_none()
    }

    /// A swing from `from` toward `aim`: everything of his within `reach`
    /// and inside the arc (`cos`, the arc's half-width as a cosine).
    fn melee(&mut self, from: V2, aim: V2, reach: f32, cos: f32, damage: f32) {
        let inside = |at: V2, body: f32| {
            let to = at - from;
            let d = to.length();
            d <= reach + body && (d < body + 0.4 || aim.dot(to.normalized()) >= cos)
        };
        if self.man_here() && inside(self.man.pos, MAN_RADIUS) && self.clear(from, self.man.pos) {
            self.damage_man(damage);
        }
        for i in 0..self.minions.len() {
            let m = &self.minions[i];
            if !m.friendly && inside(m.pos, MINION_RADIUS) && self.clear(from, m.pos) {
                self.damage_minion(i, damage);
            }
        }
    }

    /// The sprinkle staff: one super long sprinkle from `from` along
    /// `aim` (stopping at a wall) hurts everything of his it touches, and
    /// your own minions pop out where it ends.
    fn staff(&mut self, from: V2, aim: V2) {
        let mut end = from + aim * self.tuning.staff_length;
        if let Some(t) = first_hit(&self.house.blocks, from, end, 0.0, 1.0) {
            end = from + (end - from) * t;
        }
        let back = (end - from).length().min(0.7);
        let end = end - aim * back;
        self.events.push(Event::Staff(from, end));
        let damage = self.tuning.staff_damage;
        if self.man_here() && segment_distance(from, end, self.man.pos) <= MAN_RADIUS + 0.4 {
            self.damage_man(damage);
        }
        for i in 0..self.minions.len() {
            let m = &self.minions[i];
            if !m.friendly && segment_distance(from, end, m.pos) <= MINION_RADIUS + 0.4 {
                self.damage_minion(i, damage);
            }
        }
        let side = v(-aim.z, aim.x);
        let n = self.tuning.staff_minions;
        for k in 0..n {
            let spread = (k as f32 - (n as f32 - 1.0) / 2.0) * 0.8;
            let mut at = end + side * spread;
            if !self.house.open(at, MINION_RADIUS) || !self.clear(end, at) {
                at = end;
            }
            if !self.house.open(at, MINION_RADIUS) {
                at = from;
            }
            self.minions.push(Minion::new(
                at,
                self.tuning.minion_health,
                true,
                self.tuning.staff_minion_life,
            ));
        }
        let friends = self.minions.iter().filter(|m| m.friendly).count();
        if friends > MAX_FRIENDS {
            let mut extra = friends - MAX_FRIENDS;
            self.minions.retain(|m| {
                if m.friendly && extra > 0 {
                    extra -= 1;
                    return false;
                }
                true
            });
        }
        self.events.push(Event::FriendsCame(n));
    }

    /// He takes `damage` from an earned weapon or your minions; at none
    /// he melts, and that's a win.
    fn damage_man(&mut self, damage: f32) {
        if !self.man_here() || damage <= 0.0 {
            return;
        }
        self.events.push(Event::ManHit);
        self.stats.man_damage += damage.round() as u32;
        if self.man.hurt(damage) {
            self.man_melted = true;
            self.events.push(Event::Melted);
            if self.status == Status::Playing && !self.free_roam {
                self.status = Status::Melted;
                self.won = Some(Status::Melted);
            }
        }
    }

    fn damage_minion(&mut self, i: usize, damage: f32) {
        let Some(m) = self.minions.get_mut(i) else {
            return;
        };
        if m.health <= 0.0 || damage <= 0.0 {
            return;
        }
        m.health -= damage;
        m.flash = 0.25;
        let at = m.pos;
        self.events.push(Event::MinionHit(at));
    }

    /// Walking over a cone eats it; over a weapon, picks it up.
    fn pick_up_drops(&mut self) {
        for d in &mut self.drops {
            d.age += DT;
        }
        self.drops
            .retain(|d| d.kind != DropKind::Cone || d.age < CONE_LIFE);
        if self.player.hidden.is_some() {
            return;
        }
        let reach = self.tuning.key_reach + 0.2;
        let mut taken = Vec::new();
        for (i, d) in self.drops.iter().enumerate() {
            if d.at.distance(self.player.pos) <= reach {
                taken.push(i);
            }
        }
        for i in taken.into_iter().rev() {
            let d = self.drops.remove(i);
            match d.kind {
                DropKind::Cone => {
                    let base = self.tuning.max_health;
                    self.player.boost = self.tuning.cone_time;
                    self.player.health =
                        (self.player.health + base * self.tuning.cone_boost).min(self.max_health());
                    self.stats.cones_eaten += 1;
                    self.events.push(Event::ConeEaten);
                }
                DropKind::Weapon(tool) => self.earn(tool),
            }
        }
    }

    /// Each minion picks what it goes after and goes: his hunt you when
    /// you're near and not hiding; yours hunt his minions, then him, and
    /// otherwise stay by you. A bite lands when one is close enough.
    fn step_minions(&mut self) {
        let t = self.tuning;
        let player = self.player.pos;
        let hidden = self.player.hidden.is_some();
        let man = self.man_here().then_some(self.man.pos);
        for i in 0..self.minions.len() {
            self.minions[i].tick();
            if self.minions[i].health <= 0.0 {
                continue;
            }
            let me = self.minions[i].pos;
            if self.minions[i].friendly {
                let foe = self
                    .minions
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| !m.friendly && m.health > 0.0 && m.pos.distance(me) < 20.0)
                    .min_by(|a, b| a.1.pos.distance(me).total_cmp(&b.1.pos.distance(me)))
                    .map(|(j, m)| (j, m.pos));
                let speed = t.minion_speed * 1.25;
                if let Some((j, at)) = foe {
                    let house = &self.house;
                    self.minions[i].go(house, at, speed, MINION_RADIUS * 2.0 + 0.1);
                    if self.minions[i].pos.distance(at) <= MINION_RADIUS * 2.0 + 0.35
                        && self.minions[i].bite_wait <= 0.0
                    {
                        self.minions[i].bite_wait = t.minion_bite_wait;
                        self.damage_minion(j, t.minion_damage);
                    }
                } else if let Some(at) = man.filter(|at| at.distance(me) < 25.0) {
                    let house = &self.house;
                    self.minions[i].go(house, at, speed, MAN_RADIUS + MINION_RADIUS + 0.1);
                    if self.minions[i].pos.distance(at) <= MAN_RADIUS + MINION_RADIUS + 0.4
                        && self.minions[i].bite_wait <= 0.0
                    {
                        self.minions[i].bite_wait = t.minion_bite_wait;
                        self.damage_man(t.minion_damage);
                    }
                } else {
                    let house = &self.house;
                    self.minions[i].go(house, player, speed, 2.0);
                }
            } else {
                let near = me.distance(player) <= t.minion_sight;
                if near && !hidden {
                    let house = &self.house;
                    self.minions[i].go(
                        house,
                        player,
                        t.minion_speed,
                        MINION_RADIUS + PLAYER_RADIUS,
                    );
                    if self.minions[i].pos.distance(player) <= MINION_RADIUS + PLAYER_RADIUS + 0.3
                        && self.minions[i].bite_wait <= 0.0
                    {
                        self.minions[i].bite_wait = t.minion_bite_wait;
                        self.events.push(Event::MinionBite);
                        self.hurt(t.minion_damage);
                    }
                } else {
                    let (house, rng) = (&self.house, &mut self.rng);
                    self.minions[i].wander(house, rng, t.minion_speed * 0.6);
                }
            }
        }
    }

    /// Melted minions go, dropping what they drop and counting toward
    /// the sword and the sugar launcher; your own go when their time is up.
    fn reap(&mut self) {
        let mut melted = Vec::new();
        self.minions.retain(|m| {
            if m.health <= 0.0 {
                if !m.friendly {
                    melted.push(m.pos);
                }
                return false;
            }
            !(m.friendly && m.life <= 0.0)
        });
        for at in melted {
            self.stats.minions_melted += 1;
            self.events.push(Event::MinionMelted(at));
            let weapons = self.weapons_on();
            if weapons
                && !self.sword_dropped
                && !self.has(Tool::Sword)
                && self.stats.minions_melted >= self.tuning.sword_task
            {
                self.sword_dropped = true;
                self.drops.push(Drop {
                    at,
                    kind: DropKind::Weapon(Tool::Sword),
                    age: 0.0,
                });
                self.events.push(Event::WeaponDropped(Tool::Sword, at));
            } else if self.rng.unit() < self.tuning.cone_drop {
                self.drops.push(Drop {
                    at,
                    kind: DropKind::Cone,
                    age: 0.0,
                });
                self.events.push(Event::ConeDropped(at));
            }
            if weapons && self.stats.minions_melted >= self.tuning.sugar_task {
                self.earn(Tool::Sugar);
            }
        }
    }

    /// A new minion now and then, while there are fewer than the version
    /// allows: out of the Ice Cream Man when he's away from you, else in a
    /// room two or more doorways from you.
    fn spawn_minions(&mut self) {
        if self.tuning.minion_count == 0 {
            return;
        }
        self.minion_wait -= DT;
        if self.minion_wait > 0.0 {
            return;
        }
        self.minion_wait = self.tuning.minion_every;
        let enemies = self.minions.iter().filter(|m| !m.friendly).count();
        if enemies >= self.tuning.minion_count as usize {
            return;
        }
        let player = self.player.pos;
        let at = if self.man_here() && self.man.pos.distance(player) >= 8.0 {
            self.man.pos
        } else {
            let steps = self.house.distances(cell_of(player));
            let far: Vec<usize> = (0..self.house.rooms.len())
                .filter(|i| steps[*i] >= 2 && steps[*i] != usize::MAX)
                .collect();
            if far.is_empty() {
                return;
            }
            let room = far[self.rng.below(far.len())];
            centre(self.house.rooms[room].cell)
        };
        self.minions
            .push(Minion::new(at, self.tuning.minion_health, false, 0.0));
        self.events.push(Event::MinionCame(at));
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
        eat(self.minions.len() as u64 | (self.drops.len() as u64) << 16);
        for m in &self.minions {
            eat(u64::from(m.pos.x.to_bits()) << 32 | u64::from(m.pos.z.to_bits()));
        }
        eat(u64::from(self.man.health.to_bits()));
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
        // The Airbnb's bedroom, whose north wall has no doorway.
        let mut game = Game::tuned(1, Options { man: false }, Tuning::original());
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

    // ---- the designer's revision (#11626) ---------------------------------

    /// The revision's numbers: the earned weapons, minions and free roam.
    fn revision() -> Tuning {
        let mut t = Tuning::default();
        t.earned_weapons = 1;
        t.minion_count = 3;
        t.free_roam = 1;
        t
    }

    fn revised(seed: u64) -> Game {
        Game::tuned(seed, Options::default(), revision())
    }

    /// A minion of his right here, `health` left.
    fn minion_at(game: &mut Game, at: V2, health: f32) -> usize {
        game.minions.push(Minion::new(at, health, false, 0.0));
        game.minions.len() - 1
    }

    #[test]
    fn the_original_game_has_none_of_the_new_things() {
        let mut game = Game::tuned(7, Options::default(), Tuning::original());
        assert!(!game.weapons_on());
        assert_eq!(game.player.owned, Tool::START.to_vec());
        for _ in 0..30 * 120 {
            game.step(Input::default());
        }
        assert!(
            game.minions.is_empty(),
            "no minions unless the version has them"
        );
        game.step(Input {
            select: Some(Tool::Spear),
            ..Input::default()
        });
        assert_eq!(
            game.player.tool,
            Tool::Fudge,
            "a tool you don't have can't be picked"
        );
    }

    #[test]
    fn minions_come_hunt_and_bite() {
        let mut game = revised(3);
        game.man.pos = v(-100.0, -100.0);
        game.options.man = true;
        let mut came = false;
        for _ in 0..30 * (MINION_FIRST as usize + 2) {
            game.step(Input::default());
            came |= game
                .events
                .iter()
                .any(|e| matches!(e, Event::MinionCame(_)));
        }
        assert!(came && !game.minions.is_empty());
        // One right beside you bites.
        let mut game = revised(3);
        let at = game.player.pos + v(0.5, 0.0);
        minion_at(&mut game, at, MINION_HEALTH);
        let mut bitten = false;
        for _ in 0..30 {
            game.step(Input::default());
            bitten |= game.events.contains(&Event::MinionBite);
        }
        assert!(bitten);
        assert!(game.player.health <= MAX_HEALTH - MINION_DAMAGE);
    }

    #[test]
    fn the_first_melted_minion_drops_the_candy_cane_sword() {
        let mut game = revised(4);
        let at = game.player.pos + v(0.0, -2.5);
        let i = minion_at(&mut game, at, 1.0);
        game.damage_minion(i, 5.0);
        game.reap();
        assert!(game.events.contains(&Event::WeaponDropped(Tool::Sword, at)));
        assert!(
            !game.has(Tool::Sword),
            "it's on the floor until you pick it up"
        );
        walk_to(&mut game, at);
        assert!(game.has(Tool::Sword), "{:?}", game.drops);
        assert_eq!(game.player.tool, Tool::Sword);
        // A swing melts a minion in reach in front of you.
        let target = game.player.pos + v(1.0, 0.0);
        minion_at(&mut game, target, SWORD_DAMAGE);
        game.step(Input {
            fire: true,
            aim: v(1.0, 0.0),
            ..Input::default()
        });
        assert!(game.events.contains(&Event::Swing(Tool::Sword)));
        assert_eq!(game.stats.minions_melted, 2);
    }

    #[test]
    fn the_button_puzzle_opens_the_cabinet_with_the_waffle_cone_spear() {
        let mut game = Game::tuned(6, Options { man: false }, revision());
        let buttons = game.house.buttons.clone();
        assert_eq!(buttons.len(), crate::layout::BUTTONS);
        let press = |game: &mut Game| {
            game.step(Input::default());
            game.step(Input {
                interact: true,
                ..Input::default()
            });
        };
        // The cabinet is locked until they're pressed.
        let cabinet = game.house.cabinet;
        travel(&mut game, cabinet);
        assert_eq!(game.prompt(), Some(Prompt::Cabinet));
        press(&mut game);
        assert!(game.events.contains(&Event::CabinetLocked));
        // Out of order: they all go dark.
        travel(&mut game, buttons[0]);
        press(&mut game);
        assert_eq!(game.lit, 1);
        travel(&mut game, buttons[2]);
        press(&mut game);
        assert!(game.events.contains(&Event::PuzzleWrong));
        assert_eq!(game.lit, 0);
        for (n, at) in buttons.iter().enumerate() {
            travel(&mut game, *at);
            assert_eq!(game.prompt(), Some(Prompt::Button));
            press(&mut game);
            assert_eq!(game.lit, n as u32 + 1);
        }
        assert!(game.solved && game.events.contains(&Event::PuzzleSolved));
        let cabinet = game.house.cabinet;
        travel(&mut game, cabinet);
        assert!(game.has(Tool::Spear));
        // The spear reaches farther than the sword, in a narrow line.
        let far = game.player.pos + v(SPEAR_REACH - 0.2, 0.0);
        if game.clear(game.player.pos, far) {
            game.minions.push(Minion::new(far, 1.0, false, 0.0));
            game.step(Input {
                fire: true,
                aim: v(1.0, 0.0),
                ..Input::default()
            });
            assert!(game.events.contains(&Event::Swing(Tool::Spear)));
            assert!(game.minions[0].health <= 0.0, "the spear reaches it");
        }
    }

    #[test]
    fn freezing_him_earns_the_cone_pistol_and_it_fires_cones() {
        let mut game = revised(3);
        game.man.fudge_hits = FREEZE_HITS - 1;
        let at = game.man.pos + v(0.0, 3.0);
        game.player.pos = at;
        game.man.state = ManState::Wander;
        for _ in 0..30 {
            game.man.pos = at + v(0.0, -3.0);
            game.step(Input {
                fire: true,
                aim: v(0.0, -1.0),
                ..Input::default()
            });
            if game.has(Tool::Pistol) {
                break;
            }
        }
        assert!(game.has(Tool::Pistol), "{:?}", game.stats);
        assert_eq!(game.player.cones, PISTOL_AMMO);
        assert_eq!(game.player.tool, Tool::Pistol);
        let health = game.man.health;
        for _ in 0..20 {
            game.man.pos = at + v(0.0, -3.0);
            game.step(Input {
                fire: true,
                aim: v(0.0, -1.0),
                ..Input::default()
            });
        }
        assert!(game.player.cones < PISTOL_AMMO);
        assert!(game.man.health < health, "cones hurt him");
    }

    #[test]
    fn melting_minions_earns_the_sugar_launcher_and_sugar_bursts() {
        let mut game = revised(5);
        game.sword_dropped = true;
        for k in 0..SUGAR_TASK {
            let at = game.player.pos + v(0.0, -2.0 - k as f32 * 0.1);
            let i = minion_at(&mut game, at, 1.0);
            game.damage_minion(i, 5.0);
            game.reap();
        }
        assert!(game.has(Tool::Sugar));
        assert_eq!(game.player.sugar, SUGAR_AMMO);
        // A burst hurts every minion in it.
        let centre = game.player.pos + v(0.0, -2.5);
        for dx in [-0.6, 0.6] {
            minion_at(&mut game, centre + v(dx, 0.0), SUGAR_DAMAGE);
        }
        let before = game.stats.minions_melted;
        game.burst(centre);
        game.reap();
        assert_eq!(game.stats.minions_melted, before + 2);
    }

    #[test]
    fn an_eaten_cone_gives_half_again_your_health_for_ten_seconds() {
        let mut game = Game::tuned(4, Options { man: false }, revision());
        game.player.health = MAX_HEALTH;
        let at = game.player.pos + v(0.0, -1.0);
        game.drops.push(Drop {
            at,
            kind: DropKind::Cone,
            age: 0.0,
        });
        walk_to(&mut game, at);
        assert_eq!(game.stats.cones_eaten, 1);
        assert_eq!(game.player.health, MAX_HEALTH * 1.5, "50% more");
        assert_eq!(game.max_health(), MAX_HEALTH * 1.5);
        // It lasts ten seconds, then health comes back down to full.
        let left = game.player.boost;
        assert!(left > CONE_TIME - 1.0 && left <= CONE_TIME);
        let steps = (left * 30.0).floor() as usize - 1;
        for _ in 0..steps {
            game.step(Input::default());
        }
        assert!(game.player.boost > 0.0);
        assert_eq!(game.player.health, MAX_HEALTH * 1.5);
        let mut over = false;
        for _ in 0..3 {
            game.step(Input::default());
            over |= game.events.contains(&Event::BoostOver);
        }
        assert!(over);
        assert_eq!(game.player.health, MAX_HEALTH);
        assert_eq!(game.max_health(), MAX_HEALTH);
    }

    #[test]
    fn melting_him_wins_and_free_roam_gives_the_sprinkle_staff() {
        let mut game = revised(3);
        game.arm();
        assert!(Tool::EARNED.iter().all(|t| game.has(*t)));
        game.damage_man(MAN_HEALTH - 1.0);
        assert_eq!(game.status, Status::Playing);
        game.damage_man(5.0);
        assert_eq!(game.status, Status::Melted);
        assert!(game.events.contains(&Event::Melted));
        assert!(game.run_card().melted && !game.run_card().escaped);
        assert!(!game.man_here());
        // Free roam: the staff, no scooping, no front door.
        assert!(game.keep_playing());
        assert!(game.free_roam && game.status == Status::Playing);
        assert!(game.has(Tool::Staff) && game.player.tool == Tool::Staff);
        assert!(game.events.contains(&Event::FreeRoam));
        game.hurt(10_000.0);
        assert_eq!(game.status, Status::Playing);
        assert_eq!(game.player.health, 1.0);
        game.player.pos = game.house.front.stand;
        assert_ne!(game.prompt(), Some(Prompt::Unlock));
        assert_eq!(game.won, Some(Status::Melted));
        // Without free roam in the version, a win ends the run.
        let mut plain = Game::tuned(3, Options::default(), Tuning::original());
        plain.status = Status::Escaped;
        assert!(!plain.keep_playing());
        assert_eq!(plain.status, Status::Escaped);
    }

    #[test]
    fn escaping_then_free_roam_keeps_him_in_the_house() {
        let mut game = Game::tuned(8, Options::default(), revision());
        game.status = Status::Escaped;
        game.won = Some(Status::Escaped);
        assert!(game.keep_playing());
        assert!(game.man_here());
        assert!(game.run_card().escaped);
        game.step(Input::default());
        assert_eq!(game.status, Status::Playing);
    }

    #[test]
    fn the_sprinkle_staff_makes_your_own_minions_and_they_fight_for_you() {
        let mut game = revised(3);
        game.status = Status::Escaped;
        assert!(game.keep_playing());
        let foe = game.player.pos + v(0.0, -3.0);
        minion_at(&mut game, foe, MINION_HEALTH);
        game.man.pos = v(-100.0, -100.0);
        game.step(Input {
            fire: true,
            aim: v(0.0, -1.0),
            ..Input::default()
        });
        let friends = game.minions.iter().filter(|m| m.friendly).count();
        assert_eq!(friends, STAFF_MINIONS as usize);
        assert!(game.events.contains(&Event::FriendsCame(STAFF_MINIONS)));
        assert!(game.events.iter().any(|e| matches!(e, Event::Staff(_, _))));
        // The long sprinkle hit the minion in its way.
        assert!(game.events.iter().any(|e| matches!(e, Event::MinionHit(_))));
        // Your minions finish it off, then pop when their time is up.
        for _ in 0..30 * 10 {
            game.step(Input::default());
        }
        assert!(game.stats.minions_melted >= 1, "{:?}", game.minions);
        for _ in 0..(STAFF_MINION_LIFE * 30.0) as usize {
            game.step(Input::default());
        }
        assert!(game.minions.iter().all(|m| !m.friendly || m.life > 0.0));
        assert!(game.minions.iter().filter(|m| m.friendly).count() < friends);
    }

    #[test]
    fn a_revised_run_is_the_same_every_time() {
        let run = |seed: u64| {
            let mut game = Game::tuned(seed, Options::default(), revision());
            let mut bot = crate::bot::Bot::default();
            for _ in 0..30 * 120 {
                let input = bot.input(&game);
                game.step(input);
                if game.status != Status::Playing {
                    break;
                }
            }
            (game.fingerprint(), game.stats)
        };
        let (a, stats) = run(11);
        assert_eq!(run(11).0, a);
        assert_ne!(run(12).0, a);
        assert!(
            stats.minions_melted > 0 || stats.hits_taken > 0,
            "{stats:?}"
        );
    }
}
