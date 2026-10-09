//! The machine-talk guard for this site's pages in tests (#11031).
//!
//! Every page a test renders can be checked with [`assert_plain`]: the
//! visible text (tags, scripts, styles and attribute values removed) must
//! carry no `oa_copy` term or phrase. A legitimate use goes in [`allowed`]
//! for its route only, with a one-line reason.

/// Terms a route may show, each with its reason. Keep this tiny.
fn allowed(route: &str) -> &'static [&'static str] {
    match route {
        // The legal documents keep their reviewed legal wording; changing
        // it is a legal decision, not a copy fix.
        "/terms" => &["projection", "custody"],
        "/privacy" => &["projection", "retained"],
        // The component catalog's icon gallery names the "Cursor" icon.
        "/ui" => &["cursor"],
        _ => &[],
    }
}

/// Fails the test, naming the route, each term and its context, when the
/// visible text of `html` has machine talk.
pub(crate) fn assert_plain(route: &str, html: &str) {
    let text = oa_copy::visible_text(&without_preformatted(html));
    let path = route.split('?').next().unwrap_or(route);
    let hits = oa_copy::violations(&text, allowed(path));
    assert!(
        hits.is_empty(),
        "machine talk on {route}: {}",
        hits.iter()
            .map(|hit| format!("{:?} in \"{}\"", hit.term, hit.context))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

/// `html` without its `<pre>` blocks: raw records, terminal output, and
/// commands to type are shown verbatim (a JSON key or a `--digest` flag),
/// not written as copy.
fn without_preformatted(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</pre>") {
            Some(end) => rest = &rest[start + end + "</pre>".len()..],
            None => rest = "",
        }
    }
    out.push_str(rest);
    out
}

/// Files whose strings no visitor reads, each with its reason.
const SKIP: &[&str] = &[
    // The chat store's errors go to the server log; a page shows its own
    // plain message instead (`pages/chat.rs`).
    "chat_store.rs",
    // No route shows the key vault's or the host list's errors or terms.
    "cloud/byo.rs",
    "cloud/custody.rs",
    "cloud/hosts.rs",
    // The server's command line, for whoever runs it.
    "main.rs",
    "upstream.rs",
    // The archived Coder-pilot pages, kept for the record and never served.
    "pilot/archived.rs",
];

/// Lexicon terms a file may carry, each with its reason.
const ALLOW_IN: &[(&str, &[&str])] = &[
    // The purchase page shows the exact command to run, `--digest` and all.
    ("purchases.rs", &["digest"]),
    // `?cursor=` is the query of the next-steps link, not words.
    ("tasks.rs", &["cursor"]),
];

/// #11031: no string in this site's sources that reads like words, and no
/// line of its scripts, carries machine talk. Rendered pages are checked
/// too ([`assert_plain`]); this catches copy on pages no test renders.
#[test]
fn site_sources_have_no_machine_talk() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hits = oa_copy::scan_dir_allowing(&root.join("src"), SKIP, &[], ALLOW_IN);
    for script in [
        "chat.js",
        "chat-start.js",
        "components-start.js",
        "flow.js",
        "everglade.js",
    ] {
        let text = std::fs::read_to_string(root.join("static").join(script)).expect("the script");
        for (index, line) in text.lines().enumerate() {
            // `cursor` is a CSS property in scripts.
            for v in oa_copy::violations(line, &["cursor"]) {
                hits.push(format!(
                    "{script}:{}: {:?} in {:?}",
                    index + 1,
                    v.term,
                    v.context
                ));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "machine talk in the site's copy (rewrite it in plain words, see AGENTS.md):\n{}",
        hits.join("\n")
    );
}
