//! The current question and separately frozen suite form one usable contract.

use gym::questions;
use gym::suite::{Partition, Suite};

#[test]
fn current_program_suite_preserves_open_history_and_reserves_new_locked_items() {
    let previous = Suite::load(include_str!("../suites/program-selection-v1.json")).unwrap();
    let current = Suite::load(include_str!("../suites/program-selection-v2.json")).unwrap();
    let set = questions::resolve(&current, None).unwrap();
    assert_eq!(set.id, "program-selection-v2");
    let criteria = set.questions["program"]["criteria"].as_object().unwrap();
    for item in &current.items {
        assert!(criteria.contains_key(&item.truth), "{}", item.id);
        assert!(item.question.is_none());
        if item.partition == Partition::Locked {
            assert!(item.id.starts_with("authored-v2/"));
            assert!(!previous.items.iter().any(|old| old.state == item.state));
        }
    }
    for item in previous
        .items
        .iter()
        .filter(|item| item.partition != Partition::Locked)
    {
        let retained = current.items.iter().find(|new| new.id == item.id).unwrap();
        assert_eq!(
            serde_json::to_value(item).unwrap(),
            serde_json::to_value(retained).unwrap()
        );
    }
    for partition in [
        Partition::Calibration,
        Partition::Development,
        Partition::Locked,
    ] {
        for option in criteria.keys() {
            assert!(
                current
                    .items
                    .iter()
                    .any(|item| item.partition == partition && &item.truth == option),
                "{partition}: {option}"
            );
        }
    }
    assert_ne!(current.digest, previous.digest);
}
