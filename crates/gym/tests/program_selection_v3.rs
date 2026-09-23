//! The five-option question and its frozen suite form one usable contract.

use gym::questions;
use gym::suite::{Partition, Suite};

#[test]
fn the_five_option_suite_keeps_every_v2_item_and_covers_review_runs() {
    let previous = Suite::load(include_str!("../suites/program-selection-v2.json")).unwrap();
    let current = Suite::load(include_str!("../suites/program-selection-v3.json")).unwrap();
    let set = questions::resolve(&current, None).unwrap();
    assert_eq!(set.id, "program-selection-v3");
    let criteria = set.questions["program"]["criteria"].as_object().unwrap();
    assert!(criteria.contains_key("review-runs"));
    for item in &current.items {
        assert!(criteria.contains_key(&item.truth), "{}", item.id);
        assert!(item.question.is_none());
    }
    // Every v2 item is kept word for word, its label and partition too.
    for item in &previous.items {
        let retained = current.items.iter().find(|new| new.id == item.id).unwrap();
        assert_eq!(
            serde_json::to_value(item).unwrap(),
            serde_json::to_value(retained).unwrap()
        );
    }
    // Every partition asks for review-runs and for none among the new items.
    for partition in [
        Partition::Calibration,
        Partition::Development,
        Partition::Locked,
    ] {
        for truth in ["review-runs", "none"] {
            assert!(
                current
                    .items
                    .iter()
                    .any(|item| item.id.starts_with("authored-v3/")
                        && item.partition == partition
                        && item.truth == truth),
                "{partition}: {truth}"
            );
        }
    }
    assert_ne!(current.digest, previous.digest);
}
