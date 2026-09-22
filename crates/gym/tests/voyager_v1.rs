//! The voyager suite and its question set stay loadable: the digest
//! seals the items, and every family the items name is covered by the
//! set they name.

use gym::suite::Suite;

#[test]
#[ignore = "prints the digest the file records; run on suite edits"]
fn print_voyager_digest() {
    let suite: Suite =
        serde_json::from_str(include_str!("../suites/voyager-v1.json")).expect("the suite parses");
    println!("{}", suite.compute_digest().expect("a digest"));
}

#[test]
fn the_voyager_suite_loads() {
    let suite =
        Suite::load(include_str!("../suites/voyager-v1.json")).expect("the committed suite loads");
    assert_eq!(suite.items.len(), 18);
}

#[test]
fn the_voyager_question_set_covers_its_families() {
    let set = gym::questions::load("voyager-v1").expect("the question set loads");
    let suite =
        Suite::load(include_str!("../suites/voyager-v1.json")).expect("the committed suite loads");
    for family in suite
        .items
        .iter()
        .map(|item| item.family.as_str())
        .collect::<std::collections::BTreeSet<_>>()
    {
        assert!(
            set.questions.contains_key(family),
            "family {family} has no question"
        );
    }
}
