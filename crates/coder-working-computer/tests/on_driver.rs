//! The chat lifecycle on the driver contract (#11624): a computer is
//! created on first use, a finished turn checkpoints by stopping, the next
//! prompt resumes the same computer with its files, and a driver without
//! Suspend is refused before anything is written.

use coder_working_computer::driver::{Driver, Settled};
use coder_working_computer::on_driver::OnDriver;
use coder_working_computer::store::Store;
use coder_working_computer::*;
use computer_driver::fake::{FakeDriver, Inject};
use computer_driver::{Driver as _, Exec, Profile, State};
use std::time::Duration;

fn spec() -> Spec {
    Spec {
        id: "computer-1".into(),
        owner: Principal {
            workspace: "ws-1".into(),
            principal: "user-1".into(),
        },
        chat: "chat-1".into(),
        project: coder_environment::ProjectLink {
            workspace: "ws-1".into(),
            project: "proj-1".into(),
        },
        source: coder_environment::SourcePin {
            repository: Some("example/repo".into()),
            revision: "a".repeat(40),
            digest: "b".repeat(64),
        },
        base: None,
        size: "small".into(),
        credential_names: Default::default(),
        services: vec![],
        bounds: Bounds {
            idle_ms: 600_000,
            observed_extension_ms: 300_000,
            absolute_ms: 7_200_000,
        },
    }
}

fn fast<D: computer_driver::Driver>(d: D) -> OnDriver<D> {
    let mut p = OnDriver::new(d, None);
    p.poll = Duration::from_millis(10);
    p.deadline = Duration::from_secs(10);
    p
}

#[tokio::test]
async fn a_chat_computer_keeps_its_files_between_turns() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path().join("machines")).boot_observations(2);
    let store = Store::under(dir.path().join("store"));
    store.create(&Computer::new(spec(), 1).unwrap()).unwrap();
    let driver = Driver::new(store.clone(), fast(fake.clone()));

    let Settled::Dispatch {
        generation,
        computer,
    } = driver.prompt("computer-1", 10).await.unwrap()
    else {
        panic!("first prompt did not dispatch");
    };
    assert_eq!(generation, 1);
    let resource = computer.resource().unwrap().to_owned();
    let out = fake
        .exec(&resource, &Exec::new("echo turn-one > work.txt"))
        .await;
    assert!(out.is_done());

    // The finished turn checkpoints by stopping the computer.
    let settled = driver.turn_finished("computer-1", 1, 20).await.unwrap();
    assert!(
        matches!(settled.computer().phase, Phase::Stopped),
        "{settled:?}"
    );
    assert_eq!(fake.live(), vec![(resource.clone(), State::Stopped)]);

    // The next prompt resumes the same computer with the file.
    let Settled::Dispatch {
        generation,
        computer,
    } = driver.prompt("computer-1", 30).await.unwrap()
    else {
        panic!("second prompt did not dispatch");
    };
    assert_eq!(generation, 2);
    assert_eq!(computer.resource(), Some(resource.as_str()));
    let read = fake.exec(&resource, &Exec::new("cat work.txt")).await;
    assert_eq!(read.value().unwrap().stdout, "turn-one\n");

    // Delete settles to Deleted and the machine is gone.
    driver.turn_finished("computer-1", 2, 40).await.unwrap();
    let gone = driver.delete("computer-1", 50).await.unwrap();
    assert!(matches!(gone.computer().phase, Phase::Deleted), "{gone:?}");
    assert!(fake.live().is_empty());
}

#[tokio::test]
async fn a_lost_create_reply_reconciles_to_one_machine() {
    let dir = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(dir.path().join("machines"));
    fake.inject("create", Inject::LoseReply);
    let store = Store::under(dir.path().join("store"));
    store.create(&Computer::new(spec(), 1).unwrap()).unwrap();
    let driver = Driver::new(store, fast(fake.clone()));
    // The first settle sees Unknown and stops there or repeats the same
    // operation identity; either way one machine exists.
    let mut settled = driver.prompt("computer-1", 10).await.unwrap();
    if !matches!(settled, Settled::Dispatch { .. }) {
        settled = driver.prompt("computer-1", 11).await.unwrap();
    }
    assert!(matches!(settled, Settled::Dispatch { .. }), "{settled:?}");
    assert_eq!(fake.live().len(), 1);
}

#[test]
fn a_driver_without_suspend_is_refused_up_front() {
    let dir = tempfile::tempdir().unwrap();
    let core = FakeDriver::with_profiles(dir.path(), [Profile::Core]);
    let refused = OnDriver::admit(&core).unwrap_err();
    assert_eq!(refused.profile, Profile::Suspend);
    let full = FakeDriver::new(dir.path());
    assert!(OnDriver::admit(&full).is_ok());
}
