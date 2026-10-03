//! `openagents plugin new` and `openagents plugin pin`: start a plugin
//! without writing its record by hand, and keep the digests its record
//! states equal to its files (#10306).
//!
//! `new SLUG` writes a skill plugin: `package.json`, `skills/SLUG.md`, and
//! a README naming the next commands. `new SLUG --from-rule ID` packages a
//! background rule made on this computer (`openagents background add`) as
//! a plugin, pinned. `pin [DIR]` rewrites every digest `package.json`
//! states to the digest its file has now.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{Args, Output};

/// `openagents plugin new|pin …`; `None` for any other command.
pub fn run(output: &Output, words: &[String]) -> Option<u8> {
    let (command, rest) = words.split_first()?;
    if command != "new" && command != "pin" {
        return None;
    }
    let name = format!("plugin {command}");
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return Some(output.usage(&name, &message, crate::catalog::EXT_USAGE)),
    };
    let result = if command == "new" {
        new(&args)
    } else {
        pin(Path::new(
            args.positional().first().map_or(".", String::as_str),
        ))
    };
    Some(match result {
        Ok(value) => {
            output.emit(&value, |value| {
                value["text"].as_str().unwrap_or_default().to_owned()
            });
            0
        }
        Err(message) => output.fail(&name, &message),
    })
}

fn new(args: &Args) -> Result<Value, String> {
    let Some(slug) = args.positional().first() else {
        return Err(
            "new needs the plugin's short name, like `openagents plugin new changelog-nudge`"
                .into(),
        );
    };
    if !is_slug(slug) {
        return Err(format!(
            "`{slug}` can't be a plugin's short name: use lowercase letters, digits, and dashes"
        ));
    }
    let dir = args
        .option("in")
        .map_or_else(|| PathBuf::from(slug), PathBuf::from);
    if dir.join("package.json").exists() {
        return Err(format!("{} already holds a plugin", dir.display()));
    }
    let name = args
        .option("name")
        .map_or_else(|| title(slug), str::to_owned);
    if let Some(rule) = args.option("from-rule") {
        let layout = background::Layout::from_env().map_err(|error| error.to_string())?;
        let made = background::plugins::package(&layout, rule, &dir)?;
        return Ok(json!({
            "text": format!(
                "Made {} from the background rule {rule}.\nNext:\n  openagents plugin install {shown}\n  openagents plugin enable {slug}\n  openagents plugin publish {shown}",
                made.display(),
                shown = made.display(),
                slug = rule,
            ),
            "dir": made.display().to_string(),
        }));
    }
    scaffold(&dir, slug, &name)?;
    Ok(json!({
        "text": format!(
            "Made the plugin {name} in {shown}: package.json, skills/{slug}.md, and a README.\n\
             Write what Coder should do in skills/{slug}.md, then:\n  \
             cd {shown}\n  \
             openagents plugin test init     # write its tests with us\n  \
             openagents plugin install .     # use it on this computer\n  \
             openagents plugin publish .     # share it",
            shown = dir.display(),
        ),
        "dir": dir.display().to_string(),
    }))
}

/// Writes a skill plugin named `name` into `dir`.
fn scaffold(dir: &Path, slug: &str, name: &str) -> Result<(), String> {
    let write = |path: PathBuf, text: String| -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))
    };
    let record = json!({
        "v": 1,
        "slug": slug,
        "name": name,
        "summary": format!("{name}: what it helps Coder do, in one sentence."),
        "version": "0.1.0",
        "publisher": "",
        "provenance": "local file",
    });
    let mut text = serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?;
    text.push('\n');
    write(dir.join("package.json"), text)?;
    write(
        dir.join("skills").join(format!("{slug}.md")),
        format!(
            "# {name}\n\n\
             Say here what Coder should do when this plugin is on: when it applies,\n\
             what to do, and what to leave alone. Coder reads this file on every run\n\
             while the plugin is on.\n"
        ),
    )?;
    write(
        dir.join("README.md"),
        format!(
            "# {name}\n\n\
             A skill plugin for OpenAgents: `skills/{slug}.md` is the guidance Coder\n\
             follows while it is on.\n\n\
             ```sh\n\
             openagents plugin test init     # write its tests\n\
             openagents plugin test run .    # run them with it and without it\n\
             openagents plugin install .     # use it on this computer\n\
             openagents plugin publish .     # share it\n\
             ```\n"
        ),
    )
}

fn pin(dir: &Path) -> Result<Value, String> {
    let changed = coder::package::repin(dir)?;
    let text = if changed.is_empty() {
        format!(
            "Every digest in {} already matches its file.",
            dir.join("package.json").display()
        )
    } else {
        format!(
            "Updated the digests in {} for {}.",
            dir.join("package.json").display(),
            changed.join(", ")
        )
    };
    Ok(json!({"text": text, "updated": changed}))
}

/// The sentence a command adds when it updated `dir`'s digests first.
pub(crate) fn repinned_note(dir: &Path) -> Result<Option<String>, String> {
    let changed = coder::package::repin(dir)?;
    Ok((!changed.is_empty()).then(|| {
        format!(
            "Updated the digests in package.json for {}.",
            changed.join(", ")
        )
    }))
}

fn is_slug(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && !text.starts_with('-')
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `changelog-nudge` as `Changelog nudge`.
fn title(slug: &str) -> String {
    let words = slug.replace('-', " ");
    let mut chars = words.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_plugin_resolves_and_installs_without_editing_anything() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("changelog-nudge");
        scaffold(&dir, "changelog-nudge", "Changelog nudge").unwrap();
        let package = coder::package::Package::load(&dir.join("package.json")).unwrap();
        coder::package::Package::resolve(&dir, &package).unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        crate::plugin_local::install_into(&layout, &dir).unwrap();
    }

    #[test]
    fn pin_updates_a_stale_digest_and_then_has_nothing_to_do() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("p");
        std::fs::create_dir_all(dir.join("background")).unwrap();
        std::fs::write(dir.join("background/r.json"), "{\"id\": \"r\"}").unwrap();
        std::fs::write(
            dir.join("package.json"),
            "{\"v\": 1, \"slug\": \"p\", \"background\": [{\"name\": \"r\", \"digest\": \"0\"}]}",
        )
        .unwrap();
        let value = pin(&dir).unwrap();
        assert_eq!(value["updated"], json!(["background/r"]));
        let record: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).unwrap())
                .unwrap();
        assert_eq!(
            record["background"][0]["digest"],
            json!(coder::package::digest("{\"id\": \"r\"}"))
        );
        assert_eq!(pin(&dir).unwrap()["updated"], json!([]));
    }

    #[test]
    fn short_names_are_checked() {
        assert!(is_slug("changelog-nudge"));
        assert!(!is_slug("Changelog"));
        assert!(!is_slug("-x"));
        assert_eq!(title("changelog-nudge"), "Changelog nudge");
    }
}
