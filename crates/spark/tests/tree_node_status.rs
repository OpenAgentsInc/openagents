use spark_sdk_internal::tree::TreeNodeStatus;

#[test]
fn parent_exited_tree_status_is_supported() {
    assert_eq!(
        TreeNodeStatus::from("PARENT_EXITED"),
        TreeNodeStatus::ParentExited
    );
}

#[test]
fn unknown_tree_status_degrades_instead_of_failing() {
    assert_eq!(
        TreeNodeStatus::from("FUTURE_OPERATOR_STATUS"),
        TreeNodeStatus::Unknown
    );
}
