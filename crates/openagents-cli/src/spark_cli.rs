use anyhow::{Context, Result};
use bip39::Mnemonic;
use clap::{Args, Parser, Subcommand, ValueEnum};
use rand::RngCore;
use serde::Serialize;
use spark::{
    AssetFilter, BurnIssuerTokenRequest, CheckLightningAddressRequest, CheckMessageRequest,
    ClaimDepositRequest, CreateIssuerTokenRequest, Fee, FreezeIssuerTokenRequest,
    GetPaymentRequest, GetTokensMetadataRequest, InputType, ListPaymentsRequest,
    ListUnclaimedDepositsRequest, LnurlPayRequest, LnurlWithdrawRequest, MaxFee,
    MintIssuerTokenRequest, Network, PaymentStatus, PaymentType, PrepareLnurlPayRequest,
    PrepareSendPaymentRequest, ReceivePaymentMethod, ReceivePaymentRequest, RefundDepositRequest,
    RegisterLightningAddressRequest, SendPaymentMethod, SendPaymentOptions, SignMessageRequest,
    SparkHtlcOptions, SparkHtlcStatus, SparkSigner, SparkWallet, SyncWalletRequest,
    UnfreezeIssuerTokenRequest, UpdateUserSettingsRequest, WalletConfig, parse_input,
};
use std::env;
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Parser)]
pub struct SparkArgs {
    #[command(subcommand)]
    pub command: SparkCommand,
}

#[derive(Subcommand)]
pub enum SparkCommand {
    /// Generate a new mnemonic and derive Spark keys
    New(NewArgs),
    /// Derive Spark keys from an existing mnemonic
    Derive(DeriveArgs),
    /// Derive the BIP39 seed from a mnemonic
    Seed(SeedArgs),
    /// Derive a Spark public key from a secret key or mnemonic
    Pubkey(PubkeyArgs),
    /// Parse a payment input (lnurl, invoice, address)
    Parse(ParseArgs),
    /// Wallet status and sync operations
    Wallet(WalletArgs),
    /// Receive payments (addresses or invoices)
    Receive(ReceiveArgs),
    /// Send payments (Lightning, Spark, on-chain)
    Send(SendArgs),
    /// Payment history and lookups
    Payments(PaymentsArgs),
    /// LNURL pay/withdraw helpers
    Lnurl(LnurlArgs),
    /// Lightning address management
    LightningAddress(LightningAddressArgs),
    /// On-chain deposit management
    Deposits(DepositsArgs),
    /// Fiat currency helpers
    Fiat(FiatArgs),
    /// Token metadata and issuer operations
    Tokens(TokensArgs),
    /// User settings
    Settings(SettingsArgs),
    /// Message signing and verification
    Message(MessageArgs),
    /// Leaf optimization controls
    Optimize(OptimizeArgs),
    /// Regtest faucet helper
    Faucet(FaucetArgs),
}

#[derive(Args)]
pub struct NewArgs {
    /// Number of words in the mnemonic (12 or 24)
    #[arg(long, default_value = "12")]
    pub words: u16,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
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
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct PubkeyArgs {
    /// Private key hex (32 bytes)
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin", "mnemonic_file"])]
    pub private_hex: Option<String>,
    /// Mnemonic phrase (12 or 24 words)
    #[arg(long, conflicts_with_all = ["private_hex", "stdin", "mnemonic_file"])]
    pub mnemonic: Option<String>,
    /// Read mnemonic from stdin
    #[arg(long, conflicts_with_all = ["private_hex", "mnemonic", "mnemonic_file"])]
    pub stdin: bool,
    /// Read mnemonic from a file
    #[arg(long, conflicts_with_all = ["private_hex", "mnemonic", "stdin"])]
    pub mnemonic_file: Option<PathBuf>,
    /// Optional BIP39 passphrase (when using mnemonic)
    #[arg(long, default_value = "")]
    pub passphrase: String,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct ParseArgs {
    /// Input to parse (lnurl, invoice, address)
    pub input: String,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub command: WalletCommand,
}

#[derive(Subcommand)]
pub enum WalletCommand {
    /// Get wallet info
    Info(WalletInfoArgs),
    /// Get wallet balance
    Balance(WalletBalanceArgs),
    /// Check network connectivity
    Network(WalletNetworkArgs),
    /// Force wallet sync
    Sync(WalletSyncArgs),
    /// Disconnect the wallet
    Disconnect(WalletDisconnectArgs),
}

#[derive(Args)]
pub struct WalletInfoArgs {
    /// Force sync before returning info
    #[arg(long)]
    pub ensure_synced: bool,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct WalletBalanceArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct WalletNetworkArgs {
    /// Timeout in seconds
    #[arg(long, default_value = "5")]
    pub timeout_secs: u64,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct WalletSyncArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct WalletDisconnectArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Clone, Args)]
pub struct ReceiveArgs {
    /// Receive method (spark-address, spark-invoice, bitcoin, bolt11)
    #[arg(long, value_enum)]
    pub method: ReceiveMethodArg,
    /// Amount to receive (sats or token base units)
    #[arg(long)]
    pub amount: Option<u128>,
    /// Optional token identifier (spark invoice only)
    #[arg(long)]
    pub token_identifier: Option<String>,
    /// Optional invoice description
    #[arg(long)]
    pub description: Option<String>,
    /// Expiry in seconds from now (spark invoice or bolt11)
    #[arg(long)]
    pub expiry_secs: Option<u64>,
    /// Optional sender public key (spark invoice only)
    #[arg(long)]
    pub sender_public_key: Option<String>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(ValueEnum, Clone, Copy)]
#[value(rename_all = "kebab-case")]
pub enum ReceiveMethodArg {
    SparkAddress,
    SparkInvoice,
    Bitcoin,
    Bolt11,
}

#[derive(Args)]
pub struct SendArgs {
    /// Payment request (invoice, address, lnurl)
    pub payment_request: String,
    /// Amount to send (sats or token base units)
    #[arg(long)]
    pub amount: Option<u128>,
    /// Optional token identifier (spark address only)
    #[arg(long)]
    pub token_identifier: Option<String>,
    /// Optional idempotency key (UUID)
    #[arg(long)]
    pub idempotency_key: Option<String>,
    /// Prefer Spark for bolt11 invoices when possible
    #[arg(long)]
    pub prefer_spark: bool,
    /// Completion timeout in seconds for bolt11 invoices
    #[arg(long)]
    pub completion_timeout_secs: Option<u32>,
    /// HTLC payment hash (spark address only)
    #[arg(long)]
    pub htlc_payment_hash: Option<String>,
    /// HTLC expiry duration in seconds (spark address only)
    #[arg(long)]
    pub htlc_expiry_secs: Option<u64>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct PaymentsArgs {
    #[command(subcommand)]
    pub command: PaymentsCommand,
}

#[derive(Subcommand)]
pub enum PaymentsCommand {
    /// List payments
    List(PaymentsListArgs),
    /// Get a payment by id
    Get(PaymentsGetArgs),
}

#[derive(Args)]
pub struct PaymentsListArgs {
    /// Filter by payment type
    #[arg(long)]
    pub type_filter: Option<Vec<PaymentType>>,
    /// Filter by payment status
    #[arg(long)]
    pub status_filter: Option<Vec<PaymentStatus>>,
    /// Filter by asset (bitcoin, token, token:<id>)
    #[arg(long)]
    pub asset_filter: Option<String>,
    /// Filter by Spark HTLC status
    #[arg(long)]
    pub spark_htlc_status_filter: Option<Vec<SparkHtlcStatus>>,
    /// Only include payments created after this timestamp (inclusive)
    #[arg(long)]
    pub from_timestamp: Option<u64>,
    /// Only include payments created before this timestamp (exclusive)
    #[arg(long)]
    pub to_timestamp: Option<u64>,
    /// Number of payments to show
    #[arg(long, default_value = "10")]
    pub limit: u32,
    /// Number of payments to skip
    #[arg(long, default_value = "0")]
    pub offset: u32,
    /// Sort payments in ascending order
    #[arg(long)]
    pub sort_ascending: Option<bool>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct PaymentsGetArgs {
    /// Payment id
    pub payment_id: String,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LnurlArgs {
    #[command(subcommand)]
    pub command: LnurlCommand,
}

#[derive(Subcommand)]
pub enum LnurlCommand {
    /// Prepare an LNURL-pay request
    Prepare(LnurlPrepareArgs),
    /// Pay an LNURL request
    Pay(LnurlPayArgs),
    /// Withdraw using LNURL
    Withdraw(LnurlWithdrawArgs),
}

#[derive(Args)]
pub struct LnurlPrepareArgs {
    /// LNURL or lightning address
    pub lnurl: String,
    /// Amount to pay in sats
    #[arg(long)]
    pub amount_sats: u64,
    /// Optional comment
    #[arg(long)]
    pub comment: Option<String>,
    /// Validate success action URL
    #[arg(long)]
    pub validate_success_url: Option<bool>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LnurlPayArgs {
    /// LNURL or lightning address
    pub lnurl: String,
    /// Amount to pay in sats
    #[arg(long)]
    pub amount_sats: u64,
    /// Optional comment
    #[arg(long)]
    pub comment: Option<String>,
    /// Validate success action URL
    #[arg(long)]
    pub validate_success_url: Option<bool>,
    /// Optional idempotency key (UUID)
    #[arg(long)]
    pub idempotency_key: Option<String>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LnurlWithdrawArgs {
    /// LNURL-withdraw endpoint
    pub lnurl: String,
    /// Amount to withdraw in sats
    #[arg(long)]
    pub amount_sats: u64,
    /// Optional completion timeout in seconds
    #[arg(long)]
    pub completion_timeout_secs: Option<u32>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LightningAddressArgs {
    #[command(subcommand)]
    pub command: LightningAddressCommand,
}

#[derive(Subcommand)]
pub enum LightningAddressCommand {
    /// Check lightning address availability
    Check(LightningAddressCheckArgs),
    /// Get the registered lightning address
    Get(LightningAddressGetArgs),
    /// Register a lightning address
    Register(LightningAddressRegisterArgs),
    /// Delete the registered lightning address
    Delete(LightningAddressDeleteArgs),
}

#[derive(Args)]
pub struct LightningAddressCheckArgs {
    /// Username to check
    pub username: String,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LightningAddressGetArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LightningAddressRegisterArgs {
    /// Username to register
    pub username: String,
    /// Optional description
    #[arg(long)]
    pub description: Option<String>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct LightningAddressDeleteArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct DepositsArgs {
    #[command(subcommand)]
    pub command: DepositsCommand,
}

#[derive(Subcommand)]
pub enum DepositsCommand {
    /// List unclaimed deposits
    List(DepositsListArgs),
    /// Claim an on-chain deposit
    Claim(DepositsClaimArgs),
    /// Refund an on-chain deposit
    Refund(DepositsRefundArgs),
}

#[derive(Args)]
pub struct DepositsListArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct DepositsClaimArgs {
    /// Deposit txid
    pub txid: String,
    /// Deposit vout
    pub vout: u32,
    /// Max fee in sats
    #[arg(long)]
    pub fee_sat: Option<u64>,
    /// Max fee rate in sat/vbyte
    #[arg(long)]
    pub sat_per_vbyte: Option<u64>,
    /// Use network recommended fee plus leeway
    #[arg(long)]
    pub recommended_fee_leeway: Option<u64>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct DepositsRefundArgs {
    /// Deposit txid
    pub txid: String,
    /// Deposit vout
    pub vout: u32,
    /// Destination address
    pub destination_address: String,
    /// Fee in sats
    #[arg(long)]
    pub fee_sat: Option<u64>,
    /// Fee rate in sat/vbyte
    #[arg(long)]
    pub sat_per_vbyte: Option<u64>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct FiatArgs {
    #[command(subcommand)]
    pub command: FiatCommand,
}

#[derive(Subcommand)]
pub enum FiatCommand {
    /// List fiat currencies
    Currencies(FiatCurrenciesArgs),
    /// List fiat exchange rates
    Rates(FiatRatesArgs),
}

#[derive(Args)]
pub struct FiatCurrenciesArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct FiatRatesArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensArgs {
    #[command(subcommand)]
    pub command: TokensCommand,
}

#[derive(Subcommand)]
pub enum TokensCommand {
    /// Get metadata for token identifiers
    Metadata(TokensMetadataArgs),
    /// Issuer operations (create/mint/burn/freeze)
    Issuer(TokensIssuerArgs),
}

#[derive(Args)]
pub struct TokensMetadataArgs {
    /// Token identifiers
    pub token_identifiers: Vec<String>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerArgs {
    #[command(subcommand)]
    pub command: TokensIssuerCommand,
}

#[derive(Subcommand)]
pub enum TokensIssuerCommand {
    /// Get issuer token balance
    Balance(TokensIssuerBalanceArgs),
    /// Get issuer token metadata
    Metadata(TokensIssuerMetadataArgs),
    /// Create a new issuer token
    Create(TokensIssuerCreateArgs),
    /// Mint issuer token supply
    Mint(TokensIssuerMintArgs),
    /// Burn issuer token supply
    Burn(TokensIssuerBurnArgs),
    /// Freeze issuer tokens held at address
    Freeze(TokensIssuerFreezeArgs),
    /// Unfreeze issuer tokens held at address
    Unfreeze(TokensIssuerUnfreezeArgs),
}

#[derive(Args)]
pub struct TokensIssuerBalanceArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerMetadataArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerCreateArgs {
    /// Token name
    pub name: String,
    /// Token ticker
    pub ticker: String,
    /// Token decimals
    pub decimals: u32,
    /// Whether the token is freezable
    #[arg(long)]
    pub is_freezable: bool,
    /// Maximum supply
    pub max_supply: u128,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerMintArgs {
    /// Amount to mint
    pub amount: u128,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerBurnArgs {
    /// Amount to burn
    pub amount: u128,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerFreezeArgs {
    /// Spark address to freeze
    pub address: String,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct TokensIssuerUnfreezeArgs {
    /// Spark address to unfreeze
    pub address: String,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct SettingsArgs {
    #[command(subcommand)]
    pub command: SettingsCommand,
}

#[derive(Subcommand)]
pub enum SettingsCommand {
    /// Get user settings
    Get(SettingsGetArgs),
    /// Update user settings
    Set(SettingsSetArgs),
}

#[derive(Args)]
pub struct SettingsGetArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct SettingsSetArgs {
    /// Enable or disable Spark private mode
    #[arg(long)]
    pub spark_private_mode_enabled: Option<bool>,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct MessageArgs {
    #[command(subcommand)]
    pub command: MessageCommand,
}

#[derive(Subcommand)]
pub enum MessageCommand {
    /// Sign an arbitrary message
    Sign(MessageSignArgs),
    /// Verify a message signature
    Check(MessageCheckArgs),
}

#[derive(Args)]
pub struct MessageSignArgs {
    /// Message to sign
    pub message: String,
    /// Use compact signature encoding
    #[arg(long)]
    pub compact: bool,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct MessageCheckArgs {
    /// Message that was signed
    pub message: String,
    /// Public key hex
    pub pubkey: String,
    /// Signature hex (DER or compact)
    pub signature: String,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct OptimizeArgs {
    #[command(subcommand)]
    pub command: OptimizeCommand,
}

#[derive(Subcommand)]
pub enum OptimizeCommand {
    /// Start leaf optimization
    Start(OptimizeStartArgs),
    /// Cancel leaf optimization
    Cancel(OptimizeCancelArgs),
    /// Get optimization progress
    Status(OptimizeStatusArgs),
}

#[derive(Args)]
pub struct OptimizeStartArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct OptimizeCancelArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct OptimizeStatusArgs {
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Args)]
pub struct FaucetArgs {
    /// Amount in sats (default: 100000)
    #[arg(long, default_value = "100000")]
    pub amount: u64,
    #[command(flatten)]
    pub common: WalletCommandArgs,
}

#[derive(Clone, Args)]
pub struct WalletOptions {
    /// Mnemonic phrase (12 or 24 words)
    #[arg(long, conflicts_with_all = ["stdin", "mnemonic_file", "entropy_hex"])]
    pub mnemonic: Option<String>,
    /// Read mnemonic from stdin
    #[arg(long, conflicts_with_all = ["mnemonic", "mnemonic_file", "entropy_hex"])]
    pub stdin: bool,
    /// Read mnemonic from a file
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin", "entropy_hex"])]
    pub mnemonic_file: Option<PathBuf>,
    /// Raw seed entropy hex (advanced)
    #[arg(long, conflicts_with_all = ["mnemonic", "stdin", "mnemonic_file"])]
    pub entropy_hex: Option<String>,
    /// Optional BIP39 passphrase
    #[arg(long, default_value = "")]
    pub passphrase: String,
    /// Spark network (testnet maps to regtest in Breez)
    #[arg(long, value_enum, default_value = "testnet")]
    pub network: NetworkArg,
    /// Breez API key (required for mainnet)
    #[arg(long)]
    pub api_key: Option<String>,
    /// Override storage directory
    #[arg(long)]
    pub storage_dir: Option<PathBuf>,
    /// Override key set type
    #[arg(long, value_enum)]
    pub key_set: Option<KeySetArg>,
    /// Use address index for derivation
    #[arg(long)]
    pub use_address_index: bool,
    /// Override account number for derivation
    #[arg(long)]
    pub account_number: Option<u32>,
}

#[derive(Clone, Args)]
pub struct WalletCommandArgs {
    #[command(flatten)]
    pub wallet: WalletOptions,
    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Clone, Args)]
pub struct OutputArgs {
    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(ValueEnum, Clone, Copy)]
#[value(rename_all = "kebab-case")]
pub enum NetworkArg {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl From<NetworkArg> for Network {
    fn from(value: NetworkArg) -> Self {
        match value {
            NetworkArg::Mainnet => Network::Mainnet,
            NetworkArg::Testnet => Network::Testnet,
            NetworkArg::Signet => Network::Signet,
            NetworkArg::Regtest => Network::Regtest,
        }
    }
}

#[derive(ValueEnum, Clone, Copy)]
#[value(rename_all = "kebab-case")]
pub enum KeySetArg {
    Default,
    Taproot,
    NativeSegwit,
    WrappedSegwit,
    Legacy,
}

impl From<KeySetArg> for spark::KeySetType {
    fn from(value: KeySetArg) -> Self {
        match value {
            KeySetArg::Default => spark::KeySetType::Default,
            KeySetArg::Taproot => spark::KeySetType::Taproot,
            KeySetArg::NativeSegwit => spark::KeySetType::NativeSegwit,
            KeySetArg::WrappedSegwit => spark::KeySetType::WrappedSegwit,
            KeySetArg::Legacy => spark::KeySetType::Legacy,
        }
    }
}

#[derive(Serialize)]
struct KeypairOutput {
    mnemonic: Option<String>,
    public_key_hex: String,
    private_key_hex: Option<String>,
}

#[derive(Serialize)]
struct SeedOutput {
    seed_hex: String,
}

#[derive(Serialize)]
struct PubkeyOutput {
    public_key_hex: String,
}

#[derive(Serialize)]
struct StatusOutput {
    ok: bool,
}

#[derive(Serialize)]
struct LnurlPrepareOutput {
    amount_sats: u64,
    comment: Option<String>,
    fee_sats: u64,
    pay_request: serde_json::Value,
    invoice_details: serde_json::Value,
    success_action: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct MessageSignOutput {
    pubkey: String,
    signature: String,
}

#[derive(Serialize)]
struct MessageCheckOutput {
    is_valid: bool,
}

#[derive(Serialize)]
struct OptimizationProgressOutput {
    is_running: bool,
    current_round: u32,
    total_rounds: u32,
}

#[derive(Clone)]
struct SendOptionsInput {
    prefer_spark: bool,
    completion_timeout_secs: Option<u32>,
    htlc_payment_hash: Option<String>,
    htlc_expiry_secs: Option<u64>,
}

pub fn run(args: SparkArgs) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("Failed to start Tokio runtime")?;
    runtime.block_on(run_async(args))
}

async fn run_async(args: SparkArgs) -> Result<()> {
    match args.command {
        SparkCommand::New(args) => new_keypair(args),
        SparkCommand::Derive(args) => derive_keypair(args),
        SparkCommand::Seed(args) => derive_seed(args),
        SparkCommand::Pubkey(args) => derive_pubkey(args),
        SparkCommand::Parse(args) => parse_input_command(args).await,
        SparkCommand::Wallet(args) => wallet_command(args).await,
        SparkCommand::Receive(args) => receive_command(args).await,
        SparkCommand::Send(args) => send_command(args).await,
        SparkCommand::Payments(args) => payments_command(args).await,
        SparkCommand::Lnurl(args) => lnurl_command(args).await,
        SparkCommand::LightningAddress(args) => lightning_address_command(args).await,
        SparkCommand::Deposits(args) => deposits_command(args).await,
        SparkCommand::Fiat(args) => fiat_command(args).await,
        SparkCommand::Tokens(args) => tokens_command(args).await,
        SparkCommand::Settings(args) => settings_command(args).await,
        SparkCommand::Message(args) => message_command(args).await,
        SparkCommand::Optimize(args) => optimize_command(args).await,
        SparkCommand::Faucet(args) => faucet_command(args).await,
    }
}

fn new_keypair(args: NewArgs) -> Result<()> {
    let mnemonic = generate_mnemonic(args.words)?;
    let signer = SparkSigner::from_mnemonic(&mnemonic, &args.passphrase)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = KeypairOutput {
        mnemonic: if args.no_mnemonic {
            None
        } else {
            Some(mnemonic)
        },
        public_key_hex: signer.public_key_hex(),
        private_key_hex: if args.no_private {
            None
        } else {
            Some(signer.private_key_hex())
        },
    };
    print_output(&output, args.output.json)
}

fn derive_keypair(args: DeriveArgs) -> Result<()> {
    let mnemonic = read_mnemonic_input(args.mnemonic, args.mnemonic_file, args.stdin)?;
    let signer = SparkSigner::from_mnemonic(&mnemonic, &args.passphrase)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let output = KeypairOutput {
        mnemonic: if args.show_mnemonic {
            Some(mnemonic)
        } else {
            None
        },
        public_key_hex: signer.public_key_hex(),
        private_key_hex: if args.no_private {
            None
        } else {
            Some(signer.private_key_hex())
        },
    };
    print_output(&output, args.output.json)
}

fn derive_seed(args: SeedArgs) -> Result<()> {
    let mnemonic = read_mnemonic_input(args.mnemonic, args.mnemonic_file, args.stdin)?;
    let parsed = Mnemonic::parse(&mnemonic).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let seed = parsed.to_seed(&args.passphrase);
    let output = SeedOutput {
        seed_hex: hex::encode(seed),
    };
    print_output(&output, args.output.json)
}

fn derive_pubkey(args: PubkeyArgs) -> Result<()> {
    let public_key_hex = if let Some(private_hex) = args.private_hex {
        let private_key = parse_private_key_hex(&private_hex)?;
        let secp = bitcoin::secp256k1::Secp256k1::new();
        let public_key = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &private_key);
        hex::encode(public_key.serialize())
    } else {
        let mnemonic = read_mnemonic_input(args.mnemonic, args.mnemonic_file, args.stdin)?;
        let signer = SparkSigner::from_mnemonic(&mnemonic, &args.passphrase)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        signer.public_key_hex()
    };

    let output = PubkeyOutput { public_key_hex };
    print_output(&output, args.output.json)
}

async fn parse_input_command(args: ParseArgs) -> Result<()> {
    let parsed = parse_input(&args.input, None)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    print_output(&parsed, args.output.json)
}

async fn wallet_command(args: WalletArgs) -> Result<()> {
    match args.command {
        WalletCommand::Info(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let info = wallet
                .get_info(args.ensure_synced)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&info, args.common.output.json)
        }
        WalletCommand::Balance(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let balance = wallet
                .get_balance()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&balance, args.common.output.json)
        }
        WalletCommand::Network(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let status = wallet
                .network_status(Duration::from_secs(args.timeout_secs))
                .await;
            print_output(&status, args.common.output.json)
        }
        WalletCommand::Sync(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .sync_wallet(SyncWalletRequest {})
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        WalletCommand::Disconnect(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            wallet
                .disconnect()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let output = StatusOutput { ok: true };
            print_output(&output, args.common.output.json)
        }
    }
}

async fn receive_command(args: ReceiveArgs) -> Result<()> {
    let wallet = build_wallet(&args.common.wallet).await?;
    let method = match args.method {
        ReceiveMethodArg::SparkAddress => ReceivePaymentMethod::SparkAddress,
        ReceiveMethodArg::SparkInvoice => ReceivePaymentMethod::SparkInvoice {
            amount: args.amount,
            token_identifier: args.token_identifier,
            expiry_time: args.expiry_secs.map(expiry_time_from_now).transpose()?,
            description: args.description,
            sender_public_key: args.sender_public_key,
        },
        ReceiveMethodArg::Bitcoin => ReceivePaymentMethod::BitcoinAddress,
        ReceiveMethodArg::Bolt11 => ReceivePaymentMethod::Bolt11Invoice {
            description: args.description.unwrap_or_default(),
            amount_sats: args.amount.map(amount_u128_to_u64).transpose()?,
            expiry_secs: args
                .expiry_secs
                .map(|secs| u32::try_from(secs).map_err(|_| anyhow::anyhow!("Expiry exceeds u32")))
                .transpose()?,
        },
    };

    let response = wallet
        .receive_payment(ReceivePaymentRequest {
            payment_method: method,
        })
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    print_output(&response, args.common.output.json)
}

async fn send_command(args: SendArgs) -> Result<()> {
    let SendArgs {
        payment_request,
        amount,
        token_identifier,
        idempotency_key,
        prefer_spark,
        completion_timeout_secs,
        htlc_payment_hash,
        htlc_expiry_secs,
        common,
    } = args;

    let wallet = build_wallet(&common.wallet).await?;

    let prepare_request = PrepareSendPaymentRequest {
        payment_request,
        amount,
        token_identifier,
    };

    let prepare_response = wallet
        .prepare_send_payment_request(prepare_request)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let options_input = SendOptionsInput {
        prefer_spark,
        completion_timeout_secs,
        htlc_payment_hash,
        htlc_expiry_secs,
    };

    let options = build_send_options(
        &prepare_response.payment_method,
        &prepare_response,
        &options_input,
    )?;

    let response = match options {
        Some(options) => wallet
            .send_payment_with_options(prepare_response, options, idempotency_key)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        None => wallet
            .send_payment(prepare_response, idempotency_key)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    };

    print_output(&response, common.output.json)
}

async fn payments_command(args: PaymentsArgs) -> Result<()> {
    match args.command {
        PaymentsCommand::List(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let asset_filter = match args.asset_filter {
                Some(filter) => Some(parse_asset_filter(&filter)?),
                None => None,
            };
            let response = wallet
                .list_payments_request(ListPaymentsRequest {
                    limit: Some(args.limit),
                    offset: Some(args.offset),
                    type_filter: args.type_filter,
                    status_filter: args.status_filter,
                    asset_filter,
                    spark_htlc_status_filter: args.spark_htlc_status_filter,
                    from_timestamp: args.from_timestamp,
                    to_timestamp: args.to_timestamp,
                    sort_ascending: args.sort_ascending,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        PaymentsCommand::Get(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .get_payment(GetPaymentRequest {
                    payment_id: args.payment_id,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
    }
}

async fn lnurl_command(args: LnurlArgs) -> Result<()> {
    match args.command {
        LnurlCommand::Prepare(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let pay_request = resolve_lnurl_pay_request(&wallet, &args.lnurl).await?;
            validate_lnurl_amount(args.amount_sats, &pay_request)?;

            let response = wallet
                .prepare_lnurl_pay(PrepareLnurlPayRequest {
                    amount_sats: args.amount_sats,
                    comment: args.comment,
                    pay_request,
                    validate_success_action_url: args.validate_success_url,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let output = LnurlPrepareOutput {
                amount_sats: response.amount_sats,
                comment: response.comment,
                fee_sats: response.fee_sats,
                pay_request: serde_json::to_value(&response.pay_request)?,
                invoice_details: serde_json::to_value(&response.invoice_details)?,
                success_action: response
                    .success_action
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?,
            };

            print_output(&output, args.common.output.json)
        }
        LnurlCommand::Pay(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let pay_request = resolve_lnurl_pay_request(&wallet, &args.lnurl).await?;
            validate_lnurl_amount(args.amount_sats, &pay_request)?;

            let prepare_response = wallet
                .prepare_lnurl_pay(PrepareLnurlPayRequest {
                    amount_sats: args.amount_sats,
                    comment: args.comment,
                    pay_request,
                    validate_success_action_url: args.validate_success_url,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            let response = wallet
                .lnurl_pay(LnurlPayRequest {
                    prepare_response,
                    idempotency_key: args.idempotency_key,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            print_output(&response, args.common.output.json)
        }
        LnurlCommand::Withdraw(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let withdraw_request = resolve_lnurl_withdraw_request(&wallet, &args.lnurl).await?;
            validate_lnurl_withdraw_amount(args.amount_sats, &withdraw_request)?;

            let response = wallet
                .lnurl_withdraw(LnurlWithdrawRequest {
                    amount_sats: args.amount_sats,
                    withdraw_request,
                    completion_timeout_secs: args.completion_timeout_secs,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;

            print_output(&response, args.common.output.json)
        }
    }
}

async fn lightning_address_command(args: LightningAddressArgs) -> Result<()> {
    match args.command {
        LightningAddressCommand::Check(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .check_lightning_address_available(CheckLightningAddressRequest {
                    username: args.username,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        LightningAddressCommand::Get(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .get_lightning_address()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        LightningAddressCommand::Register(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .register_lightning_address(RegisterLightningAddressRequest {
                    username: args.username,
                    description: args.description,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        LightningAddressCommand::Delete(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            wallet
                .delete_lightning_address()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let output = StatusOutput { ok: true };
            print_output(&output, args.common.output.json)
        }
    }
}

async fn deposits_command(args: DepositsArgs) -> Result<()> {
    match args.command {
        DepositsCommand::List(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .list_unclaimed_deposits(ListUnclaimedDepositsRequest {})
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        DepositsCommand::Claim(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let max_fee = parse_max_fee(
                args.fee_sat,
                args.sat_per_vbyte,
                args.recommended_fee_leeway,
            )?;
            let response = wallet
                .claim_deposit(ClaimDepositRequest {
                    txid: args.txid,
                    vout: args.vout,
                    max_fee,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        DepositsCommand::Refund(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let fee = parse_fee(args.fee_sat, args.sat_per_vbyte)?;
            let response = wallet
                .refund_deposit(RefundDepositRequest {
                    txid: args.txid,
                    vout: args.vout,
                    destination_address: args.destination_address,
                    fee,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
    }
}

async fn fiat_command(args: FiatArgs) -> Result<()> {
    match args.command {
        FiatCommand::Currencies(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .list_fiat_currencies()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        FiatCommand::Rates(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .list_fiat_rates()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
    }
}

async fn tokens_command(args: TokensArgs) -> Result<()> {
    match args.command {
        TokensCommand::Metadata(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .get_tokens_metadata(GetTokensMetadataRequest {
                    token_identifiers: args.token_identifiers,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        TokensCommand::Issuer(args) => match args.command {
            TokensIssuerCommand::Balance(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .get_issuer_token_balance()
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
            TokensIssuerCommand::Metadata(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .get_issuer_token_metadata()
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
            TokensIssuerCommand::Create(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .create_issuer_token(CreateIssuerTokenRequest {
                        name: args.name,
                        ticker: args.ticker,
                        decimals: args.decimals,
                        is_freezable: args.is_freezable,
                        max_supply: args.max_supply,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
            TokensIssuerCommand::Mint(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .mint_issuer_token(MintIssuerTokenRequest {
                        amount: args.amount,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
            TokensIssuerCommand::Burn(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .burn_issuer_token(BurnIssuerTokenRequest {
                        amount: args.amount,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
            TokensIssuerCommand::Freeze(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .freeze_issuer_token(FreezeIssuerTokenRequest {
                        address: args.address,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
            TokensIssuerCommand::Unfreeze(args) => {
                let wallet = build_wallet(&args.common.wallet).await?;
                let issuer = wallet.get_token_issuer();
                let response = issuer
                    .unfreeze_issuer_token(UnfreezeIssuerTokenRequest {
                        address: args.address,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                print_output(&response, args.common.output.json)
            }
        },
    }
}

async fn settings_command(args: SettingsArgs) -> Result<()> {
    match args.command {
        SettingsCommand::Get(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .get_user_settings()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            print_output(&response, args.common.output.json)
        }
        SettingsCommand::Set(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            if args.spark_private_mode_enabled.is_none() {
                anyhow::bail!("Provide --spark-private-mode-enabled true|false");
            }
            wallet
                .update_user_settings(UpdateUserSettingsRequest {
                    spark_private_mode_enabled: args.spark_private_mode_enabled,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let output = StatusOutput { ok: true };
            print_output(&output, args.common.output.json)
        }
    }
}

async fn message_command(args: MessageArgs) -> Result<()> {
    match args.command {
        MessageCommand::Sign(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .sign_message(SignMessageRequest {
                    message: args.message,
                    compact: args.compact,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let output = MessageSignOutput {
                pubkey: response.pubkey,
                signature: response.signature,
            };
            print_output(&output, args.common.output.json)
        }
        MessageCommand::Check(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet
                .check_message(CheckMessageRequest {
                    message: args.message,
                    pubkey: args.pubkey,
                    signature: args.signature,
                })
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let output = MessageCheckOutput {
                is_valid: response.is_valid,
            };
            print_output(&output, args.common.output.json)
        }
    }
}

async fn optimize_command(args: OptimizeArgs) -> Result<()> {
    match args.command {
        OptimizeCommand::Start(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            wallet.start_leaf_optimization();
            let output = StatusOutput { ok: true };
            print_output(&output, args.common.output.json)
        }
        OptimizeCommand::Cancel(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            wallet
                .cancel_leaf_optimization()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let output = StatusOutput { ok: true };
            print_output(&output, args.common.output.json)
        }
        OptimizeCommand::Status(args) => {
            let wallet = build_wallet(&args.common.wallet).await?;
            let response = wallet.get_leaf_optimization_progress();
            let output = OptimizationProgressOutput {
                is_running: response.is_running,
                current_round: response.current_round,
                total_rounds: response.total_rounds,
            };
            print_output(&output, args.common.output.json)
        }
    }
}

async fn faucet_command(args: FaucetArgs) -> Result<()> {
    let wallet = build_wallet(&args.common.wallet).await?;
    let address = wallet
        .get_bitcoin_address()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let faucet_url = env::var("FAUCET_URL")
        .unwrap_or_else(|_| "https://api.lightspark.com/graphql/spark/rc".to_string());
    let faucet_username = env::var("FAUCET_USERNAME").ok();
    let faucet_password = env::var("FAUCET_PASSWORD").ok();

    let client = reqwest::Client::new();
    let mut request = client
        .post(&faucet_url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "operationName": "RequestRegtestFunds",
            "variables": {
                "address": address,
                "amount_sats": args.amount
            },
            "query": "mutation RequestRegtestFunds($address: String!, $amount_sats: Long!) { request_regtest_funds(input: {address: $address, amount_sats: $amount_sats}) { transaction_hash }}"
        }));

    if let (Some(username), Some(password)) = (&faucet_username, &faucet_password) {
        request = request.basic_auth(username, Some(password));
    }

    let response = request
        .send()
        .await
        .map_err(|e| anyhow::anyhow!(format!("Faucet request failed: {e}")))?;

    let result: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!(format!("Failed to parse response: {e}")))?;

    if let Some(errors) = result.get("errors").and_then(|e| e.as_array()) {
        if let Some(err) = errors.first() {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            if msg.contains("Not logged in") || msg.contains("auth") {
                anyhow::bail!(
                    "Faucet error: {msg}\n\nThe Lightspark regtest faucet requires authentication.\n\nOptions:\n  1. Use the web faucet: https://app.lightspark.com/regtest-faucet\n     Copy your Bitcoin address and paste it there.\n  2. Set FAUCET_USERNAME and FAUCET_PASSWORD if you have API credentials.\n\nSee: crates/spark/docs/REGTEST.md"
                );
            }
            anyhow::bail!("Faucet error: {msg}");
        }
    }

    print_output(&result, args.common.output.json)
}

fn build_send_options(
    payment_method: &SendPaymentMethod,
    prepare_response: &spark::PrepareSendPaymentResponse,
    args: &SendOptionsInput,
) -> Result<Option<SendPaymentOptions>> {
    let has_bolt11_options = args.prefer_spark || args.completion_timeout_secs.is_some();
    let has_htlc_options = args.htlc_payment_hash.is_some() || args.htlc_expiry_secs.is_some();

    if has_bolt11_options && has_htlc_options {
        anyhow::bail!("Choose either bolt11 options or HTLC options, not both");
    }

    if has_htlc_options {
        match payment_method {
            SendPaymentMethod::SparkAddress { .. } => {}
            _ => anyhow::bail!("HTLC options are only valid for Spark address payments"),
        }
        if prepare_response.token_identifier.is_some() {
            anyhow::bail!("HTLC options are not supported for token payments");
        }
        let payment_hash = args
            .htlc_payment_hash
            .clone()
            .ok_or_else(|| anyhow::anyhow!("--htlc-payment-hash is required"))?;
        let expiry_duration_secs = args
            .htlc_expiry_secs
            .ok_or_else(|| anyhow::anyhow!("--htlc-expiry-secs is required"))?;
        return Ok(Some(SendPaymentOptions::SparkAddress {
            htlc_options: Some(SparkHtlcOptions {
                payment_hash,
                expiry_duration_secs,
            }),
        }));
    }

    if has_bolt11_options {
        match payment_method {
            SendPaymentMethod::Bolt11Invoice { .. } => {}
            _ => anyhow::bail!("Bolt11 options are only valid for Lightning invoices"),
        }
        return Ok(Some(SendPaymentOptions::Bolt11Invoice {
            prefer_spark: args.prefer_spark,
            completion_timeout_secs: args.completion_timeout_secs,
        }));
    }

    Ok(None)
}

async fn build_wallet(options: &WalletOptions) -> Result<SparkWallet> {
    let signer = build_signer(options)?;

    let mut config = WalletConfig::default();
    config.network = options.network.into();
    config.api_key = resolve_api_key(options.api_key.clone());
    if let Some(storage_dir) = &options.storage_dir {
        config.storage_dir = storage_dir.clone();
    }

    if matches!(config.network, Network::Mainnet) && config.api_key.is_none() {
        anyhow::bail!(
            "Mainnet requires an API key. Provide --api-key or set SPARK_API_KEY/BREEZ_API_KEY"
        );
    }

    let mut builder = SparkWallet::builder(signer, config);

    if options.key_set.is_some() || options.use_address_index || options.account_number.is_some() {
        let key_set = options.key_set.unwrap_or(KeySetArg::Default).into();
        builder = builder.with_key_set(key_set, options.use_address_index, options.account_number);
    }

    builder
        .build()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn build_signer(options: &WalletOptions) -> Result<SparkSigner> {
    if let Some(entropy_hex) = &options.entropy_hex {
        let entropy = hex::decode(entropy_hex)
            .map_err(|e| anyhow::anyhow!(format!("Invalid entropy hex: {e}")))?;
        return SparkSigner::from_entropy(&entropy).map_err(|e| anyhow::anyhow!(e.to_string()));
    }

    let mnemonic = resolve_wallet_mnemonic(options)?;
    SparkSigner::from_mnemonic(&mnemonic, &options.passphrase)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn resolve_wallet_mnemonic(options: &WalletOptions) -> Result<String> {
    if options.entropy_hex.is_some() {
        anyhow::bail!("Cannot use mnemonic with --entropy-hex");
    }
    if options.stdin {
        return read_stdin_trimmed();
    }
    if let Some(mnemonic) = &options.mnemonic {
        return Ok(normalize_mnemonic(mnemonic));
    }
    if let Some(path) = &options.mnemonic_file {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read mnemonic file: {path:?}"))?;
        let normalized = normalize_mnemonic(&contents);
        if normalized.is_empty() {
            anyhow::bail!("Mnemonic file is empty");
        }
        return Ok(normalized);
    }
    if let Ok(mnemonic) = env::var("SPARK_MNEMONIC") {
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
        "Mnemonic required. Use --mnemonic, --mnemonic-file, --stdin, or set SPARK_MNEMONIC"
    )
}

fn resolve_api_key(cli_key: Option<String>) -> Option<String> {
    if let Some(key) = cli_key {
        if !key.trim().is_empty() {
            return Some(key);
        }
    }
    if let Ok(key) = env::var("SPARK_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key);
        }
    }
    if let Ok(key) = env::var("BREEZ_API_KEY") {
        if !key.trim().is_empty() {
            return Some(key);
        }
    }
    None
}

fn parse_private_key_hex(value: &str) -> Result<bitcoin::secp256k1::SecretKey> {
    let bytes =
        hex::decode(value).map_err(|e| anyhow::anyhow!(format!("Invalid private key hex: {e}")))?;
    if bytes.len() != 32 {
        anyhow::bail!("Private key must be 32 bytes (64 hex chars)");
    }
    bitcoin::secp256k1::SecretKey::from_slice(&bytes)
        .map_err(|e| anyhow::anyhow!(format!("Invalid private key: {e}")))
}

fn generate_mnemonic(words: u16) -> Result<String> {
    let mut entropy = match words {
        12 => [0u8; 16].to_vec(),
        24 => [0u8; 32].to_vec(),
        _ => return Err(anyhow::anyhow!("Invalid word count. Use 12 or 24.")),
    };

    let mut rng = rand::rng();
    rng.fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| anyhow::anyhow!(format!("Invalid entropy: {e}")))?;
    Ok(mnemonic.to_string())
}

fn read_mnemonic_input(
    mnemonic: Option<String>,
    mnemonic_file: Option<PathBuf>,
    stdin: bool,
) -> Result<String> {
    if stdin {
        return read_stdin_trimmed();
    }
    if let Some(mnemonic) = mnemonic {
        return Ok(normalize_mnemonic(&mnemonic));
    }
    if let Some(path) = mnemonic_file {
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read mnemonic file: {path:?}"))?;
        let normalized = normalize_mnemonic(&contents);
        if normalized.is_empty() {
            anyhow::bail!("Mnemonic file is empty");
        }
        return Ok(normalized);
    }
    anyhow::bail!("Mnemonic is required. Use --mnemonic, --mnemonic-file, or --stdin")
}

fn normalize_mnemonic(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn read_stdin_trimmed() -> Result<String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read from stdin")?;
    let normalized = normalize_mnemonic(&input);
    if normalized.is_empty() {
        anyhow::bail!("No input provided on stdin");
    }
    Ok(normalized)
}

fn amount_u128_to_u64(amount: u128) -> Result<u64> {
    u64::try_from(amount).map_err(|_| anyhow::anyhow!("Amount exceeds u64 range"))
}

fn expiry_time_from_now(seconds: u64) -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?;
    now.as_secs()
        .checked_add(seconds)
        .ok_or_else(|| anyhow::anyhow!("Invoice expiry time overflow"))
}

fn parse_asset_filter(value: &str) -> Result<AssetFilter> {
    value.parse::<AssetFilter>().map_err(|e| anyhow::anyhow!(e))
}

fn parse_max_fee(
    fee_sat: Option<u64>,
    sat_per_vbyte: Option<u64>,
    recommended_fee_leeway: Option<u64>,
) -> Result<Option<MaxFee>> {
    if let Some(leeway) = recommended_fee_leeway {
        if fee_sat.is_some() || sat_per_vbyte.is_some() {
            anyhow::bail!("Cannot specify fee_sat or sat_per_vbyte with recommended fee");
        }
        return Ok(Some(MaxFee::NetworkRecommended {
            leeway_sat_per_vbyte: leeway,
        }));
    }

    match (fee_sat, sat_per_vbyte) {
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "Cannot specify both fee_sat and sat_per_vbyte"
        )),
        (Some(fee_sat), None) => Ok(Some(MaxFee::Fixed { amount: fee_sat })),
        (None, Some(sat_per_vbyte)) => Ok(Some(MaxFee::Rate { sat_per_vbyte })),
        (None, None) => Ok(None),
    }
}

fn parse_fee(fee_sat: Option<u64>, sat_per_vbyte: Option<u64>) -> Result<Fee> {
    match (fee_sat, sat_per_vbyte) {
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "Cannot specify both fee_sat and sat_per_vbyte"
        )),
        (Some(fee_sat), None) => Ok(Fee::Fixed { amount: fee_sat }),
        (None, Some(sat_per_vbyte)) => Ok(Fee::Rate { sat_per_vbyte }),
        (None, None) => Err(anyhow::anyhow!(
            "Must specify either fee_sat or sat_per_vbyte"
        )),
    }
}

async fn resolve_lnurl_pay_request(
    wallet: &SparkWallet,
    input: &str,
) -> Result<spark::LnurlPayRequestDetails> {
    let parsed = wallet
        .parse_input(input)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    match parsed {
        InputType::LnurlPay(details) => Ok(details),
        InputType::LightningAddress(details) => Ok(details.pay_request),
        _ => Err(anyhow::anyhow!(
            "Input is not LNURL-pay or lightning address"
        )),
    }
}

async fn resolve_lnurl_withdraw_request(
    wallet: &SparkWallet,
    input: &str,
) -> Result<spark::LnurlWithdrawRequestDetails> {
    let parsed = wallet
        .parse_input(input)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    match parsed {
        InputType::LnurlWithdraw(details) => Ok(details),
        _ => Err(anyhow::anyhow!("Input is not LNURL-withdraw")),
    }
}

fn validate_lnurl_amount(amount_sats: u64, details: &spark::LnurlPayRequestDetails) -> Result<()> {
    let min_sats = details.min_sendable.div_ceil(1000);
    let max_sats = details.max_sendable / 1000;
    if amount_sats < min_sats || amount_sats > max_sats {
        anyhow::bail!("Amount out of bounds. min {min_sats} sat, max {max_sats} sat");
    }
    Ok(())
}

fn validate_lnurl_withdraw_amount(
    amount_sats: u64,
    details: &spark::LnurlWithdrawRequestDetails,
) -> Result<()> {
    let min_sats = details.min_withdrawable.div_ceil(1000);
    let max_sats = details.max_withdrawable / 1000;
    if amount_sats < min_sats || amount_sats > max_sats {
        anyhow::bail!("Amount out of bounds. min {min_sats} sat, max {max_sats} sat");
    }
    Ok(())
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
