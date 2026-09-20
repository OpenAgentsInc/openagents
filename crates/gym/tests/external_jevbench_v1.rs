//! `external-jevbench-v1` is JevBench's public tier vendored whole: the
//! suite loads, its question set covers every item by item id, and every
//! label sits inside the option set its own question offers.

use std::collections::BTreeSet;

use gym::questions;
use gym::suite::{Partition, Suite};

const SUITE: &str = include_str!("../suites/external-jevbench-v1.json");

#[test]
fn external_jevbench_v1_loads_and_asks_every_item_by_id() {
    let suite = Suite::load(SUITE).unwrap();
    assert_eq!(suite.name, "external-jevbench-v1");
    assert_eq!(suite.items.len(), 231);
    assert_eq!(suite.questions.as_deref(), Some("external-jevbench-v1"));
    // The items and labels are frozen upstream; the digest is the evidence
    // the vendored copy is still what JevBench froze.
    assert_eq!(
        suite.digest,
        "b75cab74ca3489ae04d9189ceb6408e1ee1de409522fce9981d665fe8536b519"
    );

    let set = questions::resolve(&suite, None).unwrap();
    assert_eq!(set.id, "external-jevbench-v1");
    assert_eq!(set.questions.len(), 231);
    assert!(set.covers(&suite));

    let mut families = BTreeSet::new();
    for item in &suite.items {
        families.insert(item.family.as_str());
        assert!(item.question.is_none(), "{}", item.id);
        assert_eq!(item.evidence().label(), "jevbench");
        let question = set.ask(item).unwrap();
        // No two items share an option set, so the set answers by item id.
        assert_eq!(question, &set.questions[&item.id], "{}", item.id);
        match item.kind.as_str() {
            "noul" => assert!(matches!(item.truth.as_str(), "yes" | "no"), "{}", item.id),
            "choice" => assert!(
                question["criteria"]
                    .as_object()
                    .unwrap()
                    .contains_key(&item.truth),
                "{}",
                item.id
            ),
            "score" => {
                let levels = question["criteria"].as_array().unwrap();
                let index: usize = item.truth.parse().unwrap();
                assert!(index < levels.len(), "{}", item.id);
            }
            other => panic!("{other}"),
        }
    }
    assert_eq!(families.len(), 20);

    // Every family reaches all three partitions, so a calibration map is
    // fittable per family and the locked slice is suite-wide.
    for family in &families {
        for partition in Partition::ALL {
            assert!(
                suite
                    .items
                    .iter()
                    .any(|item| item.family == *family && item.partition == partition),
                "{family}: {partition}"
            );
        }
    }
}
