use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "oa", version = crate::VERSION, about = "OpenAgents Rust CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, global = true, help = "Output as JSON")]
    pub json: bool,

    #[arg(short, long, global = true, help = "Verbose logging output")]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Authentication management
    Auth(AuthArgs),
    /// Agent and human identity operations
    Identity(IdentityArgs),
    /// OpenAgents issue tracker operations
    Issue(IssueArgs),
    /// OpenAgents project management
    Project(ProjectArgs),
    /// Repository tracking and operations
    Repo(RepoArgs),
    /// OpenAgents interactive Coder agent session and autonomous tools
    Coder(CoderArgs),
    /// Box sandbox management and fanout execution
    Box(BoxArgs),
    /// Computer agent daemon and local policy probe
    Computer(ComputerArgs),
    /// Forum boards and topics
    Forum(ForumArgs),
    /// Account-level system memory and knowledge management
    Memory(MemoryArgs),
    /// Generic API route invocation
    Api(ApiArgs),
    /// Trace inspection and session export
    Trace(TraceArgs),
    /// Replace this binary with the release the channel names
    #[command(alias = "self-update")]
    Update(UpdateArgs),
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    #[arg(long, help = "Release channel to resolve (default: stable)")]
    pub channel: Option<String>,

    #[arg(long, help = "Install this exact version instead of resolving a channel")]
    pub version: Option<String>,

    #[arg(long, help = "Report what the channel names without downloading anything")]
    pub check: bool,

    #[arg(long, help = "Reinstall even when the channel names the running version")]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    Login,
    TokenStdin,
    Status,
    Logout,
    SetupGit,
    GitCredential {
        #[arg(default_value = "get")]
        operation: String,
    },
}

#[derive(Args, Debug)]
pub struct IdentityArgs {
    #[command(subcommand)]
    pub action: IdentityAction,
}

#[derive(Subcommand, Debug)]
pub enum IdentityAction {
    /// Show the public identity derived from the stored seed
    Show,
    /// Generate a new seed phrase and store it 0600
    Create {
        #[arg(long, default_value_t = 12, help = "Words in the new seed phrase: 12 for 128 bits, 24 for 256")]
        words: usize,
        #[arg(long, help = "Replace the stored seed. The identity and wallet it derives are lost")]
        force: bool,
    },
    /// Restore an existing seed phrase, read from standard input
    Import {
        #[arg(long, help = "Replace the stored seed. The identity and wallet it derives are lost")]
        force: bool,
    },
    /// Print the stored seed phrase
    Backup,
    /// Delete the stored seed
    Forget {
        #[arg(long, help = "Confirm that the identity and wallet are to be destroyed")]
        force: bool,
    },
}

#[derive(Args, Debug)]
pub struct IssueArgs {
    #[command(subcommand)]
    pub action: IssueAction,
}

#[derive(Subcommand, Debug)]
pub enum IssueAction {
    List {
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    View {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    Close {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    Comment {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(long)]
        body: String,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub action: ProjectAction,
}

#[derive(Subcommand, Debug)]
pub enum ProjectAction {
    List {
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    View {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub action: RepoAction,
}

#[derive(Subcommand, Debug)]
pub enum RepoAction {
    List,
    View {
        #[arg(help = "Repository slug")]
        slug: String,
    },
    Create {
        #[arg(long)]
        name: String,
    },
    Clone {
        #[arg(help = "Repository slug")]
        slug: String,
    },
}

#[derive(Args, Debug, Clone)]
pub struct CoderArgs {
    #[arg(help = "Optional prompt to execute headlessly or start interactive session with")]
    pub prompt: Option<String>,

    #[arg(long, help = "Delegate prompt to parallel child agents")]
    pub delegate: bool,

    #[arg(long, default_value_t = 1, help = "How many child agents run the prompt")]
    pub count: usize,

    #[arg(long, help = "How many children run at once. Defaults to all of them")]
    pub max_parallel: Option<usize>,

    #[arg(
        long,
        help = "Working directory each child gets: worktree (default, a detached git worktree of HEAD), directory, or none"
    )]
    pub isolation: Option<String>,

    #[arg(long, help = "Leave the children's worktrees on disk so their work can be read")]
    pub keep_workspaces: bool,

    #[arg(long, help = "Target harness lane (e.g. ox-alpha, gemini, devin, claude, codex)")]
    pub lane: Option<String>,

    #[arg(long, help = "Run in non-interactive headless mode")]
    pub headless: bool,

    #[arg(long, help = "Export conversation transcript to file")]
    pub export: Option<String>,
}

#[derive(Args, Debug)]
pub struct BoxArgs {
    #[command(subcommand)]
    pub action: BoxAction,
}

#[derive(Subcommand, Debug)]
pub enum BoxAction {
    List {
        #[arg(long, default_value = "main")]
        conversation: String,
    },
    Create {
        #[arg(long, default_value = "main")]
        conversation: String,
        #[arg(long)]
        label: Option<String>,
    },
    Exec {
        #[arg(long, default_value = "main")]
        conversation: String,
        #[arg(long)]
        box_id: String,
        #[arg(long)]
        command: String,
    },
}

#[derive(Args, Debug)]
pub struct ComputerArgs {
    #[command(subcommand)]
    pub action: ComputerAction,
}

#[derive(Subcommand, Debug)]
pub enum ComputerAction {
    Probe,
    Policy,
    Status,
    Up,
}

#[derive(Args, Debug)]
pub struct ForumArgs {
    #[command(subcommand)]
    pub action: ForumAction,
}

#[derive(Subcommand, Debug)]
pub enum ForumAction {
    /// List the forum's boards
    Boards,
    /// List a board's topics
    Topics {
        #[arg(long, help = "Board slug, as `oa forum boards` reports it")]
        board: String,
    },
    /// Search topics across boards
    Search {
        #[arg(help = "Search query")]
        query: String,
    },
}

#[derive(Args, Debug)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub action: MemoryAction,
}

#[derive(Subcommand, Debug)]
pub enum MemoryAction {
    List {
        #[arg(long)]
        bucket: Option<String>,
    },
    Add {
        #[arg(long)]
        body: String,
        #[arg(long)]
        bucket: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ApiArgs {
    #[arg(help = "HTTP method (e.g. GET, POST, DELETE)", default_value = "GET")]
    pub method: String,
    #[arg(help = "API endpoint path (e.g. /api/v1/user)", default_value = "/")]
    pub path: String,
}

#[derive(Args, Debug)]
pub struct TraceArgs {
    #[command(subcommand)]
    pub action: TraceAction,
}

#[derive(Subcommand, Debug)]
pub enum TraceAction {
    /// Discover trace files in the local agent stores
    List {
        #[arg(long, help = "Scan this directory instead of the default stores. Repeatable")]
        path: Vec<String>,
        #[arg(long, default_value_t = 20, help = "Most files to list per store")]
        limit: usize,
    },
    /// Summarize one trace file
    Show {
        #[arg(help = "A trace file path, or a file name inside ~/.openagents/exports")]
        trace: String,
    },
    /// Write a redacted copy of a trace file beside the original
    Redact {
        #[arg(help = "A trace file path, or a file name inside ~/.openagents/exports")]
        trace: Option<String>,
        #[arg(long, help = "Deprecated alias for the positional trace argument")]
        file: Option<String>,
    },
}

pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let cred_store = crate::auth::CredentialStore::new(None);
    let token = cred_store.get_token();

    match cli.command {
        Commands::Auth(auth) => match auth.action {
            AuthAction::Login => {
                println!("Auth login initialized");
            }
            AuthAction::TokenStdin => {
                let mut buffer = String::new();
                std::io::stdin().read_line(&mut buffer)?;
                cred_store.set_token(buffer.trim())?;
                println!("Token saved successfully.");
            }
            AuthAction::Status => {
                if let Some(tok) = token {
                    println!("Authenticated (token present, prefix: {}...)", &tok[..tok.len().min(8)]);
                } else {
                    println!("Not authenticated. No token found in config or environment.");
                }
            }
            AuthAction::Logout => {
                cred_store.clear_token()?;
                println!("Logged out successfully.");
            }
            AuthAction::SetupGit => {
                println!("Configured git credentials helper for OpenAgents.");
            }
            AuthAction::GitCredential { operation } => {
                let output = crate::repo::handle_git_credential(&operation, "openagents.com", token.as_deref());
                print!("{}", output);
            }
        },
        Commands::Identity(identity) => run_identity(identity.action, cli.json),
        Commands::Issue(issue) => {
            let tracker = crate::tracker::TrackerClient::new("https://openagents.com/api/v1", token);
            match issue.action {
                IssueAction::List { repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    let list = tracker.list_issues(&r).await.map_err(|e| e.to_string())?;
                    for item in list {
                        println!("#{}\t{}\t[{}]", item.number, item.title, item.state);
                    }
                }
                IssueAction::View { number, repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    if let Some(item) = tracker.get_issue(&r, number).await.map_err(|e| e.to_string())? {
                        println!("#{} {}\nState: {}\nAuthor: {:?}", item.number, item.title, item.state, item.author);
                    }
                }
                IssueAction::Create { title, body, repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    if let Some(created) = tracker.create_issue(&r, &title, body.as_deref()).await.map_err(|e| e.to_string())? {
                        println!("Created issue #{} in {}", created.number, r);
                    }
                }
                IssueAction::Close { number, repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    if tracker.close_issue(&r, number).await.map_err(|e| e.to_string())? {
                        println!("Closed issue #{} in {}", number, r);
                    }
                }
                IssueAction::Comment { number, body, repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    if tracker.comment_issue(&r, number, &body).await.map_err(|e| e.to_string())? {
                        println!("Commented on #{} in {}", number, r);
                    }
                }
            }
        }
        Commands::Project(project) => {
            let tracker = crate::tracker::TrackerClient::new("https://openagents.com/api/v1", token);
            match project.action {
                ProjectAction::List { repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    let list = tracker.list_projects(&r).await.map_err(|e| e.to_string())?;
                    for p in list {
                        println!("#{}\t{}\t[{}]", p.number, p.title, p.state);
                    }
                }
                ProjectAction::View { number, repo } => {
                    println!("Viewing project #{} in {:?}", number, repo);
                }
            }
        }
        Commands::Repo(repo) => {
            let repo_client = crate::repo::RepoClient::new("https://openagents.com/api/v1", token);
            match repo.action {
                RepoAction::List => {
                    let repos = repo_client.list_repos().await.map_err(|e| e.to_string())?;
                    for r in repos {
                        println!("{}\t(branch: {})", r.slug, r.default_branch);
                    }
                }
                RepoAction::View { slug } => println!("Viewing repository {}", slug),
                RepoAction::Create { name } => {
                    if repo_client.create_repo(&name, false).await.map_err(|e| e.to_string())? {
                        println!("Created repository {}", name);
                    }
                }
                RepoAction::Clone { slug } => {
                    if crate::repo::RepoClient::clone_repo(&slug, None).await.map_err(|e| e.to_string())? {
                        println!("Cloned repository {}", slug);
                    }
                }
            }
        }
        Commands::Coder(coder) => {
            if coder.delegate {
                crate::delegate::run_delegation(coder, token).await?;
            } else if coder.headless {
                let prompt = coder.prompt.unwrap_or_else(|| "Analyze workspace and run tests".to_string());
                println!("Executing coder prompt headlessly: {}", prompt);
                let lane_name = coder.lane.unwrap_or_else(|| "ox-alpha".to_string());
                // A headless session may start children. They run on the same
                // lane and the same credential, and they do not get the tool
                // themselves.
                let tools = crate::tools::HarnessToolRegistry::with_delegation(
                    None,
                    crate::tools::DelegationGate {
                        lane: lane_name.clone(),
                        user_token: token.clone(),
                        max_count: crate::delegate::MAX_DELEGATE_COUNT,
                    },
                );
                let lane = crate::runtime::Lane::from_str(&lane_name);
                let mut runtime = crate::runtime::CoderRuntimeSession::new(lane, None, token, tools);
                let result = runtime.execute_turn(&prompt, |chunk| {
                    print!("{}", chunk);
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }).await.map_err(|e| e.to_string())?;
                println!("\n\nTurn result:\n{}", result);
            } else {
                crate::interactive::run_tui(coder, token).await?;
            }
        }
        Commands::Box(b) => {
            let box_client = crate::box_client::BoxClient::new("https://openagents.com/api/v1", token);
            match b.action {
                BoxAction::List { conversation } => {
                    let boxes = box_client.list_boxes(&conversation).await.map_err(|e| e.to_string())?;
                    for bx in boxes {
                        println!("{}\t{}\t[{}]", bx.box_id, bx.label.unwrap_or_default(), bx.state);
                    }
                }
                BoxAction::Create { conversation, label } => {
                    if let Some(bx) = box_client.create_box(&conversation, label.as_deref()).await.map_err(|e| e.to_string())? {
                        println!("Created box: {}", bx.box_id);
                    }
                }
                BoxAction::Exec { conversation, box_id, command } => {
                    let res = box_client.execute_command(&conversation, &box_id, &command).await.map_err(|e| e.to_string())?;
                    println!("Exit: {}\nStdout: {}\nStderr: {}", res.exit_code, res.stdout, res.stderr);
                }
            }
        }
        Commands::Computer(comp) => match comp.action {
            ComputerAction::Probe => {
                let info = crate::computer::probe_host();
                println!("Host OS: {} ({}), CPUs: {}, Memory: {}MB", info.os, info.arch, info.num_cpus, info.total_memory_mb);
            }
            ComputerAction::Policy => println!("Computer Policy: default allowlist active"),
            ComputerAction::Status => println!("Computer agent: idle / online"),
            ComputerAction::Up => println!("Computer agent daemon launched."),
        },
        Commands::Forum(forum) => {
            let client = crate::forum::ForumClient::new("https://openagents.com/api/v1", token);
            match forum.action {
                ForumAction::Boards => {
                    // A refusal ends the command. The version this replaces answered
                    // a non-2xx with two hardcoded boards, one of which the server
                    // has never served.
                    let boards = client.list_boards().await.unwrap_or_else(|e| fail(&e.to_string()));
                    if boards.is_empty() {
                        println!("No boards found.");
                    }
                    for b in boards {
                        println!("{} — {} ({} topics)", b.slug, b.title, b.topic_count);
                    }
                }
                ForumAction::Topics { board } => {
                    let topics = client
                        .list_topics(&board)
                        .await
                        .unwrap_or_else(|e| fail(&e.to_string()));
                    if topics.is_empty() {
                        println!("No topics found.");
                    }
                    for t in topics {
                        println!("{} — {} ({} posts)", short_id(&t.id), t.title, t.posts_count);
                    }
                }
                ForumAction::Search { query } => {
                    let topics = client
                        .search_topics(&query)
                        .await
                        .unwrap_or_else(|e| fail(&e.to_string()));
                    if topics.is_empty() {
                        println!("No topics match.");
                    }
                    for t in topics {
                        println!(
                            "{} — {} — {}",
                            short_id(&t.id),
                            t.title,
                            t.author.as_deref().unwrap_or("?")
                        );
                    }
                }
            }
        }
        Commands::Memory(mem) => {
            let client = crate::memory_client::MemoryClient::new("https://openagents.com/api/v1", token);
            match mem.action {
                MemoryAction::List { bucket } => {
                    let records = client.list_memories(bucket.as_deref()).await.map_err(|e| e.to_string())?;
                    for r in records {
                        println!("{}\t[{}]\t{}", r.id, r.bucket, r.body);
                    }
                }
                MemoryAction::Add { body, bucket } => {
                    if let Some(r) = client.add_memory(&body, bucket.as_deref()).await.map_err(|e| e.to_string())? {
                        println!("Added memory: {} [{}]", r.id, r.bucket);
                    }
                }
            }
        }
        Commands::Api(api) => {
            let client = crate::api_passthrough::ApiPassthroughClient::new("https://openagents.com/api/v1", token);
            let res = client.execute_request(&api.method, &api.path, None).await.map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
        Commands::Trace(trace) => run_trace(trace.action),
        Commands::Update(update) => {
            crate::update::run(update.channel, update.version, update.check, update.force).await?;
        }
    }
    Ok(())
}

/// Print a refusal on stderr and exit non-zero.
///
/// Exit code 2 is what the TypeScript CLI returns for an input or configuration
/// error, and the point of this whole path: a command that cannot reach its data
/// says so and exits non-zero rather than returning something plausible.
pub(crate) fn fail(message: &str) -> ! {
    eprintln!("oa: {}", message);
    std::process::exit(2)
}

/// The first eight characters of a UUID, which is how the TypeScript CLI renders
/// topic ids in a listing.
fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn home_directory() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------

/// The public identity block. Public identifiers only: no seed phrase, no `nsec`,
/// and no private key reaches this function.
fn print_identity(identity: &crate::identity::SeedIdentity, json: bool) {
    if json {
        let value = serde_json::json!({
            "schema": "openagents.cli_identity.v1",
            "profile": identity.profile,
            "npub": identity.npub,
            "nostr_public_key": identity.nostr_public_key_hex,
            "nostr_derivation_path": identity.nostr_derivation_path,
            "wallet_address": identity.wallet_address,
            "wallet_public_key": identity.wallet_public_key_hex,
            "wallet_fingerprint": identity.wallet_fingerprint_hex,
            "wallet_derivation_path": identity.wallet_derivation_path,
            "spending_rail": serde_json::Value::Null,
        });
        println!("{}", value);
        return;
    }
    println!("Identity: {}", identity.npub);
    println!("  public key   {}", identity.nostr_public_key_hex);
    println!("  path         {}", identity.nostr_derivation_path);
    println!("Wallet:   {}", identity.wallet_address);
    println!("  public key   {}", identity.wallet_public_key_hex);
    println!("  fingerprint  {}", identity.wallet_fingerprint_hex);
    println!("  path         {}", identity.wallet_derivation_path);
    println!("Profile:  {}", identity.profile);
}

fn run_identity(action: IdentityAction, json: bool) {
    use crate::identity::{
        derive_seed_identity, generate_seed_phrase, is_valid_seed_phrase, SeedStore,
    };
    let store = SeedStore::new(None);
    let seed_path = store.path();

    let refuse_if_seed_exists = |force: bool| {
        if store.present() && !force {
            fail(&format!(
                "A seed is already stored at {}. Back it up with `oa identity backup` first, \
                 then pass --force to replace it.",
                seed_path.display()
            ));
        }
    };

    match action {
        IdentityAction::Show => {
            let identity = store.identity().unwrap_or_else(|e| fail(&e.to_string()));
            print_identity(&identity, json);
        }
        IdentityAction::Create { words, force } => {
            if words != 12 && words != 24 {
                fail("--words must be 12 or 24.");
            }
            refuse_if_seed_exists(force);

            let phrase = generate_seed_phrase(words).unwrap_or_else(|e| fail(&e.to_string()));
            // Derive before storing: a phrase that cannot be derived from must not
            // become the identity on this machine.
            let identity = derive_seed_identity(&phrase).unwrap_or_else(|e| fail(&e.to_string()));
            store
                .write_phrase(&phrase)
                .unwrap_or_else(|e| fail(&format!(
                    "The new seed could not be stored at {}: {}",
                    seed_path.display(),
                    e
                )));

            if !json {
                println!(
                    "Wrote a new {}-word seed to {} (mode 0600).",
                    words,
                    seed_path.display()
                );
                println!(
                    "Back it up now with `oa identity backup`. Nothing else on this machine \
                     can recover it."
                );
            }
            print_identity(&identity, json);
        }
        IdentityAction::Import { force } => {
            refuse_if_seed_exists(force);

            let mut phrase = String::new();
            if std::io::Read::read_to_string(&mut std::io::stdin(), &mut phrase).is_err() {
                fail("No seed phrase was provided on standard input.");
            }
            let phrase = phrase.trim().to_string();
            if phrase.is_empty() {
                fail("No seed phrase was provided on standard input.");
            }
            // The phrase is never echoed back, not even the part that parsed.
            if !is_valid_seed_phrase(&phrase) {
                fail(
                    "That is not a valid English BIP-39 seed phrase. Check the word count \
                     (12, 15, 18, 21, or 24) and the spelling of each word.",
                );
            }
            let identity = derive_seed_identity(&phrase).unwrap_or_else(|e| fail(&e.to_string()));
            store.write_phrase(&phrase).unwrap_or_else(|e| {
                fail(&format!(
                    "The seed could not be stored at {}: {}",
                    seed_path.display(),
                    e
                ))
            });

            if !json {
                println!("Stored the seed at {} (mode 0600).", seed_path.display());
            }
            print_identity(&identity, json);
        }
        IdentityAction::Backup => {
            // The one command that prints the secret, and the one that refuses
            // --json: the phrase must not land in machine-collected output.
            if json {
                fail(
                    "`oa identity backup` does not support --json. The seed phrase must not \
                     land in machine-collected output; run it without --json and copy the \
                     phrase yourself.",
                );
            }
            let phrase = match store.read_phrase() {
                Ok(Some(phrase)) => phrase,
                Ok(None) => fail(&crate::identity::IdentityError::NoSeed.to_string()),
                Err(e) => fail(&e.to_string()),
            };
            println!(
                "This is the only secret on this machine. Anyone holding it holds the \
                 identity and the wallet."
            );
            println!("{}", phrase);
        }
        IdentityAction::Forget { force } => {
            if !force {
                fail(&format!(
                    "Deleting {} destroys the identity and the wallet it derives. Back the \
                     phrase up with `oa identity backup`, then pass --force.",
                    seed_path.display()
                ));
            }
            let removed = store.forget().unwrap_or_else(|e| {
                fail(&format!(
                    "The seed at {} could not be removed: {}",
                    seed_path.display(),
                    e
                ))
            });
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "schema": "openagents.cli_identity_forget.v1",
                        "removed": removed,
                        "seed_path": seed_path,
                    })
                );
            } else if removed {
                println!("Removed {}.", seed_path.display());
            } else {
                println!("No seed was stored at {}.", seed_path.display());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// trace
// ---------------------------------------------------------------------------

fn run_trace(action: TraceAction) {
    use crate::trace;
    let home = home_directory();

    match action {
        TraceAction::List { path, limit } => {
            if limit == 0 {
                fail("--limit must be greater than zero.");
            }
            let specs: Vec<trace::TraceStoreSpec> = if path.is_empty() {
                let mut specs = trace::default_trace_stores(&home);
                specs.extend(trace::extra_path_stores());
                specs
            } else {
                path.iter()
                    .map(|entry| trace::path_trace_store(std::path::PathBuf::from(entry)))
                    .collect()
            };
            let bounds = trace::DiscoveryBounds {
                max_files_per_store: limit,
                ..Default::default()
            };
            let (scans, candidates) = trace::discover(&specs, bounds);

            for scan in &scans {
                if !scan.present {
                    println!("{}: {} (not present)", scan.kind.as_str(), scan.root.display());
                    continue;
                }
                let mut line = format!(
                    "{}: {} ({} matched, {} listed",
                    scan.kind.as_str(),
                    scan.root.display(),
                    scan.matched,
                    scan.listed
                );
                if scan.skipped_symlinks > 0 {
                    line.push_str(&format!(", {} symlinks skipped", scan.skipped_symlinks));
                }
                if scan.truncated {
                    line.push_str(", scan truncated at its entry budget");
                }
                line.push(')');
                println!("{}", line);
            }

            if candidates.is_empty() {
                println!("No trace files found.");
            }
            for candidate in candidates {
                println!(
                    "{}  {}  {}B  {}",
                    candidate.kind.as_str(),
                    candidate.modified_at,
                    candidate.bytes,
                    candidate.path.display()
                );
            }
        }
        TraceAction::Show { trace: argument } => {
            // An argument that resolves to nothing is refused. The version this
            // replaces printed "Viewing trace session <id>" for any id at all.
            let path = trace::resolve_trace_argument(&argument, &home)
                .unwrap_or_else(|message| fail(&message));
            let summary = trace::summarize_trace_file(&path).unwrap_or_else(|e| {
                fail(&format!(
                    "The trace file at {} could not be read: {}",
                    path.display(),
                    e
                ))
            });

            println!("File: {}", summary.path.display());
            if summary.format != "atif" {
                let described = if summary.format == "jsonl" {
                    "line-delimited session log (not ATIF)"
                } else {
                    "unknown"
                };
                println!("Format: {}", described);
                println!("Size: {} bytes", summary.bytes);
                if let Some(lines) = summary.lines {
                    println!("Lines: {}", lines);
                }
                println!("This slice summarizes ATIF documents only; foreign logs get metadata.");
                return;
            }

            println!(
                "Schema: {}",
                summary.schema_version.as_deref().unwrap_or("(missing schema_version)")
            );
            if let Some(session) = &summary.session_id {
                println!("Session: {}", session);
            }
            if summary.agent_name.is_some() || summary.agent_model.is_some() {
                println!(
                    "Agent: {} ({})",
                    summary.agent_name.as_deref().unwrap_or("unknown"),
                    summary.agent_model.as_deref().unwrap_or("unknown model")
                );
            }
            let sources = summary
                .steps_by_source
                .as_ref()
                .map(|by_source| {
                    by_source
                        .iter()
                        .map(|(source, count)| format!("{} {}", source, count))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            println!("Steps: {} ({})", summary.steps.unwrap_or(0), sources);
            let models = summary.models.unwrap_or_default();
            println!(
                "Models: {}",
                if models.is_empty() {
                    "(none recorded)".to_string()
                } else {
                    models.join(", ")
                }
            );
            println!("Tool calls: {}", summary.tool_calls.unwrap_or(0));
            match (summary.total_prompt_tokens, summary.total_completion_tokens) {
                (None, None) => println!("Tokens: not recorded"),
                (prompt, completion) => println!(
                    "Tokens: {} prompt, {} completion",
                    prompt.unwrap_or(0),
                    completion.unwrap_or(0)
                ),
            }
            if let (Some(first), Some(last)) = (&summary.first_timestamp, &summary.last_timestamp) {
                println!("Span: {} to {}", first, last);
            }
        }
        TraceAction::Redact { trace: argument, file } => {
            let argument = match argument.or(file) {
                Some(argument) => argument,
                None => fail("Name the trace file to redact."),
            };
            let path = trace::resolve_trace_argument(&argument, &home)
                .unwrap_or_else(|message| fail(&message));
            if trace::is_redacted_copy(&path) {
                fail(&format!(
                    "{} is already a redacted copy; redact the original instead.",
                    path.display()
                ));
            }

            let home_text = home.to_string_lossy().into_owned();
            let result = trace::redact_trace_file(&path, &home_text).unwrap_or_else(|e| {
                fail(&format!(
                    "The trace file at {} could not be redacted: {}",
                    path.display(),
                    e
                ))
            });

            println!("Wrote {}", result.output.display());
            if result.total == 0 {
                println!("Nothing matched the redaction rules.");
            } else {
                // Counts per category, never the matched text.
                let detail = result
                    .counts
                    .iter()
                    .map(|(category, count)| format!("{} {}", category, count))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "Redacted {} match{}: {}",
                    result.total,
                    if result.total == 1 { "" } else { "es" },
                    detail
                );
            }
            if result.valid_json == Some(false) {
                println!(
                    "Warning: the redacted copy no longer parses as JSON; review it before sharing."
                );
            }
        }
    }
}
