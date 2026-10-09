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
