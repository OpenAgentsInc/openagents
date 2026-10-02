//! The background rules' unknown-folder judgment suite (#10158) stays
//! loadable: the digest seals the items, and both families the items name
//! have a question in the set they name.

use gym::suite::Suite;

const SUITE: &str = include_str!("../suites/background-cache-dir-v1.json");

#[test]
fn the_cache_dir_suite_loads_with_both_families_and_every_partition() {
    let suite = Suite::load(SUITE).expect("the committed suite loads");
    assert_eq!(suite.items.len(), 48);
    let set = gym::questions::load("background-cache-dir-v1").expect("the question set loads");
    for family in ["cache", "kind"] {
        assert!(set.questions.contains_key(family), "{family}");
    }
    for partition in gym::suite::Partition::ALL {
        assert!(suite.items.iter().any(|item| item.partition == partition));
    }
}
