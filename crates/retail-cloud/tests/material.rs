//! #10712: only admitted source and the customer's own key reach a sandbox.

mod common;

use common::{NOW, confirmed, funded_account, ready, request, rights};
use pay_ledger::Ledger;
use retail_cloud::Error;
use retail_cloud::fake::{FakeProvider, FakeSandbox};
use retail_cloud::journal::Journal;
use retail_cloud::material::{
    self, Credential, CustomerSecret, MaterialRefusal, deliver, remove_credentials, scrub,
};

const KEY_A: &str = "sk-test-customer-a-000000000000";
const KEY_B: &str = "sk-test-customer-b-111111111111";

fn key(secret: &str) -> Credential {
    Credential::ApiKey {
        provider: "openai".into(),
        secret: CustomerSecret::new(secret.into()),
    }
}

fn refusal(result: retail_cloud::Result<material::Delivered>) -> MaterialRefusal {
    match result {
        Err(Error::Material(refusal)) => refusal,
        other => panic!("{other:?}"),
    }
}

#[test]
fn source_and_key_arrive_scoped_and_leave_at_teardown() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    let provider = FakeProvider::new();
    let sandbox = FakeSandbox::new();
    funded_account(&mut ledger, "acct-a", 1_000);
    funded_account(&mut ledger, "acct-b", 1_000);
    let a = confirmed(&mut journal, "acct-a", "cf-a", &request(600));
    let b = confirmed(&mut journal, "acct-b", "cf-b", &request(600));
    let sa = ready(&mut journal, &mut ledger, &provider, &a);
    let sb = ready(&mut journal, &mut ledger, &provider, &b);
    let source = a.admission.source.clone();

    let delivered = deliver(
        &mut journal,
        &sandbox,
        &a,
        &rights(&a),
        &sa,
        &source,
        &key(KEY_A),
        NOW,
    )
    .unwrap();
    assert_eq!(delivered.key_path, material::key_path(&a.execution));
    // B's material can never go to A's sandbox.
    assert!(matches!(
        deliver(
            &mut journal,
            &sandbox,
            &b,
            &rights(&b),
            &sa,
            &source,
            &key(KEY_B),
            NOW
        ),
        Err(Error::Invalid(_))
    ));
    deliver(
        &mut journal,
        &sandbox,
        &b,
        &rights(&b),
        &sb,
        &source,
        &key(KEY_B),
        NOW,
    )
    .unwrap();
    assert!(sandbox.files(&sa).iter().all(|(_, c)| c != KEY_B));
    assert!(sandbox.files(&sb).iter().all(|(_, c)| c != KEY_A));

    // Nothing the operator could capture holds a key.
    let commands = sandbox.commands().join("\n");
    assert!(!commands.contains(KEY_A) && !commands.contains(KEY_B));
    let (record, removed) = journal.delivered(&a.execution).unwrap().unwrap();
    assert!(!removed);
    let manifest = serde_json::to_string(&record).unwrap();
    assert!(!manifest.contains(KEY_A));
    assert_eq!(
        record.key_digest,
        CustomerSecret::new(KEY_A.into()).digest()
    );
    let log = format!("engine said: auth with {KEY_A} failed");
    assert_eq!(
        scrub(&log, &CustomerSecret::new(KEY_A.into())),
        "engine said: auth with [redacted] failed"
    );
    assert!(!format!("{:?}", key(KEY_A)).contains(KEY_A));

    // Teardown removes the key, and says so.
    assert!(remove_credentials(&mut journal, &sandbox, &a.execution, NOW + 50).unwrap());
    assert!(sandbox.files(&sa).is_empty());
    assert!(journal.delivered(&a.execution).unwrap().unwrap().1);
}

#[test]
fn a_wider_disclosure_or_another_provider_needs_another_offer() {
    let (mut journal, mut ledger) = (Journal::in_memory().unwrap(), Ledger::in_memory().unwrap());
    let provider = FakeProvider::new();
    let sandbox = FakeSandbox::new();
    funded_account(&mut ledger, "acct", 1_000);
    let f = confirmed(&mut journal, "acct", "cf", &request(600));
    let s = ready(&mut journal, &mut ledger, &provider, &f);
    let source = f.admission.source.clone();
    let mut go = |source: &retail_cloud::authority::Source, credential: &Credential| {
        deliver(
            &mut journal,
            &sandbox,
            &f,
            &rights(&f),
            &s,
            source,
            credential,
            NOW,
        )
    };
    let other_provider = Credential::ApiKey {
        provider: "anthropic".into(),
        secret: CustomerSecret::new(KEY_A.into()),
    };
    assert_eq!(
        refusal(go(&source, &other_provider)),
        MaterialRefusal::ProviderChanged
    );
    let mut wider = source.clone();
    wider.repository = "https://github.com/OpenAgentsInc/private-thing".into();
    assert_eq!(
        refusal(go(&wider, &key(KEY_A))),
        MaterialRefusal::SourceChanged
    );
    assert_eq!(
        refusal(go(
            &source,
            &Credential::OwnLogin {
                engine: "codex".into()
            }
        )),
        MaterialRefusal::OwnLoginExport
    );
    assert_eq!(
        refusal(go(
            &source,
            &Credential::PaidProvider {
                provider: "openai".into()
            }
        )),
        MaterialRefusal::PaidProviderUnsupported
    );
    sandbox.set_head(&"e".repeat(40));
    assert_eq!(
        refusal(go(&source, &key(KEY_A))),
        MaterialRefusal::SourceUnverified
    );
    sandbox.set_head(&source.commit);
    sandbox.set_dirty(true);
    assert_eq!(
        refusal(go(&source, &key(KEY_A))),
        MaterialRefusal::SourceUnverified
    );
    sandbox.set_dirty(false);
    sandbox.set_no_private_files(true);
    assert_eq!(
        refusal(go(&source, &key(KEY_A))),
        MaterialRefusal::IsolationUnsupported
    );
    sandbox.set_no_private_files(false);
    // Without disclosure consent nothing is cloned or written.
    let before = sandbox.commands().len();
    let mut current = rights(&f);
    current.disclose = None;
    assert!(matches!(
        deliver(
            &mut journal,
            &sandbox,
            &f,
            &current,
            &s,
            &source,
            &key(KEY_A),
            NOW
        ),
        Err(Error::Denied(_))
    ));
    assert_eq!(sandbox.commands().len(), before);
    assert!(sandbox.files(&s).is_empty());
    assert!(journal.delivered(&f.execution).unwrap().is_none());
}
