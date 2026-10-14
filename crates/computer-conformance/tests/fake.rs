//! The suite against the in-memory driver, in CI: every profile declared,
//! and a Core-only driver whose undeclared operations must be refused.

use computer_conformance::{Options, run};
use computer_driver::Profile;
use computer_driver::fake::FakeDriver;

#[tokio::test]
async fn the_fake_driver_passes_every_profile() {
    let dir = tempfile::tempdir().unwrap();
    let driver = FakeDriver::new(dir.path()).boot_observations(2);
    let report = run(&driver, &Options::fast("conf-all")).await;
    assert!(report.passed(), "{}", report.summary());
    assert_eq!(report.declared.len(), 4);
    assert!(driver.live().is_empty(), "left {:?}", driver.live());
}

#[tokio::test]
async fn a_core_only_driver_refuses_the_rest_without_changing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let driver = FakeDriver::with_profiles(dir.path(), [Profile::Core]);
    let report = run(&driver, &Options::fast("conf-core")).await;
    assert!(report.passed(), "{}", report.summary());
    let refusals = report
        .cases
        .iter()
        .filter(|c| c.name == "undeclared is refused")
        .count();
    assert_eq!(refusals, 3);
}

#[tokio::test]
async fn a_driver_that_lies_about_a_profile_fails_its_case() {
    // Declares Fork, but every fork request is lost: the suite must not
    // pass it.
    let dir = tempfile::tempdir().unwrap();
    let driver = FakeDriver::new(dir.path());
    driver.inject("fork", computer_driver::fake::Inject::Fail);
    let report = run(&driver, &Options::fast("conf-lie")).await;
    assert!(!report.passed());
    let failed: Vec<_> = report.cases.iter().filter(|c| c.result.is_err()).collect();
    assert_eq!(failed.len(), 1, "{}", report.summary());
    assert_eq!(failed[0].profile, Profile::Fork);
}
