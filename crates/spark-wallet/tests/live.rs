//! A computer's Spark wallet against Lightspark's hosted regtest, which
//! needs no API key or funds, over the JSON store computers use. Run with
//! `--ignored`; it reaches the network.

use openagents_spark::Network;
use openagents_spark::computer;
use openagents_spark::model::{Node, QuoteFailure, SendRequest};
use openagents_spark::seed::Seed;

fn fresh() -> Seed {
    let mut entropy = vec![0u8; 16];
    getrandom::fill(&mut entropy).expect("randomness");
    Seed::from_entropy(entropy).expect("a seed")
}

#[test]
#[ignore = "reaches Lightspark's hosted regtest"]
fn a_computer_wallet_connects_reads_and_reopens_from_its_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let seed = fresh();
    let node = computer::open_with(dir.path(), &seed, Network::Regtest).expect("regtest wallet");
    node.sync().expect("sync");
    assert_eq!(node.balance().expect("balance"), 0);
    let address = node.spark_address().expect("address");
    assert!(address.starts_with("spark"), "{address}");
    assert!(node.payments(10).expect("payments").is_empty());
    assert!(matches!(
        node.quote(&SendRequest {
            input: "spark1nonsense".into(),
            ..SendRequest::default()
        }),
        Err(QuoteFailure::Refused(_) | QuoteFailure::NeedsAmount(_))
    ));
    drop(node);
    // The same seed reopens from its file and keeps its address.
    let again = computer::open_with(dir.path(), &seed, Network::Regtest).expect("reopened");
    assert_eq!(again.spark_address().expect("address"), address);
}
