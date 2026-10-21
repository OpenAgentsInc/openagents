//! Offline checks of the checked-in V4.1 oracle fixtures.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn fixtures_load_and_every_slice_matches_its_manifest() {
    let fixtures = V41OracleFixtures::load_checked_in().unwrap();
    assert_eq!(
        fixtures.revision,
        "1bae0e1088993b9b82de08f59de5be5a34e42170"
    );
    assert!(
        fixtures
            .bucket
            .starts_with("gs://openagents-psionic-oracle/")
    );
    assert_eq!(fixtures.verify(), Vec::<String>::new());
    assert!(fixtures.tensors().count() > 100);
}

#[test]
fn a_whole_tensor_hashes_to_its_recorded_sha256() {
    let fixtures = V41OracleFixtures::load_checked_in().unwrap();
    // The first prompt token's embedding: one BF16 row of 5,120 values.
    let name = "p1-chat-thinking.decode.first.embed.out";
    let entry = fixtures.tensor(name).unwrap();
    assert_eq!(entry.shape, vec![5120]);
    assert_eq!(entry.dtype, "bfloat16");
    let bytes = fixtures.full_bytes(name).expect("kept whole");
    assert!(fixtures.matches(name, bytes).unwrap());
    let mut changed = bytes.to_vec();
    changed[0] ^= 1;
    assert!(!fixtures.matches(name, &changed).unwrap());
}

#[test]
fn every_prompt_has_its_greedy_tokens_and_steps() {
    let fixtures = V41OracleFixtures::load_checked_in().unwrap();
    for id in ["p1-chat-thinking", "p2-code", "p3-2k", "p4-20k"] {
        let prompt = fixtures.prompts.iter().find(|p| p.id == id).unwrap();
        assert_eq!(prompt.n_gen, 32, "{id}");
    }
    for prompt in &fixtures.prompts {
        assert_eq!(prompt.generated_ids.len(), prompt.n_gen, "{}", prompt.id);
        assert_eq!(prompt.steps.len(), prompt.n_gen + 1, "{}", prompt.id);
        for (step, next) in prompt.steps.iter().zip(&prompt.generated_ids) {
            assert_eq!(step.argmax, *next);
            assert_eq!(step.top_ids[0], step.argmax);
            assert_eq!(step.top_ids.len(), 64);
        }
    }
    // The long prompt passes candidate_topk_blocks × candidate_block_size.
    let long = fixtures.prompts.iter().find(|p| p.id == "p4-20k").unwrap();
    assert!(long.prompt_token_ids.len() > 2048 * 8);
}
