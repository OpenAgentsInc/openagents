//! The plan and to-do panel: a task's plan items and where each stands.
//!
//! One panel serves every place a plan shows: the chat under a running
//! Coder task ([`crate::coder_run`]), an Agent Studio seat's plan, and a
//! Task Wall card's detail. It reads the plan a task recorded
//! ([`openagents_chat::plan`]) and draws it as a Rust Native view; the
//! caller keeps one [`Panel`] per task and routes the panel's button keys
//! back to [`Panel::apply`].
//!
//! The behavior reimplements Zeron's checklist tray (public MIT
//! zeronsh/zeron at `9e1a1115`, `crates/ui/src/todo_panel.rs`) in Rust
//! Native: a header with the count and the item being worked on, a list
//! that is open while work remains and compact once everything is done,
//! long lists folded to three items around the current one, and a
//! dismissal that holds until the engine writes a different plan.

use openagents_chat::plan::{Item, Status, Summary};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;

/// Lists longer than this fold to [`FOCUS_WINDOW`] items around the
/// current one.
pub const FOLD_ABOVE: usize = 6;
/// The items a folded list keeps in view.
pub const FOCUS_WINDOW: usize = 3;

/// The items of a long list that stay in view when folded: three, with the
/// current one in the middle, clamped to the list's ends. A finished list
/// shows its last three. Lists up to [`FOLD_ABOVE`] never fold.
#[must_use]
pub fn focus_window(items: &[Item]) -> Range<usize> {
    let total = items.len();
    if total <= FOLD_ABOVE {
        return 0..total;
    }
    let focus = Summary::of(items).headline().unwrap_or(total - 1);
    let start = focus.saturating_sub(1).min(total - FOCUS_WINDOW);
    start..start + FOCUS_WINDOW
}

/// Which side of the focus window a fold stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Earlier,
    Later,
}

/// One row of the open list, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// An item, by its index in the plan.
    Item(usize),
    /// A fold standing for `count` items; `open` when they show and the
    /// row hides them again.
    Fold {
        side: Side,
        count: usize,
        open: bool,
    },
}

/// The open list's rows. Items keep the plan's order; each fold sits at
/// the edge of the items it stands for.
#[must_use]
pub fn rows(items: &[Item], show_earlier: bool, show_later: bool) -> Vec<Row> {
    let window = focus_window(items);
    let earlier = window.start;
    let later = items.len() - window.end;
    let mut out = Vec::with_capacity(items.len() + 2);
    if earlier > 0 {
        out.push(Row::Fold {
            side: Side::Earlier,
            count: earlier,
            open: show_earlier,
        });
        if show_earlier {
            out.extend((0..earlier).map(Row::Item));
        }
    }
    out.extend(window.clone().map(Row::Item));
    if later > 0 {
        if show_later {
            out.extend((window.end..items.len()).map(Row::Item));
        }
        out.push(Row::Fold {
            side: Side::Later,
            count: later,
            open: show_later,
        });
    }
    out
}

/// What one of the panel's buttons does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Open or close the list.
    Toggle,
    /// Show or hide the folded items on one side.
    Fold(Side),
    /// Hide this plan until the engine writes a different one.
    Dismiss,
}

/// A plan's identity, so a dismissal holds only for the plan it hid.
fn signature(items: &[Item]) -> u64 {
    let mut hasher = DefaultHasher::new();
    items.hash(&mut hasher);
    hasher.finish()
}

/// One task's panel state. In memory only: it lasts while the app runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Panel {
    /// The person's choice; `None` follows the rule: open while work
    /// remains, compact once everything is done.
    expanded: Option<bool>,
    show_earlier: bool,
    show_later: bool,
    dismissed: Option<u64>,
    was_settled: bool,
}

impl Panel {
    /// Takes in whether the plan is settled: every item done and the task
    /// idle. Reaching it drops the person's choice once, so the panel
    /// tidies itself to compact; a later expand is respected.
    pub fn observe(&mut self, settled: bool) {
        if settled && !self.was_settled {
            self.expanded = None;
        }
        self.was_settled = settled;
    }

    /// Whether the list is open. A finished plan is compact by default,
    /// even while a new turn runs.
    #[must_use]
    pub fn is_expanded(&self, finished: bool) -> bool {
        self.expanded.unwrap_or(!finished)
    }

    /// Whether the person hid `items`. A dismissal covers an unfinished
    /// plan too, so one the engine abandoned does not stay forever.
    #[must_use]
    pub fn is_dismissed(&self, items: &[Item]) -> bool {
        self.dismissed == Some(signature(items))
    }

    /// Carries out `control` on the panel showing `items`.
    pub fn apply(&mut self, control: Control, items: &[Item]) {
        match control {
            Control::Toggle => {
                let finished = Summary::of(items).finished();
                self.expanded = Some(!self.is_expanded(finished));
            }
            Control::Fold(Side::Earlier) => self.show_earlier = !self.show_earlier,
            Control::Fold(Side::Later) => self.show_later = !self.show_later,
            Control::Dismiss => self.dismissed = Some(signature(items)),
        }
    }

    /// The panel for `items` under `key`, with each button's key and what
    /// it does, or `None` when the person dismissed this plan. `live`: the
    /// task is running, so the panel cannot be dismissed and a finished
    /// plan is not yet settled.
    pub fn view(
        &mut self,
        key: &str,
        items: &[Item],
        live: bool,
    ) -> Option<(Node<()>, Vec<(String, Control)>)> {
        if items.is_empty() {
            return None;
        }
        let summary = Summary::of(items);
        self.observe(summary.finished() && !live);
        if self.is_dismissed(items) {
            return None;
        }
        let expanded = self.is_expanded(summary.finished());
        let mut controls = Vec::new();
        let mut header = vec![button(
            &format!("{key}-toggle"),
            &header_label(items, &summary),
        )];
        controls.push((format!("{key}-toggle"), Control::Toggle));
        if !live {
            header.push(button(&format!("{key}-dismiss"), "Dismiss"));
            controls.push((format!("{key}-dismiss"), Control::Dismiss));
        }
        let mut children = vec![Node {
            key: format!("{key}-header"),
            style: Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Horizontal,
                children: header,
            },
        }];
        if expanded {
            for row in rows(items, self.show_earlier, self.show_later) {
                match row {
                    Row::Item(index) => children.push(item_row(key, index, &items[index])),
                    Row::Fold { side, count, open } => {
                        let word = match side {
                            Side::Earlier => "earlier",
                            Side::Later => "later",
                        };
                        let label = if open {
                            format!("Hide {count} {word}")
                        } else {
                            format!("Show {count} {word}")
                        };
                        let fold = format!("{key}-fold-{word}");
                        children.push(button(&fold, &label));
                        controls.push((fold, Control::Fold(side)));
                    }
                }
            }
        }
        let node = Node {
            key: key.into(),
            style: Style {
                background: Some(panel()),
                padding_top: Some(Space::Sm),
                padding_bottom: Some(Space::Sm),
                padding_start: Some(Space::Md),
                padding_end: Some(Space::Md),
                gap: Some(Space::Xs),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        };
        Some((node, controls))
    }
}

/// `Plan · 2/5 · Fix the parser`, or `Plan · 5/5 · All done`.
#[must_use]
pub fn header_label(items: &[Item], summary: &Summary) -> String {
    let doing = if summary.finished() {
        "All done"
    } else {
        summary
            .headline()
            .map_or("", |index| items[index].text.as_str())
    };
    let doing: String = match doing.char_indices().nth(HEADLINE_CHARS) {
        Some((at, _)) => format!("{}…", &doing[..at]),
        None => doing.to_owned(),
    };
    format!("Plan · {}/{} · {doing}", summary.done, summary.total)
}

/// The mark an item's status draws.
#[must_use]
pub fn mark(status: Status) -> &'static str {
    match status {
        Status::Pending => "○",
        Status::InProgress => "◐",
        Status::Completed => "✓",
    }
}

const HEADLINE_CHARS: usize = 80;
/// The panel fill, from the theme seam ([`crate::visual::inks`]).
fn panel() -> Color {
    crate::visual::inks().card
}

/// Receded text, from the theme seam.
fn quiet() -> Color {
    crate::visual::inks().quiet
}

fn item_row(key: &str, index: usize, item: &Item) -> Node<()> {
    let style = match item.status {
        Status::InProgress => Style {
            weight: Some(TextWeight::Bold),
            ..Style::default()
        },
        Status::Completed => Style {
            foreground: Some(quiet()),
            ..Style::default()
        },
        Status::Pending => Style::default(),
    };
    Node {
        key: format!("{key}-item-{index}"),
        style,
        element: Element::Text {
            value: format!("{} {}", mark(item.status), item.text),
            role: if item.status == Status::Completed {
                TextRole::Status
            } else {
                TextRole::Body
            },
        },
    }
}

fn button(key: &str, label: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style {
            intrinsic_width: Some(true),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            shortcut: None,
            enabled: true,
            icon: None,
            intent: (),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(statuses: &[Status]) -> Vec<Item> {
        statuses
            .iter()
            .enumerate()
            .map(|(n, status)| Item::new(format!("item {n}"), *status))
            .collect()
    }

    fn words(node: &Node<()>, out: &mut Vec<String>) {
        match &node.element {
            Element::Text { value, .. } => out.push(value.clone()),
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Stack { children, .. } => {
                for child in children {
                    words(child, out);
                }
            }
            _ => {}
        }
    }

    fn shown(panel: &mut Panel, items: &[Item], live: bool) -> Option<Vec<String>> {
        let (node, _) = panel.view("plan", items, live)?;
        let mut out = vec![];
        words(&node, &mut out);
        Some(out)
    }

    use Status::{Completed as C, InProgress as P, Pending as W};

    #[test]
    fn a_short_list_never_folds_and_a_long_one_centers_the_current_item() {
        assert_eq!(focus_window(&plan(&[C, P, W, W, W, W])), 0..6);
        let long = plan(&[C, C, C, P, W, W, W, W]);
        assert_eq!(focus_window(&long), 2..5);
        assert_eq!(focus_window(&plan(&[P, W, W, W, W, W, W])), 0..3);
        assert_eq!(focus_window(&plan(&[C, C, C, C, C, C, W])), 4..7);
        assert_eq!(focus_window(&plan(&[C; 8])), 5..8);
        assert_eq!(
            rows(&long, false, false),
            vec![
                Row::Fold {
                    side: Side::Earlier,
                    count: 2,
                    open: false
                },
                Row::Item(2),
                Row::Item(3),
                Row::Item(4),
                Row::Fold {
                    side: Side::Later,
                    count: 3,
                    open: false
                },
            ]
        );
        let open = rows(&long, true, true);
        assert_eq!(open.len(), 10);
        assert_eq!(open[1], Row::Item(0));
        assert_eq!(open[8], Row::Item(7));
    }

    #[test]
    fn the_list_is_open_while_work_remains_and_compact_once_settled() {
        let mut panel = Panel::default();
        let working = plan(&[C, P, W]);
        let text = shown(&mut panel, &working, true).unwrap();
        assert_eq!(text[0], "Plan · 1/3 · item 1");
        assert!(text.contains(&"✓ item 0".to_owned()));
        assert!(text.contains(&"◐ item 1".to_owned()));
        assert!(text.contains(&"○ item 2".to_owned()));
        assert!(!text.contains(&"Dismiss".to_owned()), "live: {text:?}");
        // The person closes it; an update keeps the choice.
        panel.apply(Control::Toggle, &working);
        let closed = shown(&mut panel, &plan(&[C, C, P]), true).unwrap();
        assert_eq!(closed, ["Plan · 2/3 · item 2"]);
        // Done and idle: the choice drops once and the panel is compact.
        panel.apply(Control::Toggle, &working);
        let done = plan(&[C, C, C]);
        let settled = shown(&mut panel, &done, false).unwrap();
        assert_eq!(settled, ["Plan · 3/3 · All done", "Dismiss"]);
        // A later expand is respected.
        panel.apply(Control::Toggle, &done);
        assert_eq!(shown(&mut panel, &done, false).unwrap().len(), 5);
    }

    #[test]
    fn folds_reveal_their_items_and_a_dismissal_holds_until_the_plan_changes() {
        let mut panel = Panel::default();
        let long = plan(&[C, C, C, P, W, W, W, W]);
        let (_, controls) = panel.view("plan", &long, true).unwrap();
        assert!(controls.contains(&("plan-fold-earlier".into(), Control::Fold(Side::Earlier))));
        assert!(controls.contains(&("plan-fold-later".into(), Control::Fold(Side::Later))));
        let text = shown(&mut panel, &long, true).unwrap();
        assert!(text.contains(&"Show 2 earlier".to_owned()), "{text:?}");
        assert!(!text.contains(&"✓ item 0".to_owned()));
        panel.apply(Control::Fold(Side::Earlier), &long);
        let text = shown(&mut panel, &long, true).unwrap();
        assert!(text.contains(&"Hide 2 earlier".to_owned()));
        assert!(text.contains(&"✓ item 0".to_owned()));
        panel.apply(Control::Dismiss, &long);
        assert!(shown(&mut panel, &long, false).is_none());
        let next = plan(&[C, C, C, C, P, W, W, W]);
        assert!(shown(&mut panel, &next, false).is_some());
        assert!(Panel::default().view("plan", &[], true).is_none());
    }
}
