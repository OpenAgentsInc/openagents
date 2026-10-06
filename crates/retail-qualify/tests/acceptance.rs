//! #10722: the integrated fake-payment acceptance run passes every case.

#[test]
fn every_case_passes_on_fakes() {
    let receipt = retail_qualify::acceptance::run();
    for case in &receipt.cases {
        assert!(case.passed, "{case:#?}");
        assert!(case.conserved, "{}", case.name);
        assert!(case.executors_started <= 1, "{}", case.name);
        assert_eq!(case.sandboxes_left_running, 0, "{}", case.name);
    }
    assert!(receipt.views_agree);
    assert!(receipt.passed);
    assert!(receipt.simulation);
    assert!(receipt.label.starts_with("SIMULATION"));
    assert_eq!(receipt.limits.computer_class, "retail-boat-large-v1");
    assert_eq!(receipt.measurements.cancel_acknowledgment_seconds, Some(2));
}
