//! The boards Verse draws in the glade, as the Gym draws its boards: the
//! Task Wall in the yard, a monitor on every desk, and the goal board in
//! the atrium inside the gate. They are depth-tested faces on the zone's
//! vertex-color path. The frames are part of the world ([`draw`]); the
//! cards, the log lines, and the goal's text, progress ring, and counts
//! are the studio's live data ([`live`]), redrawn when a snapshot changes.

use super::layout::{Board, DESKS, GOAL_BOARD, TASK_COLUMNS, TASK_WALL};
use super::signals::Summary;
use crate::mesh::{Mesh, Vertex};
use coder_access::day_plan::DayPlan;
use coder_access::studio::{Activity, GoalStatus, TaskStatus, View};
use glam::{Mat4, Vec3};

const WOOD: [f32; 3] = [0.2, 0.12, 0.05];
const SLATE: [f32; 3] = [0.025, 0.04, 0.03];
const CHALK: [f32; 3] = [0.82, 0.78, 0.62];
const CHALK_DIM: [f32; 3] = [0.4, 0.38, 0.3];
const BEZEL: [f32; 3] = [0.07, 0.07, 0.08];
const SCREEN: [f32; 3] = [0.015, 0.03, 0.035];
const GLOW: [f32; 3] = [0.45, 0.85, 0.55];
const GLOW_DIM: [f32; 3] = [0.14, 0.32, 0.2];

fn vertex(pos: Vec3, color: [f32; 3]) -> Vertex {
    Vertex {
        pos: pos.to_array(),
        color,
        fog: 1.0,
    }
}

/// A flat quad in board space at depth `z`, from `min` to `max` in x and y.
fn panel(mesh: &mut Mesh, min: [f32; 2], max: [f32; 2], z: f32, color: [f32; 3]) {
    let [a, b, c, d] = [
        Vec3::new(min[0], min[1], z),
        Vec3::new(max[0], min[1], z),
        Vec3::new(max[0], max[1], z),
        Vec3::new(min[0], max[1], z),
    ]
    .map(|p| vertex(p, color));
    mesh.faces.extend_from_slice(&[a, b, c, a, c, d]);
}

/// A shaded box between `min` and `max`.
fn slab(mesh: &mut Mesh, min: Vec3, max: Vec3, color: [f32; 3]) {
    let corner = |i: usize| {
        Vec3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
    };
    let corners: [Vec3; 8] = std::array::from_fn(corner);
    for [a, b, c, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        let q = [corners[a], corners[b], corners[c], corners[d]];
        let shaded = super::draw::shade(color, q[0], q[1], q[2]);
        for p in [q[0], q[1], q[2], q[0], q[2], q[3]] {
            mesh.faces.push(vertex(p, shaded));
        }
    }
}

/// Lettering centered on `x`, its bottom at `y`, just in front of the face
/// at depth `z`.
pub(super) fn letters(
    mesh: &mut Mesh,
    text: &str,
    x: f32,
    y: f32,
    z: f32,
    height: f32,
    color: [f32; 3],
) {
    let mut glyphs = Mesh::default();
    crate::doors::scene_label(
        &mut glyphs,
        text,
        Vec3::new(x, y, z),
        height,
        coder_ui::theme::Intensity::Full,
    );
    mesh.faces
        .extend(glyphs.faces.into_iter().map(|v| Vertex { color, ..v }));
}

/// Moves board-space faces into the glade.
fn place(world: &mut Mesh, board: &Board, local: Mesh) {
    let transform: Mat4 = board.transform();
    world.faces.extend(local.faces.into_iter().map(|v| Vertex {
        pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
        ..v
    }));
}

/// The Task Wall: a framed slate on two legs with its title, a column per
/// task state, and the rules between them.
fn task_wall(world: &mut Mesh) {
    let board = TASK_WALL;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut mesh = Mesh::default();
    slab(
        &mut mesh,
        Vec3::new(-hw, -hh, 0.0),
        Vec3::new(hw, hh, 0.04),
        SLATE,
    );
    let bar = 0.07;
    for (min, max) in [
        ([-hw - bar, hh], [hw + bar, hh + bar]),
        ([-hw - bar, -hh - bar], [hw + bar, -hh]),
        ([-hw - bar, -hh], [-hw, hh]),
        ([hw, -hh], [hw + bar, hh]),
    ] {
        slab(
            &mut mesh,
            Vec3::new(min[0], min[1], -0.03),
            Vec3::new(max[0], max[1], 0.06),
            WOOD,
        );
    }
    // Legs from the ground to the frame.
    let ground = -board.center.y;
    for x in [-hw + 0.05, hw - 0.05] {
        slab(
            &mut mesh,
            Vec3::new(x - 0.06, ground, 0.0),
            Vec3::new(x + 0.06, -hh - bar, 0.08),
            WOOD,
        );
    }
    let face = -0.01;
    letters(&mut mesh, "TASK WALL", 0.0, hh - 0.24, face, 0.14, CHALK);
    let rule = hh - 0.3;
    panel(
        &mut mesh,
        [-hw + 0.06, rule - 0.012],
        [hw - 0.06, rule],
        face,
        CHALK_DIM,
    );
    let column = (w - 0.12) / TASK_COLUMNS.len() as f32;
    for (i, title) in TASK_COLUMNS.into_iter().enumerate() {
        // Board -X is the viewer's right, so columns run from +X.
        let center = hw - 0.06 - (i as f32 + 0.5) * column;
        letters(&mut mesh, title, center, rule - 0.13, face, 0.07, CHALK);
        if i > 0 {
            let x = center + column / 2.0;
            panel(
                &mut mesh,
                [x - 0.006, -hh + 0.08],
                [x + 0.006, rule - 0.02],
                face,
                CHALK_DIM,
            );
        }
    }
    place(world, &board, mesh);
}

/// A desk monitor: a bezel on a short stand around a dark screen.
pub fn monitor(world: &mut Mesh, board: &Board) {
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut mesh = Mesh::default();
    slab(
        &mut mesh,
        Vec3::new(-hw - 0.03, -hh - 0.03, 0.0),
        Vec3::new(hw + 0.03, hh + 0.03, 0.05),
        BEZEL,
    );
    // The stand reaches down to the workbench's top at 0.89 m.
    let desk = 0.89 - board.center.y;
    slab(
        &mut mesh,
        Vec3::new(-0.04, desk, 0.02),
        Vec3::new(0.04, -hh - 0.03, 0.05),
        BEZEL,
    );
    slab(
        &mut mesh,
        Vec3::new(-0.16, desk, -0.06),
        Vec3::new(0.16, desk + 0.02, 0.1),
        BEZEL,
    );
    panel(&mut mesh, [-hw, -hh], [hw, hh], -0.005, SCREEN);
    place(world, board, mesh);
}

/// The goal board: a framed slate on two legs with its title and a rule.
fn goal_board(world: &mut Mesh) {
    let board = GOAL_BOARD;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut mesh = Mesh::default();
    slab(
        &mut mesh,
        Vec3::new(-hw, -hh, 0.0),
        Vec3::new(hw, hh, 0.04),
        SLATE,
    );
    let bar = 0.07;
    for (min, max) in [
        ([-hw - bar, hh], [hw + bar, hh + bar]),
        ([-hw - bar, -hh - bar], [hw + bar, -hh]),
        ([-hw - bar, -hh], [-hw, hh]),
        ([hw, -hh], [hw + bar, hh]),
    ] {
        slab(
            &mut mesh,
            Vec3::new(min[0], min[1], -0.03),
            Vec3::new(max[0], max[1], 0.06),
            WOOD,
        );
    }
    let ground = -board.center.y;
    for x in [-hw + 0.1, hw - 0.1] {
        slab(
            &mut mesh,
            Vec3::new(x - 0.06, ground, 0.0),
            Vec3::new(x + 0.06, -hh - bar, 0.08),
            WOOD,
        );
    }
    let face = -0.01;
    letters(&mut mesh, "GOAL", 0.0, hh - 0.2, face, 0.12, CHALK);
    panel(
        &mut mesh,
        [-hw + 0.06, hh - 0.27],
        [hw - 0.06, hh - 0.258],
        face,
        CHALK_DIM,
    );
    place(world, &board, mesh);
}

/// Every board in the glade.
pub(super) fn draw(world: &mut Mesh) {
    task_wall(world);
    goal_board(world);
    for desk in &DESKS {
        monitor(world, &desk.monitor);
    }
}

/// Card paper, and the ink on it.
const PAPER: [f32; 3] = [0.72, 0.66, 0.5];
const PAPER_RUNNING: [f32; 3] = [0.55, 0.72, 0.5];
const PAPER_REVIEW: [f32; 3] = [0.85, 0.66, 0.3];
const PAPER_BLOCKED: [f32; 3] = [0.75, 0.38, 0.32];
const INK: [f32; 3] = [0.06, 0.05, 0.03];
const INK_DIM: [f32; 3] = [0.22, 0.2, 0.15];
/// A card's height and the space between cards, m.
const CARD: f32 = 0.15;
const CARD_GAP: f32 = 0.025;
/// The most cards a column shows; the last says how many more there are.
pub const COLUMN_CARDS: usize = 6;
/// Log lines a monitor shows.
pub const MONITOR_LINES: usize = 4;

/// The Task Wall column a task with `status` goes in, by index into
/// [`TASK_COLUMNS`]: planned (held or queued), running, review (waiting on
/// a person), done, and blocked (blocked, failed, cancelled, or missing).
/// The snapshot does not yet say whether a done task has landed, so the
/// review column holds the tasks whose next step is the person's.
#[must_use]
pub fn column(status: TaskStatus) -> usize {
    match status {
        TaskStatus::Held | TaskStatus::Queued => 0,
        TaskStatus::Running => 1,
        TaskStatus::Waiting => 2,
        TaskStatus::Done => 3,
        TaskStatus::Blocked | TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Missing => 4,
    }
}

/// Characters of lettering `height` tall that fit in `width`.
fn fit(width: f32, height: f32) -> usize {
    (width / (height * 6.0 / 7.0)).floor().max(1.0) as usize
}

/// Lettering whose left edge is at `left`. Board +X is the viewer's left,
/// so the text runs toward -X.
fn left_letters(mesh: &mut Mesh, text: &str, left: f32, y: f32, z: f32, h: f32, color: [f32; 3]) {
    let width = text.chars().count() as f32 * 6.0 * h / 7.0;
    letters(mesh, text, left - width / 2.0, y, z, h, color);
}

/// The Task Wall's cards: per column, the newest goal's tasks first.
fn cards(world: &mut Mesh, view: &View) {
    let board = TASK_WALL;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let column_width = (w - 0.12) / TASK_COLUMNS.len() as f32;
    let submitted = |goal: &str| {
        view.goals
            .iter()
            .find(|g| g.goal == goal)
            .map_or(0, |g| g.submitted_at)
    };
    let mut tasks: Vec<_> = view.tasks.iter().collect();
    tasks.sort_by(|a, b| {
        submitted(&b.goal)
            .cmp(&submitted(&a.goal))
            .then(a.position.cmp(&b.position))
    });
    let mut mesh = Mesh::default();
    let top = hh - 0.3 - 0.2;
    for (index, _) in TASK_COLUMNS.iter().enumerate() {
        let center = hw - 0.06 - (index as f32 + 0.5) * column_width;
        let (cx0, cx1) = (
            center - column_width / 2.0 + 0.03,
            center + column_width / 2.0 - 0.03,
        );
        let left = cx1 - 0.02;
        let inner = cx1 - cx0 - 0.04;
        let here: Vec<_> = tasks
            .iter()
            .filter(|task| column(task.status) == index)
            .collect();
        let shown = if here.len() > COLUMN_CARDS {
            COLUMN_CARDS - 1
        } else {
            here.len()
        };
        for (row, task) in here.iter().take(shown).enumerate() {
            let y1 = top - row as f32 * (CARD + CARD_GAP);
            let y0 = y1 - CARD;
            let paper = match task.status {
                TaskStatus::Running => PAPER_RUNNING,
                TaskStatus::Waiting => PAPER_REVIEW,
                TaskStatus::Blocked
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::Missing => PAPER_BLOCKED,
                _ => PAPER,
            };
            panel(&mut mesh, [cx0, y0], [cx1, y1], -0.012, paper);
            let title = super::studio::lettering(&task.title, fit(inner, 0.045));
            left_letters(&mut mesh, &title, left, y1 - 0.065, -0.018, 0.045, INK);
            let seat = super::studio::lettering(&task.seat, fit(inner, 0.035));
            left_letters(&mut mesh, &seat, left, y0 + 0.02, -0.018, 0.035, INK_DIM);
        }
        if here.len() > shown {
            let y = top - shown as f32 * (CARD + CARD_GAP) - 0.07;
            let more = format!("{} MORE", here.len() - shown);
            letters(&mut mesh, &more, center, y, -0.018, 0.05, CHALK_DIM);
        }
    }
    place(world, &board, mesh);
}

/// The middle of each card the Task Wall shows, in the glade, with its
/// task's identity, laid out as [`cards`] draws them, so selecting a card
/// opens its task's details.
#[must_use]
pub fn card_targets(view: &View) -> Vec<(Vec3, String)> {
    let board = TASK_WALL;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let column_width = (w - 0.12) / TASK_COLUMNS.len() as f32;
    let submitted = |goal: &str| {
        view.goals
            .iter()
            .find(|g| g.goal == goal)
            .map_or(0, |g| g.submitted_at)
    };
    let mut tasks: Vec<_> = view.tasks.iter().collect();
    tasks.sort_by(|a, b| {
        submitted(&b.goal)
            .cmp(&submitted(&a.goal))
            .then(a.position.cmp(&b.position))
    });
    let transform = board.transform();
    let top = hh - 0.3 - 0.2;
    let mut targets = Vec::new();
    for index in 0..TASK_COLUMNS.len() {
        let center = hw - 0.06 - (index as f32 + 0.5) * column_width;
        let here: Vec<_> = tasks
            .iter()
            .filter(|task| column(task.status) == index)
            .collect();
        let shown = if here.len() > COLUMN_CARDS {
            COLUMN_CARDS - 1
        } else {
            here.len()
        };
        for (row, task) in here.iter().take(shown).enumerate() {
            let y = top - row as f32 * (CARD + CARD_GAP) - CARD / 2.0;
            let at = transform.transform_point3(Vec3::new(center, y, -0.012));
            targets.push((at, task.task.clone()));
        }
    }
    targets
}

/// What a desk's monitor shows: the seat at the desk with its activity and
/// its newest log lines, or the desk's number when no seat sits there.
fn screen(world: &mut Mesh, index: usize, view: Option<&View>) {
    let board = DESKS[index].monitor;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let left = hw - 0.06;
    let inner = w - 0.12;
    let text = -0.01;
    let mut mesh = Mesh::default();
    let seat = view.and_then(|view| view.seats.iter().find(|s| s.desk as usize == index));
    match (view, seat) {
        (Some(view), Some(seat)) => {
            let title = format!("{} {}", seat.seat, super::studio::word(seat.activity));
            let title = super::studio::lettering(&title, fit(inner, 0.06));
            left_letters(&mut mesh, &title, left, hh - 0.11, text, 0.06, GLOW);
            let lines = view
                .logs
                .iter()
                .find(|log| log.seat == seat.seat)
                .map_or(&[][..], |log| log.lines.as_slice());
            let first = lines.len().saturating_sub(MONITOR_LINES);
            for (i, line) in lines[first..].iter().enumerate() {
                let y = hh - 0.2 - i as f32 * 0.075;
                let color =
                    if line.activity == Activity::Failed || line.activity == Activity::Blocked {
                        [0.85, 0.35, 0.3]
                    } else {
                        GLOW_DIM.map(|c| c * 1.8)
                    };
                let line = super::studio::lettering(&line.text, fit(inner, 0.032));
                left_letters(&mut mesh, &line, left, y - 0.03, text, 0.032, color);
            }
        }
        _ => {
            letters(
                &mut mesh,
                &format!("SEAT {}", index + 1),
                0.0,
                hh - 0.12,
                text,
                0.07,
                GLOW,
            );
            for (i, length) in [0.62_f32, 0.44, 0.55, 0.3].into_iter().enumerate() {
                let y = hh - 0.2 - i as f32 * 0.075;
                panel(
                    &mut mesh,
                    [left - length, y - 0.025],
                    [left, y],
                    text,
                    GLOW_DIM,
                );
            }
        }
    }
    place(world, &board, mesh);
}

/// Segments in the goal board's progress ring.
pub const RING_SEGMENTS: usize = 32;
/// The ring's lit part, its lit part for a finished goal, and its unlit
/// part.
const RING_LIT: [f32; 3] = [0.45, 0.85, 0.55];
const RING_DONE: [f32; 3] = [1.0, 0.82, 0.35];
const RING_DIM: [f32; 3] = [0.16, 0.16, 0.13];
/// The goal text's lettering height and its most lines.
const GOAL_TEXT: f32 = 0.075;
const GOAL_LINES: usize = 3;

/// A flat quad through `corners` at depth `z`, wound as [`panel`] winds.
fn quad(mesh: &mut Mesh, corners: [[f32; 2]; 4], z: f32, color: [f32; 3]) {
    let [a, b, c, _] = corners;
    let turn = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let mut corners = corners;
    if turn < 0.0 {
        corners.reverse();
    }
    let [a, b, c, d] = corners.map(|[x, y]| vertex(Vec3::new(x, y, z), color));
    mesh.faces.extend_from_slice(&[a, b, c, a, c, d]);
}

/// The ring's lit segments for `progress`: none at zero, and every one
/// only at one.
#[must_use]
pub fn lit_segments(progress: f32) -> usize {
    let progress = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (progress * RING_SEGMENTS as f32).floor() as usize
}

/// A progress ring centered on `center`, between the `radius` pair, lit
/// clockwise from the top as the viewer sees it for `progress`. Board -X is
/// the viewer's right, so a clockwise turn starts toward -X.
fn ring(mesh: &mut Mesh, center: [f32; 2], radius: [f32; 2], progress: f32, lit: [f32; 3]) {
    let lit_count = lit_segments(progress);
    let step = std::f32::consts::TAU / RING_SEGMENTS as f32;
    // A hair of gap between segments, so the ring reads as a dial.
    let gap = step * 0.08;
    let point = |r: f32, angle: f32| [center[0] - r * angle.sin(), center[1] + r * angle.cos()];
    let [inner, outer] = radius;
    for segment in 0..RING_SEGMENTS {
        let (a0, a1) = (
            segment as f32 * step + gap,
            (segment + 1) as f32 * step - gap,
        );
        let color = if segment < lit_count { lit } else { RING_DIM };
        quad(
            mesh,
            [
                point(inner, a0),
                point(outer, a0),
                point(outer, a1),
                point(inner, a1),
            ],
            -0.012,
            color,
        );
    }
}

/// `text` in at most `lines` lines of at most `width` characters, broken
/// between words, in the board alphabet. A word longer than a line is cut,
/// and text past the last line is dropped.
#[must_use]
pub fn wrap(text: &str, width: usize, lines: usize) -> Vec<String> {
    let text = super::studio::lettering(text, usize::MAX);
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if out.len() == lines {
            return out;
        }
        let word: String = word.chars().take(width).collect();
        if line.is_empty() {
            line = word;
        } else if line.chars().count() + 1 + word.chars().count() <= width {
            line.push(' ');
            line.push_str(&word);
        } else {
            out.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() && out.len() < lines {
        out.push(line);
    }
    out
}

/// The goal board's live face: the ring with the task count in it on the
/// viewer's left, and to its right the goal's text, its state, and the
/// decisions waiting. A studio with no goal says so; no studio leaves the
/// ring dark.
fn goal(world: &mut Mesh, view: Option<&View>) {
    let board = GOAL_BOARD;
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let text = -0.018;
    let mut mesh = Mesh::default();
    let center = [hw - 0.42, -0.1];
    let summary = view.and_then(Summary::of);
    let (progress, lit) = match &summary {
        Some(summary) if summary.status == GoalStatus::Done => (1.0, RING_DONE),
        Some(summary) => (summary.progress(), RING_LIT),
        None => (0.0, RING_LIT),
    };
    ring(&mut mesh, center, [0.24, 0.32], progress, lit);
    // The text column runs from just right of the ring to the frame.
    let left = hw - 0.86;
    let width = left - (-hw + 0.08);
    match (view, &summary) {
        (_, Some(summary)) => {
            if summary.total > 0 {
                let count = format!("{}/{}", summary.done, summary.total);
                letters(
                    &mut mesh,
                    &count,
                    center[0],
                    center[1] - 0.05,
                    text,
                    0.1,
                    CHALK,
                );
            }
            let lines = wrap(&summary.text, fit(width, GOAL_TEXT), GOAL_LINES);
            for (i, line) in lines.iter().enumerate() {
                let y = hh - 0.43 - i as f32 * (GOAL_TEXT + 0.04);
                left_letters(&mut mesh, line, left, y, text, GOAL_TEXT, CHALK);
            }
            let state = super::studio::lettering(&summary.counts(), fit(width, 0.06));
            left_letters(&mut mesh, &state, left, -hh + 0.24, text, 0.06, CHALK_DIM);
            if summary.waiting > 0 {
                let waiting = format!("{} WAITING", summary.waiting);
                left_letters(
                    &mut mesh,
                    &waiting,
                    left,
                    -hh + 0.1,
                    text,
                    0.06,
                    PAPER_REVIEW,
                );
            }
        }
        (Some(_), None) => {
            left_letters(
                &mut mesh,
                "NO GOAL YET",
                left,
                -0.05,
                text,
                GOAL_TEXT,
                CHALK_DIM,
            );
        }
        (None, None) => {}
    }
    place(world, &board, mesh);
}

/// The most plan lines the plan board shows under its title.
pub const PLAN_LINES: usize = 9;
/// The plan board's lettering height, m.
const PLAN_TEXT: f32 = 0.095;

/// The plan board's lines, top to bottom, in the board alphabet, and
/// whether each is the block under way: a block's start and title, the
/// current block's steps under it, and the last re-plan. An idle day says
/// so.
#[must_use]
pub fn plan_lines(plan: &DayPlan, width: usize) -> Vec<(String, bool)> {
    use coder_access::day_plan::clock;
    let fit = |text: &str| super::studio::lettering(&text.replace(':', "."), width);
    if plan.idle() {
        return vec![(fit("No work today. Idle at her desk."), false)];
    }
    let current = plan.current.and_then(|i| usize::try_from(i).ok());
    let mut out = Vec::new();
    for (index, block) in plan.blocks.iter().enumerate() {
        let now = current == Some(index);
        out.push((fit(&format!("{} {}", clock(block.start), block.title)), now));
        if now {
            for step in plan.steps.iter().take(2) {
                out.push((fit(&format!("  {} {}", clock(step.start), step.text)), true));
            }
        }
    }
    if let Some(replan) = plan.replans.last() {
        out.push((fit(&format!("Replanned {}", clock(replan.minute))), false));
    }
    // Keep the block under way in view: drop finished blocks first.
    while out.len() > PLAN_LINES {
        match out.iter().position(|(_, now)| *now) {
            Some(at) if at > 0 => {
                out.remove(0);
            }
            _ => {
                out.truncate(PLAN_LINES);
            }
        }
    }
    out
}

/// The plan board in the great room: a framed slate on the wall with its
/// title, and, from `plan`, her day's blocks with the one under way lit.
/// No plan leaves the slate blank under its title.
#[must_use]
pub fn plan_board(plan: Option<&DayPlan>) -> Mesh {
    let board = super::layout::estate::plan_board();
    let [w, h] = board.size;
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut mesh = Mesh::default();
    slab(
        &mut mesh,
        Vec3::new(-hw, -hh, 0.0),
        Vec3::new(hw, hh, 0.04),
        SLATE,
    );
    let bar = 0.06;
    for (min, max) in [
        ([-hw - bar, hh], [hw + bar, hh + bar]),
        ([-hw - bar, -hh - bar], [hw + bar, -hh]),
        ([-hw - bar, -hh], [-hw, hh]),
        ([hw, -hh], [hw + bar, hh]),
    ] {
        slab(
            &mut mesh,
            Vec3::new(min[0], min[1], -0.03),
            Vec3::new(max[0], max[1], 0.06),
            WOOD,
        );
    }
    let face = -0.012;
    let title = match plan {
        Some(plan) => format!("ALICE / DAY PLAN  {}", plan.date.replace('-', ".")),
        None => "ALICE / DAY PLAN".into(),
    };
    letters(&mut mesh, &title, 0.0, hh - 0.22, face, 0.12, CHALK);
    panel(
        &mut mesh,
        [-hw + 0.06, hh - 0.28],
        [hw - 0.06, hh - 0.268],
        face,
        CHALK_DIM,
    );
    if let Some(plan) = plan {
        let left = hw - 0.1;
        // A label holds at most 32 characters.
        let width = fit(w - 0.2, PLAN_TEXT).min(32);
        for (i, (line, now)) in plan_lines(plan, width).iter().enumerate() {
            let y = hh - 0.46 - i as f32 * (PLAN_TEXT + 0.07);
            let color = if *now { PAPER_RUNNING } else { CHALK };
            left_letters(&mut mesh, line, left, y, face - 0.006, PLAN_TEXT, color);
        }
    }
    let mut world = Mesh::default();
    place(&mut world, &board, mesh);
    world
}

/// The live boards: the Task Wall's cards, every monitor's text, and the
/// goal board's face, from `view`, or idle boards without one.
#[must_use]
pub fn live(view: Option<&View>) -> Mesh {
    let mut mesh = Mesh::default();
    if let Some(view) = view {
        cards(&mut mesh, view);
    }
    goal(&mut mesh, view);
    for index in 0..DESKS.len() {
        screen(&mut mesh, index, view);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::studio::Task;

    #[test]
    fn each_shown_card_is_a_target_on_the_wall() {
        let task = |id: &str, status| Task {
            spend: Default::default(),
            task: id.into(),
            goal: "g1".into(),
            entry: id.into(),
            position: 1,
            title: "A task".into(),
            seat: "ada".into(),
            depends_on: Vec::new(),
            status,
        };
        let mut view = View::default();
        for n in 0..COLUMN_CARDS + 2 {
            view.tasks
                .push(task(&format!("held-{n}"), TaskStatus::Held));
        }
        view.tasks.push(task("running", TaskStatus::Running));
        let targets = card_targets(&view);
        // A full column shows one card fewer and says how many more.
        assert_eq!(targets.len(), COLUMN_CARDS - 1 + 1);
        let [w, h] = TASK_WALL.size;
        for (at, _) in &targets {
            assert!(at.distance(TASK_WALL.center) < w.hypot(h) / 2.0);
        }
        let running = targets.iter().find(|(_, id)| id == "running").unwrap().0;
        let held = targets.iter().find(|(_, id)| id == "held-0").unwrap().0;
        assert!(
            (running.y - held.y).abs() < 1e-4,
            "both top cards share a row"
        );
        assert!(running.distance(held) > 0.1, "in different columns");
    }

    use coder_access::studio::Goal;

    #[test]
    fn the_ring_lights_whole_segments_for_progress() {
        assert_eq!(lit_segments(0.0), 0);
        assert_eq!(lit_segments(0.5), RING_SEGMENTS / 2);
        assert_eq!(lit_segments(0.99), RING_SEGMENTS - 1);
        assert_eq!(lit_segments(1.0), RING_SEGMENTS);
        assert_eq!(lit_segments(7.0), RING_SEGMENTS);
        assert_eq!(lit_segments(f32::NAN), 0);
    }

    #[test]
    fn goal_text_wraps_between_words_into_the_board_alphabet() {
        assert_eq!(
            wrap("Add a dark mode, then ship it!", 12, 3),
            ["ADD A DARK", "MODE THEN", "SHIP IT"]
        );
        assert_eq!(wrap("one two three four", 9, 1), ["ONE TWO"]);
        assert_eq!(wrap("supercalifragilistic", 5, 2), ["SUPER"]);
        assert!(wrap("", 10, 2).is_empty());
    }

    #[test]
    fn the_goal_board_faces_the_approach_and_shows_the_goal() {
        // Its face looks down the approach path, toward the return portal.
        let front = GOAL_BOARD
            .transform()
            .transform_vector3(Vec3::NEG_Z)
            .normalize();
        assert!(front.dot(Vec3::NEG_Z) > 0.99);
        let mut idle = Mesh::default();
        goal(&mut idle, None);
        let mut empty = Mesh::default();
        goal(&mut empty, Some(&View::default()));
        let view = View {
            goals: vec![Goal {
                spend: Default::default(),
                goal: "g1".into(),
                text: "Add a dark mode".into(),
                workspace: "repo".into(),
                lead: "lead".into(),
                status: GoalStatus::Running,
                final_tasks: 1,
                total_tasks: 3,
                submitted_at: 1,
            }],
            ..View::default()
        };
        let mut shown = Mesh::default();
        goal(&mut shown, Some(&view));
        // The dark ring alone, then lettering for no goal, then the goal's.
        assert_eq!(idle.faces.len(), RING_SEGMENTS * 6);
        assert!(empty.faces.len() > idle.faces.len());
        assert!(shown.faces.len() > empty.faces.len());
        let lit = shown.faces.iter().filter(|v| v.color == RING_LIT).count();
        assert_eq!(lit, lit_segments(1.0 / 3.0) * 6);
    }

    #[test]
    fn the_plan_board_shows_her_day_in_the_great_room() {
        use super::super::layout::estate::{self, AliceSpot, OWNERS_HOUSE};
        use coder_access::day_plan::{Block, By, DayPlan, SCHEMA};
        let room = "everglade/knowledge-district/owners-house/great-room";
        let mut plan = DayPlan {
            schema: SCHEMA.into(),
            agent: "alice".into(),
            date: "2026-10-07".into(),
            utc_offset: 0,
            made_at: 0,
            bound: "everglade/knowledge-district/owners-house".into(),
            blocks: (0..8)
                .map(|i| Block {
                    start: 480 + i * 60,
                    end: 540 + i * 60,
                    title: format!("Work issue {i}: fix it"),
                    source: format!("issue:{i}"),
                    node: format!("{room}/workstation"),
                    by: By::Model,
                })
                .collect(),
            current: Some(7),
            steps: Vec::new(),
            replans: Vec::new(),
        };
        let lines = plan_lines(&plan, 40);
        assert!(lines.len() <= PLAN_LINES);
        // The block under way stays in view, in the board alphabet.
        assert_eq!(
            lines.last().unwrap(),
            &("15.00 WORK ISSUE 7. FIX IT".to_string(), true)
        );
        let empty = plan_board(None);
        let shown = plan_board(Some(&plan));
        assert!(shown.faces.len() > empty.faces.len());
        plan.blocks.clear();
        plan.current = None;
        assert_eq!(
            plan_lines(&plan, 40)[0].0,
            "NO WORK TODAY. IDLE AT HER DESK."
        );
        // It hangs inside the great room, facing into it.
        let board = estate::plan_board();
        let (center, half) = estate::ALICE_ROOM;
        let middle = OWNERS_HOUSE.world(center);
        assert!((board.center.x - middle[0]).abs() <= half + 0.5);
        assert!((board.center.z - middle[1]).abs() <= half + 0.5);
        let out = glam::Vec3::new(board.facing.sin(), 0.0, board.facing.cos());
        let inward = glam::Vec3::new(middle[0] - board.center.x, 0.0, middle[1] - board.center.z);
        assert!(out.dot(inward) > 0.0);
        // Plan nodes name her spots.
        assert_eq!(
            AliceSpot::of_node(&format!("{room}/console")),
            Some(AliceSpot::Workbench)
        );
        assert_eq!(
            AliceSpot::of_node("everglade/commons/workshop-hall/hall/library"),
            None
        );
    }
}
