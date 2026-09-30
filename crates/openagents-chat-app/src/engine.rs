//! Read-only engine and usage strip.
//!
//! Reimplemented from Zeron's account usage rings (public MIT zeronsh/zeron):
//! a read-only ring per route. The computer owns the settings; this strip
//! cannot change them or read a credential.

use openagents_connect::control::{EngineReport, RouteUsage};
use rust_native::style::{Color, Space, Style};
use rust_native::view::{Axis, Element, Node, TextRole};

const TEXT: Color = crate::visual::TEXT;
const MUTED: Color = crate::visual::MUTED;

/// The header strip for `report`. Route cards when the computer has routes;
/// otherwise the Codex and Claude Code sign-in lines and no rings.
#[must_use]
pub fn strip<I>(report: &EngineReport) -> Node<I> {
    let mut children = Vec::new();
    if report.routes.is_empty() {
        children.push(line(
            "engine-empty",
            "No engine is set up on this computer.",
            TextRole::Status,
            MUTED,
        ));
        for account in report.accounts.iter().take(4) {
            let provider = id_piece(&account.provider);
            children.push(stack(
                &format!("engine-account-{provider}"),
                Axis::Vertical,
                Space::None,
                vec![
                    line(
                        &format!("engine-account-{provider}-name"),
                        &account.name,
                        TextRole::Body,
                        TEXT,
                    ),
                    line(
                        &format!("engine-account-{provider}-signin"),
                        sign_in(account.signed_in),
                        TextRole::Status,
                        MUTED,
                    ),
                ],
            ));
        }
    } else {
        let mut cards = Vec::new();
        for (index, route) in report.routes.iter().take(8).enumerate() {
            cards.push(route_card(index, route));
        }
        children.push(stack("engine-routes", Axis::Wrap, Space::Md, cards));
    }
    stack("engine-strip", Axis::Vertical, Space::Sm, children)
}

fn route_card<I>(index: usize, route: &openagents_connect::control::EngineRoute) -> Node<I> {
    let lines = vec![
        line(
            &format!("engine-route-{index}-name"),
            &route.name,
            TextRole::Body,
            TEXT,
        ),
        line(
            &format!("engine-route-{index}-model"),
            &route.model,
            TextRole::Status,
            MUTED,
        ),
        line(
            &format!("engine-route-{index}-signin"),
            sign_in(route.signed_in),
            TextRole::Status,
            MUTED,
        ),
        line(
            &format!("engine-route-{index}-usage"),
            &usage_sentence(&route.usage),
            TextRole::Status,
            MUTED,
        ),
    ];
    let mut row = Vec::new();
    if let RouteUsage::Windows { used_percent, .. } = route.usage {
        let provider = id_piece(&route.provider);
        row.push(node(
            &format!("engine-ring-{index}"),
            Element::Surface {
                resource: format!("engine-ring:{provider}:{used_percent}"),
                label: format!("{} usage {used_percent} percent", route.name),
            },
        ));
    }
    row.push(stack(
        &format!("engine-route-{index}-text"),
        Axis::Vertical,
        Space::None,
        lines,
    ));
    stack(
        &format!("engine-route-{index}"),
        Axis::Horizontal,
        Space::Sm,
        row,
    )
}

/// The sentence for one route's usage. Fixed words, never provider text.
#[must_use]
pub fn usage_sentence(usage: &RouteUsage) -> String {
    match usage {
        RouteUsage::Off => "Usage limits aren't being read.".into(),
        RouteUsage::Unsupported => "Usage limits aren't available.".into(),
        RouteUsage::Unknown { reason } => reason_sentence(reason).into(),
        RouteUsage::Windows {
            windows,
            limit_reached,
            used_percent,
        } => {
            let mut parts: Vec<String> = windows
                .iter()
                .take(4)
                .map(|window| {
                    let mut text = format!("{} {}%", window.label, window.used_percent);
                    if let Some(resets) = &window.resets {
                        text.push_str(" until ");
                        text.push_str(resets);
                    }
                    text
                })
                .collect();
            if *limit_reached {
                parts.push("Limit reached".into());
            }
            if parts.is_empty() {
                format!("{used_percent}%")
            } else {
                parts.join(", ")
            }
        }
    }
}

fn reason_sentence(reason: &str) -> &'static str {
    match reason {
        "not_probed" => "Reading usage limits…",
        "no_credential" => "No sign-in to read",
        "expired" => "The sign-in has expired",
        "unauthorized" => "The sign-in was refused",
        "rate_limited" | "status" | "malformed" => "Usage limits are temporarily unavailable",
        "network" => "Usage limits can't be reached right now",
        _ => "Usage unavailable",
    }
}

fn sign_in(signed_in: bool) -> &'static str {
    if signed_in {
        "Signed in"
    } else {
        "Not signed in"
    }
}

fn id_piece(value: &str) -> String {
    let piece: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(24)
        .collect();
    if piece.is_empty() {
        "route".into()
    } else {
        piece
    }
}

fn node<I>(key: &str, element: Element<I>) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn stack<I>(key: &str, axis: Axis, gap: Space, children: Vec<Node<I>>) -> Node<I> {
    let mut node = node(key, Element::Stack { axis, children });
    node.style.gap = Some(gap);
    node
}

fn line<I>(key: &str, value: &str, role: TextRole, color: Color) -> Node<I> {
    let mut node = node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    );
    node.style.foreground = Some(color);
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_connect::control::{EngineAccount, EngineRoute, UsageWindow};

    fn window(label: &str, percent: u8) -> UsageWindow {
        UsageWindow {
            name: "primary".into(),
            label: label.into(),
            used_percent: percent,
            resets_at: Some(1_791_050_824),
            resets: Some("2026-10-03 18:07 UTC".into()),
        }
    }

    fn report() -> EngineReport {
        EngineReport {
            enabled: true,
            adapter: "microcoder-repository".into(),
            model: "gpt-6-luna".into(),
            routes: vec![
                EngineRoute {
                    provider: "codex".into(),
                    name: "Codex".into(),
                    model: "gpt-6-luna".into(),
                    signed_in: true,
                    usage: RouteUsage::Windows {
                        windows: vec![window("Primary", 100)],
                        limit_reached: true,
                        used_percent: 100,
                    },
                },
                EngineRoute {
                    provider: "claude".into(),
                    name: "Claude Code".into(),
                    model: "claude-opus-5-5".into(),
                    signed_in: false,
                    usage: RouteUsage::Windows {
                        windows: vec![
                            UsageWindow {
                                name: "five_hour".into(),
                                label: "5 hours".into(),
                                used_percent: 4,
                                resets_at: None,
                                resets: None,
                            },
                            window("7 days", 66),
                        ],
                        limit_reached: false,
                        used_percent: 66,
                    },
                },
            ],
            accounts: vec![
                EngineAccount {
                    provider: "codex".into(),
                    name: "Codex".into(),
                    signed_in: true,
                },
                EngineAccount {
                    provider: "claude".into(),
                    name: "Claude Code".into(),
                    signed_in: false,
                },
            ],
            usage_probe: Some(90),
            refresh_due: false,
        }
    }

    fn texts<I>(node: &Node<I>) -> Vec<String> {
        let mut out = Vec::new();
        walk(node, &mut |node| {
            if let Element::Text { value, .. } | Element::Surface { label: value, .. } =
                &node.element
            {
                out.push(value.clone());
            }
        });
        out
    }

    fn buttons<I>(node: &Node<I>) -> usize {
        let mut count = 0;
        walk(node, &mut |node| {
            if matches!(node.element, Element::Button { .. }) {
                count += 1;
            }
        });
        count
    }

    fn resources<I>(node: &Node<I>) -> Vec<String> {
        let mut out = Vec::new();
        walk(node, &mut |node| {
            if let Element::Surface { resource, .. } = &node.element {
                out.push(resource.clone());
            }
        });
        out
    }

    fn walk<I>(node: &Node<I>, visit: &mut dyn FnMut(&Node<I>)) {
        visit(node);
        if let Element::Stack { children, .. } = &node.element {
            for child in children {
                walk(child, visit);
            }
        }
    }

    #[test]
    fn route_cards_show_the_model_and_usage_and_cannot_change_them() {
        let view = strip::<()>(&report());
        let words = texts(&view).join("\n");
        assert!(words.contains("Codex"));
        assert!(words.contains("gpt-6-luna"));
        assert!(words.contains("Claude Code"));
        assert!(words.contains("claude-opus-5-5"));
        assert!(words.contains("Signed in"));
        assert!(words.contains("Not signed in"));
        assert!(words.contains("Primary 100%"));
        assert!(words.contains("Limit reached"));
        assert!(words.contains("5 hours 4%"));
        assert!(words.contains("7 days 66%"));
        assert!(!words.contains("microcoder"));
        assert!(!words.contains("access_token"));
        assert_eq!(buttons(&view), 0);
        assert_eq!(
            resources(&view),
            ["engine-ring:codex:100", "engine-ring:claude:66"]
        );
        rust_native::View::new("engine", 1, view)
            .validate()
            .expect("valid");
    }

    #[test]
    fn no_routes_show_sign_in_lines_and_no_ring() {
        let mut report = report();
        report.routes.clear();
        report.adapter.clear();
        let view = strip::<()>(&report);
        let words = texts(&view).join("\n");
        assert!(words.contains("No engine is set up on this computer."));
        assert!(words.contains("Codex"));
        assert!(words.contains("Claude Code"));
        assert!(resources(&view).is_empty());
        assert_eq!(buttons(&view), 0);
    }

    #[test]
    fn usage_sentences_stay_inside_a_closed_set() {
        assert_eq!(
            usage_sentence(&RouteUsage::Off),
            "Usage limits aren't being read."
        );
        assert_eq!(
            usage_sentence(&RouteUsage::Unknown {
                reason: "not_probed".into()
            }),
            "Reading usage limits…"
        );
        assert_eq!(
            usage_sentence(&RouteUsage::Unknown {
                reason: "no_credential".into()
            }),
            "No sign-in to read"
        );
        assert_eq!(
            usage_sentence(&RouteUsage::Unknown {
                reason: "expired".into()
            }),
            "The sign-in has expired"
        );
        assert_eq!(
            usage_sentence(&RouteUsage::Unknown {
                reason: "unauthorized".into()
            }),
            "The sign-in was refused"
        );
        assert_eq!(
            usage_sentence(&RouteUsage::Unknown {
                reason: "network".into()
            }),
            "Usage limits can't be reached right now"
        );
        assert_eq!(
            usage_sentence(&RouteUsage::Unknown {
                reason: "provider said no".into()
            }),
            "Usage unavailable"
        );
    }
}
