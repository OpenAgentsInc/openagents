//! `haunt describe`: the house as text an agent reads before any edit. A
//! top-down map, then each area's size, place beside its neighbours, the
//! way a player faces on entering, its light, sound, intent and props, the
//! free walls where an area could be added, and the budgets left.

use std::fmt::Write as _;

use haunt_assets::Catalog;

use crate::check::{MAX_AREAS, MAX_LIGHTS, MAX_PROPS, MAX_SOUNDS, sounds_at_once};
use crate::doc::{Action, Area, House, OUTSIDE, When, name};
use crate::edit::free_walls;

/// Map columns per metre east to west.
const COLS_PER_M: f32 = 2.0;
/// Map rows per metre north to south.
const ROWS_PER_M: f32 = 0.5;

/// A number with at most two decimals and no trailing zeros.
fn n1(x: f32) -> String {
    let text = format!("{:.2}", (x * 100.0).round() / 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

fn facing(yaw: f32) -> &'static str {
    let y = yaw.rem_euclid(360.0);
    const WORDS: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    WORDS[((y + 22.5) / 45.0) as usize % 8]
}

/// The top-down map: north up, each area boxed and lettered, its id inside
/// when it fits, doors as gaps, `@` where you arrive (with `v`, `^`, `<` or
/// `>` for the way you face), and `X` on the way out.
#[must_use]
pub fn map(house: &House) -> String {
    let Some(first) = house.areas.first() else {
        return String::new();
    };
    let (mut x0, mut z0, mut x1, mut z1) = (first.x0(), first.z0(), first.x1(), first.z1());
    for a in &house.areas {
        x0 = x0.min(a.x0());
        z0 = z0.min(a.z0());
        x1 = x1.max(a.x1());
        z1 = z1.max(a.z1());
    }
    let col = |x: f32| ((x - x0) * COLS_PER_M).round() as usize;
    let row = |z: f32| ((z1 - z) * ROWS_PER_M).round() as usize;
    let w = col(x1) + 1;
    let h = row(z0) + 1;
    let mut grid = vec![vec![' '; w]; h];
    let put = |r: usize, c: usize, ch: char, grid: &mut Vec<Vec<char>>| {
        if r < h && c < w {
            let cell = &mut grid[r][c];
            // Corners win over edges.
            if *cell != '+' {
                *cell = ch;
            }
        }
    };
    for a in &house.areas {
        let (c0, c1, r0, r1) = (col(a.x0()), col(a.x1()), row(a.z1()), row(a.z0()));
        for c in c0..=c1 {
            put(r0, c, '-', &mut grid);
            put(r1, c, '-', &mut grid);
        }
        for r in r0..=r1 {
            put(r, c0, '|', &mut grid);
            put(r, c1, '|', &mut grid);
        }
        for (r, c) in [(r0, c0), (r0, c1), (r1, c0), (r1, c1)] {
            if r < h && c < w {
                grid[r][c] = '+';
            }
        }
    }
    // Doors: gaps in the walls.
    for a in &house.areas {
        for d in &a.doors {
            let (s, e) = a.door_span(d);
            let (fixed, _, _) = a.wall_line(d.wall);
            let mark = if d.to == OUTSIDE && house.exit.door == d.id {
                'X'
            } else {
                ' '
            };
            if d.wall.along_x() {
                let r = row(fixed);
                for c in col(s) + 1..col(e) {
                    if r < h && c < w {
                        grid[r][c] = mark;
                    }
                }
            } else {
                let c = col(fixed);
                let (ra, rb) = (row(e), row(s));
                for r in ra..=rb {
                    if r < h && c < w && (r != ra || ra == rb) {
                        grid[r][c] = mark;
                    }
                }
            }
        }
    }
    // Labels.
    for (i, a) in house.areas.iter().enumerate() {
        let key = char::from(b'A' + (i % 26) as u8);
        let (c0, c1, r0, r1) = (col(a.x0()), col(a.x1()), row(a.z1()), row(a.z0()));
        let inner = c1.saturating_sub(c0 + 1);
        let r = (r0 + r1) / 2;
        let text: Vec<char> = if inner >= a.id.len() + 4 {
            format!("{key} {}", a.id).chars().collect()
        } else {
            vec![key]
        };
        let start = c0 + 1 + inner.saturating_sub(text.len()) / 2;
        for (k, ch) in text.into_iter().enumerate() {
            let c = start + k;
            if r < h && c < w && c < c1 {
                grid[r][c] = ch;
            }
        }
    }
    if let (Some(p), Some(a)) = (house.arrival_point(), house.area(&house.arrival.area)) {
        let _ = a;
        let (r, c) = (row(p[1]), col(p[0]));
        let arrow = match facing(house.arrival.yaw) {
            "north" => '^',
            "east" => '>',
            "west" => '<',
            _ => 'v',
        };
        if r < h && c + 1 < w {
            grid[r][c] = '@';
            grid[r][c + 1] = arrow;
        }
    }
    let mut out = String::from("north is up; 1 column is 0.5 m, 1 row is 2 m\n");
    for line in grid {
        let text: String = line.into_iter().collect();
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// Where `area` sits beside its neighbours: one phrase per door.
fn neighbours(house: &House, area: &Area) -> Vec<String> {
    area.doors
        .iter()
        .map(|d| {
            if d.to == OUTSIDE {
                format!(
                    "its {} door, {}, leads outside{}",
                    d.wall.word(),
                    d.id,
                    if house.exit.door == d.id {
                        " (the way out)"
                    } else {
                        ""
                    }
                )
            } else {
                format!(
                    "{} of {}, through its {} door ({})",
                    d.wall.opposite().word(),
                    name(&d.to),
                    d.wall.opposite().word(),
                    d.id
                )
            }
        })
        .collect()
}

/// Which way a player faces on coming in through each door.
fn entering(house: &House, area: &Area) -> Vec<String> {
    let mut out = Vec::new();
    if house.arrival.area == area.id {
        out.push(format!("you arrive facing {}", facing(house.arrival.yaw)));
    }
    for d in &area.doors {
        if d.to == OUTSIDE {
            continue;
        }
        let inward = d.wall.opposite().outward_yaw();
        out.push(format!("from {} you face {}", name(&d.to), facing(inward)));
    }
    out
}

fn action_words(a: &Action) -> String {
    match a {
        Action::Sfx(s) => format!("plays {s}"),
        Action::Light(l) => {
            let mut parts = Vec::new();
            if let Some(p) = &l.preset {
                parts.push(format!("turns the light to {p}"));
            }
            if let Some(d) = l.dim {
                parts.push(format!("dims the light by {}", n1(d)));
            }
            let mut text = parts.join(" and ");
            if let Some(s) = l.seconds {
                let _ = write!(text, " for {} s", n1(s));
            }
            text
        }
        Action::State(s) => format!("makes {} {}", s.prop, s.to),
        Action::Effect(e) => match &e.at {
            Some(at) => format!("shows {} at {at}", e.id),
            None => format!("shows {}", e.id),
        },
    }
}

fn when_words(w: &When) -> String {
    match w {
        When::Enter(_) => "on entering".into(),
        When::EnterMiddle(_) => "at the middle".into(),
        When::Near { prop, within } => format!("within {} m of {prop}", n1(*within)),
        When::Look { prop } => format!("when you look at {prop}"),
    }
}

/// The full description of the house for an agent.
#[must_use]
pub fn describe(house: &House, catalog: &Catalog) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} ({})", house.title, crate::doc::KIND);
    let _ = writeln!(
        out,
        "Axes: +x east, +z north, metres; yaw 0 faces north, 90 east. Rectangles are [x, z, width, depth] from the south-west corner; a prop's position is from its area's south-west corner."
    );
    let a = &house.audio;
    let _ = writeln!(
        out,
        "Look: vignette {}, lantern {}, grain {}, sky {}. Volumes: master {}, music {} ({}), ambience {}, effects {}, voice {}.",
        n1(house.look.vignette),
        if house.look.lantern { "on" } else { "off" },
        n1(house.look.grain),
        house.look.sky,
        n1(a.master),
        n1(a.music),
        a.music_track,
        n1(a.ambience),
        n1(a.sfx),
        n1(a.voice)
    );
    let _ = writeln!(
        out,
        "You arrive in {} at {} facing {}, with the {} cue behind you. The way out is {}'s door {}.",
        name(&house.arrival.area),
        pos(house.arrival.at),
        facing(house.arrival.yaw),
        house.arrival.cue,
        name(&house.exit.area),
        house.exit.door
    );
    let _ = writeln!(out, "\nMAP");
    out.push_str(&map(house));
    let _ = writeln!(out, "\nAREAS");
    for (i, area) in house.areas.iter().enumerate() {
        let key = char::from(b'A' + (i % 26) as u8);
        let ceiling = area
            .ceiling
            .map_or_else(|| "open sky".to_owned(), |c| format!("{} m ceiling", n1(c)));
        let _ = writeln!(
            out,
            "\n[{key}] {} ({}): {} x {} m, {ceiling}, at [{}, {}, {}, {}].",
            area.id,
            area.kind.word(),
            n1(area.width()),
            n1(area.depth()),
            n1(area.rect[0]),
            n1(area.rect[1]),
            n1(area.rect[2]),
            n1(area.rect[3])
        );
        let near = neighbours(house, area);
        if !near.is_empty() {
            let _ = writeln!(out, "  Place: {}.", near.join("; "));
        }
        let enter = entering(house, area);
        if !enter.is_empty() {
            let _ = writeln!(out, "  Facing: {}.", enter.join("; "));
        }
        let _ = writeln!(
            out,
            "  Intent: {} (intensity {} to {}){}",
            area.intent.feeling,
            area.intent.intensity[0],
            area.intent.intensity[1],
            if area.intent.beat.is_empty() {
                ".".to_owned()
            } else {
                format!("; beat: {}.", area.intent.beat)
            }
        );
        let mut light = format!("  Light: {}", area.light.preset);
        if area.light.dim > 0.0 {
            let _ = write!(light, ", dimmed {}", n1(area.light.dim));
        }
        if let Some(amb) = &area.light.ambient {
            let _ = write!(light, ", ambient {} at {}", amb.color, n1(amb.level));
        }
        if !area.light.lights.is_empty() {
            let _ = write!(light, ", {} point lights", area.light.lights.len());
        }
        let _ = writeln!(out, "{light}.");
        let _ = writeln!(
            out,
            "  Sound: {}, music at {}.",
            area.sound
                .bed
                .as_deref()
                .map_or_else(|| "no bed".to_owned(), |b| format!("{b} bed")),
            n1(area.sound.music_gain)
        );
        let _ = writeln!(
            out,
            "  Surfaces: floor {}, walls {}{}.",
            area.surfaces.floor,
            area.surfaces.walls,
            area.surfaces
                .trim
                .as_deref()
                .map_or_else(String::new, |t| format!(", trim {t}"))
        );
        if area.props.is_empty() {
            let _ = writeln!(out, "  Props: none.");
        } else {
            let _ = writeln!(out, "  Props:");
            for p in &area.props {
                let kind = catalog.asset(&p.asset).map_or("unknown", |a| match a.kind {
                    haunt_assets::Kind::Card => "card",
                    haunt_assets::Kind::Mesh => "mesh",
                    haunt_assets::Kind::Effect => "effect",
                });
                let mut line = format!(
                    "  - {} ({} {kind}) at {}, facing {}",
                    p.id,
                    p.asset,
                    pos(p.at),
                    facing(p.yaw)
                );
                if p.y > 0.0 {
                    let _ = write!(line, ", {} m up", n1(p.y));
                }
                if let Some(s) = &p.state {
                    let _ = write!(line, ", {s}");
                }
                if let Some(t) = &p.text {
                    let _ = write!(line, ", reads \"{t}\"");
                }
                if !p.note.is_empty() {
                    let _ = write!(line, ": {}", p.note);
                }
                let _ = writeln!(out, "{line}.");
            }
        }
        if !area.triggers.is_empty() {
            let _ = writeln!(out, "  Triggers:");
            for t in &area.triggers {
                let what: Vec<String> = t.actions.iter().map(action_words).collect();
                let _ = writeln!(
                    out,
                    "  - {} it {}{}.",
                    when_words(&t.when),
                    what.join(", "),
                    if t.every_visit {
                        ""
                    } else {
                        " (first visit only)"
                    }
                );
            }
        }
        let free = free_walls(house, area);
        if !free.is_empty() {
            let _ = writeln!(out, "  Free walls:");
            for f in free {
                let _ = writeln!(
                    out,
                    "  - {}: {} m free (from {} to {} along it), room for an area up to {} x {} m.",
                    f.wall.word(),
                    n1(f.length()),
                    n1(f.from),
                    n1(f.to),
                    n1(if f.wall.along_x() {
                        f.length()
                    } else {
                        f.reach
                    }),
                    n1(if f.wall.along_x() {
                        f.reach
                    } else {
                        f.length()
                    })
                );
            }
        }
        let _ = writeln!(
            out,
            "  Budget left: {} props, {} lights, {} sounds at once.",
            MAX_PROPS.saturating_sub(area.props.len()),
            MAX_LIGHTS.saturating_sub(area.light.lights.len()),
            MAX_SOUNDS.saturating_sub(sounds_at_once(area))
        );
    }
    let _ = writeln!(
        out,
        "\nBUDGET\n{} of {MAX_AREAS} areas used; {} more can be added.",
        house.areas.len(),
        MAX_AREAS.saturating_sub(house.areas.len())
    );
    let _ = writeln!(
        out,
        "\nTo add an area, name one of the areas and one of its free walls; to place something, name a library asset (describe --library lists them)."
    );
    out
}

fn pos(p: [f32; 2]) -> String {
    format!("[{}, {}]", n1(p[0]), n1(p[1]))
}
