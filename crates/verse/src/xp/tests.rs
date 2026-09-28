//! Level curve, snapshot mapping, and board text over throwaway fixtures.

use std::collections::BTreeSet;

use xp_ledger::XpTrust;

use super::fixture::{Completion, Reproduction, signer, tutorial_events};
use super::*;

const AT: u64 = 1_790_000_000;

#[tokio::test]
async fn dropping_a_quiet_board_cancels_its_relay_worker() {
    use futures_util::StreamExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut board = Board::start_with(
        &format!("ws://{}", listener.local_addr().unwrap()),
        XpTrust::default(),
        None,
        None,
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        // Receive the subscription, then keep the relay quiet: no EOSE, updates,
        // or disconnect can accidentally wake the worker's send-error exit path.
        let request = socket.next().await.unwrap().unwrap();
        assert!(request.to_text().unwrap().contains("verse-xp"));
        while !board.connected {
            board.tick();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        drop(board);
        let ended = tokio::time::timeout(Duration::from_millis(500), socket.next())
            .await
            .unwrap();
        assert!(
            ended.is_none()
                || matches!(
                    ended,
                    Some(Err(_)) | Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))
                )
        );
    })
    .await
    .unwrap();
}

fn trust(c: &Completion) -> XpTrust {
    XpTrust {
        referees: BTreeSet::from([c.referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    }
}

fn base(c: &Completion) -> Vec<Event> {
    vec![c.quest.clone(), c.entry.clone(), c.evidence.clone()]
}

#[test]
fn the_level_curve_is_100_n_to_the_1_5() {
    assert_eq!(xp_to_reach(1), 0);
    assert_eq!(xp_to_reach(2), 100);
    assert_eq!(xp_to_reach(3), 283);
    assert_eq!(xp_to_reach(4), 520);
    assert_eq!(xp_to_reach(5), 800);
    assert_eq!(level_of(0), 1);
    assert_eq!(level_of(99), 1);
    assert_eq!(level_of(100), 2);
    assert_eq!(level_of(282), 2);
    assert_eq!(level_of(283), 3);
    assert_eq!(level_of(10_000), 22);
    let mut last = 0;
    for level in 1..200 {
        let need = xp_to_reach(level);
        assert!(need >= last, "cumulative XP never falls");
        assert_eq!(level_of(need), level);
        last = need;
    }
}

#[test]
fn a_counted_award_credits_xp_titles_and_its_quest_row() {
    let c = Completion::new(11, 12, 13, AT);
    let award = c.award(AT);
    let label = Completion::label(&c.referee, &award, "beat-reference", AT);
    let mut events = base(&c);
    // The same award from two relays counts once.
    events.extend([award.clone(), award, label]);
    let snap = snapshot(&events, &trust(&c));
    let author = c.author.pubkey().to_owned();
    let runner = c.runner.pubkey().to_owned();
    assert_eq!(snap.totals.get(&author), Some(&6));
    assert_eq!(snap.totals.get(&runner), Some(&4));
    assert_eq!(
        snap.xp_of(&[author.clone(), runner.clone(), author.clone()]),
        10
    );
    assert_eq!(snap.counted, 1);
    assert_eq!(
        snap.titles_of(std::slice::from_ref(&author)),
        BTreeSet::from(["beat-reference".to_owned()])
    );

    let [row] = snap.quests.as_slice() else {
        panic!("one quest row, got {:?}", snap.quests);
    };
    assert!(row.trusted && !row.conflict);
    assert_eq!(row.address, "tb4.fix-git.beat-fable-low@1");
    assert_eq!(row.task, "fix-git");
    assert_eq!(row.max_usd_per_run, Some(0.21));
    assert_eq!(
        row.split,
        vec![("author".to_owned(), 6), ("runner".to_owned(), 4)]
    );
    assert_eq!(row.total(), 10);
    assert_eq!((row.awards, row.counted), (1, 1));
    assert_eq!(row.titles, vec!["beat-reference".to_owned()]);
    let reference = row.reference.as_ref().expect("a reference");
    assert_eq!((reference.usd, reference.seconds), (Some(0.21), Some(312)));
}

#[test]
fn untrusted_referees_quests_show_but_their_awards_count_for_nothing() {
    let c = Completion::new(21, 22, 23, AT);
    let mut events = base(&c);
    events.push(c.award(AT));
    let snap = snapshot(&events, &XpTrust::default());
    assert!(snap.totals.is_empty());
    assert_eq!(snap.referees, 0);
    let row = &snap.quests[0];
    assert!(!row.trusted);
    assert_eq!((row.awards, row.counted), (1, 0));
}

#[test]
fn labels_need_the_awards_referee_and_a_counted_award() {
    let c = Completion::new(31, 32, 33, AT);
    let award = c.award(AT);
    let stranger = signer(99);
    let mut events = base(&c);
    events.push(award.clone());
    events.push(Completion::label(&stranger, &award, "fake-title", AT));
    let snap = snapshot(&events, &trust(&c));
    assert!(snap.titles.is_empty());

    // A revoked award takes its XP and its labels with it.
    let parts = xp::revocation(&award, "graded against the wrong verifier").unwrap();
    let revocation = c
        .referee
        .sign(AT + 1, parts.kind, parts.tags, parts.content);
    events.push(Completion::label(&c.referee, &award, "beat-reference", AT));
    events.push(revocation);
    let snap = snapshot(&events, &trust(&c));
    assert!(snap.totals.is_empty() && snap.titles.is_empty());
    assert_eq!(snap.revoked, 1);
    assert_eq!(snap.quests[0].counted, 0);
}

#[test]
fn missing_names_the_events_trusted_awards_point_at() {
    let c = Completion::new(41, 42, 43, AT);
    let award = c.award(AT);
    let mut have = BTreeMap::new();
    have.insert(award.id.clone(), award);
    let want = missing(&have, &trust(&c));
    assert_eq!(
        want,
        BTreeSet::from([
            c.quest.id.clone(),
            c.entry.id.clone(),
            c.evidence.id.clone()
        ])
    );
    assert!(missing(&have, &XpTrust::default()).is_empty());
    for e in base(&c) {
        have.insert(e.id.clone(), e);
    }
    assert!(missing(&have, &trust(&c)).is_empty());
}

#[test]
fn the_hud_shows_xp_level_titles_and_the_board() {
    let c = Completion::new(51, 52, 53, AT);
    let award = c.award(AT);
    let mut events = base(&c);
    events.push(Completion::label(&c.referee, &award, "beat-reference", AT));
    events.push(award);
    let board = Board::fixed("wss://relay.openagents.com", snapshot(&events, &trust(&c)));
    let mine = vec![c.author.pubkey().to_owned()];

    let lines = strip(Some(&board), &mine);
    assert_eq!(
        lines[0].0,
        "XP 6 · level 1 (trainer-curve-v1) · 94 XP to level 2"
    );
    assert_eq!(lines[1].0, "titles: beat-reference");
    assert!(lines[2].0.contains("1 quests") && lines[2].0.contains("relay.openagents.com"));

    let text: Vec<String> = board_lines(Some(&board), AT)
        .into_iter()
        .map(|l| l.0)
        .collect();
    let all = text.join("\n");
    assert!(all.contains("Beat Fable 5.1 low's cheapest winning run on fix-git"));
    assert!(all.contains("task fix-git · bar: pass at least 100% of runs, under $0.21 a run"));
    assert!(all.contains("reference: Fable 5.1 low, cheapest winning run · $0.21 · 5m 12s"));
    assert!(all.contains("award 10 XP (author 6, runner 4) · season 2026-q4, open 90 more days"));
    assert!(all.contains("1 award (1 counted)"));
    assert!(all.contains("achievements: beat-reference"));
    assert!(all.contains("can't be spent"));
    for word in ["buy", "spend XP", "balance", "sats"] {
        assert!(
            !all.contains(word),
            "the board never implies XP buys anything: {word}"
        );
    }

    assert_eq!(
        level_tag(board.snapshot.as_ref(), &mine).as_deref(),
        Some("lv 1")
    );
    assert_eq!(
        level_tag(board.snapshot.as_ref(), &["nobody".to_owned()]),
        None
    );
}

#[test]
fn offline_the_hud_says_so() {
    assert!(strip(None, &[])[0].0.contains("offline"));
    assert!(board_lines(None, AT)[0].0.contains("offline"));
    assert_eq!(
        host("wss://relay.openagents.com/path"),
        "relay.openagents.com"
    );
    assert_eq!(host("ws://127.0.0.1:7447"), "127.0.0.1:7447");
}

#[test]
fn a_reproduction_shows_on_the_board_the_tag_and_the_card() {
    let referee = signer(61);
    let reproducer = signer(62);
    let events = tutorial_events(&referee, &reproducer, 6, AT);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let snap = snapshot(&events, &trust);
    assert_eq!(snap.counted, 6, "refused: {}", snap.refused);
    let me = reproducer.pubkey().to_owned();
    assert_eq!(snap.xp_of(std::slice::from_ref(&me)), 300);

    // The name tag is the prefix and the level; a key without XP shows its
    // prefix alone.
    assert_eq!(name_tag(Some(&snap), &me), format!("{} · lv 3", &me[..8]));
    let nobody = signer(63).pubkey().to_owned();
    assert_eq!(name_tag(Some(&snap), &nobody), nobody[..8].to_owned());
    assert_eq!(name_tag(None, &me), me[..8].to_owned());

    // The card names its curve and lists the counted awards behind the level.
    let card = card(&snap, std::slice::from_ref(&me));
    assert_eq!(card.curve, "trainer-curve-v1");
    assert_eq!((card.xp, card.level), (300, 3));
    assert_eq!(card.next_level_at, 520);
    assert_eq!(card.to_next, 220);
    assert_eq!(card.awards.len(), 6);
    assert!(card.awards.iter().all(|a| a.rule == "reproduce"
        && a.role == "reproducer"
        && a.xp == 50
        && a.referee == referee.pubkey()));
    // The claimant's zero share is never listed.
    let claimant = card
        .awards
        .iter()
        .map(|a| a.award.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(claimant.len(), 6);

    let board = Board::fixed("wss://relay.openagents.com", snap);
    let all: Vec<String> = board_lines(Some(&board), AT)
        .into_iter()
        .map(|l| l.0)
        .collect();
    let all = all.join("\n");
    assert!(all.contains("Reproduce Microcoder's pass on build-pmars"));
    assert!(all.contains("bar: reproduce the published pass from recipe"));
    assert!(all.contains("award 50 XP (claimant 0, reproducer 50)"));
}

#[test]
fn missing_names_a_reproductions_claim_and_reproduction() {
    let referee = signer(71);
    let r = Reproduction::new(&referee, &signer(72), &signer(73), "hello-world", 50, AT);
    let award = r.award(AT);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let have = BTreeMap::from([(award.id.clone(), award)]);
    let want = missing(&have, &trust);
    assert!(want.contains(&r.quest.id));
    assert!(want.contains(&r.claim.id));
    assert!(want.contains(&r.reproduction.id));
}

#[test]
fn playtest_awards_count_on_their_own_card_and_never_in_the_trainer_level() {
    let referee = fixture::signer(0x91a7);
    let tester = fixture::signer(0x7e_57);
    let at = 1_790_000_000;
    let events = fixture::playtest_events(&referee, &tester, at);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let snap = snapshot(&events, &trust);
    let me = vec![tester.pubkey().to_owned()];
    // Trainer XP, level, and titles don't move.
    assert_eq!(snap.xp_of(&me), 0);
    assert!(snap.titles_of(&me).is_empty());
    assert_eq!(card(&snap, &me).awards.len(), 0);
    assert_eq!(level_tag(Some(&snap), &me), None);
    // The playtest card has all of it.
    let playtest = playtest_card(&snap, &me);
    assert_eq!(playtest.xp, 55);
    assert_eq!(
        (
            playtest.accepted_reports,
            playtest.fixes_verified,
            playtest.sessions,
            playtest.diaries
        ),
        (1, 1, 1, 0)
    );
    assert_eq!(
        playtest.titles,
        [
            "bug-hunter",
            "fix-verifier",
            "founding-playtester",
            "playtester",
            "raider"
        ]
    );
    assert!(playtest_titles(Some(&snap), tester.pubkey()).contains("playtester"));
    // A reader that trusts only the trainer referee sees none of it.
    let trainer = snapshot(&events, &openagents_trust());
    assert_eq!(playtest_card(&trainer, &me).xp, 0);
    assert!(playtest_titles(Some(&trainer), tester.pubkey()).is_empty());
    // No playtest referee key exists yet, so the app trusts none.
    assert_eq!(PLAYTEST_REFEREE, None);
    assert!(playtest_trust().is_none());
}

#[test]
fn a_per_awardee_quest_stays_open_until_it_pays_you_or_fills() {
    let referee = fixture::signer(0x5e_7e);
    let me = fixture::signer(0x3e);
    let at = 1_790_000_000;
    let mut events = fixture::tutorial_events(&referee, &me, 2, at);
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let mine = [me.pubkey().to_owned()];
    let someone = ["ab".repeat(32)];
    // Two first quests, both paid to me: neither is open to anyone.
    let snap = snapshot(&events, &trust);
    assert_eq!(open_quests(&snap, &mine, at), 0);
    assert_eq!(open_quests(&snap, &someone, at), 0);
    // A per-awardee version stays open to others after it pays me.
    events.extend(fixture::per_awardee_events(&referee, &me, 3, at));
    let snap = snapshot(&events, &trust);
    assert_eq!(open_quests(&snap, &mine, at), 0);
    assert_eq!(open_quests(&snap, &someone, at), 1);
    let lines = board_lines(Some(&Board::fixed("wss://r", snap)), at);
    assert!(
        lines
            .iter()
            .any(|(l, _)| l.contains("pays each trainer once, up to 3")),
        "{lines:?}"
    );
}

#[test]
fn a_level_shows_over_a_head_only_after_the_trainer_opts_in() {
    let referee = fixture::signer(0x5e_7e);
    let me = fixture::signer(0x3e);
    let at = 1_790_000_000;
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let key = me.pubkey().to_owned();
    // The fixture's awards without its profile: XP, but no level shown.
    let events: Vec<Event> = fixture::tutorial_events(&referee, &me, 2, at)
        .into_iter()
        .filter(|e| e.kind != xp::PROFILE_KIND)
        .collect();
    let snap = snapshot(&events, &trust);
    assert_eq!(snap.xp_of(std::slice::from_ref(&key)), 100);
    assert_eq!(name_tag(Some(&snap), &key), key[..8]);
    assert_eq!(trainer_level_tag(Some(&snap), &key), None);
    // Opting in shows it.
    let mut shown = events.clone();
    let parts = xp::profile(&key, true, &[]).unwrap();
    shown.push(me.sign(at, parts.kind, parts.tags, parts.content));
    let snap = snapshot(&shown, &trust);
    assert_eq!(name_tag(Some(&snap), &key), format!("{} · lv 2", &key[..8]));
    // A newer profile that hides it hides it again.
    let parts = xp::profile(&key, false, &[]).unwrap();
    shown.push(me.sign(at + 1, parts.kind, parts.tags, parts.content));
    let snap = snapshot(&shown, &trust);
    assert_eq!(name_tag(Some(&snap), &key), key[..8]);
    // The trainer's own card still counts its XP.
    assert_eq!(card(&snap, std::slice::from_ref(&key)).xp, 100);
}

#[test]
fn a_linked_key_raises_the_trainers_level_over_both_heads() {
    let referee = fixture::signer(0x5e_7e);
    let laptop = fixture::signer(0x1a_97);
    let phone = fixture::signer(0x9f);
    let at = 1_790_000_000;
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    // The laptop earned 100 XP; the phone is the trainer, shown.
    let mut events: Vec<Event> = fixture::tutorial_events(&referee, &laptop, 2, at)
        .into_iter()
        .filter(|e| e.kind != xp::PROFILE_KIND)
        .collect();
    let (p, l) = (phone.pubkey().to_owned(), laptop.pubkey().to_owned());
    let parts = xp::profile(&p, true, std::slice::from_ref(&l)).unwrap();
    events.push(phone.sign(at, parts.kind, parts.tags, parts.content));
    let snap = snapshot(&events, &trust);
    // One-sided: the phone's level is its own.
    assert_eq!(name_tag(Some(&snap), &p), p[..8]);
    assert_eq!(trainer_keys(&snap, &p), std::slice::from_ref(&p));
    // Two-sided: both heads show the trainer's level.
    let parts = xp::link(&l, Some(&p)).unwrap();
    events.push(laptop.sign(at, parts.kind, parts.tags, parts.content));
    let snap = snapshot(&events, &trust);
    assert_eq!(trainer_keys(&snap, &l), [p.clone(), l.clone()]);
    assert_eq!(name_tag(Some(&snap), &p), format!("{} · lv 2", &p[..8]));
    assert_eq!(name_tag(Some(&snap), &l), format!("{} · lv 2", &l[..8]));
}

#[test]
fn a_card_checks_clean_and_each_inflation_is_reported() {
    let referee = fixture::signer(0x5e_7e);
    let me = fixture::signer(0x3e);
    let at = 1_790_000_000;
    let trust = XpTrust {
        referees: BTreeSet::from([referee.pubkey().to_owned()]),
        runners: BTreeSet::new(),
    };
    let events = fixture::tutorial_events(&referee, &me, 3, at);
    let snap = snapshot(&events, &trust);
    let relays = vec!["wss://relay.openagents.com".to_owned()];
    let card = trainer_card(&snap, me.pubkey(), &trust, &relays, at);
    assert_eq!((card.xp, card.level, card.awards.len()), (150, 2, 3));
    // Signed, it parses and checks clean.
    let parts = xp::card(&card).unwrap();
    let signed = me.sign(at, parts.kind, parts.tags, parts.content);
    let parsed = xp::parse_card(&signed).unwrap();
    let check = check_card(&parsed, me.pubkey(), &snap);
    assert!(check.differences.is_empty(), "{:?}", check.differences);
    assert_eq!((check.xp, check.level), (150, Some(2)));

    // An inflated level, a stranger's key, and a made-up award each show.
    let mut inflated = card.clone();
    inflated.level = 9;
    inflated.xp = 5_000;
    let stranger = "cd".repeat(32);
    inflated.keys.push(stranger.clone());
    inflated.awards.push(xp::CardAward {
        id: "ef".repeat(32),
        pubkey: stranger,
        role: "reproducer".into(),
        xp: 50,
        quest: "tb21.fix-git.reproduce@9".into(),
    });
    let check = check_card(&inflated, me.pubkey(), &snap);
    let text = check.differences.join("\n");
    for expected in [
        "isn't linked",
        "doesn't count",
        "claims 5000 XP",
        "claims level 9",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    // Under another trust list, the awards don't count.
    let other = snapshot(&events, &openagents_trust());
    let check = check_card(&card, me.pubkey(), &other);
    assert_eq!(check.xp, 0);
    assert!(!check.differences.is_empty());
}
