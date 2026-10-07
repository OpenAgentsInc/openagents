//! Checking a definition against a world tree, and the roster as a whole.
//!
//! [`npc`] checks one definition: its ID, name, card, and look; that its
//! home offers `sleep` and its workplace work; that its routine starts at
//! `00:00`, is sorted, stays within the row budget, sleeps at home and
//! works at the workplace, and names nodes whose affordances fit each
//! activity; that every walk ends before the next row starts; that every
//! leg routes, when a [`Router`] is given; and that its text is within
//! length limits and passes the [`Screen`]. [`exclusive`] checks that no
//! two villagers book one exclusive object at once.

use std::collections::BTreeMap;

use world_tree::{Kind, Tree};

use crate::routine::{JITTER_SECONDS, Villager, leg_meters, walk_seconds};
use crate::{
    Budgets, Code, MAX_ABOUT, MAX_LINE, MAX_NAME, MAX_ROLE, NPC_SCHEMA, Npc, Problem,
    WORK_AFFORDANCES, clock_text, good_id,
};

/// Routes between nodes in the zone: the zone supplies one over its
/// blockers.
pub trait Router {
    /// The route's length from `from`'s standing point to `to`'s, m.
    ///
    /// # Errors
    ///
    /// Why there is no route, such as a blocked doorway.
    fn meters(&self, from: &str, to: &str) -> Result<f32, String>;
}

/// A router that treats every leg as the straight line: for text previews
/// without the zone's blockers.
pub struct Straight<'a>(pub &'a Tree);

impl Router for Straight<'_> {
    fn meters(&self, from: &str, to: &str) -> Result<f32, String> {
        leg_meters(self.0, from, to).ok_or_else(|| format!("{from} or {to} isn't in the tree"))
    }
}

/// Screens text for secrets: the command line passes the secret screen.
pub trait Screen {
    /// Why `text` is refused, or `None`.
    fn refusal(&self, text: &str) -> Option<String>;
}

impl<F: Fn(&str) -> Option<String>> Screen for F {
    fn refusal(&self, text: &str) -> Option<String> {
        self(text)
    }
}

/// A screen that refuses nothing, for loading definitions already
/// admitted.
pub struct NoScreen;

impl Screen for NoScreen {
    fn refusal(&self, _: &str) -> Option<String> {
        None
    }
}

/// What a check runs against.
pub struct Checks<'a> {
    pub tree: &'a Tree,
    /// Routes every leg when given.
    pub router: Option<&'a dyn Router>,
    pub screen: &'a dyn Screen,
    pub budgets: Budgets,
}

impl<'a> Checks<'a> {
    /// Checks against `tree` with `screen`, no router, and the ceilings as
    /// budgets.
    #[must_use]
    pub fn new(tree: &'a Tree, screen: &'a dyn Screen) -> Self {
        Self {
            tree,
            router: None,
            screen,
            budgets: Budgets::CEILING,
        }
    }

    #[must_use]
    pub fn with_router(mut self, router: &'a dyn Router) -> Self {
        self.router = Some(router);
        self
    }

    #[must_use]
    pub fn with_budgets(mut self, budgets: Budgets) -> Self {
        self.budgets = budgets;
        self
    }
}

pub(crate) fn text(
    out: &mut Vec<Problem>,
    screen: &dyn Screen,
    field: String,
    value: &str,
    max: usize,
) {
    let count = value.chars().count();
    if value.trim().is_empty() {
        out.push(Problem::new(field, Code::Text, "is empty"));
        return;
    }
    if count > max {
        out.push(Problem::new(
            field,
            Code::Text,
            format!("is {count} characters, over {max}"),
        ));
        return;
    }
    if value.chars().any(char::is_control) {
        out.push(Problem::new(field, Code::Text, "holds a control character"));
        return;
    }
    if let Some(why) = screen.refusal(value) {
        out.push(Problem::new(
            field,
            Code::Secret,
            format!("the secret screen refuses it: {why}"),
        ));
    }
}

/// Checks node `id` for `field`: in the tree, and a place a villager can
/// stand (a building, a room, or an object).
pub(crate) fn place(out: &mut Vec<Problem>, tree: &Tree, field: &str, id: &str) -> bool {
    match tree.node(id) {
        None => {
            out.push(Problem::new(
                field,
                Code::Node,
                format!("{id} isn't in the world tree"),
            ));
            false
        }
        Some(n) if matches!(n.kind, Kind::Zone | Kind::District) => {
            out.push(Problem::new(
                field,
                Code::Node,
                format!("{id} is a {}, not a place to stand", n.kind.as_str()),
            ));
            false
        }
        Some(_) => true,
    }
}

/// Every problem with `npc` under `checks`; empty when it is fit to admit.
#[must_use]
pub fn npc(npc: &Npc, checks: &Checks) -> Vec<Problem> {
    let mut out = Vec::new();
    let tree = checks.tree;
    if npc.schema != NPC_SCHEMA {
        out.push(Problem::new(
            "schema",
            Code::Schema,
            format!("is {:?}, not {NPC_SCHEMA}", npc.schema),
        ));
    }
    if !good_id(&npc.id) {
        out.push(Problem::new(
            "id",
            Code::Id,
            format!(
                "{:?} isn't 1 to {} characters of a-z, 0-9, and inner hyphens",
                npc.id,
                crate::MAX_ID
            ),
        ));
    }
    text(&mut out, checks.screen, "name".into(), &npc.name, MAX_NAME);
    if !npc
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '.' | '\''))
    {
        out.push(Problem::new(
            "name",
            Code::Text,
            "uses a character the nameplate can't letter: use A-Z, 0-9, spaces, and - . '",
        ));
    }
    if !npc.card.character {
        out.push(Problem::new(
            "card.character",
            Code::Character,
            "must be true: townsfolk are labeled characters",
        ));
    }
    text(
        &mut out,
        checks.screen,
        "card.role".into(),
        &npc.card.role,
        MAX_ROLE,
    );
    text(
        &mut out,
        checks.screen,
        "card.about".into(),
        &npc.card.about,
        MAX_ABOUT,
    );
    if !npc.look.tint.iter().all(|c| (0.0..=1.0).contains(c)) {
        out.push(Problem::new(
            "look.tint",
            Code::Tint,
            "each channel must be 0 to 1",
        ));
    }
    if place(&mut out, tree, "home", &npc.home)
        && !tree
            .node(&npc.home)
            .is_some_and(|n| n.offers(world_tree::Affordance::Sleep))
    {
        out.push(Problem::new(
            "home",
            Code::Affordance,
            format!("{} doesn't offer sleep", npc.home),
        ));
    }
    if place(&mut out, tree, "workplace", &npc.workplace)
        && !tree
            .node(&npc.workplace)
            .is_some_and(|n| WORK_AFFORDANCES.iter().any(|a| n.offers(*a)))
    {
        out.push(Problem::new(
            "workplace",
            Code::Affordance,
            format!(
                "{} offers none of work, sell, shop, buy-bread, pray, read, or teach",
                npc.workplace
            ),
        ));
    }
    let lines = npc.lines.len();
    if lines > checks.budgets.lines as usize {
        out.push(Problem::new(
            "lines",
            Code::Budget,
            format!(
                "has {lines} lines, over the budget of {}",
                checks.budgets.lines
            ),
        ));
    }
    for (i, line) in npc.lines.iter().enumerate() {
        if let Some(step) = line.step()
            && !((1..=crate::MAX_STEP).contains(&step.len())
                && step
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-/".contains(&b)))
        {
            out.push(Problem::new(
                format!("lines[{i}].step"),
                Code::Text,
                format!(
                    "{step:?} isn't 1 to {} characters of a-z, 0-9, - and /",
                    crate::MAX_STEP
                ),
            ));
        }
        text(
            &mut out,
            checks.screen,
            format!("lines[{i}]"),
            line.text(),
            MAX_LINE,
        );
    }
    routine(&mut out, npc, checks);
    out
}

fn routine(out: &mut Vec<Problem>, npc: &Npc, checks: &Checks) {
    let tree = checks.tree;
    let rows = &npc.routine;
    let n = rows.len();
    if n == 0 {
        out.push(Problem::new("routine", Code::Coverage, "has no rows"));
        return;
    }
    if n > checks.budgets.routine_rows as usize {
        out.push(Problem::new(
            "routine",
            Code::Budget,
            format!(
                "has {n} rows, over the budget of {}",
                checks.budgets.routine_rows
            ),
        ));
    }
    let mut times = Vec::with_capacity(n);
    let mut nodes_ok = true;
    for (i, row) in rows.iter().enumerate() {
        let field = format!("routine[{i}]");
        match row.second() {
            Some(s) => times.push(s),
            None => out.push(Problem::new(
                format!("{field}.at"),
                Code::Time,
                format!("{:?} isn't HH:MM", row.at),
            )),
        }
        if !place(out, tree, &format!("{field}.node"), &row.node) {
            nodes_ok = false;
            continue;
        }
        let fits = row.activity.fits();
        if !tree
            .node(&row.node)
            .is_some_and(|node| fits.iter().any(|a| node.offers(*a)))
        {
            let names: Vec<&str> = fits.iter().map(|a| a.as_str()).collect();
            out.push(Problem::new(
                format!("{field}.activity"),
                Code::Affordance,
                format!(
                    "{} needs a node offering {}, and {} doesn't",
                    row.activity,
                    names.join(" or "),
                    row.node
                ),
            ));
        }
    }
    if !rows
        .iter()
        .any(|r| r.node == npc.home && r.activity == crate::Activity::Sleep)
    {
        out.push(Problem::new(
            "routine",
            Code::Coverage,
            "never sleeps at home",
        ));
    }
    if !rows.iter().any(|r| r.node == npc.workplace) {
        out.push(Problem::new(
            "routine",
            Code::Coverage,
            "never goes to the workplace",
        ));
    }
    if times.len() != n {
        return;
    }
    if times[0] != 0 {
        out.push(Problem::new(
            "routine[0].at",
            Code::Time,
            "the day starts at 00:00, so the table covers 24 hours",
        ));
        return;
    }
    let mut sorted = true;
    for i in 1..n {
        if times[i] <= times[i - 1] {
            out.push(Problem::new(
                format!("routine[{i}].at"),
                Code::Time,
                format!(
                    "{} isn't after the row before's {}",
                    rows[i].at,
                    rows[i - 1].at
                ),
            ));
            sorted = false;
        }
    }
    if !sorted || !nodes_ok {
        return;
    }
    let mut routes: BTreeMap<(&str, &str), Result<f32, String>> = BTreeMap::new();
    for i in 0..n {
        let from = rows[(i + n - 1) % n].node.as_str();
        let to = rows[i].node.as_str();
        if from == to {
            continue;
        }
        if let Some(router) = checks.router {
            let routed = routes
                .entry((from, to))
                .or_insert_with(|| router.meters(from, to));
            if let Err(why) = routed {
                out.push(Problem::new(
                    format!("routine[{i}].node"),
                    Code::Route,
                    format!("no route from {from}: {why}"),
                ));
            }
        }
        let walk = walk_seconds(leg_meters(tree, from, to).unwrap_or(0.0));
        let ends = f64::from(times[i]) + f64::from(JITTER_SECONDS) + walk;
        let next = times.get(i + 1).copied().unwrap_or(86_400);
        if ends > f64::from(next) {
            let next_text = if i + 1 < n {
                format!("the next row at {}", rows[i + 1].at)
            } else {
                "midnight".to_owned()
            };
            out.push(Problem::new(
                format!("routine[{i}].at"),
                Code::Walk,
                format!(
                    "the walk from {from} takes {} town minutes and may end at {}, after {next_text}",
                    (walk / 60.0).ceil(),
                    clock_text(ends)
                ),
            ));
        }
    }
}

/// When each row of `v` books an exclusive object: the node and the span,
/// seconds into the day.
fn bookings(v: &Villager) -> Vec<(usize, &str, u32, u32)> {
    let rows = &v.npc.routine;
    (0..rows.len())
        .filter(|&i| v.exclusive(i))
        .map(|i| {
            let end = if i + 1 < rows.len() {
                v.start(i + 1)
            } else {
                86_400
            };
            (i, rows[i].node.as_str(), v.start(i), end)
        })
        .collect()
}

/// Every pair of villagers that books one exclusive object at overlapping
/// times; each problem names the later villager in roster order.
#[must_use]
pub fn exclusive(villagers: &[Villager]) -> Vec<Problem> {
    let mut out = Vec::new();
    let booked: Vec<_> = villagers.iter().map(bookings).collect();
    for (k, later) in booked.iter().enumerate() {
        'row: for &(row, node, start, end) in later {
            for (j, earlier) in booked[..k].iter().enumerate() {
                for &(_, other, s, e) in earlier {
                    if other == node && start < e && s < end {
                        out.push(
                            Problem::new(
                                format!("routine[{row}].node"),
                                Code::Exclusive,
                                format!(
                                    "{node} is exclusive, and {} is booked there {}-{}",
                                    villagers[j].id(),
                                    clock_text(f64::from(s)),
                                    clock_text(f64::from(e))
                                ),
                            )
                            .of(villagers[k].id()),
                        );
                        continue 'row;
                    }
                }
            }
        }
    }
    out
}
