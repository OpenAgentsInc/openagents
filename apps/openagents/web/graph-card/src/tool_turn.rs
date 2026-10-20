//! The demo turns `/lander9` starts when a tool is dropped on the chat:
//! the person's question for that tool, one or two calls to it with the
//! graph's cues, and a short streamed answer. Made up, like the rest of
//! the chat; nothing is read or sent.

use openagents_proto::ToolCall;

use crate::scene::Node;
use crate::script::{Beat, call, mcp};

/// The question and the reply for a tool, or `None` for a part that isn't
/// a tool (the relay, the Pylon, the wallet).
pub fn turn(node: Node) -> Option<(&'static str, Vec<Beat>)> {
    Some(match node {
        Node::GitHub => (
            "What's new on GitHub since this morning?",
            vec![
                Beat::Think("I'll list the issues opened since 9:00."),
                call(
                    Node::GitHub,
                    mcp("GitHub", "github_list_issues"),
                    1_400,
                    "Reading new issues",
                    "2 new since 9:00",
                    "2 issues opened since 9:00 in example/web-app",
                ),
                Beat::Text(
                    "Two new issues since this morning (demo data):\n\n\
- #216 Upload stalls at 99% on slow networks\n\
- #217 Typo on the pricing page\n\n\
Want me to label them?",
                ),
            ],
        ),
        Node::Gmail => (
            "Find the last email from the design team.",
            vec![
                call(
                    Node::Gmail,
                    mcp("Gmail", "gmail_search"),
                    1_300,
                    "Searching mail",
                    "1 thread found",
                    "1 thread from design@example.com",
                ),
                Beat::Text(
                    "The latest is from Maya on the design team, yesterday at 4:12 PM: new icons \
for the settings page, with a link to the file. Want me to draft a reply?",
                ),
            ],
        ),
        Node::Drive => (
            "Find the Q4 roadmap in Drive and sum it up.",
            vec![
                call(
                    Node::Drive,
                    mcp("Google Drive", "drive_search"),
                    1_200,
                    "Searching your files",
                    "Found Q4 roadmap",
                    "Q4 roadmap (Google Doc)",
                ),
                call(
                    Node::Drive,
                    mcp("Google Drive", "drive_read"),
                    1_300,
                    "Reading a doc",
                    "Read Q4 roadmap",
                    "Q4 roadmap, 6 pages",
                ),
                Beat::Text(
                    "The Q4 roadmap (demo data) has three goals:\n\n\
- Ship the mobile app\n\
- Cut page load time in half\n\
- Launch team billing\n\n\
Mobile comes first, with a beta in November.",
                ),
            ],
        ),
        Node::Calendar => (
            "When am I free tomorrow afternoon?",
            vec![
                call(
                    Node::Calendar,
                    mcp("Google Calendar", "calendar_find_free_time"),
                    1_300,
                    "Finding free time",
                    "Free 2:00 to 3:30",
                    "Free tomorrow 2:00 to 3:30 PM",
                ),
                Beat::Text("You're free tomorrow from 2:00 to 3:30 PM. Want me to hold that time?"),
            ],
        ),
        Node::Web => (
            "Search the web for what's new in WebGPU.",
            vec![
                call(
                    Node::Web,
                    ToolCall::WebSearch {
                        query: "WebGPU news".into(),
                    },
                    1_300,
                    "Searching",
                    "3 results",
                    "3 results",
                ),
                Beat::Text(
                    "Three recent results (demo data): browsers keep adding WebGPU on more \
phones, there are new tools for debugging shaders, and more web apps run models on the GPU.",
                ),
            ],
        ),
        Node::Relay | Node::Pylon | Node::Wallet => return None,
    })
}

/// A tool turn's part line as a narrow page's small box shows it.
pub fn compact(line: &'static str) -> &'static str {
    match line {
        "2 new since 9:00" => "2 new",
        "Searching mail" => "Searching",
        "1 thread found" => "1 thread",
        "Found Q4 roadmap" | "Read Q4 roadmap" => "Roadmap",
        "Reading a doc" => "Reading",
        "Free 2:00 to 3:30" => "2:00",
        other => crate::script::compact(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::Cue;

    #[test]
    fn every_tool_has_a_turn_and_nothing_else_does() {
        for node in Node::ALL {
            assert_eq!(turn(node).is_some(), !node.tools().is_empty(), "{node:?}");
        }
    }

    #[test]
    fn each_turn_calls_only_its_own_tool_in_plain_words() {
        for node in Node::ALL {
            let Some((prompt, beats)) = turn(node) else {
                continue;
            };
            let mut texts = vec![prompt];
            for beat in &beats {
                match beat {
                    Beat::Think(t) | Beat::Text(t) => texts.push(t),
                    Beat::Tool {
                        call, output, cues, ..
                    } => {
                        texts.push(output);
                        if let ToolCall::Mcp { tool, .. } = call {
                            assert!(node.tools().contains(&tool.as_str()), "{tool}");
                        }
                        for (_, cue) in cues {
                            match cue {
                                Cue::Line { node: n, line, .. } => {
                                    assert_eq!(*n, node);
                                    assert!(compact(line).chars().count() <= 9, "{line}");
                                    texts.push(line);
                                }
                                Cue::Data { .. } => {}
                                Cue::Sats { .. } => panic!("a tool turn pays nothing"),
                            }
                        }
                    }
                    Beat::Ask { .. } => {}
                }
            }
            for text in texts {
                assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
            }
        }
    }
}
