//! `bunny.garden.v1` level files and the gardens that ship with the game.
//!
//! A level file is plain text, one statement a line; `#` starts a comment.
//! Junctions sit on a square grid and are named by column and row, `c,r`.
//!
//! ```text
//! bunny.garden.v1
//! name Kitchen Bed
//! number 1                      # place in the level list
//! set 1                         # garden set, for the bonus vegetable
//! grid 20                       # metres between neighbouring junctions
//! spawn 0,0 1,0 at 3            # in that corridor, 3 m from 0,0, running to 1,0
//! shed 3,2
//! patrol 3,2 2,2 2,1 3,1
//! farmer walk 3.0 run 5.0 windup 0.5 chase 20 scatter 10 spook 8
//! farmer ambush                 # optional: heads for the junction ahead
//! par 120                       # seconds, for the clear bonus
//! bonus 1,1 2,1 at 10 C         # where the bonus vegetable appears
//! corridor 0,0 1,0              # a corridor; the lines after it fill it
//!   seedling C 3..17/2          # lane L, C or R (right facing 0,0 to 1,0)
//!   carrot L 10, 14             # several distances
//!   radish^ R 10                # ^: hangs in the air, eaten in a jump
//!   pot R 10
//! ```
//!
//! Edibles: seedling, carrot, radish, lettuce, strawberry, pumpkin, golden.
//! Obstacles: fence, gap, tunnel, hose, puddle, tray, pot, can, gnome,
//! wire, barrow, scarecrow, birdnet. Power-ups: clover, dandelion, sunhat,
//! magnet.

use sha2::{Digest, Sha256};

use crate::garden::{Edge, Edible, FarmerPlan, Garden, Node, Obstacle, Power, Spot};
use crate::kinds::{EdibleKind, ObstacleKind, PowerKind};
use crate::{UNIT, per_tick, ticks};

/// The format's first line.
pub const FORMAT: &str = "bunny.garden.v1";

/// The gardens that ship with the game, in level order.
pub const GARDENS: [&str; 5] = [
    include_str!("../gardens/01-kitchen-bed.garden"),
    include_str!("../gardens/02-herb-corner.garden"),
    include_str!("../gardens/03-potting-row.garden"),
    include_str!("../gardens/04-pea-trellis.garden"),
    include_str!("../gardens/05-kitchen-garden.garden"),
];

/// How many gardens ship.
pub const COUNT: usize = GARDENS.len();

/// Garden `number` (1-based) of the ones that ship.
///
/// # Panics
///
/// When `number` is out of range or the shipped file doesn't parse; tests
/// check every shipped file.
#[must_use]
pub fn garden(number: usize) -> Garden {
    parse(GARDENS[number - 1]).expect("a shipped garden parses")
}

/// SHA-256 of a level file.
#[must_use]
pub fn digest(text: &str) -> [u8; 32] {
    Sha256::digest(text.as_bytes()).into()
}

fn metres(word: &str) -> Result<i32, String> {
    let value: f64 = word.parse().map_err(|_| format!("not a number: {word}"))?;
    if !value.is_finite() || !(0.0..=1000.0).contains(&value) {
        return Err(format!("out of range: {word}"));
    }
    Ok((value * f64::from(UNIT)).round() as i32)
}

fn seconds(word: &str) -> Result<u32, String> {
    let value: f64 = word.parse().map_err(|_| format!("not a number: {word}"))?;
    if !value.is_finite() || !(0.0..=600.0).contains(&value) {
        return Err(format!("out of range: {word}"));
    }
    Ok(ticks((value * 1000.0).round() as u32))
}

fn speed(word: &str) -> Result<i32, String> {
    let value: f64 = word.parse().map_err(|_| format!("not a number: {word}"))?;
    if !value.is_finite() || !(0.5..=20.0).contains(&value) {
        return Err(format!("out of range: {word}"));
    }
    Ok(per_tick((value * 1000.0).round() as i32))
}

fn grid_point(word: &str) -> Result<(i32, i32), String> {
    let (c, r) = word
        .split_once(',')
        .ok_or_else(|| format!("not a junction: {word}"))?;
    let c: i32 = c.parse().map_err(|_| format!("not a junction: {word}"))?;
    let r: i32 = r.parse().map_err(|_| format!("not a junction: {word}"))?;
    if !(0..=40).contains(&c) || !(0..=40).contains(&r) {
        return Err(format!("junction out of range: {word}"));
    }
    Ok((c, r))
}

fn lane(word: &str) -> Result<i8, String> {
    match word {
        "L" => Ok(-1),
        "C" => Ok(0),
        "R" => Ok(1),
        _ => Err(format!("not a lane: {word}")),
    }
}

/// `3..17/2, 19` to distances in units.
fn distances(words: &[&str]) -> Result<Vec<i32>, String> {
    let joined = words.join(" ");
    let mut out = Vec::new();
    for part in joined.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((from, rest)) = part.split_once("..") {
            let (to, step) = rest.split_once('/').unwrap_or((rest, "1"));
            let (from, to, step) = (metres(from)?, metres(to)?, metres(step)?);
            if step <= 0 || to < from {
                return Err(format!("bad range: {part}"));
            }
            let mut s = from;
            while s <= to {
                out.push(s);
                s += step;
                if out.len() > 4096 {
                    return Err("too many".into());
                }
            }
        } else {
            out.push(metres(part)?);
        }
    }
    Ok(out)
}

enum Item {
    Edible(EdibleKind, bool),
    Obstacle(ObstacleKind),
    Power(PowerKind),
}

fn item(word: &str) -> Option<Item> {
    let (word, air) = match word.strip_suffix('^') {
        Some(word) => (word, true),
        None => (word, false),
    };
    if let Some(kind) = EdibleKind::ALL
        .into_iter()
        .find(|k| *k != EdibleKind::Bonus && k.word() == word)
    {
        return Some(Item::Edible(kind, air));
    }
    if air {
        return None;
    }
    if let Some(kind) = ObstacleKind::ALL.into_iter().find(|k| k.word() == word) {
        return Some(Item::Obstacle(kind));
    }
    PowerKind::ALL
        .into_iter()
        .find(|k| k.word() == word)
        .map(Item::Power)
}

struct Parser {
    spacing: i32,
    grid: Vec<(i32, i32)>,
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Parser {
    fn node(&mut self, at: (i32, i32)) -> usize {
        if let Some(index) = self.grid.iter().position(|g| *g == at) {
            return index;
        }
        self.grid.push(at);
        self.nodes.push(Node {
            x: at.0 * self.spacing,
            z: at.1 * self.spacing,
        });
        self.nodes.len() - 1
    }

    fn find(&self, at: (i32, i32)) -> Result<usize, String> {
        self.grid
            .iter()
            .position(|g| *g == at)
            .ok_or_else(|| format!("no corridor reaches junction {},{}", at.0, at.1))
    }

    fn edge(&self, from: (i32, i32), to: (i32, i32)) -> Result<(usize, bool), String> {
        let (a, b) = (self.find(from)?, self.find(to)?);
        self.edges
            .iter()
            .enumerate()
            .find_map(|(index, e)| {
                if (e.a, e.b) == (a, b) {
                    Some((index, true))
                } else if (e.a, e.b) == (b, a) {
                    Some((index, false))
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                format!(
                    "no corridor between {},{} and {},{}",
                    from.0, from.1, to.0, to.1
                )
            })
    }
}

/// Reads a `bunny.garden.v1` level file.
pub fn parse(text: &str) -> Result<Garden, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(n, line)| (n + 1, line.split('#').next().unwrap_or("").trim()))
        .filter(|(_, line)| !line.is_empty());
    match lines.next() {
        Some((_, FORMAT)) => {}
        _ => return Err(format!("not a {FORMAT} file")),
    }
    let statements: Vec<(usize, Vec<&str>)> = lines
        .map(|(n, line)| (n, line.split_whitespace().collect()))
        .collect();
    let at = |n: usize, error: String| format!("line {n}: {error}");
    // Corridors first, so every other statement can name them.
    let mut spacing = None;
    for (n, words) in &statements {
        if words[0] == "grid" {
            let value = words
                .get(1)
                .ok_or_else(|| at(*n, "grid needs metres".into()))?;
            spacing = Some(metres(value).map_err(|e| at(*n, e))?);
        }
    }
    let spacing = spacing.ok_or("no grid")?;
    if spacing < 6 * UNIT {
        return Err("the grid is too tight".into());
    }
    let mut p = Parser {
        spacing,
        grid: Vec::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
    };
    for (n, words) in &statements {
        if words[0] != "corridor" {
            continue;
        }
        let [_, from, to] = words.as_slice() else {
            return Err(at(*n, "corridor needs two junctions".into()));
        };
        let (from, to) = (
            grid_point(from).map_err(|e| at(*n, e))?,
            grid_point(to).map_err(|e| at(*n, e))?,
        );
        if (from.0 != to.0) == (from.1 != to.1) {
            return Err(at(
                *n,
                "a corridor runs straight along a row or column".into(),
            ));
        }
        let (a, b) = (p.node(from), p.node(to));
        if p.edges
            .iter()
            .any(|e| (e.a, e.b) == (a, b) || (e.a, e.b) == (b, a))
        {
            return Err(at(*n, "the corridor is listed twice".into()));
        }
        let (pa, pb) = (p.nodes[a], p.nodes[b]);
        let (dx, dz) = (pb.x - pa.x, pb.z - pa.z);
        p.edges.push(Edge {
            a,
            b,
            len: dx.abs() + dz.abs(),
            dx: dx.signum(),
            dz: dz.signum(),
        });
    }
    if p.edges.is_empty() {
        return Err("no corridors".into());
    }
    let mut name = None;
    let mut number = 0_u8;
    let mut set = 1_u8;
    let mut spawn = None;
    let mut shed = None;
    let mut patrol = Vec::new();
    let mut farmer = FarmerPlan {
        walk: per_tick(3_000),
        run: per_tick(5_000),
        windup: ticks(500),
        chase: ticks(20_000),
        scatter: ticks(7_000),
        ambush: false,
        spook: ticks(8_000),
    };
    let mut par = 120;
    let mut bonus = None;
    let mut edibles = Vec::new();
    let mut obstacles = Vec::new();
    let mut powers = Vec::new();
    let mut current: Option<(usize, bool)> = None;
    for (n, words) in &statements {
        let n = *n;
        let rest = &words[1..];
        match words[0] {
            "grid" => {}
            "name" => name = Some(rest.join(" ")),
            "number" | "set" => {
                let value: u8 = rest
                    .first()
                    .and_then(|w| w.parse().ok())
                    .filter(|v| (1..=20).contains(v))
                    .ok_or_else(|| at(n, format!("{} needs 1 to 20", words[0])))?;
                if words[0] == "number" {
                    number = value;
                } else {
                    set = value;
                }
            }
            "spawn" | "bonus" => {
                let (from, to, s, lane_word) = match rest {
                    [from, to, "at", s] => (from, to, s, None),
                    [from, to, "at", s, lane_word] => (from, to, s, Some(lane_word)),
                    _ => return Err(at(n, format!("{} A B at METRES", words[0]))),
                };
                let (from, to) = (
                    grid_point(from).map_err(|e| at(n, e))?,
                    grid_point(to).map_err(|e| at(n, e))?,
                );
                let (edge, fwd) = p.edge(from, to).map_err(|e| at(n, e))?;
                let s = metres(s).map_err(|e| at(n, e))?;
                let s = if fwd { s } else { p.edges[edge].len - s };
                if words[0] == "spawn" {
                    spawn = Some((edge, s, fwd));
                } else {
                    let lane = lane(lane_word.unwrap_or(&"C")).map_err(|e| at(n, e))?;
                    bonus = Some(Spot {
                        edge,
                        s,
                        lane: if fwd { lane } else { -lane },
                    });
                }
            }
            "shed" => {
                let point =
                    grid_point(rest.first().copied().unwrap_or("")).map_err(|e| at(n, e))?;
                shed = Some(p.find(point).map_err(|e| at(n, e))?);
            }
            "patrol" => {
                for word in rest {
                    let point = grid_point(word).map_err(|e| at(n, e))?;
                    patrol.push(p.find(point).map_err(|e| at(n, e))?);
                }
            }
            "farmer" => {
                let mut words = rest.iter();
                while let Some(key) = words.next() {
                    if *key == "ambush" {
                        farmer.ambush = true;
                        continue;
                    }
                    let value = words
                        .next()
                        .ok_or_else(|| at(n, format!("farmer {key} needs a value")))?;
                    match *key {
                        "walk" => farmer.walk = speed(value).map_err(|e| at(n, e))?,
                        "run" => farmer.run = speed(value).map_err(|e| at(n, e))?,
                        "windup" => farmer.windup = seconds(value).map_err(|e| at(n, e))?,
                        "chase" => farmer.chase = seconds(value).map_err(|e| at(n, e))?,
                        "scatter" => farmer.scatter = seconds(value).map_err(|e| at(n, e))?,
                        "spook" => farmer.spook = seconds(value).map_err(|e| at(n, e))?,
                        _ => return Err(at(n, format!("unknown farmer setting {key}"))),
                    }
                }
            }
            "par" => {
                par = rest
                    .first()
                    .and_then(|w| w.parse().ok())
                    .filter(|v| (10..=3_600).contains(v))
                    .ok_or_else(|| at(n, "par needs seconds".into()))?;
            }
            "corridor" => {
                let from = grid_point(rest[0]).map_err(|e| at(n, e))?;
                let to = grid_point(rest[1]).map_err(|e| at(n, e))?;
                current = Some(p.edge(from, to).map_err(|e| at(n, e))?);
            }
            word => {
                let Some(found) = item(word) else {
                    return Err(at(n, format!("unknown statement {word}")));
                };
                let (edge, fwd) =
                    current.ok_or_else(|| at(n, format!("{word} outside a corridor")))?;
                let lane = lane(rest.first().copied().unwrap_or("")).map_err(|e| at(n, e))?;
                let lane = if fwd { lane } else { -lane };
                let at_list = distances(&rest[1..]).map_err(|e| at(n, e))?;
                if at_list.is_empty() {
                    return Err(at(n, format!("{word} needs a distance")));
                }
                let len = p.edges[edge].len;
                for s in at_list {
                    let s = if fwd { s } else { len - s };
                    match found {
                        Item::Edible(kind, air) => edibles.push(Edible {
                            edge,
                            s,
                            lane,
                            kind,
                            air,
                        }),
                        Item::Obstacle(kind) => obstacles.push(Obstacle {
                            edge,
                            s,
                            lane,
                            kind,
                        }),
                        Item::Power(kind) => powers.push(Power {
                            edge,
                            s,
                            lane,
                            kind,
                        }),
                    }
                }
            }
        }
    }
    let (spawn_edge, spawn_s, spawn_fwd) = spawn.ok_or("no spawn")?;
    let mut garden = Garden {
        name: name.ok_or("no name")?,
        number,
        set,
        nodes: p.nodes,
        edges: p.edges,
        edibles,
        obstacles,
        powers,
        spawn_edge,
        spawn_s,
        spawn_fwd,
        shed: shed.ok_or("no shed")?,
        patrol,
        farmer,
        par,
        bonus: bonus.ok_or("no bonus spot")?,
        digest: digest(text),
        exits: Vec::new(),
    };
    garden.link();
    Ok(garden)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_garden_parses_in_order_with_its_digest() {
        for (index, text) in GARDENS.iter().enumerate() {
            let garden = parse(text).unwrap_or_else(|e| panic!("garden {}: {e}", index + 1));
            assert_eq!(usize::from(garden.number), index + 1);
            assert_eq!(garden.digest, digest(text));
            assert!(!garden.name.is_empty());
        }
        assert_ne!(garden(1).digest, garden(2).digest);
    }

    #[test]
    fn ranges_lanes_and_reversed_corridors_read_right() {
        let text = "bunny.garden.v1\nname T\nnumber 1\ngrid 20\nspawn 1,0 0,0 at 3\n\
                    shed 1,1\npatrol 1,1\nbonus 1,0 0,0 at 6 R\n\
                    corridor 0,0 1,0\n  seedling C 3..7/2, 12\n\
                    corridor 1,1 1,0\n  carrot R 4\n  radish^ L 6\n  pot L 10\n\
                    corridor 1,0 0,0\n  carrot R 4\n";
        assert!(parse(text).unwrap_err().contains("twice"));
        let text = text.replace("corridor 1,0 0,0\n  carrot R 4\n", "");
        let g = parse(&text).unwrap();
        let s: Vec<i32> = g.edibles.iter().take(4).map(|e| e.s / UNIT).collect();
        assert_eq!(s, [3, 5, 7, 12]);
        let carrot = g.edibles[4];
        assert_eq!((carrot.edge, carrot.s, carrot.lane), (1, 4 * UNIT, 1));
        assert!(g.edibles[5].air);
        assert_eq!(g.obstacles[0].lane, -1);
        // Named from its far end, a spot turns around: 3 m from 1,0 is
        // 17 m from 0,0, running toward 0,0, and its right is the
        // corridor's left.
        assert_eq!(
            (g.spawn_edge, g.spawn_s, g.spawn_fwd),
            (0, 17 * UNIT, false)
        );
        assert_eq!((g.bonus.s, g.bonus.lane), (14 * UNIT, -1));
        assert!(parse("bunny.garden.v2\n").is_err());
        assert!(parse(&text.replace("pot", "teapot")).is_err());
        assert!(parse(&text.replace("corridor 1,1 1,0", "corridor 1,1 0,0")).is_err());
    }
}
