use super::*;

fn key() -> Ed25519KeyPair {
    Ed25519KeyPair::from_seed_unchecked(&[24; 32]).unwrap()
}

fn outcome(id: &str) -> Outcome {
    Outcome {
        v: SCHEMA.into(),
        source: Source::CoderAcceptance,
        run_id: id.into(),
        class: TaskClass::Code,
        model: "test/model".into(),
        mode: Mode::Real,
        accepted: true,
        paid_msat: 1000,
        cost_micros: 100_000,
        payment_digest: "a".repeat(64),
        verification_digest: "b".repeat(64),
    }
}

fn config(key: &Ed25519KeyPair) -> Config {
    Config {
        path: std::env::temp_dir()
            .join(crate::upstream::emit::fresh_id("inference-outcomes"))
            .join("outcomes.jsonl"),
        issuers: vec![Issuer {
            source: Source::CoderAcceptance,
            key: STANDARD.encode(key.public_key().as_ref()),
        }],
    }
}

#[test]
fn verifies_source_payment_and_signature_before_counting() {
    let key = key();
    let config = config(&key);
    let book = Book::open(&config).unwrap();
    let original = Receipt::sign(outcome("one"), &key);
    for change in 0..7 {
        let mut receipt = original.clone();
        match change {
            0 => receipt.outcome.source = Source::GymHeadToHead,
            1 => receipt.outcome.paid_msat = 0,
            2 => receipt.outcome.verification_digest.clear(),
            3 => receipt.outcome.accepted = false,
            4 => receipt.outcome.model = "another/model".into(),
            5 => receipt.signature.clear(),
            _ => {
                receipt = Receipt::sign(
                    outcome("one"),
                    &Ed25519KeyPair::from_seed_unchecked(&[25; 32]).unwrap(),
                )
            }
        }
        assert!(book.record(receipt).is_err());
    }
    assert!(book.summary().unwrap().by_class.is_empty());
    assert!(book.record(original.clone()).unwrap());
    assert!(!book.record(original).unwrap());
    let mut conflict = outcome("one");
    conflict.accepted = false;
    assert!(book.record(Receipt::sign(conflict, &key)).is_err());
    drop(book);
    let book = Book::open(&config).unwrap();
    assert_eq!(
        book.summary().unwrap().by_class[&TaskClass::Code]["test/model"].samples,
        1
    );
    assert!(Book::open(&config).is_err());
    drop(book);
    std::fs::remove_dir_all(config.path.parent().unwrap()).unwrap();
}

#[test]
fn paid_rejections_lower_quality_without_fixtures_or_cross_class_leakage() {
    let key = key();
    let config = config(&key);
    let book = Book::open(&config).unwrap();
    let gym = Scores {
        by_class: BTreeMap::from([
            (
                TaskClass::Code,
                BTreeMap::from([("test/model".into(), 0.9), ("test/new".into(), 0.8)]),
            ),
            (
                TaskClass::Chat,
                BTreeMap::from([("test/model".into(), 0.95)]),
            ),
        ]),
    };
    let mut scores = gym.clone();
    book.summary().unwrap().apply(&mut scores);
    assert_eq!(scores, gym);
    for (id, accepted, mode) in [
        ("one", true, Mode::Real),
        ("fixture", false, Mode::Fixture),
        ("synthetic", false, Mode::Synthetic),
    ] {
        let mut o = outcome(id);
        o.accepted = accepted;
        o.mode = mode;
        book.record(Receipt::sign(o, &key)).unwrap();
    }
    let mut scores = gym.clone();
    book.summary().unwrap().apply(&mut scores);
    assert_eq!(scores.get(TaskClass::Code, "test/model"), Some(0.9));
    let mut o = outcome("two");
    o.accepted = false;
    o.cost_micros = 300_000;
    book.record(Receipt::sign(o, &key)).unwrap();
    let summary = book.summary().unwrap();
    let mut scores = gym.clone();
    summary.apply(&mut scores);
    assert_eq!(scores.get(TaskClass::Code, "test/model"), Some(0.5));
    assert_eq!(scores.get(TaskClass::Code, "test/new"), Some(0.8));
    assert_eq!(scores.get(TaskClass::Chat, "test/model"), Some(0.95));
    assert_eq!(summary.fixtures, 1);
    assert_eq!(summary.synthetic, 1);
    let public = summary.model("test/model");
    assert_eq!(public["code"]["samples"], 2);
    assert_eq!(public["code"]["cost_per_accepted_usd"], "0.4");
    let mut no_gym = Scores::default();
    summary.apply(&mut no_gym);
    assert_eq!(no_gym.get(TaskClass::Code, "test/model"), Some(0.5));
    assert!(summary.model("test/new").as_object().unwrap().is_empty());
    drop(book);
    let restored = Book::open(&config).unwrap().summary().unwrap();
    assert_eq!(restored.model("test/model"), public);
    assert_eq!((restored.fixtures, restored.synthetic), (1, 1));
    std::fs::remove_dir_all(config.path.parent().unwrap()).unwrap();
}

#[test]
fn rejects_corrupt_or_partial_history_and_overflow_without_changing_counts() {
    let key = key();
    let config = config(&key);
    let book = Book::open(&config).unwrap();
    let mut o = outcome("one");
    o.cost_micros = u64::MAX;
    book.record(Receipt::sign(o, &key)).unwrap();
    assert!(book.record(Receipt::sign(outcome("two"), &key)).is_err());
    assert_eq!(
        book.summary().unwrap().by_class[&TaskClass::Code]["test/model"].samples,
        1
    );
    drop(book);
    let mut file = OpenOptions::new().append(true).open(&config.path).unwrap();
    file.write_all(b"{\"outcome\":").unwrap();
    drop(file);
    assert!(Book::open(&config).is_err());
    std::fs::remove_dir_all(config.path.parent().unwrap()).unwrap();
}

#[test]
fn both_sources_count_but_work_cannot_be_recounted_after_a_key_rotation() {
    let key = key();
    let rotated = Ed25519KeyPair::from_seed_unchecked(&[25; 32]).unwrap();
    let mut config = config(&key);
    for (source, key) in [
        (Source::GymHeadToHead, &key),
        (Source::CoderAcceptance, &rotated),
    ] {
        config.issuers.push(Issuer {
            source,
            key: STANDARD.encode(key.public_key().as_ref()),
        });
    }
    let book = Book::open(&config).unwrap();
    book.record(Receipt::sign(outcome("coder"), &key)).unwrap();
    assert!(
        !book
            .record(Receipt::sign(outcome("coder"), &rotated))
            .unwrap()
    );
    let mut gym = outcome("gym-side");
    gym.source = Source::GymHeadToHead;
    gym.accepted = false;
    book.record(Receipt::sign(gym.clone(), &key)).unwrap();
    gym.run_id = "coder".into();
    assert!(book.record(Receipt::sign(gym, &key)).is_err());
    let summary = book.summary().unwrap();
    assert_eq!(
        summary.by_class[&TaskClass::Code]["test/model"].rate(),
        Some(0.5)
    );
    for change in 0..2 {
        let mut o = outcome("unverified");
        if change == 0 {
            o.paid_msat = 0;
        } else {
            o.verification_digest.clear();
        }
        assert!(book.record(Receipt::sign(o, &key)).is_err());
    }
    drop(book);
    assert_eq!(
        Book::open(&config)
            .unwrap()
            .summary()
            .unwrap()
            .model("test/model"),
        summary.model("test/model")
    );
    assert_eq!(
        Counts {
            samples: 1,
            ..Counts::default()
        }
        .public()["cost_per_accepted_usd"],
        serde_json::Value::Null
    );
    std::fs::remove_dir_all(config.path.parent().unwrap()).unwrap();
}

#[test]
fn damaged_storage_does_not_silently_fall_back_to_gym_only() {
    let key = key();
    let config = config(&key);
    let book = Book::open(&config).unwrap();
    book.record(Receipt::sign(outcome("one"), &key)).unwrap();
    OpenOptions::new()
        .write(true)
        .open(&config.path)
        .unwrap()
        .set_len(0)
        .unwrap();
    assert!(book.summary().is_err());
    assert!(book.record(Receipt::sign(outcome("two"), &key)).is_err());
    drop(book);
    std::fs::remove_dir_all(config.path.parent().unwrap()).unwrap();
}
