#![cfg(all(feature = "supervisor", unix))]
use openagents_wallet as contract_wallet;
#[path = "../../wallet/tests/support/contract.rs"]
mod contract;
use lnd_sdk::{Error, service::Wallet, supervisor::Regtest};
#[test]
#[ignore = "isolated Docker LightningWallet contract; regtest funds only"]
fn real_lnd_wallet_uses_the_shared_contract() {
    let root = std::path::PathBuf::from(
        std::env::var_os("LND_SDK_REGTEST_ROOT").expect("explicit scratch root"),
    )
    .join("wallet-contract");
    let nodes = Regtest::start(&root).unwrap();
    let address = nodes.lncli("alice", &["newaddress", "p2wkh"]).unwrap()["address"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes.bitcoin(&["sendtoaddress", &address, "0.02"]).unwrap();
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            if nodes.lncli("alice", &["walletbalance"])?["confirmed_balance"]
                .as_str()
                .unwrap_or("0")
                != "0"
            {
                Ok(())
            } else {
                Err(Error::NotSynced)
            }
        })
        .unwrap();
    let alice = nodes.lncli("alice", &["getinfo"]).unwrap()["identity_pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    let bob = nodes.lncli("bob", &["getinfo"]).unwrap()["identity_pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes
        .lncli("alice", &["connect", &format!("{bob}@bob:9735")])
        .unwrap();
    nodes
        .lncli(
            "alice",
            &[
                "openchannel",
                "--node_key",
                &bob,
                "--local_amt",
                "500000",
                "--push_amt",
                "200000",
            ],
        )
        .unwrap();
    nodes.mine(7).unwrap();
    nodes
        .wait(|| {
            for (n, peer) in [("alice", &bob), ("bob", &alice)] {
                if nodes.lncli(n, &["getinfo"])?["num_active_channels"]
                    .as_u64()
                    .unwrap_or(0)
                    == 0
                {
                    return Err(Error::NotSynced);
                }
                nodes.lncli(n, &["getnodeinfo", peer])?;
            }
            Ok(())
        })
        .unwrap();
    let (c, m) = nodes.config("bob").unwrap();
    let receiver = Wallet::connect(c, m).unwrap();
    let (c, m) = nodes.config("alice").unwrap();
    let payer = Wallet::connect(c, m).unwrap();
    contract::bound(&receiver);
    contract::bound(&payer);
    let invoice = contract::receive(&receiver);
    contract::paid(&payer, &invoice.bolt11, 1000);
}
