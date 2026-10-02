//! Jev on the person's own keys (BYOK, #10176): under `mine` the resolver
//! never reaches the hosted decision service or a key of ours, and a person
//! with no key that serves Jev gets one plain line.

use jev_hosted::{DOOR, Door, Via, theirs};
use model_access::{Access, ApiKey, Keys, Mode, Provider};

fn door() -> Door<'static> {
    Door {
        url: DOOR,
        model: "jev-1.13.0",
    }
}

#[test]
fn mine_asks_the_persons_keys_in_the_fixed_order() {
    let mut keys = Keys::none();
    keys.insert(Provider::OpenRouter, ApiKey::new("their-openrouter"));
    keys.insert(Provider::Vercel, ApiKey::new("their-gateway"));
    let resolved = theirs(&Access::theirs(keys), &door(), &|config| config)
        .unwrap()
        .expect("mine resolves on their keys");
    match &resolved.via {
        Via::Direct { source } => {
            assert_eq!(source, "your own keys (Vercel AI Gateway, then OpenRouter)")
        }
        other => panic!("not their keys: {other:?}"),
    }
    let described = format!("{:?}", resolved.client);
    assert!(!described.contains("their-openrouter") && !described.contains("their-gateway"));
}

#[test]
fn ours_and_other_doors_keep_the_old_path() {
    let mut keys = Keys::none();
    keys.insert(Provider::TypeSafe, ApiKey::new("ts"));
    let ours = Access::new(Mode::Ours, keys.clone(), &Keys::none());
    assert!(theirs(&ours, &door(), &|c| c).unwrap().is_none());
    let local = Door {
        url: "http://127.0.0.1:9000",
        model: "kev",
    };
    assert!(
        theirs(&Access::theirs(keys), &local, &|c| c)
            .unwrap()
            .is_none()
    );
}

#[test]
fn mine_without_a_jev_key_fails_plainly() {
    let error = theirs(&Access::theirs(Keys::none()), &door(), &|c| c).unwrap_err();
    assert_eq!(error, "Your keys can't use Jev.");
}
