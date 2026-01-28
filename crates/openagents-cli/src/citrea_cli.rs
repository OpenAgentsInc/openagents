use anyhow::{Context, Result};
use bip39::Mnemonic;
use clap::{Args, Parser, Subcommand, ValueEnum};
use citrea::{
    address_from_uncompressed_pubkey, create2_address, derive_keypair_full,
    eoa_address_from_secret, erc20_balance_of_data, format_address, parse_hex_bytes,
    parse_hex_u128, parse_hex_vec, sign_schnorr, strip_0x, verify_schnorr,
    xonly_pubkey_from_secret, BlockTag, RpcCallRequest, RpcClient,
};
use rand::RngCore;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Parser)]
pub struct CitreaArgs {
    #[command(subcommand)]
    pub command: CitreaCommand,
}

#[derive(Subcommand)]
pub enum CitreaCommand {
    /// Generate a new mnemonic and derive a Schnorr keypair
    New(NewArgs),
    /// Derive a Schnorr keypair from an existing mnemonic
    Derive(DeriveArgs),
    /// Derive the BIP39 seed from a mnemonic
    Seed(SeedArgs),
    /// Derive a Schnorr public key from a secret key or mnemonic
    Pubkey(PubkeyArgs),
    /// Sign a 32-byte hash with Schnorr (BIP340)
    Sign(SignArgs),
    /// Verify a 32-byte hash signature with Schnorr (BIP340)
    Verify(VerifyArgs),
    /// Address helpers (EOA or CREATE2)
    Address(AddressArgs),
    /// Chain info (RPC)
    Chain(ChainArgs),
    /// Balance lookup (RPC)
    Balance(BalanceArgs),
    /// Nonce lookup (RPC)
    Nonce(NonceArgs),
    /// eth_call helper (RPC)
    Call(CallArgs),
    /// eth_sendRawTransaction helper (RPC)
    Send(SendArgs),
    /// Transaction receipt lookup (RPC)
    Receipt(ReceiptArgs),
    /// Citrea-specific deposit submission
    Deposit(DepositArgs),
    /// txpool content (RPC)
    Txpool(TxpoolArgs),
}

#[derive(Args)]
pub struct NewArgs {
    /// Number of words in the mnemonic (12 or 24)
    #[arg(long, default_value = "12")]
    pub words: u16,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
    /// Account index for derivation (m/44'/1237'/<account>'/0/0)
    #[arg(long)]
    pub account: Option<u32>,
    /// Agent index (maps to account = agent + 1)
    #[arg(long)]
    pub agent: Option<u32>,
    /// Do not print the mnemonic
    #[arg(long)]
    pub no_mnemonic: bool,
    /// Do not print the private key
    #[arg(long)]
    pub no_private: bool,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct DeriveArgs {
    #[command(flatten)]
    pub input: KeyDerivationArgs,
    /// Print the mnemonic in output
    #[arg(long)]
    pub show_mnemonic: bool,
    /// Do not print the private key
    #[arg(long)]
    pub no_private: bool,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct SeedArgs {
    #[command(flatten)]
    pub input: MnemonicInput,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct PubkeyArgs {
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin", "mnemonic_file"])]
    pub secret_hex: Option<String>,
    #[command(flatten)]
    pub input: KeyDerivationArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct SignArgs {
    /// 32-byte hash (hex)
    #[arg(long, conflicts_with_all = ["hash_file", "hash_stdin"])]
    pub hash: Option<String>,
    /// Read hash from a file
    #[arg(long, conflicts_with_all = ["hash", "hash_stdin"])]
    pub hash_file: Option<PathBuf>,
    /// Read hash from stdin
    #[arg(long = "hash-stdin", conflicts_with_all = ["hash", "hash_file"])]
    pub hash_stdin: bool,
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin", "mnemonic_file"])]
    pub secret_hex: Option<String>,
    #[command(flatten)]
    pub input: KeyDerivationArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct VerifyArgs {
    /// 32-byte hash (hex)
    #[arg(long, conflicts_with_all = ["hash_file", "hash_stdin"])]
    pub hash: Option<String>,
    /// Read hash from a file
    #[arg(long, conflicts_with_all = ["hash", "hash_stdin"])]
    pub hash_file: Option<PathBuf>,
    /// Read hash from stdin
    #[arg(long = "hash-stdin", conflicts_with_all = ["hash", "hash_file"])]
    pub hash_stdin: bool,
    /// X-only public key hex (32 bytes)
    #[arg(long)]
    pub pubkey: String,
    /// Schnorr signature hex (64 bytes)
    #[arg(long)]
    pub signature: String,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct AddressArgs {
    #[command(subcommand)]
    pub command: AddressCommand,
}

#[derive(Subcommand)]
pub enum AddressCommand {
    /// Compute an EOA address from a secret key or mnemonic
    Eoa(AddressEoaArgs),
    /// Compute a CREATE2 address
    Create2(AddressCreate2Args),
    /// Compute an address from an uncompressed public key
    Pubkey(AddressPubkeyArgs),
}

#[derive(Args)]
pub struct AddressEoaArgs {
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin", "mnemonic_file"])]
    pub secret_hex: Option<String>,
    #[command(flatten)]
    pub input: KeyDerivationArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct AddressCreate2Args {
    /// Factory address (20-byte hex)
    #[arg(long)]
    pub factory: String,
    /// Salt (32-byte hex)
    #[arg(long)]
    pub salt: String,
    /// Init code hash (32-byte hex)
    #[arg(long)]
    pub init_code_hash: String,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct AddressPubkeyArgs {
    /// Uncompressed public key hex (64 or 65 bytes)
    #[arg(long)]
    pub public_key: String,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct ChainArgs {
    #[command(subcommand)]
    pub command: ChainCommand,
}

#[derive(Subcommand)]
pub enum ChainCommand {
    /// Fetch chain id and latest block
    Info(ChainInfoArgs),
}

#[derive(Args)]
pub struct ChainInfoArgs {
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct BalanceArgs {
    /// Address to query (20-byte hex)
    #[arg(long)]
    pub address: String,
    /// Optional ERC20 token address to query
    #[arg(long)]
    pub token: Option<String>,
    #[command(flatten)]
    pub block: BlockArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct NonceArgs {
    /// Address to query (20-byte hex)
    #[arg(long)]
    pub address: String,
    #[command(flatten)]
    pub block: BlockArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct CallArgs {
    /// Contract address
    #[arg(long)]
    pub to: String,
    /// Call data hex
    #[arg(long)]
    pub data: String,
    /// Optional sender address
    #[arg(long)]
    pub from: Option<String>,
    /// Optional value (hex)
    #[arg(long)]
    pub value: Option<String>,
    /// Optional gas (hex)
    #[arg(long)]
    pub gas: Option<String>,
    /// Optional gas price (hex)
    #[arg(long)]
    pub gas_price: Option<String>,
    #[command(flatten)]
    pub block: BlockArgs,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct SendArgs {
    /// Raw signed transaction hex
    #[arg(long)]
    pub raw: String,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct ReceiptArgs {
    /// Transaction hash
    #[arg(long)]
    pub tx_hash: String,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct DepositArgs {
    #[command(subcommand)]
    pub command: DepositCommand,
}

#[derive(Subcommand)]
pub enum DepositCommand {
    /// Submit a raw deposit transaction (citrea_sendRawDepositTransaction)
    Submit(DepositSubmitArgs),
}

#[derive(Args)]
pub struct DepositSubmitArgs {
    /// Raw deposit hex
    #[arg(long)]
    pub raw: String,
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct TxpoolArgs {
    #[command(flatten)]
    pub rpc: RpcArgs,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Clone, Args)]
pub struct OutputArgs {
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Clone, Args)]
pub struct RpcArgs {
    /// RPC URL (or set CITREA_RPC / CITREA_RPC_URL)
    #[arg(long)]
    pub rpc: Option<String>,
}

#[derive(Clone, Args)]
pub struct BlockArgs {
    /// Block tag
    #[arg(long, value_enum, default_value = "latest")]
    pub tag: BlockTagArg,
    /// Block number override
    #[arg(long)]
    pub number: Option<u64>,
}

#[derive(ValueEnum, Clone, Copy)]
#[value(rename_all = "kebab-case")]
pub enum BlockTagArg {
    Latest,
    Pending,
    Earliest,
}

#[derive(Clone, Args)]
pub struct MnemonicInput {
    /// Mnemonic phrase (12 or 24 words)
    #[arg(long, conflicts_with_all = ["stdin", "mnemonic_file"])]
    pub mnemonic: Option<String>,
    /// Read mnemonic from stdin
    #[arg(long, conflicts_with_all = ["mnemonic", "mnemonic_file"])]
    pub stdin: bool,
    /// Read mnemonic from a file
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin"])]
    pub mnemonic_file: Option<PathBuf>,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
}

#[derive(Clone, Args)]
pub struct KeyDerivationArgs {
    #[command(flatten)]
    pub mnemonic: MnemonicInput,
    /// Account index for derivation (m/44'/1237'/<account>'/0/0)
    #[arg(long)]
    pub account: Option<u32>,
    /// Agent index (maps to account = agent + 1)
    #[arg(long)]
    pub agent: Option<u32>,
}

#[derive(Serialize)]
struct KeypairOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    mnemonic: Option<String>,
    account: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<u32>,
    public_key_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    private_key_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eoa_address: Option<String>,
}

#[derive(Serialize)]
struct SeedOutput {
    seed_hex: String,
}

#[derive(Serialize)]
struct PubkeyOutput {
    public_key_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    eoa_address: Option<String>,
}

#[derive(Serialize)]
struct SignOutput {
    signature_hex: String,
    public_key_hex: String,
}

#[derive(Serialize)]
struct VerifyOutput {
    valid: bool,
}

#[derive(Serialize)]
struct AddressOutput {
    address: String,
}

#[derive(Serialize)]
struct ChainInfoOutput {
    rpc_url: String,
    chain_id: u64,
    block_number: u64,
}

#[derive(Serialize)]
struct BalanceOutput {
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    balance_hex: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    balance_u128: Option<u128>,
}

#[derive(Serialize)]
struct NonceOutput {
    address: String,
    nonce: u64,
    nonce_hex: String,
}

#[derive(Serialize)]
struct CallOutput {
    data_hex: String,
}

#[derive(Serialize)]
struct SendOutput {
    tx_hash: String,
}

pub fn run(args: CitreaArgs) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("Failed to start Tokio runtime")?;
    runtime.block_on(run_async(args))
}

async fn run_async(args: CitreaArgs) -> Result<()> {
    match args.command {
        CitreaCommand::New(args) => new_keypair(args),
        CitreaCommand::Derive(args) => derive_keypair(args),
        CitreaCommand::Seed(args) => derive_seed(args),
        CitreaCommand::Pubkey(args) => derive_pubkey(args),
        CitreaCommand::Sign(args) => sign_hash(args),
        CitreaCommand::Verify(args) => verify_hash(args),
        CitreaCommand::Address(args) => address_command(args),
        CitreaCommand::Chain(args) => chain_command(args).await,
        CitreaCommand::Balance(args) => balance_command(args).await,
        CitreaCommand::Nonce(args) => nonce_command(args).await,
        CitreaCommand::Call(args) => call_command(args).await,
        CitreaCommand::Send(args) => send_command(args).await,
        CitreaCommand::Receipt(args) => receipt_command(args).await,
        CitreaCommand::Deposit(args) => deposit_command(args).await,
        CitreaCommand::Txpool(args) => txpool_command(args).await,
    }
}

fn new_keypair(args: NewArgs) -> Result<()> {
    let mnemonic = generate_mnemonic(args.words)?;
    let (account, agent) = resolve_account(args.account, args.agent)?;
    let keypair = derive_keypair_full(&mnemonic, &args.passphrase, account)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let eoa_address = if args.no_private {
        None
    } else {
        eoa_address_from_secret(&keypair.private_key)
            .ok()
            .map(|addr| format_address(&addr))
    };

    let output = KeypairOutput {
        mnemonic: if args.no_mnemonic { None } else { Some(mnemonic) },
        account,
        agent,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: if args.no_private {
            None
        } else {
            Some(keypair.private_key_hex())
        },
        eoa_address,
    };

    print_output(&output, args.output.json)
}

fn derive_keypair(args: DeriveArgs) -> Result<()> {
    let mnemonic = resolve_mnemonic(&args.input.mnemonic)?;
    let (account, agent) = resolve_account(args.input.account, args.input.agent)?;
    let keypair = derive_keypair_full(&mnemonic, &args.input.mnemonic.passphrase, account)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let eoa_address = if args.no_private {
        None
    } else {
        eoa_address_from_secret(&keypair.private_key)
            .ok()
            .map(|addr| format_address(&addr))
    };

    let output = KeypairOutput {
        mnemonic: if args.show_mnemonic {
            Some(mnemonic)
        } else {
            None
        },
        account,
        agent,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: if args.no_private {
            None
        } else {
            Some(keypair.private_key_hex())
        },
        eoa_address,
    };

    print_output(&output, args.output.json)
}

fn derive_seed(args: SeedArgs) -> Result<()> {
    let mnemonic = resolve_mnemonic(&args.input)?;
    let mnemonic = Mnemonic::parse(&mnemonic)
        .map_err(|e| anyhow::anyhow!(format!("Invalid mnemonic: {e}")))?;
    let seed = mnemonic.to_seed(&args.input.passphrase);
    let output = SeedOutput {
        seed_hex: hex::encode(seed),
    };
    print_output(&output, args.output.json)
}

fn derive_pubkey(args: PubkeyArgs) -> Result<()> {
    let secret = resolve_secret_key(args.secret_hex, &args.input)?;
    let pubkey = xonly_pubkey_from_secret(&secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let eoa_address = eoa_address_from_secret(&secret)
        .ok()
        .map(|addr| format_address(&addr));
    let output = PubkeyOutput {
        public_key_hex: hex::encode(pubkey),
        eoa_address,
    };
    print_output(&output, args.output.json)
}

fn sign_hash(args: SignArgs) -> Result<()> {
    let hash = read_hash_input(args.hash, args.hash_file, args.hash_stdin)?;
    let secret = resolve_secret_key(args.secret_hex, &args.input)?;
    let signature = sign_schnorr(&secret, &hash)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let pubkey = xonly_pubkey_from_secret(&secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = SignOutput {
        signature_hex: hex::encode(signature),
        public_key_hex: hex::encode(pubkey),
    };
    print_output(&output, args.output.json)
}

fn verify_hash(args: VerifyArgs) -> Result<()> {
    let hash = read_hash_input(args.hash, args.hash_file, args.hash_stdin)?;
    let pubkey = parse_hex_bytes::<32>(&args.pubkey)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let signature = parse_hex_bytes::<64>(&args.signature)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let valid = verify_schnorr(&pubkey, &hash, &signature)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = VerifyOutput { valid };
    print_output(&output, args.output.json)
}

fn address_command(args: AddressArgs) -> Result<()> {
    match args.command {
        AddressCommand::Eoa(args) => address_eoa(args),
        AddressCommand::Create2(args) => address_create2(args),
        AddressCommand::Pubkey(args) => address_pubkey(args),
    }
}

fn address_eoa(args: AddressEoaArgs) -> Result<()> {
    let secret = resolve_secret_key(args.secret_hex, &args.input)?;
    let address = eoa_address_from_secret(&secret)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = AddressOutput {
        address: format_address(&address),
    };
    print_output(&output, args.output.json)
}

fn address_create2(args: AddressCreate2Args) -> Result<()> {
    let factory = parse_hex_bytes::<20>(&args.factory)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let salt = parse_hex_bytes::<32>(&args.salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let init_code_hash = parse_hex_bytes::<32>(&args.init_code_hash)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let address = create2_address(&factory, &salt, &init_code_hash);
    let output = AddressOutput {
        address: format_address(&address),
    };
    print_output(&output, args.output.json)
}

fn address_pubkey(args: AddressPubkeyArgs) -> Result<()> {
    let bytes = parse_hex_vec(&args.public_key).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let address = address_from_uncompressed_pubkey(&bytes)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = AddressOutput {
        address: format_address(&address),
    };
    print_output(&output, args.output.json)
}

async fn chain_command(args: ChainArgs) -> Result<()> {
    match args.command {
        ChainCommand::Info(args) => chain_info(args).await,
    }
}

async fn chain_info(args: ChainInfoArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let chain_id = rpc.chain_id().await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let block_number = rpc
        .block_number()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = ChainInfoOutput {
        rpc_url: rpc.url().to_string(),
        chain_id,
        block_number,
    };
    print_output(&output, args.output.json)
}

async fn balance_command(args: BalanceArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let address = parse_hex_bytes::<20>(&args.address)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let block = resolve_block_tag(&args.block);

    let (balance_hex, token) = if let Some(token) = args.token {
        let token_address = parse_hex_bytes::<20>(&token)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let data = erc20_balance_of_data(&address);
        let request = RpcCallRequest {
            to: format_address(&token_address),
            from: None,
            data: Some(data),
            value: None,
            gas: None,
            gas_price: None,
        };
        let result = rpc
            .call(request, block)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        (result, Some(format_address(&token_address)))
    } else {
        (
            rpc.get_balance(&address, block)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
            None,
        )
    };

    let balance_u128 = parse_hex_u128(&balance_hex).ok();
    let output = BalanceOutput {
        address: format_address(&address),
        token,
        balance_hex,
        balance_u128,
    };
    print_output(&output, args.output.json)
}

async fn nonce_command(args: NonceArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let address = parse_hex_bytes::<20>(&args.address)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let block = resolve_block_tag(&args.block);
    let nonce = rpc
        .get_transaction_count(&address, block)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = NonceOutput {
        address: format_address(&address),
        nonce,
        nonce_hex: format!("0x{nonce:x}"),
    };
    print_output(&output, args.output.json)
}

async fn call_command(args: CallArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let request = RpcCallRequest {
        to: ensure_hex_prefixed(&args.to),
        from: args.from.as_ref().map(|value| ensure_hex_prefixed(value)),
        data: Some(ensure_hex_prefixed(&args.data)),
        value: args.value.as_ref().map(|value| ensure_hex_prefixed(value)),
        gas: args.gas.as_ref().map(|value| ensure_hex_prefixed(value)),
        gas_price: args.gas_price.as_ref().map(|value| ensure_hex_prefixed(value)),
    };
    let block = resolve_block_tag(&args.block);
    let result = rpc
        .call(request, block)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = CallOutput { data_hex: result };
    print_output(&output, args.output.json)
}

async fn send_command(args: SendArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let tx_hash = rpc
        .send_raw_transaction(&ensure_hex_prefixed(&args.raw))
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = SendOutput { tx_hash };
    print_output(&output, args.output.json)
}

async fn receipt_command(args: ReceiptArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let receipt = rpc
        .transaction_receipt(&ensure_hex_prefixed(&args.tx_hash))
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    print_output(&receipt, args.output.json)
}

async fn deposit_command(args: DepositArgs) -> Result<()> {
    match args.command {
        DepositCommand::Submit(args) => deposit_submit(args).await,
    }
}

async fn deposit_submit(args: DepositSubmitArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let result = rpc
        .send_raw_deposit_transaction(&ensure_hex_prefixed(&args.raw))
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    print_output(&result, args.output.json)
}

async fn txpool_command(args: TxpoolArgs) -> Result<()> {
    let rpc = resolve_rpc(&args.rpc)?;
    let result = rpc
        .txpool_content()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    print_output(&result, args.output.json)
}

fn resolve_block_tag(args: &BlockArgs) -> BlockTag {
    if let Some(number) = args.number {
        BlockTag::Number(number)
    } else {
        match args.tag {
            BlockTagArg::Latest => BlockTag::Latest,
            BlockTagArg::Pending => BlockTag::Pending,
            BlockTagArg::Earliest => BlockTag::Earliest,
        }
    }
}

fn resolve_rpc(args: &RpcArgs) -> Result<RpcClient> {
    let rpc_url = if let Some(rpc) = &args.rpc {
        rpc.to_string()
    } else if let Ok(value) = env::var("CITREA_RPC") {
        value
    } else if let Ok(value) = env::var("CITREA_RPC_URL") {
        value
    } else {
        anyhow::bail!("RPC URL required. Use --rpc or set CITREA_RPC/CITREA_RPC_URL");
    };

    Ok(RpcClient::new(rpc_url))
}

fn resolve_secret_key(secret_hex: Option<String>, input: &KeyDerivationArgs) -> Result<[u8; 32]> {
    if let Some(secret_hex) = secret_hex {
        return parse_hex_bytes::<32>(&secret_hex).map_err(|e| anyhow::anyhow!(e.to_string()));
    }

    let mnemonic = resolve_mnemonic(&input.mnemonic)?;
    let (account, _agent) = resolve_account(input.account, input.agent)?;
    let keypair = derive_keypair_full(&mnemonic, &input.mnemonic.passphrase, account)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(keypair.private_key)
}

fn resolve_mnemonic(input: &MnemonicInput) -> Result<String> {
    if input.stdin {
        return read_stdin_trimmed();
    }
    if let Some(mnemonic) = &input.mnemonic {
        return Ok(normalize_mnemonic(mnemonic));
    }
    if let Some(path) = &input.mnemonic_file {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read mnemonic file: {path:?}"))?;
        let normalized = normalize_mnemonic(&contents);
        if normalized.is_empty() {
            anyhow::bail!("Mnemonic file is empty");
        }
        return Ok(normalized);
    }
    if let Ok(mnemonic) = env::var("CITREA_MNEMONIC") {
        let normalized = normalize_mnemonic(&mnemonic);
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }
    if let Ok(mnemonic) = env::var("OPENAGENTS_MNEMONIC") {
        let normalized = normalize_mnemonic(&mnemonic);
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }

    anyhow::bail!(
        "Mnemonic required. Use --mnemonic, --mnemonic-file, --stdin, or set CITREA_MNEMONIC"
    )
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

fn read_hash_input(
    hash: Option<String>,
    hash_file: Option<PathBuf>,
    stdin: bool,
) -> Result<[u8; 32]> {
    let raw = if stdin {
        read_stdin_raw()?
    } else if let Some(path) = hash_file {
        fs::read_to_string(path).context("Failed to read hash file")?
    } else {
        hash.unwrap_or_default()
    };

    if raw.trim().is_empty() {
        anyhow::bail!("Hash is required (--hash, --hash-file, or --stdin)");
    }

    let normalized = normalize_hex_input(raw.trim());
    let normalized = strip_0x(&normalized);
    parse_hex_bytes::<32>(normalized).map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn read_stdin_trimmed() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(normalize_mnemonic(&buffer))
}

fn read_stdin_raw() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn normalize_mnemonic(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_hex_input(input: &str) -> String {
    input.chars().filter(|c| !c.is_whitespace()).collect()
}

fn generate_mnemonic(words: u16) -> Result<String> {
    let mut entropy = match words {
        12 => [0u8; 16].to_vec(),
        24 => [0u8; 32].to_vec(),
        _ => return Err(anyhow::anyhow!("Invalid word count. Use 12 or 24.")),
    };
    rand::rng().fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| anyhow::anyhow!(format!("Mnemonic error: {e}")))?;
    Ok(mnemonic.to_string())
}

fn ensure_hex_prefixed(value: &str) -> String {
    if value.starts_with("0x") || value.starts_with("0X") {
        value.to_string()
    } else {
        format!("0x{}", value)
    }
}

fn print_output<T: Serialize>(value: &T, json: bool) -> Result<()> {
    if json {
        let output = serde_json::to_string_pretty(value)?;
        println!("{output}");
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
                println!("{key}: {rendered}");
            }
        }
        other => {
            println!("{other}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sign_command() {
        let args = CitreaArgs::try_parse_from([
            "citrea",
            "sign",
            "--hash",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "--secret-hex",
            "0000000000000000000000000000000000000000000000000000000000000002",
        ])
        .expect("parse args");

        match args.command {
            CitreaCommand::Sign(sign) => {
                assert!(sign.hash.is_some());
                assert!(sign.secret_hex.is_some());
            }
            _ => panic!("expected sign command"),
        }
    }
}
