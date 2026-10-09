//! BYO-05 through the real confirm path: the account's stored Claude key is
//! released over the native channel for exactly the turn its effect starts,
//! and never lands in the effect journal, the resident, or the job.

use super::*;
use crate::cloud::byo::{Computers, Owner};
use crate::cloud::custody::{Key, Material};
use sha2::{Digest, Sha256};

const FAKE_KEY: &str = "sk-ant-api03-fake-byo05-web-release-for-tests";

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether any file under `root`, outside the custody vault, holds `needle`.
fn anywhere(root: &std::path::Path, vault: &std::path::Path, needle: &[u8]) -> bool {
    std::fs::read_dir(root).unwrap().any(|entry| {
        let path = entry.unwrap().path();
        if path == vault {
            false
        } else if path.is_dir() {
            anywhere(&path, vault, needle)
        } else {
            std::fs::read(&path)
                .unwrap_or_default()
                .windows(needle.len())
                .any(|w| w == needle)
        }
    })
}

async fn submit_and_finish(fixture: &Fixture, cookies: &Cookies) -> coder_cloud::Record {
    let (id, _) = preview(fixture, cookies).await;
    let (confirm, input) = confirm_form(fixture, cookies, &id).await;
    expect(
        &post(fixture, cookies, &confirm, &input).await,
        StatusCode::OK,
    );
    finished(fixture, &id).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_users_stored_key_is_released_to_their_claude_turn_and_stays_out_of_every_record() {
    let mut fixture = fixture().await;
    let private = directory(&fixture);
    std::fs::write(private.join("operator-executor"), "claude").unwrap();
    let vault = private.join("byo");
    std::fs::create_dir(&vault).unwrap();
    std::fs::set_permissions(&vault, std::fs::Permissions::from_mode(0o700)).unwrap();
    let computers = Arc::new(Computers::open(&vault).unwrap());
    let alice = Owner {
        account: "alice".into(),
        workspace: "alice-personal".into(),
        members_epoch: 3,
    };
    computers
        .store(
            &alice,
            Material::AnthropicApiKey,
            Key::for_material(Material::AnthropicApiKey, FAKE_KEY.into()).unwrap(),
            true,
            now(),
        )
        .unwrap();
    fixture.config.cloud_byo = Some(computers.clone());
    let native = resident_with_services(
        &mut fixture,
        coder_access::Rights::new([coder_access::Right::Observe, coder_access::Right::Operate])
            .unwrap(),
        true,
        false,
        true,
    )
    .await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    enroll(&fixture, &cookies).await;

    let record = submit_and_finish(&fixture, &cookies).await;
    assert_eq!(
        record.binding["claude"]["evidence"]["credential"],
        "anthropic_api_key"
    );
    let turns = |fixture: &Fixture| {
        std::fs::read_to_string(directory(fixture).join(operator_fixture::RELEASED_TURNS))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(turns(&fixture), [hex(FAKE_KEY.as_bytes())]);
    // The turn's end disarms the job: the release served one turn.
    for _ in 0..500 {
        if !coder_cloud::release::armed(&record.id) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!coder_cloud::release::armed(&record.id));

    // Removal: the next job runs on the plan login made in the computer.
    assert!(computers.revoke(&alice).unwrap());
    let record = submit_and_finish(&fixture, &cookies).await;
    assert_eq!(
        record.binding["claude"]["evidence"]["credential"],
        "claude_plan_login"
    );
    assert_eq!(turns(&fixture)[1], "none");

    // The effect journal, the resident's access book, the operator's jobs,
    // admissions, and request journals never held the key.
    assert!(!anywhere(&private, &vault, FAKE_KEY.as_bytes()));
    native.stop().await;
}
