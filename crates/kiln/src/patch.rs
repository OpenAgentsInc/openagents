//! NIP-34 patches from a Kiln workspace: `git format-patch` text built from
//! the changed files, the kind `1617` event that carries it, the
//! repository announcement (`30617`) a demo runner publishes, and the
//! runner's NIP-22 check comment.
//!
//! See `docs/kiln/workstation.md`.

use std::collections::BTreeMap;

use nostr::domain::Tag;
use serde_json::Value;
use sha1::Digest as _;
use similar::TextDiff;

/// The patch kind.
pub const PATCH_KIND: u16 = 1_617;
/// The repository announcement kind.
pub const REPO_KIND: u16 = 30_617;
/// The NIP-22 comment kind.
pub const COMMENT_KIND: u16 = 1_111;
/// NIP-34's ceiling for a patch event; larger changes are pull requests.
pub const MAX_PATCH_BYTES: usize = 60 * 1024;
/// The repository the demo announces: not the owner's `openagents`
/// announcement, which the owner's repository key publishes.
pub const DEMO_REPO_ID: &str = "openagents-kiln-demo";

/// Git's blob id: SHA-1 of `blob <len>\0<content>`.
#[must_use]
pub fn blob_id(content: &[u8]) -> String {
    let mut hash = sha1::Sha1::new();
    hash.update(format!("blob {}\0", content.len()).as_bytes());
    hash.update(content);
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// `Tue, 21 Oct 2026 17:00:00 +0000` for a Unix time.
#[must_use]
pub fn rfc2822(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let weekday = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"][days.rem_euclid(7) as usize];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{weekday}, {day} {} {year} {:02}:{:02}:{:02} +0000",
        months[(month - 1) as usize],
        secs / 3_600,
        secs % 3_600 / 60,
        secs % 60
    )
}

/// One file's change: the old content (`None` when added) and the new
/// (`None` when deleted).
pub struct Change<'a> {
    pub path: &'a str,
    pub old: Option<&'a str>,
    pub new: Option<&'a str>,
}

/// Who and what a patch says.
pub struct Header<'a> {
    pub author: &'a str,
    pub email: &'a str,
    pub subject: &'a str,
    pub body: &'a str,
    pub date: u64,
}

/// `git format-patch` text for `changes`, which `git am` and `git apply`
/// take.
#[must_use]
pub fn format_patch(header: &Header<'_>, changes: &[Change<'_>]) -> String {
    let mut stat = String::new();
    let mut diffs = String::new();
    let (mut added, mut removed) = (0usize, 0usize);
    for change in changes {
        let old = change.old.unwrap_or("");
        let new = change.new.unwrap_or("");
        let diff = TextDiff::from_lines(old, new);
        let (mut plus, mut minus) = (0, 0);
        for op in diff.iter_all_changes() {
            match op.tag() {
                similar::ChangeTag::Insert => plus += 1,
                similar::ChangeTag::Delete => minus += 1,
                similar::ChangeTag::Equal => {}
            }
        }
        added += plus;
        removed += minus;
        stat.push_str(&format!(
            " {} | {} {}{}\n",
            change.path,
            plus + minus,
            "+".repeat(plus.min(40)),
            "-".repeat(minus.min(40))
        ));
        let path = change.path;
        diffs.push_str(&format!("diff --git a/{path} b/{path}\n"));
        let short = |c: Option<&str>| match c {
            Some(c) => blob_id(c.as_bytes())[..7].to_string(),
            None => "0000000".to_string(),
        };
        match (change.old, change.new) {
            (None, _) => diffs.push_str(&format!(
                "new file mode 100644\nindex 0000000..{}\n",
                short(change.new)
            )),
            (_, None) => diffs.push_str(&format!(
                "deleted file mode 100644\nindex {}..0000000\n",
                short(change.old)
            )),
            _ => diffs.push_str(&format!(
                "index {}..{} 100644\n",
                short(change.old),
                short(change.new)
            )),
        }
        let from = if change.old.is_some() {
            format!("a/{path}")
        } else {
            "/dev/null".into()
        };
        let to = if change.new.is_some() {
            format!("b/{path}")
        } else {
            "/dev/null".into()
        };
        diffs.push_str(
            &diff
                .unified_diff()
                .context_radius(3)
                .header(&from, &to)
                .to_string(),
        );
    }
    let files = changes.len();
    stat.push_str(&format!(
        " {files} file{} changed, {added} insertion{}(+), {removed} deletion{}(-)\n",
        if files == 1 { "" } else { "s" },
        if added == 1 { "" } else { "s" },
        if removed == 1 { "" } else { "s" },
    ));
    let body = if header.body.trim().is_empty() {
        String::new()
    } else {
        format!("{}\n", header.body.trim_end())
    };
    format!(
        "From 0000000000000000000000000000000000000000 Mon Sep 17 00:00:00 2001\nFrom: {} <{}>\nDate: {}\nSubject: [PATCH] {}\n\n{body}---\n{stat}\n{diffs}-- \nkiln\n\n",
        header.author,
        header.email,
        rfc2822(header.date),
        header.subject.lines().next().unwrap_or("Kiln change"),
    )
}

/// The changes between `base` and `current` file maps.
#[must_use]
pub fn changes<'a>(
    base: &'a BTreeMap<String, String>,
    current: &'a BTreeMap<String, String>,
) -> Vec<Change<'a>> {
    let mut out = Vec::new();
    for (path, new) in current {
        match base.get(path) {
            Some(old) if old == new => {}
            old => out.push(Change {
                path,
                old: old.map(String::as_str),
                new: Some(new),
            }),
        }
    }
    for (path, old) in base {
        if !current.contains_key(path) {
            out.push(Change {
                path,
                old: Some(old),
                new: None,
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(b.path));
    out
}

/// The tags of a root `1617` patch to `repo` (`30617:<owner>:<id>`).
#[must_use]
pub fn patch_tags(repo: &str, owner: &str, base: &str, crate_name: &str) -> Vec<Tag> {
    vec![
        Tag::new(vec!["a".into(), repo.into()]),
        Tag::new(vec!["p".into(), owner.into()]),
        Tag::new(vec!["t".into(), "root".into()]),
        Tag::new(vec!["parent-commit".into(), base.into()]),
        Tag::new(vec!["t".into(), format!("crate:{crate_name}")]),
        Tag::new(vec!["alt".into(), "A git patch made in Kiln".into()]),
    ]
}

/// The tags of the demo repository announcement.
#[must_use]
pub fn announcement_tags(maintainers: &[&str], relay: &str) -> Vec<Tag> {
    let mut tags = vec![
        Tag::new(vec!["d".into(), DEMO_REPO_ID.into()]),
        Tag::new(vec![
            "name".into(),
            "OpenAgents (Kiln demo announcement)".into(),
        ]),
        Tag::new(vec![
            "description".into(),
            "Patches proposed from the Kiln tech demo. Not the owner's announcement.".into(),
        ]),
        Tag::new(vec![
            "web".into(),
            "https://github.com/OpenAgentsInc/openagents".into(),
        ]),
        Tag::new(vec![
            "clone".into(),
            "https://github.com/OpenAgentsInc/openagents.git".into(),
        ]),
        Tag::new(vec!["relays".into(), relay.into()]),
        Tag::new(vec!["t".into(), "rust".into()]),
    ];
    let mut list = vec!["maintainers".to_string()];
    list.extend(maintainers.iter().map(|m| (*m).to_string()));
    tags.push(Tag::new(list));
    tags
}

/// The tags of a runner's NIP-22 check comment on a patch.
#[must_use]
pub fn check_tags(patch_id: &str, author: &str, relay: &str, passed: bool) -> Vec<Tag> {
    vec![
        Tag::new(vec![
            "E".into(),
            patch_id.into(),
            relay.into(),
            author.into(),
        ]),
        Tag::new(vec!["K".into(), PATCH_KIND.to_string()]),
        Tag::new(vec!["P".into(), author.into()]),
        Tag::new(vec![
            "e".into(),
            patch_id.into(),
            relay.into(),
            author.into(),
        ]),
        Tag::new(vec!["k".into(), PATCH_KIND.to_string()]),
        Tag::new(vec!["p".into(), author.into()]),
        Tag::new(vec![
            "t".into(),
            if passed {
                "kiln-check:pass"
            } else {
                "kiln-check:fail"
            }
            .into(),
        ]),
    ]
}

/// Key-shaped strings a patch must not carry: a Nostr secret, common API
/// token prefixes, and PEM private keys. Returns what was found.
#[must_use]
pub fn secret_findings(text: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let checks: [(&str, &str); 6] = [
        ("nsec1", "a Nostr secret key (nsec1...)"),
        ("-----BEGIN", "a PEM block"),
        ("ghp_", "a GitHub token"),
        ("github_pat_", "a GitHub token"),
        ("sk-ant-", "an Anthropic key"),
        ("AKIA", "an AWS key id"),
    ];
    for (needle, what) in checks {
        if text
            .lines()
            .filter(|l| l.starts_with('+'))
            .any(|l| l.contains(needle))
            && !found.contains(&what)
        {
            found.push(what);
        }
    }
    found
}

/// What a check comment says, read back: pass or fail and its text.
#[must_use]
pub fn read_check(event: &nostr::domain::Event) -> Option<(bool, String)> {
    if event.kind != COMMENT_KIND {
        return None;
    }
    let passed = match event
        .tag_values("t")
        .find(|t| t.starts_with("kiln-check:"))?
    {
        "kiln-check:pass" => true,
        "kiln-check:fail" => false,
        _ => return None,
    };
    Some((passed, event.content.clone()))
}

/// The subject line of format-patch text.
#[must_use]
pub fn subject(patch: &str) -> String {
    patch
        .lines()
        .find_map(|l| l.strip_prefix("Subject: "))
        .map(|s| s.trim_start_matches("[PATCH] ").to_string())
        .unwrap_or_default()
}

/// A JSON value's string field.
#[must_use]
pub fn field(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_as_rfc_2822() {
        assert_eq!(rfc2822(0), "Thu, 1 Jan 1970 00:00:00 +0000");
        assert_eq!(rfc2822(1_792_540_800), "Wed, 21 Oct 2026 00:00:00 +0000");
        assert_eq!(
            rfc2822(1_792_627_200 + 3_725),
            "Thu, 22 Oct 2026 01:02:05 +0000"
        );
    }

    #[test]
    fn blob_ids_match_git() {
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(
            blob_id(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[test]
    fn a_patch_applies_with_git() {
        let base = BTreeMap::from([
            (
                "src/lib.rs".to_string(),
                "fn a() {}\nfn b() {}\n".to_string(),
            ),
            ("old.txt".to_string(), "gone\n".to_string()),
        ]);
        let current = BTreeMap::from([
            (
                "src/lib.rs".to_string(),
                "fn a() {}\nfn c() {}\nfn b() {}\n".to_string(),
            ),
            ("new.txt".to_string(), "fresh\n".to_string()),
        ]);
        let header = Header {
            author: "Kiln agent",
            email: "npub1x@nostr",
            subject: "Add c",
            body: "Adds c.",
            date: 1_792_627_200,
        };
        let text = format_patch(&header, &changes(&base, &current));
        assert!(text.contains("Subject: [PATCH] Add c"));
        assert!(text.contains(" 3 files changed, 2 insertions(+), 1 deletion(-)"));
        assert_eq!(subject(&text), "Add c");
        assert!(secret_findings(&text).is_empty());
        assert_eq!(
            secret_findings("+let k = \"nsec1abc\";"),
            vec!["a Nostr secret key (nsec1...)"]
        );
        // `git apply` takes the text in a scratch directory, when git exists.
        let dir = std::env::temp_dir().join(format!("kiln-patch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        for (path, content) in &base {
            std::fs::write(dir.join(path), content).unwrap();
        }
        std::fs::write(dir.join("p.patch"), &text).unwrap();
        if let Ok(out) = std::process::Command::new("git")
            .args(["apply", "p.patch"])
            .current_dir(&dir)
            .output()
        {
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            for (path, content) in &current {
                assert_eq!(&std::fs::read_to_string(dir.join(path)).unwrap(), content);
            }
            assert!(!dir.join("old.txt").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
