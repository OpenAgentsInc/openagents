use anyhow::{Context, Result};
use bech32::{Bech32, Hrp};
use bip39::Mnemonic;
use bitcoin::Network;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::{PublicKey, SecretKey};

const NOSTR_COIN_TYPE: u32 = 1237;
const NSEC_HRP: &str = "nsec";
const NPUB_HRP: &str = "npub";

#[derive(Clone)]
pub struct Keypair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
}

impl Keypair {
    pub fn npub(&self) -> Result<String> {
        encode_bech32(NPUB_HRP, &self.public_key)
    }

    pub fn nsec(&self) -> Result<String> {
        encode_bech32(NSEC_HRP, &self.private_key)
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }

    pub fn private_key_hex(&self) -> String {
        hex::encode(self.private_key)
    }
}

pub fn derive_keypair(mnemonic: &str) -> Result<Keypair> {
    derive_keypair_with_account(mnemonic, 0)
}

pub fn derive_keypair_with_account(mnemonic: &str, account: u32) -> Result<Keypair> {
    let parsed_mnemonic = Mnemonic::parse(mnemonic.trim()).context("invalid mnemonic")?;
    let seed = parsed_mnemonic.to_seed("");

    let secp = Secp256k1::new();
    let master =
        Xpriv::new_master(Network::Bitcoin, &seed).context("failed to create master key")?;

    let path = DerivationPath::from(vec![
        ChildNumber::from_hardened_idx(44).context("failed to derive purpose")?,
        ChildNumber::from_hardened_idx(NOSTR_COIN_TYPE).context("failed to derive coin type")?,
        ChildNumber::from_hardened_idx(account).context("failed to derive account")?,
        ChildNumber::from_normal_idx(0).context("failed to derive change")?,
        ChildNumber::from_normal_idx(0).context("failed to derive index")?,
    ]);

    let derived = master
        .derive_priv(&secp, &path)
        .context("failed to derive nostr private key")?;

    let private_key = derived.private_key.secret_bytes();
    let secret_key =
        SecretKey::from_slice(&private_key).context("failed to create secp256k1 secret key")?;
    let public_key_full = PublicKey::from_secret_key(&secp, &secret_key).serialize();
    let mut public_key = [0_u8; 32];
    public_key.copy_from_slice(&public_key_full[1..33]);

    Ok(Keypair {
        private_key,
        public_key,
    })
}

fn encode_bech32(hrp: &str, data: &[u8; 32]) -> Result<String> {
    let parsed_hrp = Hrp::parse(hrp).context("invalid bech32 hrp")?;
    bech32::encode::<Bech32>(parsed_hrp, data).context("failed to encode bech32")
}

#[cfg(test)]
mod tests {
    use super::{derive_keypair, derive_keypair_with_account};

    #[test]
    fn nip06_vector_1_matches() -> anyhow::Result<()> {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let keypair = derive_keypair(mnemonic)?;

        assert_eq!(
            keypair.private_key_hex(),
            "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a"
        );
        assert_eq!(
            keypair.public_key_hex(),
            "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917"
        );
        assert_eq!(
            keypair.nsec()?,
            "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp"
        );
        assert_eq!(
            keypair.npub()?,
            "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu"
        );

        Ok(())
    }

    #[test]
    fn account_index_changes_keys() -> anyhow::Result<()> {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let account_zero = derive_keypair_with_account(mnemonic, 0)?;
        let account_one = derive_keypair_with_account(mnemonic, 1)?;

        assert_ne!(account_zero.private_key, account_one.private_key);
        assert_ne!(account_zero.public_key, account_one.public_key);

        Ok(())
    }
}
