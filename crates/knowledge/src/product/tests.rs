use super::*;

/// A product entry with `cites` and `answer`, parsed from its file.
fn entry(id: &str, kind: &str, cites: &[&str], answer: &str) -> Entry {
    let cites: String = cites.iter().map(|c| format!("\n    - {c}")).collect();
    let answer = if answer.is_empty() {
        String::new()
    } else {
        format!("answer: >-\n  {answer}\n")
    };
    Entry::parse(&format!(
        "---\nid: {id}\nversion: 1\nkind: {kind}\ntitle: T\nsummary: S.\ntags: [x]\napplies_when: >-\n  The user asks.\n{answer}status: admitted\nauthor: openagents\nprovenance:\n  written_from: [reference]\n  cites:{cites}\nevidence: []\n---\n\n## Details\n\nWe document this.\n"
    ))
    .expect("the fixture parses")
}

fn problems(entry: &Entry) -> Vec<String> {
    check(std::slice::from_ref(entry), Some(&repository()))
        .into_iter()
        .map(|p| p.message)
        .collect()
}

#[test]
fn the_committed_corpus_is_sourced_public_and_plural() {
    let corpus = Corpus::load(&default_dir(), Some(&repository())).expect("the corpus loads");
    assert!(
        corpus.base.entries.len() >= 40,
        "{}",
        corpus.base.entries.len()
    );
    for entry in &corpus.base.entries {
        assert_eq!(entry.kind, Kind::Product, "{}", entry.id);
        assert!(!entry.cites.is_empty(), "{}", entry.id);
        let answer = entry.answer.as_deref().expect("every entry answers");
        assert!(answer.chars().count() <= MAX_ANSWER_CHARS, "{}", entry.id);
    }
    assert!(corpus.tag().starts_with("openagents-product@"));
    assert_eq!(corpus.tag().len(), "openagents-product@".len() + 12);
}

#[test]
fn a_product_entry_keeps_its_answer_through_render() {
    let e = entry(
        "openagents.a",
        "product",
        &["README.md"],
        "We are open source.",
    );
    assert_eq!(e.kind, Kind::Product);
    assert_eq!(e.answer.as_deref(), Some("We are open source."));
    let again = Entry::parse(&e.render()).expect("the render parses");
    assert_eq!(again.answer, e.answer);
    assert_eq!(again.kind, Kind::Product);
}

#[test]
fn a_well_formed_entry_has_no_problems() {
    let e = entry(
        "openagents.a",
        "product",
        &["README.md", "docs/breez/amounts.md#the-rule"],
        "We show amounts in BIP 177 form.",
    );
    assert!(problems(&e).is_empty(), "{:?}", problems(&e));
}

#[test]
fn an_entry_must_be_a_product_under_the_prefix() {
    let e = entry("other.a", "method", &["README.md"], "");
    let found = problems(&e);
    assert!(found.iter().any(|m| m.contains("not product")), "{found:?}");
    assert!(found.iter().any(|m| m.contains("openagents.")), "{found:?}");
}

#[test]
fn every_cite_is_an_existing_public_repository_path() {
    for (cite, why) in [
        ("https://example.com/doc", "not a repository path"),
        ("/etc/passwd", "relative"),
        ("~/work/alpha/plan.md", "relative"),
        ("../alpha/plan.md", "leaves the repository"),
        ("alpha/ROADMAP.md", "private"),
        (".secrets/key.env", "private"),
        ("docs/no-such-file.md", "names no file"),
    ] {
        let e = entry("openagents.a", "product", &[cite], "");
        let found = problems(&e);
        assert!(found.iter().any(|m| m.contains(why)), "{cite}: {found:?}");
    }
    let uncited = entry("openagents.a", "product", &[], "");
    assert!(
        problems(&uncited)
            .iter()
            .any(|m| m.contains("cites no source"))
    );
}

#[test]
fn answers_speak_in_the_plural_and_stay_short() {
    for answer in [
        "I can help with that.",
        "Ask me anything.",
        "That's my job.",
        "I’ll check.",
    ] {
        let e = entry("openagents.a", "product", &["README.md"], answer);
        assert!(
            problems(&e)
                .iter()
                .any(|m| m.contains("first person singular")),
            "{answer}"
        );
    }
    let fine = entry(
        "openagents.a",
        "product",
        &["README.md"],
        "We run on iOS; see `my-file` and **My reports** for details.",
    );
    assert!(problems(&fine).is_empty(), "{:?}", problems(&fine));
    let long = entry(
        "openagents.a",
        "product",
        &["README.md"],
        &"We. ".repeat(200),
    );
    assert!(problems(&long).iter().any(|m| m.contains("characters")));
}

#[test]
fn the_corpus_keeps_admitted_entries_and_its_digest_follows_them() {
    let a = entry("openagents.a", "product", &["README.md"], "We.");
    let b = entry("openagents.b", "product", &["README.md"], "We.");
    let mut withdrawn = entry("openagents.c", "product", &["README.md"], "We.");
    withdrawn.status = Status::Withdrawn;
    let both = Corpus::of(vec![b.clone(), a.clone(), withdrawn]);
    assert_eq!(
        both.base
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["openagents.a", "openagents.b"]
    );
    let one = Corpus::of(vec![a]);
    assert_ne!(both.digest, one.digest);
    assert_eq!(
        Corpus::of(vec![b.clone()]).digest,
        Corpus::of(vec![b]).digest
    );
}

#[test]
fn instructions_hold_only_the_kept_entries_and_the_citation_rule() {
    let e = entry(
        "openagents.a",
        "product",
        &["README.md"],
        "We are open source.",
    );
    let text = instructions(&[Reference::of(&e)]);
    assert!(text.contains("<entry id=\"openagents.a\""));
    assert!(text.contains("We document this."));
    assert!(text.contains("[openagents.wallet-send]"));
    assert!(text.contains("don't have that documented yet"));
    assert_eq!(instructions(&[]), NO_DOCUMENTED_ANSWER);
}

/// A summary of our essays is given from their overview entries, which
/// carry each essay's link, and the model is told to pass the link on
/// (#10102).
#[test]
fn an_essay_summary_is_grounded_with_each_essays_link() {
    let corpus = Corpus::load(&default_dir(), Some(&repository())).expect("the corpus loads");
    let overview = |id: &str| {
        corpus
            .base
            .entries
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let both = [
        Reference::of(overview("openagents.ttc-overview")),
        Reference::of(overview("openagents.gen-overview")),
    ];
    let text = instructions(&both);
    assert!(text.contains("give its link from the entry"));
    for essay in [
        "docs/essays/2026-09-29-test-time-capabilities.md",
        "docs/essays/2026-10-01-the-return-of-the-general-agent.md",
    ] {
        assert!(
            text.contains(&format!(
                "https://github.com/OpenAgentsInc/openagents/blob/main/{essay}"
            )),
            "{essay}"
        );
    }
}

#[test]
fn a_reply_cites_only_the_entries_it_was_given() {
    let a = Reference::of(&entry("openagents.a", "product", &["README.md"], ""));
    let b = Reference::of(&entry("openagents.b", "product", &["README.md"], ""));
    assert_eq!(a.id, "openagents.a@1");
    let cited = check_reply(
        "We do [openagents.a]. Also [openagents.b@1, openagents.z] and [1] [not.ours] [openagents.a].",
        &[a.clone(), b],
    );
    assert_eq!(cited.known, ["openagents.a", "openagents.b"]);
    assert_eq!(cited.unknown, ["openagents.z"]);
    assert!(!cited.grounded());
    assert!(check_reply("[openagents.a]", std::slice::from_ref(&a)).grounded());
    assert!(!check_reply("No citation here.", &[a]).grounded());
}

#[test]
fn download_guidance_matches_the_published_apps() {
    let corpus = Corpus::load(&default_dir(), Some(&repository())).expect("the corpus loads");
    let download_source = "crates/openagents-web/src/pages/download.rs";
    // The page offers Coder and the command-line program, with one-line
    // installers; its desktop downloads stay hidden until the desktop
    // release is out.
    let page = std::fs::read_to_string(repository().join(download_source)).expect("the page");
    assert!(page.contains("install.sh"));
    assert!(
        page.contains("DESKTOP_RELEASED: bool = false"),
        "the download page offers the desktop apps now: update these notes"
    );
    for (id, minimum_version, claims) in [
        (
            "openagents.get-the-app",
            8,
            &[
                "openagents.com/download",
                "macOS, Linux, and Windows",
                "https://testflight.apple.com/join/dvQdns5B",
                "from source",
            ][..],
        ),
        ("openagents.overview", 4, &["openagents.com/download"][..]),
        (
            "openagents.playtesting",
            5,
            &[
                "openagents.com/download",
                "macOS, Linux, and Windows",
                "https://testflight.apple.com/join/dvQdns5B",
                "from source",
            ][..],
        ),
        (
            "openagents.install-coder",
            6,
            &["openagents.com/download", "install.sh", "install.ps1"][..],
        ),
    ] {
        let entry = corpus
            .base
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .expect("the download entry exists");
        assert!(
            entry.version >= minimum_version,
            "{id} must bump its version."
        );
        assert!(entry.cites.iter().any(|cite| cite == download_source));
        for text in [entry.answer.as_deref().unwrap(), &entry.body] {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            for claim in claims {
                assert!(
                    text.contains(claim),
                    "{id} must include the following download guidance:\n{claim}"
                );
            }
            for outdated in [
                // The iPhone app ships on TestFlight, never as a source
                // build.
                "Build the iPhone",
                "The iPhone, Android",
                "signed APK",
                "Android app is still in testing",
                ".dmg",
                "Mac 1.0.0-rc.2",
                "Terminal 1.0.0-rc.2",
                // The engine Coder runs with is part of Coder, never a
                // separate download.
                "microcoder",
                "Microcoder",
            ] {
                assert!(
                    !text.contains(outdated),
                    "{id} must not include the following outdated guidance:\n{outdated}"
                );
            }
        }
    }
}

#[test]
fn desktop_grid_guidance_keeps_the_world_off_chat() {
    let corpus = Corpus::load(&default_dir(), Some(&repository())).expect("the corpus loads");
    let entry = corpus
        .base
        .entries
        .iter()
        .find(|entry| entry.id == "openagents.verse-grid")
        .expect("the Verse entry exists");
    assert!(entry.version >= 2);
    assert!(
        entry
            .cites
            .iter()
            .any(|cite| cite == "crates/openagents-desktop/src/grid.rs")
    );
    for claim in [
        "sidebar footer beside the Local profile and Settings",
        "only on the desktop's Verse page and the deck's title slide",
        "not behind chat or other screens",
        "leaving it closes the relay connection",
        "releases the world's GPU resources",
        "Grid, Watch, Play, and Reduce motion",
    ] {
        assert!(
            entry.body.contains(claim),
            "missing desktop guidance: {claim}"
        );
    }
}

/// Advice to do something carries the exact page or the one command to
/// run (the owner's rule of 2026-10-09); a description is not advice.
#[test]
fn an_instruction_carries_its_link_or_command() {
    for text in [
        "Open OpenAgents for Mac and it shows a QR code.",
        "To keep your chats, sign in with GitHub at the top right.",
        "In the Wallet, choose Send.",
        "Get Coder at openagents.com/download and run it.",
        "We can help. Then run the installer.",
        "Install it from the menu.",
    ] {
        assert!(unlinked_instruction(text).is_some(), "{text}");
    }
    for text in [
        "Install Coder: `curl -fsSL https://openagents.com/cli/install.sh | bash`.",
        "Sign in at https://openagents.com/login.",
        "Coder runs on your own computer, with its own git login.",
        "When you add a result to the Gym, other trainers check it.",
        "The Wallet opens to your balance; Send and Receive are under it.",
        "We can't run code, browse the web, or open your files from here.",
    ] {
        assert_eq!(unlinked_instruction(text), None, "{text}");
    }
    // An unclosed backtick is not a command.
    assert!(unlinked_instruction("Run the ` thing.").is_some());
}

#[test]
fn an_unlinked_instruction_is_a_problem_unless_in_app() {
    let e = entry(
        "openagents.a",
        "product",
        &["README.md"],
        "Open Settings and choose Delete.",
    );
    assert!(
        problems(&e)
            .iter()
            .any(|p| p.contains("tells the reader to act")),
        "{:?}",
        problems(&e)
    );
    let mut tagged = e.clone();
    tagged.tags.push(IN_APP_TAG.into());
    assert!(problems(&tagged).is_empty(), "{:?}", problems(&tagged));
}
