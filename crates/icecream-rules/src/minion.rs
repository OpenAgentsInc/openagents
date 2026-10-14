//! Minions: mini Ice Cream Men that hunt the player, and the friendly
//! ones the sprinkle staff makes, which hunt them and him. A minion only
//! moves; the game ([`crate::game`]) picks what it goes after and carries
//! out its bites.

use crate::DT;
use crate::geom::{V2, first_hit, slide, v};
use crate::layout::{House, cell_of, centre};
use crate::rng::Rng;
use crate::tuning::MINION_RADIUS;

#[derive(Clone, Debug, PartialEq)]
pub struct Minion {
    pub pos: V2,
    pub facing: V2,
    pub health: f32,
    /// One of the player's own, from the sprinkle staff.
    pub friendly: bool,
    /// Seconds left before a friendly minion pops (enemies don't).
    pub life: f32,
    /// Whether it moved this step (for its bob).
    pub moving: bool,
    /// Seconds since it was hurt, counting down from a short flash.
    pub flash: f32,
    /// Seconds until it can bite again.
    pub bite_wait: f32,
    path: Vec<V2>,
    goal: Option<V2>,
    replan: f32,
}

impl Minion {
    #[must_use]
    pub fn new(at: V2, health: f32, friendly: bool, life: f32) -> Self {
        Self {
            pos: at,
            facing: v(0.0, 1.0),
            health,
            friendly,
            life,
            moving: false,
            flash: 0.0,
            bite_wait: 0.5,
            path: Vec::new(),
            goal: None,
            replan: 0.0,
        }
    }

    /// The timers, once a step.
    pub fn tick(&mut self) {
        self.moving = false;
        self.flash = (self.flash - DT).max(0.0);
        self.bite_wait = (self.bite_wait - DT).max(0.0);
        self.replan = (self.replan - DT).max(0.0);
        if self.friendly {
            self.life -= DT;
        }
    }

    /// Goes toward `goal`: straight when nothing is in the way, else by
    /// the house's doorways, planning again now and then. Stops `near`
    /// short of it.
    pub fn go(&mut self, house: &House, goal: V2, speed: f32, near: f32) {
        if self.pos.distance(goal) <= near {
            self.facing = (goal - self.pos).normalized();
            return;
        }
        let straight = first_hit(&house.blocks, self.pos, goal, MINION_RADIUS * 0.9, 0.0).is_none();
        if straight {
            self.path.clear();
            self.goal = Some(goal);
            self.step_toward(house, goal, speed);
            return;
        }
        let moved = self.goal.is_none_or(|g| g.distance(goal) > 1.5);
        if self.path.is_empty() || self.replan <= 0.0 || moved {
            self.path = house.route(self.pos, goal, MINION_RADIUS);
            self.goal = Some(goal);
            self.replan = 1.0;
        }
        while let Some(&next) = self.path.first() {
            if self.pos.distance(next) < 0.3 {
                self.path.remove(0);
            } else {
                break;
            }
        }
        if let Some(&next) = self.path.first() {
            self.step_toward(house, next, speed);
        }
    }

    /// Wanders to a neighbouring room's middle, a new one each time it
    /// gets there.
    pub fn wander(&mut self, house: &House, rng: &mut Rng, speed: f32) {
        let arrived = self.goal.is_none_or(|g| g.distance(self.pos) < 0.5);
        if arrived {
            let here = cell_of(self.pos);
            let next = house.neighbours(here);
            let pick = if next.is_empty() {
                here
            } else {
                next[rng.below(next.len())]
            };
            self.goal = Some(centre(pick));
            self.path = house.route(self.pos, centre(pick), MINION_RADIUS);
        }
        if let Some(goal) = self.goal {
            self.go(house, goal, speed, 0.4);
        }
    }

    fn step_toward(&mut self, house: &House, toward: V2, speed: f32) {
        let to = toward - self.pos;
        let dir = to.normalized();
        if dir == V2::ZERO {
            return;
        }
        let step = (speed * DT).min(to.length());
        self.pos = slide(&house.blocks, self.pos, dir * step, MINION_RADIUS, 0.0);
        self.facing = dir;
        self.moving = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minion_finds_its_way_through_the_doorways() {
        let house = House::new(3);
        let mut minion = Minion::new(house.man_start, 30.0, false, 0.0);
        let goal = house.spawn;
        for _ in 0..30 * 60 {
            minion.tick();
            minion.go(&house, goal, 4.0, 0.5);
        }
        assert!(minion.pos.distance(goal) < 1.0, "{:?}", minion.pos);
    }

    #[test]
    fn a_friendly_minion_runs_out_of_time() {
        let mut minion = Minion::new(V2::ZERO, 30.0, true, 1.0);
        for _ in 0..31 {
            minion.tick();
        }
        assert!(minion.life <= 0.0);
        let mut enemy = Minion::new(V2::ZERO, 30.0, false, 0.0);
        enemy.tick();
        assert_eq!(enemy.life, 0.0);
    }
}
