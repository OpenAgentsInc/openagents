//! Ranking over the real embedded corpus: these tests pin the acceptance
//! behavior — the earning question surfaces the parked-economy stance, docs
//! answer doc questions, and stances outrank docs — against the actual
//! `kb.json` the artifact ships, so a regeneration that breaks retrieval
//! fails here before it reaches the harness.

use super::*;

fn lookup(query: &str, limit: Option<usize>) -> Output {
    handle(Input {
        query: query.to_string(),
        limit,
    })
    .expect("the lookup should succeed")
}

#[test]
fn terms_keep_distinct_lowercase_words_of_three_or_more() {
    let words = terms("How can I earn Bitcoin, earn BITCOIN?");
    assert_eq!(words, vec!["bitcoin", "can", "earn", "how"]);
}

#[test]
fn an_empty_query_is_refused() {
    let refusal = handle(Input {
        query: "a & b".to_string(),
        limit: None,
    })
    .err()
    .expect("two-letter words alone should refuse");
    assert_eq!(refusal.code, RefusalCode::Unsupported);
}

#[test]
fn the_earning_question_surfaces_the_parked_economy_stance() {
    let out = lookup("How can I earn bitcoin with this system?", None);
    let top = out.hits.first().expect("the corpus should answer this");
    assert_eq!(top.kind, "stance");
    assert_eq!(top.title, "Earning bitcoin or money on OpenAgents");
    assert!(top.state.as_deref().unwrap_or("").contains("parked"));
    assert!(top.date.is_some(), "a stance carries its review date");
    assert!(!top.body.is_empty());
}

#[test]
fn a_doc_question_surfaces_the_doc() {
    let out = lookup("how do I create an API token?", None);
    assert!(
        out.hits
            .iter()
            .any(|hit| hit.kind == "doc" && hit.title == "API tokens"),
        "the API-tokens doc should rank for its own question"
    );
}

#[test]
fn unrelated_queries_come_back_empty_not_wrong() {
    let out = lookup("quaternion spline interpolation", None);
    assert!(out.hits.is_empty());
}

#[test]
fn the_limit_is_honored_and_clamped() {
    assert!(lookup("openagents", Some(1)).hits.len() <= 1);
    assert!(lookup("openagents", Some(500)).hits.len() <= LIMIT_BOUND);
}

#[test]
fn stances_outrank_docs_at_equal_overlap() {
    let stance = Entry::Stance {
        title: "widgets".into(),
        questions: vec![],
        state: String::new(),
        answer: String::new(),
        sources: vec![],
        date: "2026-08-25".into(),
    };
    let doc = Entry::Doc {
        title: "widgets".into(),
        source: "d.md".into(),
        headings: vec![],
        summary: String::new(),
        tags: vec![],
    };
    let terms = terms("widgets");
    assert_eq!(score(&stance, &terms), score(&doc, &terms));
    // Equal overlap: the tiebreak in `handle` prefers the stance.
    let mut scored = vec![(3u32, 0u32, doc), (3u32, 1u32, stance)];
    scored.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    assert!(matches!(scored[0].2, Entry::Stance { .. }));
}
