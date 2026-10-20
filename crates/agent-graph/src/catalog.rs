//! What the OpenAgents app gives an agent that runs as one of its chats:
//! its tools, the commands an agent may run, and the places those read.
//!
//! A graph lists a tool, command or place only while the agent's own text
//! names it ([`named`]), so the graph changes when the definition does.
//! `/agent/foreman`'s server page reads the same tables.

/// The app's tools, grouped, in the order a check-in uses them.
pub const TOOLS: [(&str, &[&str]); 4] = [
    (
        "Chats",
        &[
            "list_devices",
            "list_chats",
            "read_chat",
            "get_chat",
            "changed_paths",
            "send_message",
            "interrupt_chat",
            "create_chat",
            "create_chats",
            "wait_for_turn",
        ],
    ),
    ("Issue work", &["work_issues", "land_status", "list_runs"]),
    (
        "This computer",
        &["machine_status", "machine_action", "upgrade_host"],
    ),
    ("Memory", &["record_decision"]),
];

/// Commands an agent runs itself in its shell.
pub const COMMANDS: [&str; 5] = [
    "gh",
    "openagents land submit",
    "openagents issue release",
    "openagents agents revise",
    "openagents agents bind",
];

/// The machine scopes `machine_action` and `upgrade_host` run under.
pub const MACHINE_SCOPES: [&str; 3] = ["build.prune", "process.stop", "host.upgrade"];

/// Places an agent looks, what it finds there, and the tools or commands
/// that read each one.
pub const SOURCES: [(&str, &str, &[&str]); 5] = [
    (
        "GitHub",
        "Open and recently closed issues, claim comments, blockers, commits, and the two boards.",
        &["gh"],
    ),
    (
        "The app",
        "Every device and chat: its status, last activity, and what it changed.",
        &["list_devices", "list_chats", "read_chat", "changed_paths"],
    ),
    (
        "The landing queue",
        "Finished changes waiting to land on main, and the ones that bounced.",
        &["land_status"],
    ),
    (
        "The cloud pool",
        "Issue work running on cloud computers, its events, and what it spends.",
        &["work_issues"],
    ),
    (
        "This computer",
        "Disk, capacity, the landing queue's health, and what waits on the owner.",
        &["machine_status"],
    ),
];

/// Whether an agent's text names `name` in code formatting.
pub fn named(text: &str, name: &str) -> bool {
    text.contains(&format!("`{name}`")) || text.contains(&format!("`{name} "))
}

/// The app tools an agent's text names, by group, groups with none left out.
pub fn tools(text: &str) -> Vec<(&'static str, Vec<&'static str>)> {
    TOOLS
        .iter()
        .map(|(group, names)| {
            (
                *group,
                names.iter().copied().filter(|n| named(text, n)).collect(),
            )
        })
        .filter(|(_, names): &(&str, Vec<&str>)| !names.is_empty())
        .collect()
}

/// The commands an agent's text names.
pub fn commands(text: &str) -> Vec<&'static str> {
    COMMANDS
        .iter()
        .copied()
        .filter(|c| named(text, c))
        .collect()
}

/// The places an agent reads, with the tools or commands its text names for
/// each; a place none of them reaches is left out.
pub fn sources(text: &str) -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    SOURCES
        .iter()
        .map(|(place, what, via)| {
            (
                *place,
                *what,
                via.iter().copied().filter(|v| named(text, v)).collect(),
            )
        })
        .filter(|(_, _, via): &(&str, &str, Vec<&str>)| !via.is_empty())
        .collect()
}

/// A playbook skill's name in plain words: "Dispatching issue work" reads
/// "Handing out issue work".
pub fn plain(name: &str) -> String {
    const WORDS: [(&str, &str); 1] = [("dispatching", "handing out")];
    name.split(' ')
        .enumerate()
        .map(|(at, word)| {
            let lower = word.to_lowercase();
            let swap = WORDS
                .iter()
                .find(|(from, _)| *from == lower)
                .map(|(_, to)| (*to).to_owned());
            match (swap, at) {
                (Some(to), 0) => capitalized(&to),
                (Some(to), _) => to,
                (None, _) => word.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` with its first letter in capitals.
pub fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A line of an agent's Markdown as plain text: `**` and backticks dropped.
pub fn plain_line(text: &str) -> String {
    text.replace("**", "").replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_counts_only_in_code_formatting() {
        assert!(named("call `read_chat` first", "read_chat"));
        assert!(named("run `gh issue list`", "gh"));
        assert!(!named("read_chat without code", "read_chat"));
    }

    #[test]
    fn plain_names_swap_the_one_internal_word() {
        assert_eq!(plain("Dispatching issue work"), "Handing out issue work");
        assert_eq!(plain("Where I look"), "Where I look");
    }
}
