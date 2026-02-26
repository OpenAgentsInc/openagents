use bip39::Mnemonic;
use bitcoin::Network;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::{PublicKey, SecretKey};

use crate::SparkError;

const BITCOIN_COIN_TYPE: u32 = 0;

#[derive(Clone)]
pub struct SparkSigner {
    mnemonic: String,
    passphrase: String,
    private_key: [u8; 32],
    public_key: [u8; 33],
}

impl SparkSigner {
    pub fn from_mnemonic(mnemonic: &str, passphrase: &str) -> Result<Self, SparkError> {
        let parsed_mnemonic = Mnemonic::parse(mnemonic)
            .map_err(|error| SparkError::InvalidMnemonic(error.to_string()))?;
        let seed = parsed_mnemonic.to_seed(passphrase);

        let secp = Secp256k1::new();
        let master = Xpriv::new_master(Network::Bitcoin, &seed)
            .map_err(|error| SparkError::KeyDerivation(error.to_string()))?;
        let path = DerivationPath::from(vec![
            ChildNumber::from_hardened_idx(44)
                .map_err(|error| SparkError::KeyDerivation(error.to_string()))?,
            ChildNumber::from_hardened_idx(BITCOIN_COIN_TYPE)
                .map_err(|error| SparkError::KeyDerivation(error.to_string()))?,
            ChildNumber::from_hardened_idx(0)
                .map_err(|error| SparkError::KeyDerivation(error.to_string()))?,
            ChildNumber::from_normal_idx(0)
                .map_err(|error| SparkError::KeyDerivation(error.to_string()))?,
            ChildNumber::from_normal_idx(0)
                .map_err(|error| SparkError::KeyDerivation(error.to_string()))?,
        ]);
        let derived = master
            .derive_priv(&secp, &path)
            .map_err(|error| SparkError::KeyDerivation(error.to_string()))?;

        let private_key = derived.private_key.secret_bytes();
        let secret_key = SecretKey::from_slice(&private_key)
            .map_err(|error| SparkError::KeyDerivation(error.to_string()))?;
        let public_key = PublicKey::from_secret_key(&secp, &secret_key).serialize();

        Ok(Self {
            mnemonic: mnemonic.to_string(),
            passphrase: passphrase.to_string(),
            private_key,
            public_key,
        })
    }

    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn passphrase(&self) -> &str {
        &self.passphrase
    }

    pub fn private_key_hex(&self) -> String {
        hex::encode(self.private_key)
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }
}
