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

/// Derive an SA agent keypair for a specific hardened account index.
///
/// This is an alias for NIP-06 account derivation (`m/44'/1237'/account'/0/0`)
/// kept explicit for SA call sites.
pub fn derive_agent_keypair(mnemonic: &str, agent_account: u32) -> Result<Keypair> {
    derive_keypair_with_account(mnemonic, agent_account)
}

pub fn derive_keypair_with_account(mnemonic: &str, account: u32) -> Result<Keypair> {
    derive_keypair_with_segments(mnemonic, vec![(account, true), (0, false), (0, false)])
}

/// Derive an SKL skill keypair under the NIP-06 seed using:
///
/// `m/44'/1237'/agent_account'/skill_type'/skill_index'`
pub fn derive_skill_keypair(
    mnemonic: &str,
    agent_account: u32,
    skill_type: u32,
    skill_index: u32,
) -> Result<Keypair> {
    derive_keypair_with_segments(
        mnemonic,
        vec![
            (agent_account, true),
            (skill_type, true),
            (skill_index, true),
        ],
    )
}

fn derive_keypair_with_segments(mnemonic: &str, tail_segments: Vec<(u32, bool)>) -> Result<Keypair> {
    let parsed_mnemonic = Mnemonic::parse(mnemonic.trim()).context("invalid mnemonic")?;
    let seed = parsed_mnemonic.to_seed("");

    let secp = Secp256k1::new();
    let master =
        Xpriv::new_master(Network::Bitcoin, &seed).context("failed to create master key")?;

    let mut segments = vec![
        ChildNumber::from_hardened_idx(44).context("failed to derive purpose")?,
        ChildNumber::from_hardened_idx(NOSTR_COIN_TYPE).context("failed to derive coin type")?,
    ];
    for (value, hardened) in tail_segments {
        let child = if hardened {
            ChildNumber::from_hardened_idx(value)
                .with_context(|| format!("failed to derive hardened segment {value}"))?
        } else {
            ChildNumber::from_normal_idx(value)
                .with_context(|| format!("failed to derive normal segment {value}"))?
        };
        segments.push(child);
    }
    let path = DerivationPath::from(segments);

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
    use super::{
        derive_agent_keypair, derive_keypair, derive_keypair_with_account, derive_skill_keypair,
    };

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

    #[test]
    fn agent_keypair_alias_matches_account_derivation() -> anyhow::Result<()> {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let via_account = derive_keypair_with_account(mnemonic, 3)?;
        let via_agent = derive_agent_keypair(mnemonic, 3)?;

        assert_eq!(via_account.private_key, via_agent.private_key);
        assert_eq!(via_account.public_key, via_agent.public_key);

        Ok(())
    }

    #[test]
    fn skill_derivation_changes_by_type_and_index() -> anyhow::Result<()> {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let payment_skill = derive_skill_keypair(mnemonic, 0, 1, 0)?;
        let shell_skill = derive_skill_keypair(mnemonic, 0, 3, 0)?;
        let payment_skill_next = derive_skill_keypair(mnemonic, 0, 1, 1)?;

        assert_ne!(payment_skill.private_key, shell_skill.private_key);
        assert_ne!(payment_skill.private_key, payment_skill_next.private_key);
        assert_ne!(payment_skill.public_key, shell_skill.public_key);
        assert_ne!(payment_skill.public_key, payment_skill_next.public_key);

        Ok(())
    }
}
