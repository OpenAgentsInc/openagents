use anyhow::{Context, Result};
use bip39::Mnemonic;
use clap::{Args, Parser, Subcommand, ValueEnum};
use nostr::{
    AddressPointer, Event, EventPointer, EventTemplate, HttpAuth, HttpMethod, KeySecurity,
    Nip05Identifier, Nip05Response, Nip19Entity, ProfilePointer, UnsignedEvent, ValidationParams,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
pub struct NostrArgs {
    #[command(subcommand)]
    pub command: NostrCommand,
}

#[derive(Subcommand)]
pub enum NostrCommand {
    /// Generate a new mnemonic and derive a keypair (NIP-06)
    New(NewArgs),
    /// Derive a keypair from an existing mnemonic (NIP-06)
    Derive(DeriveArgs),
    /// Encode hex keys to bech32 (npub/nsec)
    Encode(EncodeArgs),
    /// Decode bech32 keys (npub/nsec) to hex
    Decode(DecodeArgs),
    /// Derive the BIP39 seed from a mnemonic (NIP-06)
    Seed(SeedArgs),
    /// Derive public key/npub from a secret key
    Pubkey(PubkeyArgs),
    /// NIP-01 event helpers (sign/verify/hash/validate)
    Event(EventArgs),
    /// NIP-19 bech32 entities (npub/nsec/note/nprofile/nevent/naddr)
    Nip19(Nip19Args),
    /// NIP-21 nostr: URI helpers
    Uri(UriArgs),
    /// NIP-04 encryption helpers
    Nip04(Nip04Args),
    /// NIP-44 encryption helpers
    Nip44(Nip44Args),
    /// NIP-26 delegation helpers
    Nip26(Nip26Args),
    /// NIP-42 relay auth helpers
    Nip42(Nip42Args),
    /// NIP-49 private key encryption helpers
    Nip49(Nip49Args),
    /// NIP-98 HTTP auth helpers
    Nip98(Nip98Args),
    /// NIP-05 identifier helpers
    Nip05(Nip05Args),
    /// NIP-13 proof-of-work helpers
    Pow(PowArgs),
}

#[derive(Args)]
pub struct NewArgs {
    /// Number of words in the mnemonic (12 or 24)
    #[arg(long, default_value = "12")]
    pub words: u16,
    /// Account index for derivation (m/44'/1237'/<account>'/0/0)
    #[arg(long)]
    pub account: Option<u32>,
    /// Agent index (maps to account = agent + 1)
    #[arg(long)]
    pub agent: Option<u32>,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
    /// Do not print the mnemonic
    #[arg(long)]
    pub no_mnemonic: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DeriveArgs {
    /// Mnemonic phrase (12 or 24 words)
    #[arg(long, required_unless_present = "stdin")]
    pub mnemonic: Option<String>,
    /// Read mnemonic from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Account index for derivation (m/44'/1237'/<account>'/0/0)
    #[arg(long)]
    pub account: Option<u32>,
    /// Agent index (maps to account = agent + 1)
    #[arg(long)]
    pub agent: Option<u32>,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
    /// Print the mnemonic in output
    #[arg(long)]
    pub show_mnemonic: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EncodeArgs {
    /// Public key hex (64 chars) to encode as npub
    #[arg(long)]
    pub public: Option<String>,
    /// Private key hex (64 chars) to encode as nsec
    #[arg(long)]
    pub private: Option<String>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct DecodeArgs {
    /// npub to decode to hex
    #[arg(long)]
    pub npub: Option<String>,
    /// nsec to decode to hex
    #[arg(long)]
    pub nsec: Option<String>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct SeedArgs {
    /// Mnemonic phrase (12 or 24 words)
    #[arg(long, required_unless_present = "stdin")]
    pub mnemonic: Option<String>,
    /// Read mnemonic from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct PubkeyArgs {
    /// Private key hex (64 chars)
    #[arg(long)]
    pub secret: Option<String>,
    /// Private key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EventArgs {
    #[command(subcommand)]
    pub command: EventCommand,
}

#[derive(Subcommand)]
pub enum EventCommand {
    /// Sign an event template with a secret key
    Sign(EventSignArgs),
    /// Verify a signed event (hash + signature)
    Verify(EventVerifyArgs),
    /// Compute the event id from event data
    Hash(EventHashArgs),
    /// Validate event structure (no signature verification)
    Validate(EventValidateArgs),
    /// Serialize an unsigned event for hashing
    Serialize(EventSerializeArgs),
    /// Classify an event kind (regular/replaceable/ephemeral/addressable)
    Kind(EventKindArgs),
}

#[derive(Args)]
pub struct EventSignArgs {
    /// Secret key hex (64 chars)
    #[arg(long)]
    pub secret: Option<String>,
    /// Secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Event template JSON string
    #[arg(long)]
    pub template_json: Option<String>,
    /// Path to JSON file containing an event template
    #[arg(long)]
    pub template_file: Option<PathBuf>,
    /// Read event template JSON from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Event kind (when building a template from flags)
    #[arg(long)]
    pub kind: Option<u16>,
    /// Event content (when building a template from flags)
    #[arg(long)]
    pub content: Option<String>,
    /// Tags JSON (array of arrays)
    #[arg(long)]
    pub tags_json: Option<String>,
    /// created_at timestamp (seconds since epoch)
    #[arg(long)]
    pub created_at: Option<u64>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Clone)]
pub struct EventInputArgs {
    /// Signed event JSON string
    #[arg(long)]
    pub event_json: Option<String>,
    /// Path to JSON file containing the event
    #[arg(long)]
    pub event_file: Option<PathBuf>,
    /// Read event JSON from stdin
    #[arg(long)]
    pub stdin: bool,
}

#[derive(Args)]
pub struct EventVerifyArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EventHashArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EventValidateArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EventSerializeArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EventKindArgs {
    /// Event kind to classify
    #[arg(long)]
    pub kind: u16,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip19Args {
    #[command(subcommand)]
    pub command: Nip19Command,
}

#[derive(Subcommand)]
pub enum Nip19Command {
    /// Encode NIP-19 entities
    Encode(Nip19EncodeArgs),
    /// Decode a NIP-19 entity
    Decode(Nip19DecodeArgs),
}

#[derive(ValueEnum, Clone, Debug)]
pub enum Nip19EntityKind {
    Npub,
    Nsec,
    Note,
    Nprofile,
    Nevent,
    Naddr,
}

#[derive(Args)]
pub struct Nip19EncodeArgs {
    /// Entity type to encode
    #[arg(long, value_enum)]
    pub entity: Nip19EntityKind,
    /// Public key hex (64 chars)
    #[arg(long)]
    pub pubkey: Option<String>,
    /// Public key in npub format
    #[arg(long)]
    pub npub: Option<String>,
    /// Secret key hex (64 chars)
    #[arg(long)]
    pub secret: Option<String>,
    /// Secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Event id hex (64 chars)
    #[arg(long)]
    pub event_id: Option<String>,
    /// Address identifier (d tag)
    #[arg(long)]
    pub identifier: Option<String>,
    /// Author public key hex (64 chars)
    #[arg(long)]
    pub author: Option<String>,
    /// Event kind (for nevent/naddr)
    #[arg(long)]
    pub kind: Option<u32>,
    /// Relay hint (repeatable)
    #[arg(long)]
    pub relay: Vec<String>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip19DecodeArgs {
    /// NIP-19 bech32 string to decode
    #[arg(long, required_unless_present = "stdin")]
    pub bech32: Option<String>,
    /// Read entity from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct UriArgs {
    #[command(subcommand)]
    pub command: UriCommand,
}

#[derive(Subcommand)]
pub enum UriCommand {
    /// Convert a NIP-19 entity to nostr: URI
    Encode(UriEncodeArgs),
    /// Parse a nostr: URI into a NIP-19 entity
    Decode(UriDecodeArgs),
    /// Strip the nostr: prefix from a URI
    Strip(UriStripArgs),
}

#[derive(Args)]
pub struct UriEncodeArgs {
    /// NIP-19 entity (npub/note/nprofile/nevent/naddr)
    #[arg(long, required_unless_present = "stdin")]
    pub entity: Option<String>,
    /// Read entity from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct UriDecodeArgs {
    /// nostr: URI or NIP-19 entity
    #[arg(long, required_unless_present = "stdin")]
    pub uri: Option<String>,
    /// Read URI from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct UriStripArgs {
    /// nostr: URI to strip
    #[arg(long, required_unless_present = "stdin")]
    pub uri: Option<String>,
    /// Read URI from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip04Args {
    #[command(subcommand)]
    pub command: Nip04Command,
}

#[derive(Subcommand)]
pub enum Nip04Command {
    Encrypt(Nip04EncryptArgs),
    Decrypt(Nip04DecryptArgs),
}

#[derive(Args)]
pub struct Nip04EncryptArgs {
    /// Sender secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Sender secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Recipient public key hex (32/33/65 bytes)
    #[arg(long)]
    pub pubkey: Option<String>,
    /// Recipient public key in npub format
    #[arg(long)]
    pub npub: Option<String>,
    /// Plaintext to encrypt
    #[arg(long)]
    pub plaintext: Option<String>,
    /// Read plaintext from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip04DecryptArgs {
    /// Recipient secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Recipient secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Sender public key hex (32/33/65 bytes)
    #[arg(long)]
    pub pubkey: Option<String>,
    /// Sender public key in npub format
    #[arg(long)]
    pub npub: Option<String>,
    /// Ciphertext to decrypt
    #[arg(long)]
    pub ciphertext: Option<String>,
    /// Read ciphertext from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip44Args {
    #[command(subcommand)]
    pub command: Nip44Command,
}

#[derive(Subcommand)]
pub enum Nip44Command {
    Encrypt(Nip44EncryptArgs),
    Decrypt(Nip44DecryptArgs),
}

#[derive(Args)]
pub struct Nip44EncryptArgs {
    /// Sender secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Sender secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Recipient public key hex (32/33/65 bytes)
    #[arg(long)]
    pub pubkey: Option<String>,
    /// Recipient public key in npub format
    #[arg(long)]
    pub npub: Option<String>,
    /// Plaintext to encrypt
    #[arg(long)]
    pub plaintext: Option<String>,
    /// Read plaintext from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip44DecryptArgs {
    /// Recipient secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Recipient secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Sender public key hex (32/33/65 bytes)
    #[arg(long)]
    pub pubkey: Option<String>,
    /// Sender public key in npub format
    #[arg(long)]
    pub npub: Option<String>,
    /// Payload to decrypt
    #[arg(long)]
    pub payload: Option<String>,
    /// Read payload from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip26Args {
    #[command(subcommand)]
    pub command: Nip26Command,
}

#[derive(Subcommand)]
pub enum Nip26Command {
    /// Create a delegation token
    Create(Nip26CreateArgs),
    /// Verify a delegation token
    Verify(Nip26VerifyArgs),
    /// Validate delegation token and conditions against an event
    Validate(Nip26ValidateArgs),
}

#[derive(Args)]
pub struct Nip26CreateArgs {
    /// Delegator secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Delegator secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Delegatee public key hex
    #[arg(long)]
    pub delegatee_pubkey: Option<String>,
    /// Delegatee public key in npub format
    #[arg(long)]
    pub delegatee_npub: Option<String>,
    /// Conditions query string (e.g. kind=1&created_at>123)
    #[arg(long)]
    pub conditions: String,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip26VerifyArgs {
    /// Delegator public key hex
    #[arg(long)]
    pub delegator_pubkey: Option<String>,
    /// Delegator public key in npub format
    #[arg(long)]
    pub delegator_npub: Option<String>,
    /// Delegatee public key hex
    #[arg(long)]
    pub delegatee_pubkey: Option<String>,
    /// Delegatee public key in npub format
    #[arg(long)]
    pub delegatee_npub: Option<String>,
    /// Conditions query string
    #[arg(long)]
    pub conditions: String,
    /// Delegation token (hex signature)
    #[arg(long)]
    pub token: String,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip26ValidateArgs {
    /// Delegator public key hex
    #[arg(long)]
    pub delegator_pubkey: Option<String>,
    /// Delegator public key in npub format
    #[arg(long)]
    pub delegator_npub: Option<String>,
    /// Delegatee public key hex
    #[arg(long)]
    pub delegatee_pubkey: Option<String>,
    /// Delegatee public key in npub format
    #[arg(long)]
    pub delegatee_npub: Option<String>,
    /// Conditions query string
    #[arg(long)]
    pub conditions: String,
    /// Delegation token (hex signature)
    #[arg(long)]
    pub token: String,
    /// Event kind to validate
    #[arg(long)]
    pub event_kind: u16,
    /// Event created_at to validate
    #[arg(long)]
    pub event_created_at: u64,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip42Args {
    #[command(subcommand)]
    pub command: Nip42Command,
}

#[derive(Subcommand)]
pub enum Nip42Command {
    /// Create and sign a relay auth event
    Auth(Nip42AuthArgs),
    /// Validate a relay auth event
    Validate(Nip42ValidateArgs),
}

#[derive(Args)]
pub struct Nip42AuthArgs {
    /// Relay URL
    #[arg(long)]
    pub relay: String,
    /// Challenge string
    #[arg(long)]
    pub challenge: String,
    /// Secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Override created_at (seconds since epoch)
    #[arg(long)]
    pub created_at: Option<u64>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip42ValidateArgs {
    /// Relay URL
    #[arg(long)]
    pub relay: String,
    /// Challenge string
    #[arg(long)]
    pub challenge: String,
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Current timestamp override
    #[arg(long)]
    pub now: Option<u64>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip49Args {
    #[command(subcommand)]
    pub command: Nip49Command,
}

#[derive(Subcommand)]
pub enum Nip49Command {
    Encrypt(Nip49EncryptArgs),
    Decrypt(Nip49DecryptArgs),
}

#[derive(ValueEnum, Clone, Debug)]
pub enum KeySecurityArg {
    Insecure,
    Secure,
    Unknown,
}

#[derive(Args)]
pub struct Nip49EncryptArgs {
    /// Private key hex (64 chars)
    #[arg(long)]
    pub secret: Option<String>,
    /// Private key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Password
    #[arg(long)]
    pub password: String,
    /// scrypt log_n (10-30, default 16)
    #[arg(long, default_value = "16")]
    pub log_n: u8,
    /// Key security indicator
    #[arg(long, value_enum, default_value = "unknown")]
    pub key_security: KeySecurityArg,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip49DecryptArgs {
    /// ncryptsec bech32 string
    #[arg(long)]
    pub ncryptsec: String,
    /// Password
    #[arg(long)]
    pub password: String,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip98Args {
    #[command(subcommand)]
    pub command: Nip98Command,
}

#[derive(Subcommand)]
pub enum Nip98Command {
    /// Create and sign an HTTP auth event
    Create(Nip98CreateArgs),
    /// Validate an HTTP auth event
    Validate(Nip98ValidateArgs),
    /// Decode a NIP-98 Authorization header
    Decode(Nip98DecodeArgs),
}

#[derive(Args)]
pub struct Nip98CreateArgs {
    /// Absolute URL (including query parameters)
    #[arg(long)]
    pub url: String,
    /// HTTP method (GET/POST/...)
    #[arg(long)]
    pub method: String,
    /// Secret key hex
    #[arg(long)]
    pub secret: Option<String>,
    /// Secret key in nsec format
    #[arg(long)]
    pub nsec: Option<String>,
    /// Request payload as a string
    #[arg(long)]
    pub payload: Option<String>,
    /// Read payload from file
    #[arg(long)]
    pub payload_file: Option<PathBuf>,
    /// Read payload from stdin
    #[arg(long)]
    pub payload_stdin: bool,
    /// Override created_at (seconds since epoch)
    #[arg(long)]
    pub created_at: Option<u64>,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip98ValidateArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Absolute URL (including query parameters)
    #[arg(long)]
    pub url: String,
    /// HTTP method (GET/POST/...)
    #[arg(long)]
    pub method: String,
    /// Request payload as a string
    #[arg(long)]
    pub payload: Option<String>,
    /// Read payload from file
    #[arg(long)]
    pub payload_file: Option<PathBuf>,
    /// Read payload from stdin
    #[arg(long)]
    pub payload_stdin: bool,
    /// Current timestamp override
    #[arg(long)]
    pub now: Option<u64>,
    /// Timestamp window in seconds
    #[arg(long)]
    pub window: Option<u64>,
    /// Verify event signature as well
    #[arg(long)]
    pub verify_sig: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip98DecodeArgs {
    /// Authorization header ("Nostr <base64>")
    #[arg(long, required_unless_present = "stdin")]
    pub header: Option<String>,
    /// Read header from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip05Args {
    #[command(subcommand)]
    pub command: Nip05Command,
}

#[derive(Subcommand)]
pub enum Nip05Command {
    /// Parse a NIP-05 identifier
    Parse(Nip05ParseArgs),
    /// Compute the .well-known URL for an identifier
    WellKnown(Nip05WellKnownArgs),
    /// Verify a NIP-05 response JSON against an identifier
    Verify(Nip05VerifyArgs),
}

#[derive(Args)]
pub struct Nip05ParseArgs {
    /// Identifier (name@domain)
    #[arg(long, required_unless_present = "stdin")]
    pub identifier: Option<String>,
    /// Read identifier from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip05WellKnownArgs {
    /// Identifier (name@domain)
    #[arg(long, required_unless_present = "stdin")]
    pub identifier: Option<String>,
    /// Read identifier from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct Nip05VerifyArgs {
    /// Identifier (name@domain)
    #[arg(long)]
    pub identifier: String,
    /// Expected pubkey hex
    #[arg(long)]
    pub pubkey: Option<String>,
    /// Expected pubkey in npub format
    #[arg(long)]
    pub npub: Option<String>,
    /// NIP-05 response JSON string
    #[arg(long)]
    pub response_json: Option<String>,
    /// Path to JSON file containing NIP-05 response
    #[arg(long)]
    pub response_file: Option<PathBuf>,
    /// Read response JSON from stdin
    #[arg(long)]
    pub stdin: bool,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct PowArgs {
    #[command(subcommand)]
    pub command: PowCommand,
}

#[derive(Subcommand)]
pub enum PowCommand {
    /// Calculate difficulty from an event id or event JSON
    Difficulty(PowDifficultyArgs),
    /// Check if an event meets a minimum difficulty
    Check(PowCheckArgs),
    /// Parse nonce tag from an event
    Nonce(PowNonceArgs),
}

#[derive(Args)]
pub struct PowDifficultyArgs {
    /// Event id hex (64 chars)
    #[arg(long)]
    pub event_id: Option<String>,
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct PowCheckArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Minimum difficulty required
    #[arg(long)]
    pub min: u32,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct PowNonceArgs {
    #[command(flatten)]
    pub input: EventInputArgs,
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Serialize)]
struct KeypairOutput {
    mnemonic: Option<String>,
    account: u32,
    agent: Option<u32>,
    public_key_hex: String,
    private_key_hex: String,
    npub: String,
    nsec: String,
}

#[derive(Serialize)]
struct EncodeOutput {
    npub: Option<String>,
    nsec: Option<String>,
}

#[derive(Serialize)]
struct DecodeOutput {
    public_key_hex: Option<String>,
    private_key_hex: Option<String>,
}

#[derive(Serialize)]
struct SeedOutput {
    seed_hex: String,
}

#[derive(Serialize)]
struct PubkeyOutput {
    public_key_hex: String,
    npub: String,
}

#[derive(Serialize)]
struct EventVerifyOutput {
    struct_valid: bool,
    id_matches: bool,
    signature_valid: bool,
}

#[derive(Serialize)]
struct EventHashOutput {
    id: String,
}

#[derive(Serialize)]
struct EventValidateOutput {
    valid: bool,
    signed: bool,
}

#[derive(Serialize)]
struct EventSerializeOutput {
    serialized: String,
}

#[derive(Serialize)]
struct EventKindOutput {
    classification: String,
    regular: bool,
    replaceable: bool,
    ephemeral: bool,
    addressable: bool,
}

#[derive(Serialize)]
struct Nip19EncodeOutput {
    entity: String,
    value: String,
}

#[derive(Serialize)]
struct Nip19DecodeOutput {
    entity: String,
    pubkey_hex: Option<String>,
    secret_key_hex: Option<String>,
    event_id_hex: Option<String>,
    identifier: Option<String>,
    relays: Vec<String>,
    author_hex: Option<String>,
    kind: Option<u32>,
}

#[derive(Serialize)]
struct UriOutput {
    uri: String,
}

#[derive(Serialize)]
struct UriStripOutput {
    stripped: String,
}

#[derive(Serialize)]
struct Nip04Output {
    ciphertext: String,
}

#[derive(Serialize)]
struct Nip04DecryptOutput {
    plaintext: String,
}

#[derive(Serialize)]
struct Nip44Output {
    payload: String,
}

#[derive(Serialize)]
struct Nip44DecryptOutput {
    plaintext: String,
}

#[derive(Serialize)]
struct Nip26CreateOutput {
    delegation_string: String,
    token: String,
}

#[derive(Serialize)]
struct Nip26VerifyOutput {
    valid: bool,
}

#[derive(Serialize)]
struct Nip26ValidateOutput {
    valid: bool,
}

#[derive(Serialize)]
struct Nip42AuthOutput {
    event: Event,
}

#[derive(Serialize)]
struct Nip42ValidateOutput {
    valid: bool,
}

#[derive(Serialize)]
struct Nip49EncryptOutput {
    ncryptsec: String,
}

#[derive(Serialize)]
struct Nip49DecryptOutput {
    private_key_hex: String,
    nsec: String,
    log_n: u8,
    key_security: String,
}

#[derive(Serialize)]
struct Nip98CreateOutput {
    authorization_header: String,
    event: Event,
}

#[derive(Serialize)]
struct Nip98ValidateOutput {
    valid: bool,
    signature_valid: Option<bool>,
}

#[derive(Serialize)]
struct Nip98DecodeOutput {
    event_json: String,
}

#[derive(Serialize)]
struct Nip05ParseOutput {
    identifier: String,
    local: String,
    domain: String,
    is_root: bool,
}

#[derive(Serialize)]
struct Nip05WellKnownOutput {
    url: String,
}

#[derive(Serialize)]
struct Nip05VerifyOutput {
    valid: bool,
    relays: Option<Vec<String>>,
}

#[derive(Serialize)]
struct PowDifficultyOutput {
    difficulty: u32,
}

#[derive(Serialize)]
struct PowCheckOutput {
    meets_minimum: bool,
    difficulty: u32,
}

#[derive(Serialize)]
struct PowNonceOutput {
    nonce: Option<String>,
    target: Option<u32>,
}

#[derive(Deserialize)]
struct EventTemplateJson {
    #[serde(default)]
    created_at: Option<u64>,
    kind: u16,
    #[serde(default)]
    tags: Vec<Vec<String>>,
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct UnsignedEventJson {
    pubkey: String,
    created_at: u64,
    kind: u16,
    #[serde(default)]
    tags: Vec<Vec<String>>,
    #[serde(default)]
    content: String,
}

pub fn run(args: NostrArgs) -> Result<()> {
    match args.command {
        NostrCommand::New(args) => new_keypair(args),
        NostrCommand::Derive(args) => derive_keypair(args),
        NostrCommand::Encode(args) => encode_keys(args),
        NostrCommand::Decode(args) => decode_keys(args),
        NostrCommand::Seed(args) => derive_seed(args),
        NostrCommand::Pubkey(args) => derive_pubkey(args),
        NostrCommand::Event(args) => run_event(args),
        NostrCommand::Nip19(args) => run_nip19(args),
        NostrCommand::Uri(args) => run_uri(args),
        NostrCommand::Nip04(args) => run_nip04(args),
        NostrCommand::Nip44(args) => run_nip44(args),
        NostrCommand::Nip26(args) => run_nip26(args),
        NostrCommand::Nip42(args) => run_nip42(args),
        NostrCommand::Nip49(args) => run_nip49(args),
        NostrCommand::Nip98(args) => run_nip98(args),
        NostrCommand::Nip05(args) => run_nip05(args),
        NostrCommand::Pow(args) => run_pow(args),
    }
}

fn new_keypair(args: NewArgs) -> Result<()> {
    let mnemonic = generate_mnemonic(args.words)?;
    let (account, agent) = resolve_account(args.account, args.agent)?;
    let keypair = nostr::derive_keypair_full(&mnemonic, &args.passphrase, account)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = KeypairOutput {
        mnemonic: if args.no_mnemonic { None } else { Some(mnemonic) },
        account,
        agent,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: keypair.private_key_hex(),
        npub: keypair.npub().map_err(|e| anyhow::anyhow!(e.to_string()))?,
        nsec: keypair.nsec().map_err(|e| anyhow::anyhow!(e.to_string()))?,
    };

    print_output(&output, args.json)
}

fn derive_keypair(args: DeriveArgs) -> Result<()> {
    let mnemonic = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.mnemonic
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Mnemonic is required"))?
    };

    validate_mnemonic(&mnemonic)?;

    let (account, agent) = resolve_account(args.account, args.agent)?;
    let keypair = nostr::derive_keypair_full(&mnemonic, &args.passphrase, account)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = KeypairOutput {
        mnemonic: if args.show_mnemonic { Some(mnemonic) } else { None },
        account,
        agent,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: keypair.private_key_hex(),
        npub: keypair.npub().map_err(|e| anyhow::anyhow!(e.to_string()))?,
        nsec: keypair.nsec().map_err(|e| anyhow::anyhow!(e.to_string()))?,
    };

    print_output(&output, args.json)
}

fn encode_keys(args: EncodeArgs) -> Result<()> {
    if args.public.is_none() && args.private.is_none() {
        return Err(anyhow::anyhow!("Provide --public and/or --private"));
    }

    let npub = match args.public.as_deref() {
        Some(hex_str) => {
            let bytes = parse_hex_32(hex_str)?;
            Some(nostr::public_key_to_npub(&bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?)
        }
        None => None,
    };

    let nsec = match args.private.as_deref() {
        Some(hex_str) => {
            let bytes = parse_hex_32(hex_str)?;
            Some(nostr::private_key_to_nsec(&bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?)
        }
        None => None,
    };

    let output = EncodeOutput { npub, nsec };
    print_output(&output, args.json)
}

fn decode_keys(args: DecodeArgs) -> Result<()> {
    if args.npub.is_none() && args.nsec.is_none() {
        return Err(anyhow::anyhow!("Provide --npub and/or --nsec"));
    }

    let public_key_hex = match args.npub.as_deref() {
        Some(npub) => {
            let bytes = nostr::npub_to_public_key(npub).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Some(hex::encode(bytes))
        }
        None => None,
    };

    let private_key_hex = match args.nsec.as_deref() {
        Some(nsec) => {
            let bytes = nostr::nsec_to_private_key(nsec).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Some(hex::encode(bytes))
        }
        None => None,
    };

    let output = DecodeOutput {
        public_key_hex,
        private_key_hex,
    };

    print_output(&output, args.json)
}

fn derive_seed(args: SeedArgs) -> Result<()> {
    let mnemonic = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.mnemonic
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Mnemonic is required"))?
    };

    validate_mnemonic(&mnemonic)?;

    let seed = nostr::mnemonic_to_seed(&mnemonic, &args.passphrase)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = SeedOutput {
        seed_hex: hex::encode(seed),
    };
    print_output(&output, args.json)
}

fn derive_pubkey(args: PubkeyArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let public_key_hex = nostr::get_public_key_hex(&secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let npub = nostr::public_key_to_npub(&parse_hex_32(&public_key_hex)?)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = PubkeyOutput {
        public_key_hex,
        npub,
    };

    print_output(&output, args.json)
}

fn run_event(args: EventArgs) -> Result<()> {
    match args.command {
        EventCommand::Sign(args) => event_sign(args),
        EventCommand::Verify(args) => event_verify(args),
        EventCommand::Hash(args) => event_hash(args),
        EventCommand::Validate(args) => event_validate(args),
        EventCommand::Serialize(args) => event_serialize(args),
        EventCommand::Kind(args) => event_kind(args),
    }
}

fn event_sign(args: EventSignArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;

    let template = if args.template_json.is_some() || args.template_file.is_some() || args.stdin {
        let raw = read_input_string(
            args.template_json,
            args.template_file,
            args.stdin,
            "event template",
            true,
        )?;
        let template_json: EventTemplateJson =
            serde_json::from_str(&raw).context("invalid template JSON")?;
        EventTemplate {
            created_at: template_json.created_at.unwrap_or_else(now_timestamp),
            kind: template_json.kind,
            tags: template_json.tags,
            content: template_json.content,
        }
    } else {
        let kind = args
            .kind
            .ok_or_else(|| anyhow::anyhow!("--kind or --template-json/--template-file/--stdin is required"))?;
        let content = args.content.unwrap_or_default();
        let tags = parse_tags_json(args.tags_json)?;
        let created_at = args.created_at.unwrap_or_else(now_timestamp);
        EventTemplate {
            created_at,
            kind,
            tags,
            content,
        }
    };

    let event = nostr::finalize_event(&template, &secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    print_output(&event, args.json)
}

fn event_verify(args: EventVerifyArgs) -> Result<()> {
    let event = read_signed_event(&args.input)?;

    let struct_valid = nostr::validate_event(&event);
    let unsigned = UnsignedEvent {
        pubkey: event.pubkey.clone(),
        created_at: event.created_at,
        kind: event.kind,
        tags: event.tags.clone(),
        content: event.content.clone(),
    };
    let computed_id = nostr::get_event_hash(&unsigned)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let id_matches = computed_id == event.id;
    let signature_valid = nostr::verify_event(&event)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = EventVerifyOutput {
        struct_valid,
        id_matches,
        signature_valid,
    };
    print_output(&output, args.json)
}

fn event_hash(args: EventHashArgs) -> Result<()> {
    let input = read_event_or_unsigned(&args.input)?;
    let unsigned = match input {
        EventOrUnsigned::Signed(event) => UnsignedEvent {
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
        },
        EventOrUnsigned::Unsigned(event) => UnsignedEvent {
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
        },
    };

    let id = nostr::get_event_hash(&unsigned).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = EventHashOutput { id };
    print_output(&output, args.json)
}

fn event_validate(args: EventValidateArgs) -> Result<()> {
    let input = read_event_or_unsigned(&args.input)?;
    let (valid, signed) = match input {
        EventOrUnsigned::Signed(event) => (nostr::validate_event(&event), true),
        EventOrUnsigned::Unsigned(event) => {
            let unsigned = UnsignedEvent {
                pubkey: event.pubkey,
                created_at: event.created_at,
                kind: event.kind,
                tags: event.tags,
                content: event.content,
            };
            (nostr::validate_unsigned_event(&unsigned), false)
        }
    };

    let output = EventValidateOutput { valid, signed };
    print_output(&output, args.json)
}

fn event_serialize(args: EventSerializeArgs) -> Result<()> {
    let input = read_event_or_unsigned(&args.input)?;
    let unsigned = match input {
        EventOrUnsigned::Signed(event) => UnsignedEvent {
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
        },
        EventOrUnsigned::Unsigned(event) => UnsignedEvent {
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
        },
    };

    let serialized = nostr::serialize_event(&unsigned)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = EventSerializeOutput { serialized };
    print_output(&output, args.json)
}

fn event_kind(args: EventKindArgs) -> Result<()> {
    let classification = match nostr::classify_kind(args.kind) {
        nostr::KindClassification::Regular => "regular",
        nostr::KindClassification::Replaceable => "replaceable",
        nostr::KindClassification::Ephemeral => "ephemeral",
        nostr::KindClassification::Addressable => "addressable",
        nostr::KindClassification::Unknown => "unknown",
    };

    let output = EventKindOutput {
        classification: classification.to_string(),
        regular: nostr::is_regular_kind(args.kind),
        replaceable: nostr::is_replaceable_kind(args.kind),
        ephemeral: nostr::is_ephemeral_kind(args.kind),
        addressable: nostr::is_addressable_kind(args.kind),
    };

    print_output(&output, args.json)
}

fn run_nip19(args: Nip19Args) -> Result<()> {
    match args.command {
        Nip19Command::Encode(args) => nip19_encode(args),
        Nip19Command::Decode(args) => nip19_decode(args),
    }
}

fn nip19_encode(args: Nip19EncodeArgs) -> Result<()> {
    let value = match args.entity {
        Nip19EntityKind::Npub => {
            let pubkey = parse_pubkey_32(args.pubkey, args.npub)?;
            nostr::encode_npub(&pubkey).map_err(|e| anyhow::anyhow!(e.to_string()))?
        }
        Nip19EntityKind::Nsec => {
            let secret = parse_secret_key(args.secret, args.nsec)?;
            nostr::encode_nsec(&secret).map_err(|e| anyhow::anyhow!(e.to_string()))?
        }
        Nip19EntityKind::Note => {
            let event_id = parse_hex_32_required(args.event_id.as_deref(), "--event-id")?;
            nostr::encode_note(&event_id).map_err(|e| anyhow::anyhow!(e.to_string()))?
        }
        Nip19EntityKind::Nprofile => {
            let pubkey = parse_pubkey_32(args.pubkey, args.npub)?;
            let profile = ProfilePointer {
                pubkey,
                relays: args.relay,
            };
            nostr::encode_nprofile(&profile).map_err(|e| anyhow::anyhow!(e.to_string()))?
        }
        Nip19EntityKind::Nevent => {
            let event_id = parse_hex_32_required(args.event_id.as_deref(), "--event-id")?;
            let author = match args.author.as_deref() {
                Some(hex_str) => Some(parse_hex_32(hex_str)?),
                None => None,
            };
            let event = EventPointer {
                id: event_id,
                relays: args.relay,
                author,
                kind: args.kind,
            };
            nostr::encode_nevent(&event).map_err(|e| anyhow::anyhow!(e.to_string()))?
        }
        Nip19EntityKind::Naddr => {
            let identifier = args
                .identifier
                .clone()
                .ok_or_else(|| anyhow::anyhow!("--identifier is required for naddr"))?;
            let pubkey = parse_pubkey_32(args.pubkey, args.npub)?;
            let kind = args
                .kind
                .ok_or_else(|| anyhow::anyhow!("--kind is required for naddr"))?;
            let addr = AddressPointer {
                identifier,
                pubkey,
                kind,
                relays: args.relay,
            };
            nostr::encode_naddr(&addr).map_err(|e| anyhow::anyhow!(e.to_string()))?
        }
    };

    let output = Nip19EncodeOutput {
        entity: format!("{:?}", args.entity).to_lowercase(),
        value,
    };
    print_output(&output, args.json)
}

fn nip19_decode(args: Nip19DecodeArgs) -> Result<()> {
    let raw = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.bech32
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--bech32 is required"))?
    };

    let entity = nostr::decode(&raw).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = nip19_entity_to_output(entity)?;
    print_output(&output, args.json)
}

fn run_uri(args: UriArgs) -> Result<()> {
    match args.command {
        UriCommand::Encode(args) => uri_encode(args),
        UriCommand::Decode(args) => uri_decode(args),
        UriCommand::Strip(args) => uri_strip(args),
    }
}

fn uri_encode(args: UriEncodeArgs) -> Result<()> {
    let entity_str = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.entity
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--entity is required"))?
    };

    let entity = nostr::decode(&entity_str).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let uri = nostr::to_nostr_uri(&entity).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = UriOutput { uri };
    print_output(&output, args.json)
}

fn uri_decode(args: UriDecodeArgs) -> Result<()> {
    let uri = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.uri
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--uri is required"))?
    };

    let entity = nostr::from_nostr_uri(&uri).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = nip19_entity_to_output(entity)?;
    print_output(&output, args.json)
}

fn uri_strip(args: UriStripArgs) -> Result<()> {
    let uri = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.uri
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--uri is required"))?
    };

    let stripped = nostr::strip_nostr_prefix(&uri).to_string();
    let output = UriStripOutput { stripped };
    print_output(&output, args.json)
}

fn run_nip04(args: Nip04Args) -> Result<()> {
    match args.command {
        Nip04Command::Encrypt(args) => nip04_encrypt(args),
        Nip04Command::Decrypt(args) => nip04_decrypt(args),
    }
}

fn nip04_encrypt(args: Nip04EncryptArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let pubkey = parse_pubkey_bytes(args.pubkey, args.npub)?;
    let plaintext = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.plaintext
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--plaintext is required"))?
    };

    let ciphertext = nostr::encrypt(&secret, &pubkey, &plaintext)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip04Output { ciphertext };
    print_output(&output, args.json)
}

fn nip04_decrypt(args: Nip04DecryptArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let pubkey = parse_pubkey_bytes(args.pubkey, args.npub)?;
    let ciphertext = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.ciphertext
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--ciphertext is required"))?
    };

    let plaintext = nostr::decrypt(&secret, &pubkey, &ciphertext)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip04DecryptOutput { plaintext };
    print_output(&output, args.json)
}

fn run_nip44(args: Nip44Args) -> Result<()> {
    match args.command {
        Nip44Command::Encrypt(args) => nip44_encrypt(args),
        Nip44Command::Decrypt(args) => nip44_decrypt(args),
    }
}

fn nip44_encrypt(args: Nip44EncryptArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let pubkey = parse_pubkey_bytes(args.pubkey, args.npub)?;
    let plaintext = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.plaintext
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--plaintext is required"))?
    };

    let payload = nostr::encrypt_v2(&secret, &pubkey, &plaintext)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip44Output { payload };
    print_output(&output, args.json)
}

fn nip44_decrypt(args: Nip44DecryptArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let pubkey = parse_pubkey_bytes(args.pubkey, args.npub)?;
    let payload = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.payload
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--payload is required"))?
    };

    let plaintext = nostr::decrypt_v2(&secret, &pubkey, &payload)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip44DecryptOutput { plaintext };
    print_output(&output, args.json)
}

fn run_nip26(args: Nip26Args) -> Result<()> {
    match args.command {
        Nip26Command::Create(args) => nip26_create(args),
        Nip26Command::Verify(args) => nip26_verify(args),
        Nip26Command::Validate(args) => nip26_validate(args),
    }
}

fn nip26_create(args: Nip26CreateArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let delegatee_pubkey = parse_pubkey_hex(args.delegatee_pubkey, args.delegatee_npub)?;
    let delegation_string =
        nostr::create_delegation_string(&delegatee_pubkey, &args.conditions);
    let token = nostr::create_delegation_token(&secret, &delegatee_pubkey, &args.conditions)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = Nip26CreateOutput {
        delegation_string,
        token,
    };
    print_output(&output, args.json)
}

fn nip26_verify(args: Nip26VerifyArgs) -> Result<()> {
    let delegator_pubkey = parse_pubkey_hex(args.delegator_pubkey, args.delegator_npub)?;
    let delegatee_pubkey = parse_pubkey_hex(args.delegatee_pubkey, args.delegatee_npub)?;

    nostr::verify_delegation_token(
        &delegator_pubkey,
        &delegatee_pubkey,
        &args.conditions,
        &args.token,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = Nip26VerifyOutput { valid: true };
    print_output(&output, args.json)
}

fn nip26_validate(args: Nip26ValidateArgs) -> Result<()> {
    let delegator_pubkey = parse_pubkey_hex(args.delegator_pubkey, args.delegator_npub)?;
    let delegatee_pubkey = parse_pubkey_hex(args.delegatee_pubkey, args.delegatee_npub)?;

    nostr::validate_delegation(
        &delegator_pubkey,
        &delegatee_pubkey,
        &args.conditions,
        &args.token,
        args.event_kind,
        args.event_created_at,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = Nip26ValidateOutput { valid: true };
    print_output(&output, args.json)
}

fn run_nip42(args: Nip42Args) -> Result<()> {
    match args.command {
        Nip42Command::Auth(args) => nip42_auth(args),
        Nip42Command::Validate(args) => nip42_validate(args),
    }
}

fn nip42_auth(args: Nip42AuthArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let mut template = nostr::create_auth_event_template(&args.relay, &args.challenge);
    if let Some(created_at) = args.created_at {
        template.created_at = created_at;
    }

    let event = nostr::finalize_event(&template, &secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip42AuthOutput { event };
    print_output(&output, args.json)
}

fn nip42_validate(args: Nip42ValidateArgs) -> Result<()> {
    let event = read_signed_event(&args.input)?;
    let now = args.now;
    nostr::validate_auth_event(&event, &args.relay, &args.challenge, now)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = Nip42ValidateOutput { valid: true };
    print_output(&output, args.json)
}

fn run_nip49(args: Nip49Args) -> Result<()> {
    match args.command {
        Nip49Command::Encrypt(args) => nip49_encrypt(args),
        Nip49Command::Decrypt(args) => nip49_decrypt(args),
    }
}

fn nip49_encrypt(args: Nip49EncryptArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let key_security = match args.key_security {
        KeySecurityArg::Insecure => KeySecurity::Insecure,
        KeySecurityArg::Secure => KeySecurity::Secure,
        KeySecurityArg::Unknown => KeySecurity::Unknown,
    };

    let ncryptsec = nostr::nip49_encrypt(&secret, &args.password, args.log_n, key_security)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip49EncryptOutput { ncryptsec };
    print_output(&output, args.json)
}

fn nip49_decrypt(args: Nip49DecryptArgs) -> Result<()> {
    let (secret, log_n, key_security) =
        nostr::nip49_decrypt(&args.ncryptsec, &args.password)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let private_key_hex = hex::encode(secret);
    let nsec = nostr::private_key_to_nsec(&secret).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = Nip49DecryptOutput {
        private_key_hex,
        nsec,
        log_n,
        key_security: format!("{:?}", key_security).to_lowercase(),
    };
    print_output(&output, args.json)
}

fn run_nip98(args: Nip98Args) -> Result<()> {
    match args.command {
        Nip98Command::Create(args) => nip98_create(args),
        Nip98Command::Validate(args) => nip98_validate(args),
        Nip98Command::Decode(args) => nip98_decode(args),
    }
}

fn nip98_create(args: Nip98CreateArgs) -> Result<()> {
    let secret = parse_secret_key(args.secret, args.nsec)?;
    let method = HttpMethod::parse(&args.method).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let payload_bytes = read_payload_bytes(args.payload, args.payload_file, args.payload_stdin)?;
    let mut auth = HttpAuth::new(args.url.clone(), method);
    if let Some(bytes) = payload_bytes.as_ref() {
        auth = auth.with_payload_hash(nostr::hash_payload(bytes));
    }

    let template = EventTemplate {
        kind: nostr::KIND_HTTP_AUTH,
        tags: auth.to_tags(),
        content: String::new(),
        created_at: args.created_at.unwrap_or_else(now_timestamp),
    };

    let event = nostr::finalize_event(&template, &secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let event_json = serde_json::to_string(&event).context("serialize event")?;
    let authorization_header =
        nostr::encode_authorization_header(&event_json).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let output = Nip98CreateOutput {
        authorization_header,
        event,
    };
    print_output(&output, args.json)
}

fn nip98_validate(args: Nip98ValidateArgs) -> Result<()> {
    let event = read_signed_event(&args.input)?;
    let method = HttpMethod::parse(&args.method).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let now = args.now.unwrap_or_else(now_timestamp);

    let payload_bytes = read_payload_bytes(args.payload, args.payload_file, args.payload_stdin)?;
    let mut params = ValidationParams::new(args.url.clone(), method, now);
    if let Some(bytes) = payload_bytes.as_ref() {
        params = params.with_payload_hash(nostr::hash_payload(bytes));
    }
    if let Some(window) = args.window {
        params = params.with_timestamp_window(window);
    }

    nostr::validate_http_auth_event(event.kind, event.created_at, &event.tags, &params)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let signature_valid = if args.verify_sig {
        Some(
            nostr::verify_event(&event)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        )
    } else {
        None
    };

    let output = Nip98ValidateOutput {
        valid: true,
        signature_valid,
    };
    print_output(&output, args.json)
}

fn nip98_decode(args: Nip98DecodeArgs) -> Result<()> {
    let header = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.header
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--header is required"))?
    };

    let event_json =
        nostr::decode_authorization_header(&header).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip98DecodeOutput { event_json };
    print_output(&output, args.json)
}

fn run_nip05(args: Nip05Args) -> Result<()> {
    match args.command {
        Nip05Command::Parse(args) => nip05_parse(args),
        Nip05Command::WellKnown(args) => nip05_well_known(args),
        Nip05Command::Verify(args) => nip05_verify(args),
    }
}

fn nip05_parse(args: Nip05ParseArgs) -> Result<()> {
    let identifier = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.identifier
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--identifier is required"))?
    };

    let parsed = Nip05Identifier::parse(&identifier).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip05ParseOutput {
        identifier: parsed.to_string(),
        local: parsed.local.clone(),
        domain: parsed.domain.clone(),
        is_root: parsed.is_root(),
    };

    print_output(&output, args.json)
}

fn nip05_well_known(args: Nip05WellKnownArgs) -> Result<()> {
    let identifier = if args.stdin {
        read_stdin_trimmed()?
    } else {
        args.identifier
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--identifier is required"))?
    };

    let parsed = Nip05Identifier::parse(&identifier).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = Nip05WellKnownOutput {
        url: parsed.well_known_url(),
    };
    print_output(&output, args.json)
}

fn nip05_verify(args: Nip05VerifyArgs) -> Result<()> {
    let expected_pubkey = parse_pubkey_hex(args.pubkey, args.npub)?;
    let response_json = read_input_string(
        args.response_json,
        args.response_file,
        args.stdin,
        "response JSON",
        true,
    )?;

    let response = Nip05Response::from_json(&response_json)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let identifier = Nip05Identifier::parse(&args.identifier)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    response
        .verify(&identifier, &expected_pubkey)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let relays = response.get_relays(&expected_pubkey).cloned();
    let output = Nip05VerifyOutput {
        valid: true,
        relays,
    };

    print_output(&output, args.json)
}

fn run_pow(args: PowArgs) -> Result<()> {
    match args.command {
        PowCommand::Difficulty(args) => pow_difficulty(args),
        PowCommand::Check(args) => pow_check(args),
        PowCommand::Nonce(args) => pow_nonce(args),
    }
}

fn pow_difficulty(args: PowDifficultyArgs) -> Result<()> {
    let difficulty = if let Some(event_id) = args.event_id.as_deref() {
        nostr::calculate_difficulty(event_id).map_err(|e| anyhow::anyhow!(e.to_string()))?
    } else {
        let event = read_signed_event(&args.input)?;
        nostr::get_difficulty(&event).map_err(|e| anyhow::anyhow!(e.to_string()))?
    };

    let output = PowDifficultyOutput { difficulty };
    print_output(&output, args.json)
}

fn pow_check(args: PowCheckArgs) -> Result<()> {
    let event = read_signed_event(&args.input)?;
    let meets_minimum =
        nostr::validate_pow(&event, args.min).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let difficulty = nostr::get_difficulty(&event).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = PowCheckOutput {
        meets_minimum,
        difficulty,
    };
    print_output(&output, args.json)
}

fn pow_nonce(args: PowNonceArgs) -> Result<()> {
    let event = read_signed_event(&args.input)?;
    let parsed = nostr::parse_nonce_tag(&event).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let (nonce, target) = match parsed {
        Some((value, target)) => (Some(value), target),
        None => (None, None),
    };
    let output = PowNonceOutput { nonce, target };
    print_output(&output, args.json)
}

fn generate_mnemonic(words: u16) -> Result<String> {
    let mut entropy = match words {
        12 => [0u8; 16].to_vec(),
        24 => [0u8; 32].to_vec(),
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid word count. Use 12 or 24."
            ))
        }
    };

    let mut rng = rand::rng();
    rng.fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| anyhow::anyhow!(format!("Invalid entropy: {}", e)))?;
    Ok(mnemonic.to_string())
}

fn resolve_account(account: Option<u32>, agent: Option<u32>) -> Result<(u32, Option<u32>)> {
    if let Some(agent_id) = agent {
        let account = agent_id
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Agent index overflow"))?;
        return Ok((account, Some(agent_id)));
    }

    Ok((account.unwrap_or(0), None))
}

fn parse_hex_32(hex_str: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex_str)
        .with_context(|| format!("Invalid hex: {}", hex_str))?;
    if bytes.len() != 32 {
        return Err(anyhow::anyhow!(
            "Expected 32 bytes (64 hex chars), got {} bytes",
            bytes.len()
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_hex_32_required(hex_str: Option<&str>, label: &str) -> Result<[u8; 32]> {
    let value = hex_str.ok_or_else(|| anyhow::anyhow!("{} is required", label))?;
    parse_hex_32(value)
}

fn parse_secret_key(secret: Option<String>, nsec: Option<String>) -> Result<[u8; 32]> {
    if secret.is_some() && nsec.is_some() {
        return Err(anyhow::anyhow!("Provide either --secret or --nsec, not both"));
    }

    if let Some(secret) = secret {
        return parse_hex_32(&secret);
    }

    if let Some(nsec) = nsec {
        return nostr::nsec_to_private_key(&nsec).map_err(|e| anyhow::anyhow!(e.to_string()));
    }

    Err(anyhow::anyhow!("Provide --secret or --nsec"))
}

fn parse_pubkey_32(pubkey: Option<String>, npub: Option<String>) -> Result<[u8; 32]> {
    if pubkey.is_some() && npub.is_some() {
        return Err(anyhow::anyhow!("Provide either --pubkey or --npub, not both"));
    }

    if let Some(pubkey) = pubkey {
        return parse_hex_32(&pubkey);
    }

    if let Some(npub) = npub {
        return nostr::npub_to_public_key(&npub).map_err(|e| anyhow::anyhow!(e.to_string()));
    }

    Err(anyhow::anyhow!("Provide --pubkey or --npub"))
}

fn parse_pubkey_hex(pubkey: Option<String>, npub: Option<String>) -> Result<String> {
    let pubkey_bytes = parse_pubkey_32(pubkey, npub)?;
    Ok(hex::encode(pubkey_bytes))
}

fn parse_pubkey_bytes(pubkey: Option<String>, npub: Option<String>) -> Result<Vec<u8>> {
    let pubkey_hex = if let Some(hex) = pubkey {
        hex
    } else if let Some(npub) = npub {
        hex::encode(
            nostr::npub_to_public_key(&npub).map_err(|e| anyhow::anyhow!(e.to_string()))?,
        )
    } else {
        return Err(anyhow::anyhow!("Provide --pubkey or --npub"));
    };

    let bytes = hex::decode(&pubkey_hex)
        .with_context(|| format!("Invalid hex: {}", pubkey_hex))?;

    match bytes.len() {
        32 => {
            let mut out = Vec::with_capacity(33);
            out.push(0x02);
            out.extend_from_slice(&bytes);
            Ok(out)
        }
        33 | 65 => Ok(bytes),
        other => Err(anyhow::anyhow!(
            "Expected 32/33/65 bytes for public key, got {} bytes",
            other
        )),
    }
}

fn read_stdin_trimmed() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let trimmed = buffer.trim().to_string();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("No input provided on stdin"));
    }
    Ok(trimmed)
}

fn read_input_string(
    inline: Option<String>,
    file: Option<PathBuf>,
    stdin: bool,
    label: &str,
    trim: bool,
) -> Result<String> {
    if let Some(value) = inline {
        return Ok(if trim { value.trim().to_string() } else { value });
    }

    if let Some(path) = file {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {} from {}", label, path.display()))?;
        return Ok(if trim { content.trim().to_string() } else { content });
    }

    if stdin {
        if trim {
            return read_stdin_trimmed();
        }
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        if buffer.is_empty() {
            return Err(anyhow::anyhow!("No input provided on stdin"));
        }
        return Ok(buffer);
    }

    Err(anyhow::anyhow!("No {} provided", label))
}

fn read_payload_bytes(
    inline: Option<String>,
    file: Option<PathBuf>,
    stdin: bool,
) -> Result<Option<Vec<u8>>> {
    if inline.is_none() && file.is_none() && !stdin {
        return Ok(None);
    }

    if let Some(value) = inline {
        return Ok(Some(value.into_bytes()));
    }

    if let Some(path) = file {
        let bytes = std::fs::read(&path)
            .with_context(|| format!("Failed to read payload from {}", path.display()))?;
        return Ok(Some(bytes));
    }

    if stdin {
        let mut buffer = Vec::new();
        io::stdin().read_to_end(&mut buffer)?;
        if buffer.is_empty() {
            return Err(anyhow::anyhow!("No payload provided on stdin"));
        }
        return Ok(Some(buffer));
    }

    Ok(None)
}

fn validate_mnemonic(mnemonic: &str) -> Result<()> {
    Mnemonic::parse(mnemonic)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!(format!("Invalid mnemonic: {}", e)))
}

fn parse_tags_json(tags_json: Option<String>) -> Result<Vec<Vec<String>>> {
    match tags_json {
        Some(raw) => serde_json::from_str(&raw).context("Invalid tags JSON"),
        None => Ok(Vec::new()),
    }
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn read_signed_event(input: &EventInputArgs) -> Result<Event> {
    let raw = read_input_string(
        input.event_json.clone(),
        input.event_file.clone(),
        input.stdin,
        "event JSON",
        true,
    )?;
    let event: Event = serde_json::from_str(&raw).context("Invalid event JSON")?;
    Ok(event)
}

enum EventOrUnsigned {
    Signed(Event),
    Unsigned(UnsignedEventJson),
}

fn read_event_or_unsigned(input: &EventInputArgs) -> Result<EventOrUnsigned> {
    let raw = read_input_string(
        input.event_json.clone(),
        input.event_file.clone(),
        input.stdin,
        "event JSON",
        true,
    )?;

    if let Ok(event) = serde_json::from_str::<Event>(&raw) {
        return Ok(EventOrUnsigned::Signed(event));
    }

    let unsigned: UnsignedEventJson =
        serde_json::from_str(&raw).context("Invalid event JSON")?;
    Ok(EventOrUnsigned::Unsigned(unsigned))
}

fn nip19_entity_to_output(entity: Nip19Entity) -> Result<Nip19DecodeOutput> {
    match entity {
        Nip19Entity::Pubkey(pubkey) => Ok(Nip19DecodeOutput {
            entity: "npub".to_string(),
            pubkey_hex: Some(hex::encode(pubkey)),
            secret_key_hex: None,
            event_id_hex: None,
            identifier: None,
            relays: Vec::new(),
            author_hex: None,
            kind: None,
        }),
        Nip19Entity::Secret(secret) => Ok(Nip19DecodeOutput {
            entity: "nsec".to_string(),
            pubkey_hex: None,
            secret_key_hex: Some(hex::encode(secret)),
            event_id_hex: None,
            identifier: None,
            relays: Vec::new(),
            author_hex: None,
            kind: None,
        }),
        Nip19Entity::Note(note_id) => Ok(Nip19DecodeOutput {
            entity: "note".to_string(),
            pubkey_hex: None,
            secret_key_hex: None,
            event_id_hex: Some(hex::encode(note_id)),
            identifier: None,
            relays: Vec::new(),
            author_hex: None,
            kind: None,
        }),
        Nip19Entity::Profile(profile) => Ok(Nip19DecodeOutput {
            entity: "nprofile".to_string(),
            pubkey_hex: Some(hex::encode(profile.pubkey)),
            secret_key_hex: None,
            event_id_hex: None,
            identifier: None,
            relays: profile.relays,
            author_hex: None,
            kind: None,
        }),
        Nip19Entity::Event(event) => Ok(Nip19DecodeOutput {
            entity: "nevent".to_string(),
            pubkey_hex: None,
            secret_key_hex: None,
            event_id_hex: Some(hex::encode(event.id)),
            identifier: None,
            relays: event.relays,
            author_hex: event.author.map(hex::encode),
            kind: event.kind,
        }),
        Nip19Entity::Address(addr) => Ok(Nip19DecodeOutput {
            entity: "naddr".to_string(),
            pubkey_hex: Some(hex::encode(addr.pubkey)),
            secret_key_hex: None,
            event_id_hex: None,
            identifier: Some(addr.identifier),
            relays: addr.relays,
            author_hex: None,
            kind: Some(addr.kind),
        }),
    }
}

fn print_output<T: Serialize>(value: &T, json: bool) -> Result<()> {
    if json {
        let output = serde_json::to_string_pretty(value)?;
        println!("{}", output);
    } else {
        print_human(value)?;
    }
    Ok(())
}

fn print_human<T: Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_value(value)?;
    match json {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let rendered = match value {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Null => continue,
                    other => other.to_string(),
                };
                println!("{}: {}", key, rendered);
            }
        }
        other => {
            println!("{}", other);
        }
    }
    Ok(())
}
