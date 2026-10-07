use super::*;
use crate::task::agent_engrams::{core_hash, insight_slug};
use crate::task::agent_memory::Author;
use secp256k1::SecretKey;

const OWNER: [u8; 32] = [7; 32];

/// A scripted model: each call returns the next reply, and records the
/// prompt it was given.
#[derive(Default)]
struct Scripted {
    replies: Vec<String>,
    prompts: Vec<String>,
}

impl Writer for Scripted {
    fn write(&mut self, _: &str, prompt: &str) -> Result<Reply, String> {
        self.prompts.push(prompt.to_string());
        if self.replies.is_empty() {
            return Err("no reply left".into());
        }
        Ok(Reply {
            text: self.replies.remove(0),
            model: "scripted".into(),
            usd: Some(0.02),
        })
    }
}

fn scripted(profile: &str) -> Scripted {
    Scripted {
        replies: vec![serde_json::json!({ "profile": profile }).to_string()],
        prompts: Vec::new(),
    }
}

/// Alice with her own key, attested by [`OWNER`], and one active insight.
fn alice(dir: &tempfile::TempDir) -> (Memory, u64, String) {
    let store = Store::new(&dir.path().join("host"), "alice").unwrap();
    let record = store.open(dir.path(), 1).unwrap();
    let record = store.ensure_key(record, 1).unwrap();
    let record = store
        .attest(
            record,
            &SecretKey::from_byte_array(OWNER).unwrap(),
            1_000_000,
            1,
        )
        .unwrap();
    let memory = Memory::new(store, secret_screen::Screen::shapes());
    let insight = memory
        .add(
            MemoryKind::Insight,
            Author::Agent,
            "The owner merges small changes the same day.",
            vec!["journal:1".into()],
            10,
        )
        .unwrap();
    (memory, insight, record.charter)
}

fn engrams(memory: &Memory) -> EngramStore {
    match EngramStore::read(memory.store(), memory.screen()) {
        Opened::Ready(engrams) => engrams,
        other => panic!("not ready: {other:?}"),
    }
}

fn profile(charter: &str, insight: u64) -> String {
    format!(
        "I am alice, my owner's workshop agent.\n\nMy charter: {charter}\n\nThe owner merges \
         small changes the same day. [[mem/insight/{insight}]]"
    )
}

fn journal_lines(memory: &Memory) -> Vec<String> {
    memory
        .store()
        .journal(500)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .filter(|t| t.starts_with(RUN_PREFIX))
        .collect()
}

#[test]
fn an_accepted_proposal_becomes_core_and_reaches_its_insight() {
    let dir = tempfile::tempdir().unwrap();
    let (memory, insight, charter) = alice(&dir);
    let before = engrams(&memory).core().unwrap().to_string();
    assert!(
        engrams(&memory)
            .reach()
            .orphans
            .contains(&format!("mem/insight/{insight}"))
    );
    let mut writer = scripted(&profile(&charter, insight));
    let (proposed, reply) = propose(&memory, &mut writer, 100).unwrap();
    assert!(
        matches!(proposed, Proposed::Waiting { links: 1, .. }),
        "{proposed:?}"
    );
    assert_eq!(reply.unwrap().usd, Some(0.02));
    assert!(writer.prompts[0].contains(&format!("[[mem/insight/{insight}]]")));
    // Nothing changed core yet; the proposal waits under its base.
    let store = engrams(&memory);
    assert_eq!(store.core(), Some(before.as_str()));
    let waiting = pending(&store).unwrap();
    assert_eq!(waiting.base, core_hash(Some(&before)));
    // F2 shows it as row 0, a core candidate.
    let rows = rows(memory.store(), memory.screen());
    assert_eq!((rows[0].id, rows[0].kind.as_str()), (0, "core"));
    assert_eq!(rows[0].state, "candidate");
    // A second run waits for the owner instead of proposing again.
    let mut again = scripted("unused");
    let (proposed, _) = propose(&memory, &mut again, 101).unwrap();
    assert!(matches!(proposed, Proposed::Skipped(_)));
    assert!(again.prompts.is_empty());

    let hash = decide(&memory, true, 110).unwrap();
    let store = engrams(&memory);
    assert_eq!(store.core(), Some(profile(&charter, insight).as_str()));
    assert_eq!(hash, core_hash(store.core()));
    assert!(pending(&store).is_none());
    let reach = store.reach();
    assert_eq!(reach.reachable, vec![format!("mem/insight/{insight}")]);
    assert!(reach.dangling.is_empty());
    assert!(decide(&memory, true, 111).is_err(), "decided once");
    assert!(
        journal_lines(&memory)
            .iter()
            .any(|l| l.contains("accepted by the owner"))
    );
}

#[test]
fn a_rejected_proposal_leaves_core_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (memory, insight, charter) = alice(&dir);
    let before = engrams(&memory).core().unwrap().to_string();
    propose(&memory, &mut scripted(&profile(&charter, insight)), 100).unwrap();
    decide(&memory, false, 110).unwrap();
    let store = engrams(&memory);
    assert_eq!(store.core(), Some(before.as_str()));
    assert!(pending(&store).is_none());
    assert!(
        journal_lines(&memory)
            .iter()
            .any(|l| l.contains("rejected by the owner"))
    );
}

#[test]
fn a_core_that_changed_since_the_proposal_refuses_the_write() {
    let dir = tempfile::tempdir().unwrap();
    let (memory, insight, charter) = alice(&dir);
    propose(&memory, &mut scripted(&profile(&charter, insight)), 100).unwrap();
    // Another device writes core after the proposal.
    let mut store = engrams(&memory);
    store
        .put(
            Body::core(format!("Elsewhere.\n\nMy charter: {charter}")),
            105,
        )
        .unwrap();
    let changed = store.core().unwrap().to_string();
    let refused = decide(&memory, true, 110).unwrap_err();
    assert!(refused.contains("her core changed"), "{refused}");
    let store = engrams(&memory);
    assert_eq!(store.core(), Some(changed.as_str()), "core kept");
    assert!(pending(&store).is_none(), "the stale proposal is discarded");
    assert!(
        journal_lines(&memory)
            .iter()
            .any(|l| l.contains("refused: her core changed"))
    );
}

#[test]
fn a_proposal_is_checked_before_it_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let (memory, insight, charter) = alice(&dir);
    let token = format!("ghp_{}", "Ab1".repeat(12));
    let cases = [
        (
            format!("{} My token is {token}.", profile(&charter, insight)),
            "secret screen",
        ),
        (
            "I am alice and I keep no charter.".to_string(),
            "drops her charter",
        ),
        (
            format!("{} [[mem/entry/99]]", profile(&charter, insight)),
            "[[mem/entry/99]]",
        ),
        (
            format!("{} {}", profile(&charter, insight), "x".repeat(CORE_MAX)),
            "core cap",
        ),
        ("not json".to_string(), "no JSON"),
    ];
    for (n, (reply, why)) in cases.into_iter().enumerate() {
        let mut writer = Scripted {
            replies: vec![if why == "no JSON" {
                reply
            } else {
                serde_json::json!({ "profile": reply }).to_string()
            }],
            prompts: Vec::new(),
        };
        let (proposed, _) = propose(&memory, &mut writer, 100 + n as u64).unwrap();
        match proposed {
            Proposed::Dropped(said) => assert!(said.contains(why), "{said} lacks {why}"),
            other => panic!("{why}: {other:?}"),
        }
        assert!(pending(&engrams(&memory)).is_none());
    }
    let journal = std::fs::read_to_string(memory.store().dir().join("journal.jsonl")).unwrap();
    assert!(
        !journal.contains(&token),
        "the journal never holds a refused secret"
    );
}

#[test]
fn a_candidate_preference_is_never_linked_and_nothing_new_skips() {
    let dir = tempfile::tempdir().unwrap();
    let (memory, insight, charter) = alice(&dir);
    let candidate = memory
        .add(
            MemoryKind::Preference,
            Author::Agent,
            "Owner wants squashed commits.",
            vec![],
            11,
        )
        .unwrap();
    let mut writer = scripted(&format!(
        "{} Squash. [[mem/entry/{candidate}]]",
        profile(&charter, insight)
    ));
    let (proposed, _) = propose(&memory, &mut writer, 100).unwrap();
    assert!(matches!(proposed, Proposed::Dropped(_)), "{proposed:?}");
    // Once core reaches every active insight, there is nothing to propose.
    propose(&memory, &mut scripted(&profile(&charter, insight)), 101).unwrap();
    decide(&memory, true, 102).unwrap();
    let mut idle = scripted("unused");
    let (proposed, reply) = propose(&memory, &mut idle, 103).unwrap();
    assert!(matches!(proposed, Proposed::Skipped(_)) && reply.is_none());
    assert!(idle.prompts.is_empty());
}

#[test]
fn an_unreadable_store_never_proposes_or_writes_core() {
    let dir = tempfile::tempdir().unwrap();
    let (memory, insight, charter) = alice(&dir);
    propose(&memory, &mut scripted(&profile(&charter, insight)), 100).unwrap();
    let store = engrams(&memory);
    let file = store.dir().join(format!(
        "{}.json",
        store.pair().d_tag(&insight_slug(insight))
    ));
    std::fs::write(&file, "{ not an event").unwrap();
    let mut writer = scripted(&profile(&charter, insight));
    assert!(propose(&memory, &mut writer, 101).is_err());
    assert!(writer.prompts.is_empty(), "no model call");
    assert!(decide(&memory, true, 102).is_err());
    let rows = rows(memory.store(), memory.screen());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, "unreadable");
}
