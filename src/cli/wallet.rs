//! Wallet CLI subcommands
//!
//! Wraps wallet crate CLI functions for unified binary.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum WalletCommands {
    /// Initialize a new wallet with generated mnemonic
    Init {
        /// Display the mnemonic (WARNING: insecure)
        #[arg(long)]
        show_mnemonic: bool,
    },

    /// Import existing wallet from mnemonic
    Import {
        /// Mnemonic phrase (will prompt if not provided)
        #[arg(long)]
        mnemonic: Option<String>,
    },

    /// Export wallet mnemonic (requires confirmation)
    Export,

    /// Wallet password management
    #[command(subcommand)]
    Password(PasswordCommands),

    /// Display wallet information (npub, balances, profile)
    Whoami,

    /// Show wallet balance
    Balance,

    /// Generate receiving address
    Receive {
        /// Amount to request (optional)
        #[arg(short, long)]
        amount: Option<u64>,
    },

    /// Send payment
    Send {
        /// Destination address (Bitcoin, Lightning, or Spark)
        address: String,
        /// Amount in sats
        amount: u64,
    },

    /// Transaction history
    History {
        /// Number of transactions to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Nostr profile commands
    #[command(subcommand)]
    Profile(ProfileCommands),

    /// Contact management
    #[command(subcommand)]
    Contacts(ContactsCommands),

    /// Post a note to Nostr
    Post {
        /// Content to post
        content: String,
    },

    /// Direct message commands
    #[command(subcommand)]
    Dm(DmCommands),
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// Show profile information
    Show,

    /// Set profile fields
    Set {
        /// Display name
        #[arg(long)]
        name: Option<String>,

        /// About/bio
        #[arg(long)]
        about: Option<String>,

        /// Profile picture URL
        #[arg(long)]
        picture: Option<String>,

        /// NIP-05 identifier (name@domain.com)
        #[arg(long)]
        nip05: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ContactsCommands {
    /// List contacts
    List,

    /// Add a contact
    Add {
        /// Contact npub
        npub: String,

        /// Petname (optional)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Remove a contact
    Remove {
        /// Contact npub
        npub: String,
    },
}

#[derive(Subcommand)]
pub enum DmCommands {
    /// Send an encrypted DM
    Send {
        /// Recipient npub or nprofile
        recipient: String,

        /// Message content
        message: String,
    },

    /// List received DMs
    List {
        /// Number of messages to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Read a specific DM by event ID
    Read {
        /// Event ID of the message
        event_id: String,
    },
}

#[derive(Subcommand)]
pub enum PasswordCommands {
    /// Set or change the wallet password
    Set {
        /// New password (will prompt if not provided)
        #[arg(long)]
        password: Option<String>,

        /// Current password (required if already protected)
        #[arg(long)]
        current_password: Option<String>,
    },
}

pub fn run(cmd: WalletCommands) -> anyhow::Result<()> {
    match cmd {
        WalletCommands::Init { show_mnemonic } => wallet::cli::identity::init(show_mnemonic),
        WalletCommands::Import { mnemonic } => wallet::cli::identity::import(mnemonic),
        WalletCommands::Export => wallet::cli::identity::export(),
        WalletCommands::Password(cmd) => match cmd {
            PasswordCommands::Set { password, current_password } => {
                wallet::cli::password::set(password, current_password)
            }
        },
        WalletCommands::Whoami => wallet::cli::identity::whoami(),
        WalletCommands::Balance => wallet::cli::bitcoin::balance(),
        WalletCommands::Receive { amount } => wallet::cli::bitcoin::receive(amount),
        WalletCommands::Send { address, amount } => wallet::cli::bitcoin::send(address, amount),
        WalletCommands::History { limit } => wallet::cli::bitcoin::history(limit),
        WalletCommands::Profile(cmd) => match cmd {
            ProfileCommands::Show => wallet::cli::identity::profile_show(),
            ProfileCommands::Set {
                name,
                about,
                picture,
                nip05,
            } => wallet::cli::identity::profile_set(name, about, picture, nip05),
        },
        WalletCommands::Contacts(cmd) => match cmd {
            ContactsCommands::List => wallet::cli::identity::contacts_list(),
            ContactsCommands::Add { npub, name } => wallet::cli::identity::contacts_add(npub, name),
            ContactsCommands::Remove { npub } => wallet::cli::identity::contacts_remove(npub),
        },
        WalletCommands::Post { content } => wallet::cli::identity::post(content),
        WalletCommands::Dm(cmd) => match cmd {
            DmCommands::Send { recipient, message } => {
                wallet::cli::identity::dm_send(recipient, message)
            }
            DmCommands::List { limit } => wallet::cli::identity::dm_list(limit),
            DmCommands::Read { event_id } => wallet::cli::identity::dm_read(event_id),
        },
    }
}
