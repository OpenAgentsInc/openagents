//! The x402 acceptance check on public testnet. It reaches
//! `https://blockstream.info/testnet/api`, so it is ignored by default:
//! `cargo test -p openagents-wallet --test testnet -- --ignored`.

use std::time::Duration;

use openagents_wallet::ldk::{LdkWallet, generate_mnemonic};
use openagents_wallet::{LightningWallet, Network, WalletConfig, WalletError, config};

#[test]
#[ignore = "reaches public testnet Esplora"]
fn issues_an_invoice_the_x402_validator_accepts() {
    let home =
        std::env::temp_dir().join(format!("openagents-wallet-signet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let wallet_config = WalletConfig::new(Network::Testnet, None).unwrap();
    wallet_config.save(&home).unwrap();
    let (mnemonic, created) = config::load_or_create_seed(&home, true, generate_mnemonic).unwrap();
    assert!(created);
    let wallet = LdkWallet::open(&home, &wallet_config, &mnemonic).unwrap();

    let request_hash = [0x5a; 32];
    let issued = wallet.receive_exact(1000, request_hash, 600).unwrap();
    assert_eq!(issued.pay_to, wallet.node_id());

    let decoded = nostr::x402::decode_invoice(&issued.bolt11).unwrap();
    assert_eq!(decoded.amount_msat(), 1000);
    assert_eq!(decoded.description_hash(), request_hash);
    assert_eq!(hex::encode(decoded.payee()), wallet.node_id());
    assert_eq!(hex::encode(decoded.payment_hash()), issued.payment_hash);
    assert_eq!(decoded.expiry_seconds(), 600);
    assert_eq!(decoded.currency(), "tb");

    // The wallet refuses its own invoice and never exposes an unpaid inbound preimage.
    let own = wallet
        .pay(&issued.bolt11, 10, Duration::from_secs(1))
        .unwrap_err();
    assert!(matches!(own, WalletError::Invalid(_)), "{own}");
    let record = wallet.lookup(decoded.payment_hash()).unwrap().unwrap();
    assert_eq!(record.preimage, None);

    wallet.stop().unwrap();
    let _ = std::fs::remove_dir_all(&home);
}
