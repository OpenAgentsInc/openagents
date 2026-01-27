use anyhow::{Context, Result};
use bip39::Mnemonic;
use clap::{Args, Parser, Subcommand};
use nostr::{
    derive_keypair_full, mnemonic_to_seed, nsec_to_private_key, npub_to_public_key,
    private_key_to_nsec, public_key_to_npub,
};
use rand::RngCore;
use serde::Serialize;
use std::io::{self, Read};

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

pub fn run(args: NostrArgs) -> Result<()> {
    match args.command {
        NostrCommand::New(args) => new_keypair(args),
        NostrCommand::Derive(args) => derive_keypair(args),
        NostrCommand::Encode(args) => encode_keys(args),
        NostrCommand::Decode(args) => decode_keys(args),
        NostrCommand::Seed(args) => derive_seed(args),
    }
}

fn new_keypair(args: NewArgs) -> Result<()> {
    let mnemonic = generate_mnemonic(args.words)?;
    let (account, agent) = resolve_account(args.account, args.agent)?;
    let keypair = derive_keypair_full(&mnemonic, &args.passphrase, account)
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
        read_stdin_mnemonic()?
    } else {
        args.mnemonic
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Mnemonic is required"))?
    };

    validate_mnemonic(&mnemonic)?;

    let (account, agent) = resolve_account(args.account, args.agent)?;
    let keypair = derive_keypair_full(&mnemonic, &args.passphrase, account)
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
            Some(public_key_to_npub(&bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?)
        }
        None => None,
    };

    let nsec = match args.private.as_deref() {
        Some(hex_str) => {
            let bytes = parse_hex_32(hex_str)?;
            Some(private_key_to_nsec(&bytes).map_err(|e| anyhow::anyhow!(e.to_string()))?)
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
            let bytes = npub_to_public_key(npub).map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Some(hex::encode(bytes))
        }
        None => None,
    };

    let private_key_hex = match args.nsec.as_deref() {
        Some(nsec) => {
            let bytes = nsec_to_private_key(nsec).map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
        read_stdin_mnemonic()?
    } else {
        args.mnemonic
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Mnemonic is required"))?
    };

    validate_mnemonic(&mnemonic)?;

    let seed = mnemonic_to_seed(&mnemonic, &args.passphrase)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = SeedOutput {
        seed_hex: hex::encode(seed),
    };
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

fn read_stdin_mnemonic() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    let mnemonic = buffer.trim().to_string();
    if mnemonic.is_empty() {
        return Err(anyhow::anyhow!("No mnemonic provided on stdin"));
    }
    Ok(mnemonic)
}

fn validate_mnemonic(mnemonic: &str) -> Result<()> {
    Mnemonic::parse(mnemonic)
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!(format!("Invalid mnemonic: {}", e)))
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
