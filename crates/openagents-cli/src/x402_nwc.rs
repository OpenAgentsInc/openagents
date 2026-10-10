//! NWC (NIP-47) as a buyer wallet (#11147): `--pay-with nwc` pays a
//! Lightning challenge (x402 `lnbtc`, the `Payment` scheme's `lightning`)
//! from a wallet connected over Nostr Wallet Connect.
//!
//! The connection string (`nostr+walletconnect://…`) is a secret: it is
//! read from [`ENV`] and never printed, logged, or written down. The
//! request is a kind `23194` `pay_invoice` sealed to the wallet (NIP-44 v2
//! when its kind `13194` info offers it, NIP-04 otherwise) with a short
//! `expiration`; the answer is the kind `23195` that names it. The payment
//! counts only when the returned preimage hashes to the invoice's payment
//! hash: a wallet's word that it paid is not a proof.
//!
//! NIP-47 has no fee cap on `pay_invoice`, so the connected wallet's own
//! budget bounds fees; a reply whose `fees_paid` is over the cap is
//! reported after the fact.

use std::time::Duration;

use nostr::domain::{
    Event, RelaySigner, Tag, WalletConnection, WalletEncryption, negotiated_encryption,
    open_wallet_info, open_wallet_response, parse_connection, read_wallet_message,
    read_wallet_reply, seal_wallet_message,
};
use openagents_wallet::Proof;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::relay::{Client, unix_now};

/// Where the connection string comes from.
pub(crate) const ENV: &str = "OPENAGENTS_NWC";

const REQUEST_KIND: u16 = 23_194;
const RESPONSE_KIND: u16 = 23_195;
const INFO_KIND: u16 = 13_194;
/// How long the wallet may act on a request.
const REQUEST_LIFETIME: u64 = 60;

/// One `pay_invoice` on its way to the wallet.
pub(crate) struct Request {
    pub(crate) event: Event,
    encryption: WalletEncryption,
    payment_hash: [u8; 32],
    amount_msat: u64,
    bolt11: String,
}

/// The connection from [`ENV`].
pub(crate) fn connection() -> Result<WalletConnection, String> {
    let uri = std::env::var(ENV).map_err(|_| {
        format!(
            "--pay-with nwc needs your wallet's connection string in {ENV} (copy it from your wallet's Nostr Wallet Connect settings: https://nwc.dev)"
        )
    })?;
    parse_connection(uri.trim())
        .map_err(|_| format!("{ENV} isn't a nostr+walletconnect:// connection string"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 32 bytes from 64 lowercase hex digits.
fn unhex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut out = [0_u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn keys(connection: &WalletConnection) -> Result<(SecretKey, XOnlyPublicKey), String> {
    let secret = unhex32(&connection.secret)
        .and_then(|bytes| SecretKey::from_byte_array(bytes).ok())
        .ok_or("the connection's secret isn't a key")?;
    let wallet = connection
        .wallet_pubkey
        .parse::<XOnlyPublicKey>()
        .map_err(|_| "the connection's wallet key isn't a key".to_owned())?;
    Ok((secret, wallet))
}

/// The invoice's network, as x402 names it.
fn network_of(currency: &str) -> Option<&'static str> {
    match currency {
        "bc" => Some(nostr::x402::MAINNET),
        "tb" => Some(nostr::x402::TESTNET),
        _ => None,
    }
}

/// Seal a `pay_invoice` for `bolt11` (on `network`) to the wallet.
pub(crate) fn request(
    connection: &WalletConnection,
    encryption: WalletEncryption,
    bolt11: &str,
    network: &str,
    now: u64,
    nonce: [u8; 32],
) -> Result<Request, String> {
    let invoice =
        nostr::x402::decode_invoice(bolt11).map_err(|error| format!("the invoice: {error}"))?;
    if network_of(invoice.currency()) != Some(network) {
        return Err(format!("the invoice isn't on {network}"));
    }
    let (secret, wallet) = keys(connection)?;
    let command = json!({"method": "pay_invoice", "params": {"invoice": bolt11}}).to_string();
    let sealed = seal_wallet_message(&secret, &wallet, encryption, &command, nonce)
        .map_err(|error| error.to_string())?;
    let signer = RelaySigner::from_secret_hex(&connection.secret)
        .map_err(|_| "the connection's secret isn't a key".to_owned())?;
    let mut tags = vec![
        Tag::new(vec!["p".into(), connection.wallet_pubkey.clone()]),
        Tag::new(vec![
            "expiration".into(),
            (now + REQUEST_LIFETIME).to_string(),
        ]),
    ];
    if encryption == WalletEncryption::Nip44V2 {
        tags.insert(0, Tag::new(vec!["encryption".into(), "nip44_v2".into()]));
    }
    Ok(Request {
        event: signer.sign(now, REQUEST_KIND, tags, sealed),
        encryption,
        payment_hash: invoice.payment_hash(),
        amount_msat: invoice.amount_msat(),
        bolt11: bolt11.to_owned(),
    })
}

/// The proof in `response`, when it answers `request`: `Ok(None)` for an
/// event that answers something else, an error for a wallet error or a
/// preimage that doesn't match the invoice.
pub(crate) fn proof(
    connection: &WalletConnection,
    request: &Request,
    response: &Event,
) -> Result<Option<Proof>, String> {
    if response.kind != RESPONSE_KIND || response.pubkey != connection.wallet_pubkey {
        return Ok(None);
    }
    if response.validate_structure().is_err() || response.validate_crypto().is_err() {
        return Ok(None);
    }
    let Ok(opened) = open_wallet_response(response) else {
        return Ok(None);
    };
    if opened.request_id != request.event.id || opened.client_pubkey != connection.client_pubkey {
        return Ok(None);
    }
    let (secret, wallet) = keys(connection)?;
    let plain = read_wallet_message(&secret, &wallet, request.encryption, &response.content)
        .map_err(|_| "the wallet's answer couldn't be read".to_owned())?;
    let reply = read_wallet_reply(&plain).map_err(|error| error.to_string())?;
    if reply.result_type != "pay_invoice" {
        return Err("the wallet answered something other than the payment".into());
    }
    if let Some(error) = reply.error {
        return Err(format!(
            "the wallet didn't pay ({}): {}",
            error.code, error.message
        ));
    }
    let preimage = reply.preimage.ok_or("the wallet sent no preimage")?;
    let bytes = unhex32(&preimage).ok_or("the preimage isn't 32 bytes of hex")?;
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    if hash != request.payment_hash {
        return Err(
            "the wallet's preimage doesn't match the invoice; nothing is proven paid".into(),
        );
    }
    Ok(Some(Proof {
        payment_hash: hex(&request.payment_hash),
        preimage,
        amount_msat: request.amount_msat,
        fee_msat: reply.fees_paid.unwrap_or(0),
        bolt11: request.bolt11.clone(),
    }))
}

/// The encryption the wallet's kind `13194` offers: NIP-44 v2 when listed,
/// NIP-04 when it has no `encryption` tag. No info event at all is NIP-04,
/// as NIP-47 reads a missing tag.
fn encryption(client: &mut Client, connection: &WalletConnection) -> WalletEncryption {
    let mut found = None;
    let _ = client.subscribe(
        vec![json!({"kinds": [INFO_KIND], "authors": [connection.wallet_pubkey], "limit": 1})],
        false,
        Duration::from_secs(10),
        |event| {
            if event.validate_crypto().is_ok()
                && let Ok(info) = open_wallet_info(event)
            {
                found = Some(negotiated_encryption(&info));
            }
        },
    );
    found.unwrap_or(WalletEncryption::Nip04)
}

/// Pay `bolt11` from the connected wallet, waiting up to `wait` seconds.
pub(crate) fn pay(
    bolt11: &str,
    network: &str,
    max_fee_msat: u64,
    wait: u64,
) -> Result<Proof, String> {
    let connection = connection()?;
    let relay = connection
        .relays
        .first()
        .cloned()
        .ok_or("the connection names no relay")?;
    let signer = RelaySigner::from_secret_hex(&connection.secret)
        .map_err(|_| "the connection's secret isn't a key".to_owned())?;
    let mut client = Client::connect(&relay, signer);
    let encryption = encryption(&mut client, &connection);
    let mut nonce = [0_u8; 32];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut nonce)
        .map_err(|_| "no randomness to seal the request".to_owned())?;
    let request = request(&connection, encryption, bolt11, network, unix_now(), nonce)?;
    // Listen before asking, so the answer can't arrive unseen.
    client.listen(vec![json!({
        "kinds": [RESPONSE_KIND],
        "authors": [connection.wallet_pubkey],
        "#e": [request.event.id],
    })])?;
    let published = client.publish(request.event.clone(), Duration::from_secs(20));
    match published {
        Ok(ack) if ack.accepted => {}
        Ok(ack) => {
            client.close();
            return Err(format!(
                "{relay} refused the payment request: {}",
                ack.message
            ));
        }
        Err(message) => {
            client.close();
            return Err(message);
        }
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(wait.max(1));
    let outcome = loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break Err(format!(
                "the wallet didn't answer within {wait} seconds; check it before paying again"
            ));
        }
        let Some(event) = client.recv(remaining) else {
            continue;
        };
        match proof(&connection, &request, &event) {
            Ok(None) => {}
            Ok(Some(proof)) => break Ok(proof),
            Err(message) => break Err(message),
        }
    };
    client.close();
    let proof = outcome?;
    if proof.fee_msat > max_fee_msat {
        eprintln!(
            "The wallet paid {} msat in fees, over the {max_fee_msat} msat cap; set a lower fee budget in the wallet.",
            proof.fee_msat
        );
    }
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::format_connection;
    use nostr::x402::test_invoice::{number, signed_by, tag, words};

    fn encode(bytes: &[u8]) -> String {
        super::hex(bytes)
    }

    /// A wallet and a connection to it.
    fn pair() -> (RelaySigner, SecretKey, WalletConnection) {
        let wallet = RelaySigner::from_secret_hex(&"47".repeat(32)).unwrap();
        let wallet_secret = SecretKey::from_byte_array([0x47; 32]).unwrap();
        let client = RelaySigner::from_secret_hex(&"48".repeat(32)).unwrap();
        let connection = parse_connection(&format_connection(&WalletConnection {
            wallet_pubkey: wallet.pubkey().to_owned(),
            relays: vec!["wss://relay.example".to_owned()],
            secret: "48".repeat(32),
            client_pubkey: client.pubkey().to_owned(),
            lud16: None,
        }))
        .unwrap();
        (wallet, wallet_secret, connection)
    }

    /// A 1 sat mainnet invoice for `preimage`, shaped as an x402 one.
    fn invoice(preimage: [u8; 32]) -> String {
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&hash));
        fields.extend(tag(16, &words(&[9; 32])));
        fields.extend(tag(23, &words(&[7; 32])));
        fields.extend(tag(6, &number(600)));
        signed_by([0x31; 32], "lnbc10n", fields, false, false, unix_now())
    }

    fn answer(
        wallet: &RelaySigner,
        wallet_secret: &SecretKey,
        connection: &WalletConnection,
        request: &Request,
        body: &str,
    ) -> Event {
        let client: XOnlyPublicKey = connection.client_pubkey.parse().unwrap();
        let sealed =
            seal_wallet_message(wallet_secret, &client, request.encryption, body, [0x49; 32])
                .unwrap();
        wallet.sign(
            unix_now(),
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["p".into(), connection.client_pubkey.clone()]),
                Tag::new(vec!["e".into(), request.event.id.clone()]),
            ],
            sealed,
        )
    }

    #[test]
    fn a_paid_invoice_is_proven_by_its_preimage_only() {
        let (wallet, wallet_secret, connection) = pair();
        let preimage = [0x21; 32];
        let bolt11 = invoice(preimage);
        let request = request(
            &connection,
            WalletEncryption::Nip44V2,
            &bolt11,
            nostr::x402::MAINNET,
            unix_now(),
            [0x50; 32],
        )
        .unwrap();
        assert_eq!(request.event.kind, REQUEST_KIND);
        assert_eq!(request.event.pubkey, connection.client_pubkey);
        let opened = nostr::domain::open_wallet_request(&request.event).unwrap();
        assert_eq!(opened.wallet_pubkey, connection.wallet_pubkey);
        assert_eq!(opened.encryption, WalletEncryption::Nip44V2);
        assert!(opened.expiration.is_some(), "a short-lived request");
        // The wallet reads a pay_invoice for exactly this invoice.
        let client: XOnlyPublicKey = connection.client_pubkey.parse().unwrap();
        let command = nostr::domain::read_wallet_command(
            &read_wallet_message(
                &wallet_secret,
                &client,
                WalletEncryption::Nip44V2,
                &request.event.content,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            command,
            nostr::domain::WalletCommand::PayInvoice { ref invoice, amount_msat: None } if *invoice == bolt11
        ));
        // The connection secret never travels in the request.
        assert!(!request.event.content.contains(&connection.secret));

        let good = answer(
            &wallet,
            &wallet_secret,
            &connection,
            &request,
            &format!(
                r#"{{"result_type":"pay_invoice","result":{{"preimage":"{}","fees_paid":2000}}}}"#,
                encode(&preimage)
            ),
        );
        let proof = proof(&connection, &request, &good).unwrap().unwrap();
        assert_eq!(proof.preimage, encode(&preimage));
        assert_eq!(proof.payment_hash, encode(&Sha256::digest(preimage)));
        assert_eq!(proof.amount_msat, 1_000);
        assert_eq!(proof.fee_msat, 2_000);

        // A preimage for another invoice proves nothing.
        let wrong = answer(
            &wallet,
            &wallet_secret,
            &connection,
            &request,
            &format!(
                r#"{{"result_type":"pay_invoice","result":{{"preimage":"{}"}}}}"#,
                "22".repeat(32)
            ),
        );
        assert!(super::proof(&connection, &request, &wrong).is_err());
        // A wallet error is reported, not taken as paid.
        let failed = answer(
            &wallet,
            &wallet_secret,
            &connection,
            &request,
            r#"{"result_type":"pay_invoice","error":{"code":"INSUFFICIENT_BALANCE","message":"empty"}}"#,
        );
        let error = super::proof(&connection, &request, &failed).unwrap_err();
        assert!(error.contains("INSUFFICIENT_BALANCE"), "{error}");
        // An answer from someone else is ignored.
        let stranger = RelaySigner::from_secret_hex(&"49".repeat(32)).unwrap();
        let forged = answer(&stranger, &wallet_secret, &connection, &request, "{}");
        assert!(
            super::proof(&connection, &request, &forged)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn nip04_requests_carry_no_encryption_tag_and_the_network_must_match() {
        let (_, _, connection) = pair();
        let bolt11 = invoice([0x23; 32]);
        let request = request(
            &connection,
            WalletEncryption::Nip04,
            &bolt11,
            nostr::x402::MAINNET,
            unix_now(),
            [0x51; 32],
        )
        .unwrap();
        assert!(request.event.tag_values("encryption").next().is_none());
        let opened = nostr::domain::open_wallet_request(&request.event).unwrap();
        assert_eq!(opened.encryption, WalletEncryption::Nip04);
        assert!(
            super::request(
                &connection,
                WalletEncryption::Nip04,
                &bolt11,
                nostr::x402::TESTNET,
                unix_now(),
                [0x51; 32],
            )
            .is_err()
        );
    }
}
