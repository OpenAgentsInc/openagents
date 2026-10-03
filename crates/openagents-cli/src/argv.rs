//! Shared argument handling before command-specific parsing.

pub use coder::argv::Args;

/// Route help flags to the nearest command group with a usage handler.
/// Literal words after `--` and message text such as `chat send help` stay intact.
pub fn normalize_help(arguments: &mut Vec<String>) {
    let end = arguments
        .iter()
        .position(|word| word == "--")
        .unwrap_or(arguments.len());
    let Some(index) = arguments[..end]
        .iter()
        .position(|word| matches!(word.as_str(), "--help" | "-h"))
    else {
        // `help` is a command in group position, not arbitrary message text.
        if arguments.get(1).is_some_and(|word| word == "help")
            && matches!(arguments[0].as_str(), "task" | "xp" | "version" | "doctor")
        {
            arguments[1] = "--help".into();
        }
        return;
    };
    if index == 0 {
        arguments.truncate(1);
        return;
    }
    let group = arguments[0].as_str();
    let nested = arguments.get(1).map(String::as_str);
    let depth = match (group, nested) {
        ("x402" | "wallet", Some(command)) if index > 1 && !command.starts_with('-') => 2,
        ("plugin" | "plugins" | "ext", Some("test" | "eval" | "defaults" | "run"))
        | ("host", Some("spend"))
            if index > 1 =>
        {
            2
        }
        _ => 1,
    };
    arguments.truncate(depth);
    arguments.push("--help".into());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_routes_before_value_parsing() {
        for (input, expected) in [
            (vec!["chat", "work", "--help"], vec!["chat", "--help"]),
            (vec!["chat", "send", "text", "-h"], vec!["chat", "--help"]),
            (
                vec!["plugin", "test", "run", "--help"],
                vec!["plugin", "test", "--help"],
            ),
            (
                vec!["chat", "send", "--", "--help"],
                vec!["chat", "send", "--", "--help"],
            ),
            (vec!["chat", "send", "help"], vec!["chat", "send", "help"]),
        ] {
            let mut words = input.into_iter().map(str::to_owned).collect();
            normalize_help(&mut words);
            assert_eq!(words, expected);
        }
    }
}

/// A command's existing syntax and description, without the other commands.
pub fn command_usage(group: &str, command: &str, usage: &str) -> Option<String> {
    let prefix = format!("  {command}");
    let lines: Vec<_> = usage.lines().collect();
    let start = lines.iter().position(|line| {
        line.strip_prefix(&prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })?;
    let end = (start + 1..lines.len())
        .find(|&i| !lines[i].starts_with("    "))
        .unwrap_or(lines.len());
    Some(format!(
        "usage: openagents {group} {}",
        lines[start..end].join("\n").trim_start()
    ))
}

/// Parse only the options declared by this command, before looking for values.
pub fn scoped_args(
    words: &[String],
    command: &str,
    options: &[&str],
    switches: &[&str],
    min: usize,
    max: usize,
) -> Result<Args, String> {
    let mut index = 0;
    while index < words.len() {
        let word = &words[index];
        if word == "--" {
            break;
        }
        if let Some(flag) = word.strip_prefix("--") {
            let name = flag.split('=').next().unwrap_or(flag);
            if !options.contains(&name) && !switches.contains(&name) {
                return Err(format!("--{name} isn't an option of {command}"));
            }
            if options.contains(&name) && !flag.contains('=') {
                if words
                    .get(index + 1)
                    .is_none_or(|value| value.starts_with("--"))
                {
                    return Err(format!("--{name} needs a value"));
                }
                index += 1;
            }
        }
        index += 1;
    }
    let args = Args::parse(words, switches)?;
    if args.positional().len() > max {
        return Err(format!(
            "unexpected argument `{}` for {command}",
            args.positional()[max]
        ));
    }
    if args.positional().len() < min {
        return Err(format!("{command} needs an argument"));
    }
    Ok(args)
}
