//! The wallet's seed: BIP39 entropy and its recovery words.

/// The wallet's seed. It has no `Debug`, so it cannot reach a log line.
pub struct Seed {
    pub entropy: Vec<u8>,
    pub mnemonic: String,
}

impl Seed {
    /// The seed for 16 or 32 bytes of entropy.
    pub fn from_entropy(entropy: Vec<u8>) -> Result<Self, String> {
        if entropy.len() != 16 && entropy.len() != 32 {
            return Err("The wallet key is unreadable.".into());
        }
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy)
            .map_err(|_| "The wallet key is unreadable.".to_string())?
            .to_string();
        Ok(Self { entropy, mnemonic })
    }

    /// A new seed with 12 recovery words, from the system's randomness.
    pub fn generate() -> Result<Self, String> {
        let mut entropy = vec![0_u8; 16];
        getrandom::fill(&mut entropy).map_err(|_| "No randomness is available.".to_string())?;
        Self::from_entropy(entropy)
    }

    /// The recovery words, one per item, in order.
    pub fn words(&self) -> Vec<&str> {
        self.mnemonic.split_whitespace().collect()
    }

    /// A one-way fingerprint that tells this seed apart from another, for
    /// the words-saved marker. It reveals nothing about the seed.
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"openagents-wallet-words-saved-v1");
        hash.update(&self.entropy);
        hex(&hash.finalize()[..16])
    }
}

/// Check recovery words for a restore and return their entropy as hex, for
/// the host to put in its key store. Errors never repeat the words.
pub fn restore_entropy(words: &str) -> Result<String, String> {
    let words: Vec<String> = words
        .split_whitespace()
        .map(|word| word.to_lowercase())
        .collect();
    if words.is_empty() {
        return Err("No recovery words were entered. Type your 12 or 24 words.".into());
    }
    if words.len() != 12 && words.len() != 24 {
        return Err(format!(
            "Enter 12 or 24 recovery words; that was {}.",
            words.len()
        ));
    }
    let mnemonic =
        bip39::Mnemonic::parse_in(bip39::Language::English, words.join(" ")).map_err(|error| {
            match error {
                bip39::Error::UnknownWord(index) => {
                    format!(
                        "Word {} isn't a recovery word. Check its spelling.",
                        index + 1
                    )
                }
                bip39::Error::InvalidChecksum => {
                    "These words don't form a valid recovery phrase. Check each word and its order."
                        .to_string()
                }
                _ => "These recovery words can't be read.".to_string(),
            }
        })?;
    Ok(hex(&mnemonic.to_entropy()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_seed_has_twelve_words_that_restore_to_it() {
        let seed = Seed::generate().expect("seed");
        assert_eq!(seed.words().len(), 12);
        let entropy = restore_entropy(&seed.mnemonic).expect("valid words");
        assert_eq!(entropy, hex(&seed.entropy));
        assert_ne!(Seed::generate().expect("seed").entropy, seed.entropy);
    }

    #[test]
    fn no_words_says_none_were_entered() {
        let none = restore_entropy("  \n").unwrap_err();
        assert!(none.starts_with("No recovery words were entered"), "{none}");
        let three = restore_entropy("one two three").unwrap_err();
        assert!(three.contains("that was 3"), "{three}");
    }
}
