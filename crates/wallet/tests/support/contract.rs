//! Shared assertions for embedded and hosted Lightning wallets.
#![allow(dead_code)]
use super::contract_wallet::{
    IssuedInvoice, LightningWallet, PaymentDirection, PaymentRecord, PaymentStatus, Proof,
    WalletError,
};
use bitcoin::hashes::Hash;
use sha2::{Digest, Sha256};
use std::time::Duration;
pub const AMOUNT: u64 = 21_001;
pub const REQUEST: [u8; 32] = [8; 32];
pub fn fixture(
    nonce: u8,
    amount: u64,
    request: [u8; 32],
    expiry: u32,
) -> (IssuedInvoice, PaymentRecord) {
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[42; 32]).unwrap();
    let preimage = [nonce; 32];
    let invoice = lightning_invoice::InvoiceBuilder::new(lightning_invoice::Currency::Regtest)
        .description_hash(bitcoin::hashes::sha256::Hash::from_byte_array(request))
        .payment_hash(bitcoin::hashes::sha256::Hash::hash(&preimage))
        .payment_secret(lightning_invoice::PaymentSecret([3; 32]))
        .amount_milli_satoshis(amount)
        .expiry_time(Duration::from_secs(expiry.into()))
        .current_timestamp()
        .min_final_cltv_expiry_delta(18)
        .build_signed(|m| bitcoin::secp256k1::Secp256k1::new().sign_ecdsa_recoverable(m, &key))
        .unwrap();
    let issued = IssuedInvoice {
        bolt11: invoice.to_string(),
        payment_hash: invoice.payment_hash().to_string(),
        amount_msat: amount,
        description_hash: hex::encode(request),
        expiry_secs: expiry,
        pay_to: invoice.recover_payee_pub_key().to_string(),
    };
    let record = PaymentRecord {
        payment_hash: issued.payment_hash.clone(),
        direction: PaymentDirection::Outbound,
        status: PaymentStatus::Succeeded,
        amount_msat: Some(amount),
        fee_msat: Some(3),
        preimage: Some(hex::encode(preimage)),
        bolt11: Some(issued.bolt11.clone()),
        updated_at: 1,
    };
    (issued, record)
}
pub fn issued(
    wallet: &dyn LightningWallet,
    invoice: &IssuedInvoice,
    amount: u64,
    request: [u8; 32],
    expiry: u32,
) {
    let signed: lightning_invoice::Bolt11Invoice = invoice.bolt11.parse().unwrap();
    assert_eq!(invoice.amount_msat, amount);
    assert_eq!(signed.amount_milli_satoshis(), Some(amount));
    assert_eq!(invoice.payment_hash, signed.payment_hash().to_string());
    assert_eq!(invoice.pay_to, wallet.node_id());
    assert_eq!(signed.recover_payee_pub_key().to_string(), wallet.node_id());
    assert_eq!(invoice.description_hash, hex::encode(request));
    match signed.description() {
        lightning_invoice::Bolt11InvoiceDescriptionRef::Hash(h) => {
            assert_eq!(h.0.to_string(), hex::encode(request))
        }
        _ => panic!("the invoice needs a description hash"),
    }
    assert_eq!(invoice.expiry_secs, expiry);
    assert_eq!(signed.expiry_time().as_secs(), u64::from(expiry));
    assert_eq!(signed.currency(), lightning_invoice::Currency::Regtest);
}
pub fn receive(wallet: &dyn LightningWallet) -> IssuedInvoice {
    assert!(wallet.receive_exact(0, REQUEST, 600).is_err());
    let a = wallet
        .receive_exact_from_node(&wallet.node_id(), AMOUNT, REQUEST, 600)
        .unwrap();
    issued(wallet, &a, AMOUNT, REQUEST, 600);
    let b = wallet.receive_exact(AMOUNT, REQUEST, 600).unwrap();
    issued(wallet, &b, AMOUNT, REQUEST, 600);
    assert_ne!(a.payment_hash, b.payment_hash);
    for record in wallet.payments().unwrap() {
        assert!(record.preimage.is_none());
        assert!(record.bolt11.is_none());
    }
    a
}
pub fn bound(wallet: &dyn LightningWallet) {
    let wrong = "ff".repeat(33);
    assert_ne!(wrong, wallet.node_id());
    assert!(matches!(
        wallet.receive_exact_from_node(&wrong, AMOUNT, REQUEST, 600),
        Err(WalletError::NodeMismatch { .. })
    ));
    assert!(matches!(
        wallet.pay_from_node(&wrong, "invalid invoice", 0, Duration::ZERO),
        Err(WalletError::NodeMismatch { .. })
    ));
    assert!(matches!(
        wallet.lookup_from_node(&wrong, [0; 32]),
        Err(WalletError::NodeMismatch { .. })
    ));
}
pub fn completed(proof: &Proof, record: &PaymentRecord, invoice: &str, max_fee: u64) {
    let signed: lightning_invoice::Bolt11Invoice = invoice.parse().unwrap();
    let hash = signed.payment_hash().to_string();
    assert_eq!(proof.payment_hash, hash);
    assert_eq!(record.payment_hash, hash);
    assert_eq!(record.status, PaymentStatus::Succeeded);
    assert_eq!(record.direction, PaymentDirection::Outbound);
    assert_eq!(proof.amount_msat, signed.amount_milli_satoshis().unwrap());
    assert_eq!(record.amount_msat, Some(proof.amount_msat));
    assert_eq!(record.fee_msat, Some(proof.fee_msat));
    assert!(proof.fee_msat <= max_fee);
    assert_eq!(proof.bolt11, invoice);
    let preimage = super::contract_wallet::parse_hash32(&proof.preimage).unwrap();
    assert_eq!(hex::encode(Sha256::digest(preimage)), hash);
}
pub fn paid(wallet: &dyn LightningWallet, invoice: &str, max_fee: u64) {
    let a = wallet
        .pay_from_node(&wallet.node_id(), invoice, max_fee, Duration::from_secs(30))
        .unwrap();
    let record = wallet
        .lookup_from_node(
            &wallet.node_id(),
            super::contract_wallet::parse_hash32(&a.payment_hash).unwrap(),
        )
        .unwrap()
        .unwrap();
    completed(&a, &record, invoice, max_fee);
    let b = wallet
        .pay(invoice, max_fee, Duration::from_secs(5))
        .unwrap();
    assert_eq!(a, b);
    for p in wallet.payments().unwrap() {
        assert!(p.preimage.is_none());
        assert!(p.bolt11.is_none());
    }
}
