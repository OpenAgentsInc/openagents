# Identity Management

This document describes how OpenAgents manages cryptographic identities.

## Overview

OpenAgents uses a **Unified Identity** system where a single BIP39 mnemonic seed phrase derives:

1. **Nostr keypair** - For social identity and event signing (NIP-06)
2. **Spark/Bitcoin keypair** - For Lightning payments and wallet operations (BIP44)

This means one seed phrase = one identity across both Nostr and Bitcoin.

## Key Derivation Paths

From the same mnemonic, two different keypairs are derived using BIP32:

| Purpose | Standard | Derivation Path | Key Type |
|---------|----------|-----------------|----------|
| Nostr | NIP-06 | `m/44'/1237'/0'/0/0` | secp256k1 Schnorr |
| Bitcoin/Spark | BIP44 | `m/44'/0'/0'/0/0` | secp256k1 ECDSA |

**Why different paths?**
- `1237` is the Nostr coin type (from NIP-06)
- `0` is the Bitcoin coin type (from BIP44)
- Different paths ensure the same mnemonic produces different keys for different purposes

## UnifiedIdentity API

The `UnifiedIdentity` struct (in `crates/compute/src/domain/identity.rs`) manages both keypairs:

```rust
use compute::domain::identity::UnifiedIdentity;

// Generate a new 12-word identity
let identity = UnifiedIdentity::generate()?;

// Or generate a more secure 24-word identity
let identity = UnifiedIdentity::generate_24_words()?;

// Or restore from existing mnemonic
let identity = UnifiedIdentity::from_mnemonic(
    "word1 word2 ... word12",
    ""  // Optional passphrase
)?;
```

### Available Methods

**Identity Information:**
```rust
// Get the mnemonic (SENSITIVE - handle carefully)
identity.mnemonic()           // Full mnemonic string
identity.mnemonic_words()     // Vec<&str> of words

// Nostr identity
identity.npub()               // "npub1..." (bech32 public key)
identity.nsec()               // "nsec1..." (bech32 private key - SENSITIVE)
identity.public_key_hex()     // 64-char hex public key
identity.private_key_bytes()  // [u8; 32] raw private key - SENSITIVE
identity.public_key_bytes()   // [u8; 32] raw public key
identity.npub_short()         // "npub1abc...xyz" (truncated for display)
identity.keypair()            // Full Nostr Keypair struct

// Spark/Bitcoin identity
identity.spark_signer()       // SparkSigner for payments
identity.spark_public_key_hex()  // 66-char compressed pubkey
```

### Example: Complete Identity Setup

```rust
use compute::domain::identity::UnifiedIdentity;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate new identity
    let identity = UnifiedIdentity::generate()?;

    // Display for user backup
    println!("Backup these words securely:");
    for (i, word) in identity.mnemonic_words().iter().enumerate() {
        println!("  {}. {}", i + 1, word);
    }

    // Get identifiers
    println!("\nNostr identity: {}", identity.npub()?);
    println!("Spark pubkey: {}", identity.spark_public_key_hex());

    // Use for signing
    let keypair = identity.keypair();
    // ... sign Nostr events with keypair

    // Use for payments
    let signer = identity.spark_signer();
    // ... sign Bitcoin transactions with signer

    Ok(())
}
```

## Mnemonic Formats

OpenAgents supports standard BIP39 mnemonics:

| Word Count | Entropy | Security Level |
|------------|---------|----------------|
| 12 words | 128 bits | Standard |
| 24 words | 256 bits | High security |

**Supported Languages**: English (default BIP39 wordlist)

**Passphrase**: Optional BIP39 passphrase for additional security. The passphrase is used during seed derivation (not stored).

## Storage Locations

Different components store identity in different ways:

### Pylon Provider Identity

**Location**: Referenced in `~/.openagents/pylon/config.toml`

The provider identity mnemonic may be stored in:
- Environment variable
- System keychain
- Plaintext file (development only)

### Agent Identities

**Location**: `~/.openagents/agents/{npub}.toml`

Each agent has its own mnemonic stored in the agent config file:

```toml
name = "MyAgent"
npub = "npub1..."
mnemonic_encrypted = "word1 word2 ... word12"  # TODO: actual encryption
```

**Security Note**: The `mnemonic_encrypted` field is currently **NOT encrypted** - this is a TODO for production.

### Identity Registry

**Location**: `~/.openagents/identities.json`

Tracks multiple identities:

```json
{
  "current": "default",
  "identities": ["default", "agent1", "agent2"]
}
```

Actual mnemonics are stored in the system keychain or `~/.openagents/keychain.txt`.

## Security Considerations

### Sensitive Data

The following data must be handled carefully:

| Data | Sensitivity | Notes |
|------|-------------|-------|
| Mnemonic | CRITICAL | Full access to identity + funds |
| `nsec` | CRITICAL | Nostr private key |
| `private_key_bytes` | CRITICAL | Raw private key |
| Spark private key | CRITICAL | Bitcoin private key |
| `npub` | PUBLIC | Safe to share |
| `public_key_hex` | PUBLIC | Safe to share |

### Best Practices

1. **Never log mnemonics or private keys**
2. **Never transmit mnemonics over network** (except encrypted)
3. **Store mnemonics in system keychain** when available
4. **Set restrictive file permissions** (`chmod 600`)
5. **Prompt user to backup mnemonic** on first generation

### Debug Safety

The `Debug` impl for `UnifiedIdentity` redacts sensitive data:

```rust
impl std::fmt::Debug for UnifiedIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedIdentity")
            .field("npub", &self.npub_short())
            .field("mnemonic", &"[redacted]")
            .finish()
    }
}
```

## SigningService Abstraction

The runtime provides a `SigningService` trait for abstracted signing:

```rust
pub trait SigningService: Send + Sync {
    /// Sign a message and return the signature
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, SigningError>;

    /// Get the public key
    fn public_key(&self) -> &[u8];

    /// Verify a signature
    fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, SigningError>;
}
```

### Implementations

1. **InMemorySigner** - Stub for development
2. **NostrSigner** - In-memory key cache
3. **UnifiedIdentitySigner** - Wraps UnifiedIdentity

## NIP-06 Compliance

OpenAgents follows [NIP-06](https://github.com/nostr-protocol/nips/blob/master/06.md) for Nostr key derivation:

1. Use BIP39 mnemonic (12 or 24 words)
2. Derive seed using BIP39 (with optional passphrase)
3. Derive keypair using BIP32 path `m/44'/1237'/0'/0/0`
4. Use secp256k1 for the keypair

**Test Vector** (from NIP-06):
```
Mnemonic: leader monkey parrot ring guide accident before fence cannon height naive bean
Expected npub: npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
```

## Spark Integration

The Spark signer uses standard Bitcoin BIP44 derivation:

1. Derive seed from mnemonic using BIP39
2. Derive master key using BIP32
3. Derive account key at `m/44'/0'/0'`
4. Derive first receiving address key at `m/44'/0'/0'/0/0`

This ensures compatibility with standard Bitcoin wallets if needed.

## Cross-Platform Support

| Platform | Mnemonic Generation | Storage |
|----------|--------------------| --------|
| Native (Linux/macOS) | Full support | File + Keychain |
| Windows | Full support | File + Credential Manager |
| WASM | Full support | In-memory only |

**WASM Note**: Browser environments cannot persist keys - the identity is lost on page refresh. Consider prompting users to backup their mnemonic.

## Migration Guide

### Importing from Other Wallets

If you have a mnemonic from another wallet:

```rust
// Import existing mnemonic
let identity = UnifiedIdentity::from_mnemonic(
    "your twelve word mnemonic phrase goes here now",
    ""  // Passphrase if used
)?;

// Verify the npub matches expectations
println!("Imported identity: {}", identity.npub()?);
```

### Exporting for Other Wallets

The mnemonic can be imported into any BIP39-compatible wallet:

- **Bitcoin**: Import at path `m/44'/0'/0'/0/0`
- **Nostr**: Import with NIP-06 compatible app
- **Generic**: Use the mnemonic directly

## Troubleshooting

### "Invalid mnemonic"

The mnemonic words must be:
- From the BIP39 English wordlist
- Correctly spelled
- In the correct order
- Have valid checksum

### "Key derivation failed"

Check that:
- Mnemonic is valid
- Passphrase is correct (if used)
- No trailing/leading whitespace

### Different Keys Than Expected

Verify:
- Correct derivation path is being used
- Passphrase matches (empty string vs no passphrase can differ)
- Same mnemonic with no typos
