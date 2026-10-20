//! The demo chat's script: a person asks the agent to triage today's new
//! GitHub issues and email the team a summary. The agent reads the issues,
//! checks the team's triage guide in Drive, searches the web about a crash,
//! pays a Pylon 21 sats over Lightning to label the issues, finds time for a
//! call, drafts the summary in Gmail, and asks before sending it.
//!
//! Each step also says what the graph shows while it runs: data moving
//! along a link, sats moving from the wallet, and the line on each part.
//! Everything here is made up; nothing is read, paid, or sent.

use openagents_proto::ToolCall;

use crate::scene::{End, Node};

/// The person's message.
pub const PROMPT: &str = "Triage today's new GitHub issues and email the team a summary.";

/// The wallet's balance when the chat starts, in sats.
pub const BALANCE: u64 = 50_000;
/// What one decision costs, in sats.
pub const PRICE: u64 = 21;

/// Something the graph shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Cue {
    /// Data moves from one end to the other.
    Data { from: End, to: End },
    /// Sats move from the wallet.
    Sats { to: Node, amount: u64 },
    /// A part's line changes; `busy` while it works.
    Line {
        node: Node,
        line: &'static str,
        busy: bool,
    },
}

/// One step of the reply.
#[derive(Clone, Debug, PartialEq)]
pub enum Beat {
    Think(&'static str),
    /// A tool call that runs for `run_ms`, with what the graph shows at
    /// each moment after it starts. The output shows when it finishes.
    Tool {
        call: ToolCall,
        run_ms: u64,
        output: &'static str,
        cues: Vec<(u64, Cue)>,
    },
    /// Markdown, streamed word by word.
    Text(&'static str),
    /// The agent waits for the person's yes.
    Ask {
        title: &'static str,
        line: &'static str,
    },
}

fn mcp(server: &str, tool: &str) -> ToolCall {
    ToolCall::Mcp {
        server: server.into(),
        tool: tool.into(),
        input: None,
    }
}

const TRAVEL: u64 = crate::graph::TRAVEL_MS;

/// A call to one part: data out, the part busy, data back, and its new line.
fn call(
    node: Node,
    call: ToolCall,
    run_ms: u64,
    busy: &'static str,
    done: &'static str,
    output: &'static str,
) -> Beat {
    let back = run_ms.saturating_sub(TRAVEL);
    Beat::Tool {
        call,
        run_ms,
        output,
        cues: vec![
            (
                0,
                Cue::Data {
                    from: End::Chat,
                    to: End::Node(node),
                },
            ),
            (
                TRAVEL / 2,
                Cue::Line {
                    node,
                    line: busy,
                    busy: true,
                },
            ),
            (
                back,
                Cue::Data {
                    from: End::Node(node),
                    to: End::Chat,
                },
            ),
            (
                back,
                Cue::Line {
                    node,
                    line: done,
                    busy: false,
                },
            ),
        ],
    }
}

const TRIAGE: &str = "Here's today's triage:

| Issue | Label |
| --- | --- |
| #212 Safari login | bug, high |
| #213 Blank on wake | bug, high |
| #214 CSV export | question |
| #215 Dark mode | feature, low |";

const DRAFT: &str = "I drafted the summary in Gmail for team@example.com, with the two bugs first, the web results on #213, and a 30-minute triage call tomorrow at 10:30.";

pub fn reply() -> Vec<Beat> {
    let decide = 4_200;
    vec![
        Beat::Think(
            "I'll read the new issues, check the team's triage guide, and look into the crash before I label anything.",
        ),
        call(
            Node::GitHub,
            mcp("GitHub", "github_list_issues"),
            1_600,
            "Reading new issues",
            "Read 4 new issues",
            "4 issues opened today in example/web-app",
        ),
        call(
            Node::Drive,
            mcp("Google Drive", "drive_search"),
            1_500,
            "Searching your files",
            "Read Triage guide",
            "Triage guide (Google Doc)",
        ),
        call(
            Node::Web,
            ToolCall::WebSearch {
                query: "Safari blank page after sleep".into(),
            },
            1_500,
            "Searching",
            "3 results",
            "3 results",
        ),
        Beat::Tool {
            call: mcp("Pylon", "classify_issues"),
            run_ms: decide,
            output: "4 labels, paid 21 sats (demo)",
            cues: vec![
                (
                    0,
                    Cue::Data {
                        from: End::Chat,
                        to: End::Node(Node::Relay),
                    },
                ),
                (
                    TRAVEL / 2,
                    Cue::Line {
                        node: Node::Relay,
                        line: "Carrying a job",
                        busy: true,
                    },
                ),
                (
                    TRAVEL,
                    Cue::Data {
                        from: End::Node(Node::Relay),
                        to: End::Node(Node::Pylon),
                    },
                ),
                (
                    TRAVEL + 200,
                    Cue::Data {
                        from: End::Chat,
                        to: End::Node(Node::Wallet),
                    },
                ),
                (
                    TRAVEL + TRAVEL / 2,
                    Cue::Line {
                        node: Node::Pylon,
                        line: "Asks 21 sats",
                        busy: true,
                    },
                ),
                (
                    2 * TRAVEL + 200,
                    Cue::Sats {
                        to: Node::Pylon,
                        amount: PRICE,
                    },
                ),
                (
                    2 * TRAVEL + 200,
                    Cue::Line {
                        node: Node::Wallet,
                        line: "Paying 21 sats",
                        busy: true,
                    },
                ),
                (
                    3 * TRAVEL + 200,
                    Cue::Line {
                        node: Node::Wallet,
                        line: "Paid 21 sats, demo receipt",
                        busy: false,
                    },
                ),
                (
                    3 * TRAVEL + 200,
                    Cue::Line {
                        node: Node::Pylon,
                        line: "Labeling 4 issues",
                        busy: true,
                    },
                ),
                (
                    decide - 2 * TRAVEL,
                    Cue::Line {
                        node: Node::Pylon,
                        line: "Labeled 4 issues, 21 sats",
                        busy: false,
                    },
                ),
                (
                    decide - 2 * TRAVEL,
                    Cue::Data {
                        from: End::Node(Node::Pylon),
                        to: End::Node(Node::Relay),
                    },
                ),
                (
                    decide - TRAVEL,
                    Cue::Data {
                        from: End::Node(Node::Relay),
                        to: End::Chat,
                    },
                ),
                (
                    decide - TRAVEL,
                    Cue::Line {
                        node: Node::Relay,
                        line: "Carried 1 job",
                        busy: false,
                    },
                ),
            ],
        },
        Beat::Text(TRIAGE),
        call(
            Node::Calendar,
            mcp("Google Calendar", "calendar_find_free_time"),
            1_500,
            "Finding free time",
            "Free tomorrow 10:30",
            "Free tomorrow 10:30 to 11:00",
        ),
        call(
            Node::Gmail,
            mcp("Gmail", "gmail_compose"),
            1_600,
            "Writing a draft",
            "Draft waits for you",
            "Draft saved to team@example.com",
        ),
        Beat::Text(DRAFT),
        Beat::Ask {
            title: "Send the summary?",
            line: "To team@example.com. Nothing is sent until you say yes.",
        },
    ]
}

/// A part's line as a narrow page's small box shows it.
pub fn compact(line: &'static str) -> &'static str {
    match line {
        "Reading new issues" => "Reading",
        "Read 4 new issues" => "4 issues",
        "Searching your files" | "Searching" => "Searching",
        "Read Triage guide" => "Guide",
        "Carrying a job" => "Carrying",
        "Carried 1 job" => "1 job",
        "Asks 21 sats" => "21 sats",
        "Labeling 4 issues" => "Labeling",
        "Labeled 4 issues, 21 sats" => "4 labels",
        "Finding free time" => "Looking",
        "Free tomorrow 10:30" => "10:30",
        "Writing a draft" => "Writing",
        "Draft waits for you" => "Drafted",
        "relay.openagents.com" => "Nostr",
        "Decision model" => "Model",
        other => other,
    }
}

/// Every part's line and the balance once the whole reply has played, for
/// readers who ask for less motion.
pub fn settled() -> (Vec<(Node, &'static str)>, u64) {
    let mut lines = Vec::new();
    let mut balance = BALANCE;
    for beat in reply() {
        if let Beat::Tool { cues, .. } = beat {
            for (_, cue) in cues {
                match cue {
                    Cue::Line { node, line, .. } => {
                        lines.retain(|(n, _)| *n != node);
                        lines.push((node, line));
                    }
                    Cue::Sats { amount, .. } => balance -= amount,
                    Cue::Data { .. } => {}
                }
            }
        }
    }
    (lines, balance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene;

    #[test]
    fn every_cue_moves_along_a_real_link_in_time() {
        for beat in reply() {
            let Beat::Tool { cues, run_ms, .. } = beat else {
                continue;
            };
            let mut last = 0;
            for (at, cue) in &cues {
                assert!(*at >= last, "cues in order");
                last = *at;
                assert!(*at + TRAVEL <= run_ms + TRAVEL, "cue after the call ends");
                match cue {
                    Cue::Data { from, to } => {
                        assert!(scene::link(*from, *to).is_some(), "{from:?} {to:?}")
                    }
                    Cue::Sats { to, .. } => {
                        assert!(scene::link(End::Node(Node::Wallet), End::Node(*to)).is_some())
                    }
                    Cue::Line { .. } => {}
                }
            }
        }
    }

    #[test]
    fn the_wallet_pays_once_and_each_call_ends_idle() {
        let (lines, balance) = settled();
        assert_eq!(balance, BALANCE - PRICE);
        for (node, line) in lines {
            assert!(!line.is_empty(), "{node:?}");
        }
        for beat in reply() {
            if let Beat::Tool { cues, .. } = beat {
                for node in scene::Node::ALL {
                    let last = cues.iter().rev().find_map(|(_, c)| match c {
                        Cue::Line { node: n, busy, .. } if *n == node => Some(*busy),
                        _ => None,
                    });
                    assert_ne!(last, Some(true), "{node:?} is left busy");
                }
            }
        }
    }

    #[test]
    fn every_line_fits_a_small_box() {
        let mut lines: Vec<&'static str> = scene::Node::ALL.iter().map(|n| n.idle()).collect();
        for beat in reply() {
            if let Beat::Tool { cues, .. } = beat {
                for (_, cue) in cues {
                    if let Cue::Line { line, node, .. } = cue
                        && node != Node::Wallet
                    {
                        lines.push(line);
                    }
                }
            }
        }
        for line in lines {
            assert!(compact(line).chars().count() <= 9, "{line}");
        }
    }

    /// Every prefix the chat streams, word by word, parses (as the chat
    /// views parse a streaming part) to text with no raw table pipes or
    /// bold markers, so no frame of the loop shows Markdown syntax.
    #[test]
    fn no_streamed_frame_shows_raw_markdown() {
        // The check sees raw syntax when a parser leaves it in.
        let raw = format!("{:?}", openagents_markdown::parse_full("| Issue | Label"));
        assert!(raw.contains("text: \"| Issue"), "{raw}");
        for beat in reply() {
            let Beat::Text(body) = beat else { continue };
            let mut shown = String::new();
            for word in crate::split_words(body) {
                shown.push_str(&word);
                let tree = format!("{:?}", openagents_markdown::parse_streaming(&shown));
                for text in tree.split("text: \"").skip(1) {
                    let text = text.split('"').next().unwrap_or_default();
                    assert!(
                        !text.contains('|') && !text.contains("**"),
                        "{text:?} after {shown:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_tools_are_the_catalog_s() {
        for beat in reply() {
            if let Beat::Tool {
                call: ToolCall::Mcp { tool, server, .. },
                ..
            } = beat
            {
                if server == "Pylon" {
                    continue;
                }
                assert!(
                    Node::ALL.iter().any(|n| n.tools().contains(&tool.as_str())),
                    "{tool}"
                );
            }
        }
    }

    #[test]
    fn the_copy_reads_as_plain_words_and_says_demo() {
        let mut texts = vec![PROMPT];
        for beat in reply() {
            match beat {
                Beat::Think(t) | Beat::Text(t) => texts.push(t),
                Beat::Tool { output, cues, .. } => {
                    texts.push(output);
                    for (_, cue) in cues {
                        if let Cue::Line { line, .. } = cue {
                            texts.push(line);
                        }
                    }
                }
                Beat::Ask { title, line } => texts.extend([title, line]),
            }
        }
        for text in &texts {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
        // Every payment the chat names says it is a demo.
        for text in texts
            .iter()
            .filter(|t| t.contains("paid") || t.contains("Paid"))
        {
            assert!(text.contains("demo"), "{text}");
        }
    }
}
