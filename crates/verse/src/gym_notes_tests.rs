//! Agents' notes: the wire shape, grounding, rendering, and the rules for
//! when an agent speaks.

use std::collections::{BTreeMap, BTreeSet};

use nostr::domain::{Event, RelaySigner};

use super::*;
use crate::gym_evals::fixture::{self, Spec};
use crate::gym_evals::{self, Names};

const AT: u64 = 1_790_000_000;
const WORLD: &str = "verse-bare";
const RELAY: &str = "wss://relay.openagents.com";

fn signer(label: &str) -> RelaySigner {
    use sha2::Digest;
    let hex: String = sha2::Sha256::digest(label.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    RelaySigner::from_secret_hex(&hex).expect("throwaway key")
}

struct World {
    alice: RelaySigner,
    bob: RelaySigner,
    events: Vec<Event>,
    a: Event,
    b: Event,
    b_other: Event,
}

fn world() -> World {
    let (op, alice, bob) = (signer("operator"), signer("alice"), signer("bob"));
    let find = fixture::release(&op, "starter-find", "1", AT - 900);
    let tests = fixture::release(&op, "starter-tests", "1", AT - 900);
    let finder = fixture::release(&op, "code-finder", "0.3.0", AT - 900);
    let reader = fixture::release(&op, "test-reader", "1.0.0", AT - 900);
    let spec = |suite, tool, with, without| Spec {
        suite,
        tool,
        with,
        without: Some(without),
        total: 8,
        lock: "lock",
        checks: None,
    };
    let a = fixture::result(&alice, &spec(&find, &finder, 6, 3), AT - 300);
    let b = fixture::result(&bob, &spec(&find, &reader, 5, 4), AT - 400);
    let b_other = fixture::result(&bob, &spec(&tests, &reader, 2, 2), AT - 200);
    let events = vec![
        find,
        tests,
        finder,
        reader,
        a.clone(),
        b.clone(),
        b_other.clone(),
    ];
    World {
        alice,
        bob,
        events,
        a,
        b,
        b_other,
    }
}

fn sign(signer: &RelaySigner, plan: &Plan, text: &str, at: u64) -> Event {
    signer.sign(at, 9, tags(WORLD, RELAY, plan), text.to_owned())
}

#[test]
fn a_note_round_trips_and_malformed_notes_are_refused() {
    let w = world();
    let open = Plan::Open {
        ours: w.a.id.clone(),
    };
    let event = sign(&w.alice, &open, "hello", AT);
    let note = parse(&event, WORLD).unwrap();
    assert!(note.opens());
    assert_eq!(note.sources, std::slice::from_ref(&w.a.id));
    assert_eq!(parse(&event, "verse-plaza"), Err("scope"));

    let answer = Plan::Answer {
        note: note.id.clone(),
        peer: w.alice.pubkey().to_owned(),
        ours: Some(w.b.id.clone()),
    };
    let reply = parse(&sign(&w.bob, &answer, "hi", AT), WORLD).unwrap();
    assert_eq!(
        reply.answers,
        Some((note.id.clone(), w.alice.pubkey().to_owned()))
    );

    let edit = |edit: &dyn Fn(&mut Event)| {
        let mut e = event.clone();
        edit(&mut e);
        parse(&e, WORLD)
    };
    assert_eq!(edit(&|e| e.kind = 1), Err("kind"));
    assert_eq!(
        edit(&|e| e.tags.retain(|t| t.name() != Some("L"))),
        Err("label namespace")
    );
    assert_eq!(
        edit(&|e| e.tags.retain(|t| t.name() != Some("l"))),
        Err("label")
    );
    assert_eq!(
        edit(&|e| e.tags.retain(|t| t.name() != Some("e"))),
        Err("an opening note cites a result")
    );
    assert_eq!(edit(&|e| e.content = "x".repeat(MAX_TEXT + 1)), Err("text"));
    assert_eq!(edit(&|e| e.content = "a\u{7}b".into()), Err("text"));
    assert_eq!(
        edit(&|e| e.tags.push(Tag::new(vec![
            "e".into(),
            "ab".repeat(32),
            String::new(),
            SOURCE.into()
        ]))),
        Err("too many sources")
    );
    // An agent can't answer itself.
    let own = Plan::Answer {
        note: note.id.clone(),
        peer: w.alice.pubkey().to_owned(),
        ours: None,
    };
    assert_eq!(parse(&sign(&w.alice, &own, "me", AT), WORLD), Err("quote"));
}

#[test]
fn readers_render_their_own_text_from_verified_results() {
    let w = world();
    let publications = gym_evals::verified(&w.events);
    let names = Names::from_events(&w.events);
    let open = sign(
        &w.alice,
        &Plan::Open {
            ours: w.a.id.clone(),
        },
        "anything at all",
        AT,
    );
    let opener = parse(&open, WORLD).unwrap();
    let mut notes = BTreeMap::from([(opener.id.clone(), opener.clone())]);
    let shown = check(&opener, &notes, &publications, &names, w.bob.pubkey()).unwrap();
    assert_eq!(
        shown.text,
        "We tested code-finder 0.3.0 on starter-find 1: 6 of 8 cases passed with it, 3 of 8 \
without. It helped. Has anyone here run starter-find 1?"
    );
    assert!(!shown.mine && !shown.answer);

    // Bob answers with his result on the same test set.
    let answer = Plan::Answer {
        note: opener.id.clone(),
        peer: w.alice.pubkey().to_owned(),
        ours: Some(w.b.id.clone()),
    };
    let reply = parse(&sign(&w.bob, &answer, "x", AT + 5), WORLD).unwrap();
    notes.insert(reply.id.clone(), reply.clone());
    let shown = check(&reply, &notes, &publications, &names, w.alice.pubkey()).unwrap();
    assert_eq!(
        shown.text,
        "We ran starter-find 1 too, with test-reader 1.0.0: 5 of 8 cases passed with it, 4 of 8 \
without. It helped. code-finder 0.3.0 added more than test-reader 1.0.0 here (+3 cases against \
+1). Why did code-finder 0.3.0 help more on your run?"
    );
    assert_eq!(shown.answers_tag.as_deref(), Some(&w.alice.pubkey()[..8]));

    // Had Alice answered Bob instead, her tool lifted more: no question.
    let (theirs, ours) = (&publications[&w.a.id], &publications[&w.b.id]);
    assert!(render_answer(Some(theirs), ours, &names).ends_with("(+3 cases against +1)."));
    assert_eq!(
        render_answer(None, theirs, &names),
        "We haven't run starter-find 1 yet, so we have nothing to compare with your code-finder 0.3.0."
    );
}

#[test]
fn ungrounded_notes_are_held_back() {
    let w = world();
    let publications = gym_evals::verified(&w.events);
    let names = Names::from_events(&w.events);
    // Bob cites Alice's result as his own.
    let stolen = parse(
        &sign(
            &w.bob,
            &Plan::Open {
                ours: w.a.id.clone(),
            },
            "mine",
            AT,
        ),
        WORLD,
    )
    .unwrap();
    let notes = BTreeMap::from([(stolen.id.clone(), stolen.clone())]);
    assert_eq!(
        check(&stolen, &notes, &publications, &names, ""),
        Err(Refusal::Ungrounded)
    );
    // A source the reader hasn't fetched yet waits.
    let unknown = parse(
        &sign(
            &w.bob,
            &Plan::Open {
                ours: "cd".repeat(32),
            },
            "?",
            AT,
        ),
        WORLD,
    )
    .unwrap();
    assert_eq!(
        check(&unknown, &notes, &publications, &names, ""),
        Err(Refusal::Waiting)
    );
    assert_eq!(wanted(&unknown), ["cd".repeat(32)]);
    // An answer citing a result on another test set than the opener's.
    let opener = parse(
        &sign(
            &w.alice,
            &Plan::Open {
                ours: w.a.id.clone(),
            },
            "o",
            AT,
        ),
        WORLD,
    )
    .unwrap();
    let off = Plan::Answer {
        note: opener.id.clone(),
        peer: w.alice.pubkey().to_owned(),
        ours: Some(w.b_other.id.clone()),
    };
    let off = parse(&sign(&w.bob, &off, "o", AT), WORLD).unwrap();
    let mut notes = BTreeMap::from([(opener.id.clone(), opener.clone())]);
    assert_eq!(
        check(&off, &notes, &publications, &names, ""),
        Err(Refusal::Ungrounded)
    );
    // An answer to an answer.
    let answer = Plan::Answer {
        note: opener.id.clone(),
        peer: w.alice.pubkey().to_owned(),
        ours: None,
    };
    let answer = parse(&sign(&w.bob, &answer, "a", AT), WORLD).unwrap();
    notes.insert(answer.id.clone(), answer.clone());
    let chain = Plan::Answer {
        note: answer.id.clone(),
        peer: w.bob.pubkey().to_owned(),
        ours: None,
    };
    let chain = parse(&sign(&w.alice, &chain, "c", AT), WORLD).unwrap();
    assert_eq!(
        check(&chain, &notes, &publications, &names, ""),
        Err(Refusal::Ungrounded)
    );
}

struct Agent {
    me: String,
    opted_in: bool,
    peers: BTreeSet<String>,
    policy: Policy,
}

impl Agent {
    fn new(signer: &RelaySigner, peer: &RelaySigner) -> Self {
        Self {
            me: signer.pubkey().to_owned(),
            opted_in: true,
            peers: BTreeSet::from([peer.pubkey().to_owned()]),
            policy: Policy::default(),
        }
    }

    fn next(
        &mut self,
        notes: &BTreeMap<String, Note>,
        publications: &BTreeMap<String, nostr::eval_ext::Publication>,
        now: u64,
    ) -> Option<Plan> {
        let shown: BTreeSet<String> = notes.keys().cloned().collect();
        let context = Context {
            me: &self.me,
            opted_in: self.opted_in,
            here: true,
            peers: &self.peers,
            notes,
            shown: &shown,
            publications,
        };
        let plan = self.policy.next(&context, now);
        if let Some(plan) = &plan {
            self.policy.record(plan, now);
        }
        plan
    }
}

#[test]
fn two_agents_exchange_one_opener_and_one_answer_then_wait() {
    let w = world();
    let publications = gym_evals::verified(&w.events);
    let mut notes: BTreeMap<String, Note> = BTreeMap::new();
    let mut alice = Agent::new(&w.alice, &w.bob);
    let mut bob = Agent::new(&w.bob, &w.alice);
    let post = |signer: &RelaySigner, plan: &Plan, at: u64, notes: &mut BTreeMap<String, Note>| {
        let note = parse(&sign(signer, plan, "n", at), WORLD).unwrap();
        notes.insert(note.id.clone(), note);
    };

    // Alice speaks first, with her newest own result.
    let plan = alice.next(&notes, &publications, AT).unwrap();
    assert_eq!(
        plan,
        Plan::Open {
            ours: w.a.id.clone()
        }
    );
    post(&w.alice, &plan, AT, &mut notes);
    // Bob answers with his result on the same test set, not his newest.
    let plan = bob.next(&notes, &publications, AT + 1).unwrap();
    let opener = notes.values().next().unwrap().id.clone();
    assert_eq!(
        plan,
        Plan::Answer {
            note: opener,
            peer: w.alice.pubkey().to_owned(),
            ours: Some(w.b.id.clone()),
        }
    );
    post(&w.bob, &plan, AT + 1, &mut notes);
    // Then both stay quiet: Alice doesn't answer an answer, Bob doesn't
    // open right after speaking, and neither repeats within the interval.
    for t in [AT + 2, AT + 60, AT + OPEN_INTERVAL - 2] {
        assert_eq!(alice.next(&notes, &publications, t), None);
        assert_eq!(bob.next(&notes, &publications, t), None);
    }
    // After the interval, Alice opens again (Bob answered her, so he
    // waits on his own interval to open).
    assert!(matches!(
        alice.next(&notes, &publications, AT + OPEN_INTERVAL + 1),
        Some(Plan::Open { .. })
    ));
}

#[test]
fn an_agent_is_quiet_when_off_alone_or_without_results() {
    let w = world();
    let publications = gym_evals::verified(&w.events);
    let notes = BTreeMap::new();
    let mut alice = Agent::new(&w.alice, &w.bob);
    alice.opted_in = false;
    assert_eq!(alice.next(&notes, &publications, AT), None);
    alice.opted_in = true;
    alice.peers.clear();
    assert_eq!(alice.next(&notes, &publications, AT), None);
    let stranger = signer("stranger");
    let mut nobody = Agent::new(&stranger, &w.alice);
    assert_eq!(
        nobody.next(&notes, &publications, AT),
        None,
        "no result to talk about"
    );
}

#[test]
fn the_hourly_cap_and_restarts_hold() {
    let w = world();
    let publications = gym_evals::verified(&w.events);
    let mut notes: BTreeMap<String, Note> = BTreeMap::new();
    // Four of Alice's notes already on the relay this hour, from before a
    // restart: a fresh policy stays quiet.
    for i in 0..MAX_PER_HOUR as u64 {
        let note = parse(
            &sign(
                &w.alice,
                &Plan::Open {
                    ours: w.a.id.clone(),
                },
                "n",
                AT - 3000 + i,
            ),
            WORLD,
        )
        .unwrap();
        notes.insert(note.id.clone(), note);
    }
    let mut alice = Agent::new(&w.alice, &w.bob);
    assert_eq!(alice.next(&notes, &publications, AT), None);
    // A stale opener from Bob gets no answer either.
    let mut bob = Agent::new(&w.bob, &w.alice);
    assert_eq!(
        bob.next(&notes, &publications, AT + ANSWER_WINDOW + 1)
            .map(|p| matches!(p, Plan::Answer { .. })),
        Some(false),
        "Bob opens instead of answering stale notes"
    );
}
