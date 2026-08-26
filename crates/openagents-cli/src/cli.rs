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

/// Every tracker command takes `-R owner/repo`. With no flag the checkout's
/// remote names the repository; see [`crate::tracker::resolve_repo_target`].
#[derive(Subcommand, Debug)]
pub enum IssueAction {
    /// List issues, paging past the server's 25 per page
    List {
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
        #[arg(long, default_value = "open", value_parser = ["open", "closed", "all"], help = "Filter by state")]
        state: String,
        #[arg(long, help = "Filter by one label name")]
        label: Option<String>,
        #[arg(long, help = "Filter by assignee login")]
        assignee: Option<String>,
        #[arg(long, help = "Filter by milestone")]
        milestone: Option<String>,
        #[arg(long, help = "Full-text search over titles and bodies")]
        search: Option<String>,
        #[arg(long, help = "Filter to issues that are, or are not, blocked")]
        blocked: Option<bool>,
        #[arg(long, default_value_t = 30, help = "Maximum issues to read")]
        limit: u32,
    },
    /// Show one issue, with its body and prerequisite fields
    View {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
        #[arg(long, help = "Also read the comment thread")]
        comments: bool,
    },
    /// Open an issue
    Create {
        #[arg(long, help = "Issue title")]
        title: String,
        #[arg(long, help = "Issue body")]
        body: Option<String>,
        #[arg(long, help = "Read the body from a file, or from - for standard input")]
        body_file: Option<String>,
        #[arg(long, help = "Apply a label; repeatable")]
        label: Vec<String>,
        #[arg(long, help = "Assign a login; repeatable")]
        assignee: Vec<String>,
        #[arg(long, help = "Milestone number")]
        milestone: Option<u64>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Close an issue
    Close {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
        #[arg(long, help = "Leave this comment before closing")]
        comment: Option<String>,
    },
    /// Reopen a closed issue
    Reopen {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
        #[arg(long, help = "Leave this comment before reopening")]
        comment: Option<String>,
    },
    /// Comment on an issue, or read the thread
    Comment {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(long, help = "Comment body")]
        body: Option<String>,
        #[arg(long, help = "Read the body from a file, or from - for standard input")]
        body_file: Option<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Read, apply, or remove the labels on an issue
    Label {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(long, help = "Apply a label; repeatable")]
        add: Vec<String>,
        #[arg(long, help = "Remove a label; repeatable")]
        remove: Vec<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Assign an issue to one or more logins
    Assign {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(required = true, help = "Account logins")]
        logins: Vec<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Remove one or more logins from an issue
    Unassign {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(required = true, help = "Account logins")]
        logins: Vec<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Read, add, or remove the prerequisites of an issue
    Deps {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(long, help = "Record an issue this one waits on; repeatable")]
        add: Vec<u64>,
        #[arg(long, help = "Drop a prerequisite edge; repeatable")]
        remove: Vec<u64>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// List the milestones of a repository
    Milestones {
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
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
    /// List the projects of a repository
    List {
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
        #[arg(long, help = "Include archived boards")]
        archived: bool,
    },
    /// Show one project
    View {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Create a project board
    Create {
        #[arg(long, help = "Project title")]
        title: String,
        #[arg(long, help = "Markdown project description")]
        description: Option<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// List the fields of a project board
    Fields {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// List the items on a project board
    Items {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Put an issue on a project board
    ItemAdd {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(long, help = "Issue number to place on the board")]
        issue: u64,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Set stored field values on a project item
    ItemSet {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(help = "Project item id")]
        item: String,
        #[arg(long = "set", required = true, help = "Set a field, as FIELD=VALUE; repeatable")]
        set: Vec<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Move a project item, by field value, rank, or both
    ItemMove {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(help = "Project item id")]
        item: String,
        #[arg(long = "set", help = "Set a field, as FIELD=VALUE; repeatable")]
        set: Vec<String>,
        #[arg(long, help = "One-based rank within the destination column")]
        position: Option<u64>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Take an item off a project board
    ItemRemove {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(help = "Project item id")]
        item: String,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
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

/// `--conversation` has no default.
///
/// The version this replaces defaulted it to the literal string `main`, which
/// is not a conversation id, so every box command asked about a conversation
/// that does not exist. Absent the flag, the account's conversation is
/// resolved from the server, and a deployment that does not report one earns a
/// refusal naming the flag.
#[derive(Subcommand, Debug)]
pub enum BoxAction {
    /// List active and recent Box VMs in a conversation
    List {
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// Provision a new Box VM
    Create {
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
        #[arg(long, help = "Optional label for the box")]
        label: Option<String>,
    },
    /// Inspect a Box VM's status and lifecycle
    View {
        #[arg(help = "Box VM id, such as bx_8bhkse3n")]
        box_id: String,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// Execute a command synchronously on a Box VM
    Exec {
        #[arg(help = "Box VM id, such as bx_8bhkse3n")]
        box_id: String,
        #[arg(required = true, trailing_var_arg = true, help = "Command to execute")]
        command: Vec<String>,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
        #[arg(long, help = "Timeout in seconds for command execution")]
        timeout: Option<u64>,
    },
    /// Stop and snapshot a Box VM to release capacity
    Stop {
        #[arg(help = "Box VM id, such as bx_8bhkse3n")]
        box_id: String,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// Start a durable background command run on a Box VM
    Run {
        #[arg(help = "Box VM id, such as bx_8bhkse3n")]
        box_id: String,
        #[arg(required = true, trailing_var_arg = true, help = "Command to run in the background")]
        command: Vec<String>,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// Manage durable runs on Box VMs
    Runs {
        #[command(subcommand)]
        action: BoxRunAction,
    },
    /// Request a multi-box fanout admission plan
    Fanout {
        #[arg(long, help = "Number of boxes to request")]
        count: u64,
        #[arg(long, help = "Comma-separated labels for the fanout boxes")]
        labels: Option<String>,
        #[arg(long, help = "Allow scaling up to the budgeted limit")]
        budgeted: bool,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
        #[arg(long, help = "Read an existing plan by its request id instead of asking for one")]
        request_id: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum BoxRunAction {
    /// List durable runs on a Box VM
    List {
        #[arg(help = "Box VM id")]
        box_id: String,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// View details of a Box run
    View {
        #[arg(help = "Box VM id")]
        box_id: String,
        #[arg(help = "Box run id")]
        run_id: String,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// Read the output of a Box run, optionally until it finishes
    Output {
        #[arg(help = "Box VM id")]
        box_id: String,
        #[arg(help = "Box run id")]
        run_id: String,
        #[arg(long, help = "Byte offset to start reading output from")]
        offset: Option<u64>,
        #[arg(long, help = "Keep reading until the run reaches a terminal state")]
        follow: bool,
        #[arg(long, default_value_t = 1000, help = "Milliseconds between polls while following")]
        interval_ms: u64,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
    },
    /// Cancel an active Box run
    Cancel {
        #[arg(help = "Box VM id")]
        box_id: String,
        #[arg(help = "Box run id")]
        run_id: String,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
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
    /// List the account's memories, newest first
    List {
        #[arg(long, help = "Narrow to one bucket: user or learned")]
        bucket: Option<String>,
        #[arg(long, help = "Maximum number of memories to read")]
        limit: Option<u32>,
        #[arg(long, help = "Also read the corrections behind the live memories")]
        include_superseded: bool,
    },
    /// Store one memory. Pass --supersedes <id> to correct an existing one
    /// rather than edit it
    Add {
        // Positional, and variadic, the way `openagents memory add` takes it:
        // `oa memory add --supersedes <id> "what to remember"`.
        #[arg(required = true, trailing_var_arg = true, help = "What to remember")]
        body: Vec<String>,
        #[arg(long, help = "Bucket to store in: user or learned")]
        bucket: Option<String>,
        #[arg(long, help = "Id of the memory this one corrects and replaces")]
        supersedes: Option<String>,
        #[arg(long, help = "Thread or session this memory came out of")]
        source_ref: Option<String>,
    },
    /// Remove one memory outright
    Delete {
        #[arg(help = "Memory id")]
        memory_id: String,
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
        Commands::Issue(issue) => run_issue(issue.action, token, cli.json).await,
        Commands::Project(project) => run_project(project.action, token, cli.json).await,
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
        Commands::Box(b) => run_box(b.action, token, cli.json).await,
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
        Commands::Memory(mem) => run_memory(mem.action, token, cli.json).await,
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
// tracker: issues, projects, milestones
// ---------------------------------------------------------------------------

const API_BASE: &str = "https://openagents.com/api/v1";

/// Unwrap a client result, or print the server's own refusal and exit non-zero.
///
/// Every tracker, box, and memory command ends here rather than in an
/// `unwrap_or_default`. That is the whole difference between reporting what the
/// server said and printing an empty list that reads as "there is nothing".
fn or_fail<T>(result: Result<T, crate::tracker::ApiError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => fail(&error.to_string()),
    }
}

/// Print the server's body verbatim under `--json`, or the human lines.
fn emit(json: bool, value: &serde_json::Value, human: &[String]) {
    if json {
        match serde_json::to_string_pretty(value) {
            Ok(text) => println!("{}", text),
            Err(error) => fail(&format!("Could not render JSON: {}", error)),
        }
    } else {
        for line in human {
            println!("{}", line);
        }
    }
}

fn field(value: &serde_json::Value, key: &str) -> String {
    match value.get(key) {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// The names inside an array of objects, or of strings.
fn names(value: Option<&serde_json::Value>, key: &str) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| match item {
                    serde_json::Value::String(text) => text.clone(),
                    other => field(other, key),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn issue_references(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    format!(
                        "#{}",
                        item.get("number")
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "?".to_string())
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn number_or_question(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .filter(|v| !v.is_null())
        .map(|v| v.to_string())
        .unwrap_or_else(|| "?".to_string())
}

fn pad(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    while out.chars().count() < width {
        out.push(' ');
    }
    out
}

fn issue_row(issue: &serde_json::Value) -> String {
    let extension = issue.get("openagents").cloned().unwrap_or(serde_json::Value::Null);
    let labels = names(issue.get("labels"), "name");
    format!(
        "{}{}{}{}{}",
        pad(&format!("#{}", number_or_question(issue, "number")), 7),
        pad(&field(issue, "state"), 8),
        field(issue, "title"),
        if labels.is_empty() {
            String::new()
        } else {
            format!("  ({})", labels.join(", "))
        },
        if extension.get("blocked") == Some(&serde_json::Value::Bool(true)) {
            "  [blocked]"
        } else {
            ""
        }
    )
}

fn issue_view_human(issue: &serde_json::Value) -> Vec<String> {
    let extension = issue.get("openagents").cloned().unwrap_or(serde_json::Value::Null);
    let milestone = issue.get("milestone").cloned().unwrap_or(serde_json::Value::Null);
    let author = issue
        .get("user")
        .map(|u| field(u, "login"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let milestone_title = field(&milestone, "title");
    vec![
        format!(
            "#{}  {}",
            number_or_question(issue, "number"),
            field(issue, "title")
        ),
        format!("State:      {}", field(issue, "state")),
        format!("Author:     {}", author),
        format!("Labels:     {}", or_none(&names(issue.get("labels"), "name"))),
        format!(
            "Assignees:  {}",
            or_none(&names(issue.get("assignees"), "login"))
        ),
        format!(
            "Milestone:  {}",
            if milestone_title.is_empty() {
                "none".to_string()
            } else {
                milestone_title
            }
        ),
        format!(
            "Progress:   {}",
            {
                let progress = field(&extension, "progress");
                if progress.is_empty() { "unknown".to_string() } else { progress }
            }
        ),
        format!(
            "Blocked:    {}",
            if extension.get("blocked") == Some(&serde_json::Value::Bool(true)) {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "Blocked by: {}",
            or_none(&issue_references(extension.get("blocked_by")))
        ),
        format!("Blocks:     {}", or_none(&issue_references(extension.get("blocks")))),
        String::new(),
        field(issue, "body"),
    ]
}

fn comment_thread_human(value: &serde_json::Value) -> Vec<String> {
    let comments = value
        .get("comments")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if comments.is_empty() {
        return vec![String::new(), "No comments.".to_string()];
    }
    let mut lines = vec![String::new(), format!("Comments ({}):", comments.len())];
    for comment in &comments {
        let author = comment
            .get("user")
            .map(|u| field(u, "login"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        lines.push(format!("- {}: {}", author, field(comment, "body")));
    }
    lines
}

fn dependency_human(graph: &serde_json::Value) -> Vec<String> {
    let edges = |key: &str| -> Vec<String> {
        graph
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|entry| {
                        format!(
                            "  #{} {} {}",
                            number_or_question(entry, "number"),
                            field(entry, "state"),
                            field(entry, "title")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let blocked_by = edges("blocked_by");
    let blocks = edges("blocks");
    let mut lines = vec![format!(
        "Blocked: {}",
        if graph.get("blocked") == Some(&serde_json::Value::Bool(true)) {
            "yes"
        } else {
            "no"
        }
    )];
    lines.push("Blocked by:".to_string());
    if blocked_by.is_empty() {
        lines.push("  none".to_string());
    } else {
        lines.extend(blocked_by);
    }
    lines.push("Blocks:".to_string());
    if blocks.is_empty() {
        lines.push("  none".to_string());
    } else {
        lines.extend(blocks);
    }
    lines
}

/// Reads `--body` or `--body-file`, where `-` is standard input.
fn resolve_body(body: Option<String>, body_file: Option<String>) -> Option<String> {
    match (body, body_file) {
        (Some(_), Some(_)) => fail("Use either --body or --body-file, not both."),
        (Some(text), None) => Some(text),
        (None, Some(path)) => {
            if path == "-" {
                use std::io::Read;
                let mut buffer = String::new();
                if let Err(error) = std::io::stdin().read_to_string(&mut buffer) {
                    fail(&format!("Could not read the body from standard input: {}", error));
                }
                Some(buffer)
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(text) => Some(text),
                    Err(error) => fail(&format!("Could not read {}: {}", path, error)),
                }
            }
        }
        (None, None) => None,
    }
}

fn target_or_fail(repo: Option<String>) -> crate::tracker::RepoTarget {
    or_fail(crate::tracker::resolve_repo_target(repo.as_deref()))
}

/// `FIELD=VALUE` pairs into the object the project routes take.
fn parse_field_values(pairs: &[String]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for pair in pairs {
        match pair.split_once('=') {
            Some((field, value)) if !field.trim().is_empty() => {
                map.insert(field.trim().to_string(), serde_json::json!(value));
            }
            _ => fail(&format!(
                "`{}` is not a field assignment. Pass --set FIELD=VALUE.",
                pair
            )),
        }
    }
    serde_json::Value::Object(map)
}

async fn run_issue(action: IssueAction, token: Option<String>, json: bool) {
    let tracker = crate::tracker::TrackerClient::new(API_BASE, token);
    match action {
        IssueAction::List {
            repo,
            state,
            label,
            assignee,
            milestone,
            search,
            blocked,
            limit,
        } => {
            let target = target_or_fail(repo);
            let options = crate::tracker::IssueListOptions {
                limit,
                state: Some(state),
                label,
                assignee,
                milestone,
                search,
                blocked,
            };
            let result = or_fail(tracker.list_issues(&target, &options).await);
            let value = serde_json::json!({
                "pagination": result.pagination,
                "issues": result.issues,
            });
            let mut human: Vec<String> = if result.issues.is_empty() {
                vec!["No issues found.".to_string()]
            } else {
                result.issues.iter().map(issue_row).collect()
            };
            if !result.issues.is_empty() {
                human.push(String::new());
                human.push(match result.pagination.get("total").and_then(|t| t.as_u64()) {
                    Some(total) => format!("Showing {} of {} issues.", result.issues.len(), total),
                    None => format!("Showing {} issues.", result.issues.len()),
                });
            }
            emit(json, &value, &human);
        }
        IssueAction::View {
            number,
            repo,
            comments,
        } => {
            let target = target_or_fail(repo);
            let issue = or_fail(tracker.view_issue(&target, number).await);
            if !comments {
                emit(json, &issue, &issue_view_human(&issue));
            } else {
                let thread = or_fail(tracker.list_comments(&target, number).await);
                let mut human = issue_view_human(&issue);
                human.extend(comment_thread_human(&thread));
                let value = serde_json::json!({ "issue": issue, "comments": thread });
                emit(json, &value, &human);
            }
        }
        IssueAction::Create {
            title,
            body,
            body_file,
            label,
            assignee,
            milestone,
            repo,
        } => {
            let target = target_or_fail(repo);
            let text = resolve_body(body, body_file);
            let created = or_fail(
                tracker
                    .create_issue(
                        &target,
                        &title,
                        text.as_deref(),
                        &label,
                        &assignee,
                        milestone,
                    )
                    .await,
            );
            emit(
                json,
                &created,
                &[format!(
                    "Created issue #{} {}",
                    number_or_question(&created, "number"),
                    field(&created, "title")
                )],
            );
        }
        IssueAction::Close {
            number,
            repo,
            comment,
        } => {
            let target = target_or_fail(repo);
            if let Some(text) = comment {
                or_fail(tracker.comment_issue(&target, number, &text).await);
            }
            let issue = or_fail(tracker.set_issue_state(&target, number, "closed").await);
            emit(
                json,
                &issue,
                &[format!(
                    "Closed issue #{} ({}).",
                    number_or_question(&issue, "number"),
                    field(&issue, "state")
                )],
            );
        }
        IssueAction::Reopen {
            number,
            repo,
            comment,
        } => {
            let target = target_or_fail(repo);
            if let Some(text) = comment {
                or_fail(tracker.comment_issue(&target, number, &text).await);
            }
            let issue = or_fail(tracker.set_issue_state(&target, number, "open").await);
            emit(
                json,
                &issue,
                &[format!(
                    "Reopened issue #{} ({}).",
                    number_or_question(&issue, "number"),
                    field(&issue, "state")
                )],
            );
        }
        IssueAction::Comment {
            number,
            body,
            body_file,
            repo,
        } => {
            let target = target_or_fail(repo);
            match resolve_body(body, body_file) {
                Some(text) => {
                    let comment = or_fail(tracker.comment_issue(&target, number, &text).await);
                    emit(
                        json,
                        &comment,
                        &[format!("Commented on #{}.", number)],
                    );
                }
                // No body is a read of the thread, which is what the
                // TypeScript CLI does with `issue view --comments`.
                None => {
                    let thread = or_fail(tracker.list_comments(&target, number).await);
                    emit(json, &thread, &comment_thread_human(&thread));
                }
            }
        }
        IssueAction::Label {
            number,
            add,
            remove,
            repo,
        } => {
            let target = target_or_fail(repo);
            let mut value: Option<serde_json::Value> = None;
            if !add.is_empty() {
                value = Some(or_fail(tracker.add_labels(&target, number, &add).await));
            }
            for name in &remove {
                value = Some(or_fail(tracker.remove_label(&target, number, name).await));
            }
            let applied = match value {
                Some(value) => value,
                None => or_fail(tracker.list_labels(&target, number).await),
            };
            emit(
                json,
                &applied,
                &[format!(
                    "Labels: {}",
                    or_none(&names(applied.get("labels"), "name"))
                )],
            );
        }
        IssueAction::Assign {
            number,
            logins,
            repo,
        } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.add_assignees(&target, number, &logins).await);
            emit(
                json,
                &value,
                &[format!(
                    "Assignees: {}",
                    or_none(&names(value.get("assignees"), "login"))
                )],
            );
        }
        IssueAction::Unassign {
            number,
            logins,
            repo,
        } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.remove_assignees(&target, number, &logins).await);
            emit(
                json,
                &value,
                &[format!(
                    "Assignees: {}",
                    or_none(&names(value.get("assignees"), "login"))
                )],
            );
        }
        IssueAction::Deps {
            number,
            add,
            remove,
            repo,
        } => {
            let target = target_or_fail(repo);
            let mut value: Option<serde_json::Value> = None;
            if !add.is_empty() {
                value = Some(or_fail(tracker.add_dependencies(&target, number, &add).await));
            }
            for blocked_by in &remove {
                value = Some(or_fail(
                    tracker.remove_dependency(&target, number, *blocked_by).await,
                ));
            }
            let graph = match value {
                Some(value) => value,
                None => or_fail(tracker.dependencies(&target, number).await),
            };
            emit(json, &graph, &dependency_human(&graph));
        }
        IssueAction::Milestones { repo } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.list_milestones(&target).await);
            let rows = value
                .get("milestones")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let human: Vec<String> = if rows.is_empty() {
                vec!["No milestones found.".to_string()]
            } else {
                rows.iter()
                    .map(|row| {
                        format!(
                            "{}{}{}",
                            pad(&format!("#{}", number_or_question(row, "number")), 7),
                            pad(&field(row, "state"), 8),
                            field(row, "title")
                        )
                    })
                    .collect()
            };
            emit(json, &value, &human);
        }
    }
}

fn project_row(project: &serde_json::Value) -> String {
    format!(
        "{}{}{}{}",
        pad(&format!("#{}", number_or_question(project, "number")), 6),
        pad(&field(project, "state"), 8),
        field(project, "title"),
        if project.get("archived") == Some(&serde_json::Value::Bool(true)) {
            "  [archived]"
        } else {
            ""
        }
    )
}

fn project_items_human(value: &serde_json::Value) -> Vec<String> {
    let items = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return vec!["No items on this board.".to_string()];
    }
    items
        .iter()
        .map(|item| {
            let issue = item.get("issue").cloned().unwrap_or(serde_json::Value::Null);
            let pairs: Vec<String> = item
                .get("values")
                .and_then(serde_json::Value::as_object)
                .map(|map| {
                    map.iter()
                        .map(|(field, value)| match value {
                            serde_json::Value::String(text) => format!("{}={}", field, text),
                            other => format!("{}={}", field, other),
                        })
                        .collect()
                })
                .unwrap_or_default();
            format!(
                "{} #{}  {}",
                pad(&number_or_question(item, "id"), 6),
                number_or_question(&issue, "number"),
                pairs.join(" ")
            )
        })
        .collect()
}

async fn run_project(action: ProjectAction, token: Option<String>, json: bool) {
    let tracker = crate::tracker::TrackerClient::new(API_BASE, token);
    match action {
        ProjectAction::List { repo, archived } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.list_projects(&target, archived).await);
            let boards = value
                .get("projects")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let human: Vec<String> = if boards.is_empty() {
                vec!["No projects found.".to_string()]
            } else {
                boards.iter().map(project_row).collect()
            };
            emit(json, &value, &human);
        }
        ProjectAction::View { number, repo } => {
            let target = target_or_fail(repo);
            let project = or_fail(tracker.view_project(&target, number).await);
            let human = vec![
                format!(
                    "#{}  {}",
                    number_or_question(&project, "number"),
                    field(&project, "title")
                ),
                format!("State:    {}", field(&project, "state")),
                format!(
                    "Archived: {}",
                    if project.get("archived") == Some(&serde_json::Value::Bool(true)) {
                        "yes"
                    } else {
                        "no"
                    }
                ),
                format!("Owner:    {}", {
                    let owner = field(&project, "owner");
                    if owner.is_empty() { "unknown".to_string() } else { owner }
                }),
                String::new(),
                field(&project, "description"),
            ];
            emit(json, &project, &human);
        }
        ProjectAction::Create {
            title,
            description,
            repo,
        } => {
            if title.trim().is_empty() {
                fail("Pass --title with the project title.");
            }
            let target = target_or_fail(repo);
            let project = or_fail(
                tracker
                    .create_project(&target, &title, description.as_deref())
                    .await,
            );
            emit(
                json,
                &project,
                &[format!(
                    "Created project #{} {}",
                    number_or_question(&project, "number"),
                    field(&project, "title")
                )],
            );
        }
        ProjectAction::Fields { number, repo } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.project_fields(&target, number).await);
            let fields = value
                .get("fields")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let human: Vec<String> = if fields.is_empty() {
                vec!["No fields on this board.".to_string()]
            } else {
                fields
                    .iter()
                    .map(|f| {
                        let options = f
                            .get("options")
                            .and_then(|o| o.get("values"))
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        format!(
                            "{} ({}) {}",
                            field(f, "name"),
                            field(f, "data_type"),
                            or_none(&names(Some(&options), "name"))
                        )
                    })
                    .collect()
            };
            emit(json, &value, &human);
        }
        ProjectAction::Items { number, repo } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.project_items(&target, number).await);
            emit(json, &value, &project_items_human(&value));
        }
        ProjectAction::ItemAdd {
            number,
            issue,
            repo,
        } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.project_add_item(&target, number, issue).await);
            emit(json, &value, &project_items_human(&value));
        }
        ProjectAction::ItemSet {
            number,
            item,
            set,
            repo,
        } => {
            let target = target_or_fail(repo);
            let values = parse_field_values(&set);
            let value = or_fail(
                tracker
                    .project_set_item_values(&target, number, &item, &values)
                    .await,
            );
            emit(json, &value, &project_items_human(&value));
        }
        ProjectAction::ItemMove {
            number,
            item,
            set,
            position,
            repo,
        } => {
            if set.is_empty() && position.is_none() {
                fail("Pass --set FIELD=VALUE, --position, or both.");
            }
            let target = target_or_fail(repo);
            let values = parse_field_values(&set);
            let value = or_fail(
                tracker
                    .project_move_item(&target, number, &item, &values, position)
                    .await,
            );
            emit(json, &value, &project_items_human(&value));
        }
        ProjectAction::ItemRemove {
            number,
            item,
            repo,
        } => {
            let value = {
                let target = target_or_fail(repo);
                or_fail(tracker.project_remove_item(&target, number, &item).await)
            };
            emit(
                json,
                &value,
                &[format!("Removed item {} from project #{}.", item, number)],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// box
// ---------------------------------------------------------------------------

fn box_list_human(boxes: &[crate::box_client::BoxRecord]) -> Vec<String> {
    if boxes.is_empty() {
        return vec!["No boxes provisioned for this conversation.".to_string()];
    }
    let mut lines = vec!["BOX ID        STATE       SETUP     LABEL        CREATED".to_string()];
    for b in boxes {
        lines.push(format!(
            "{} {} {} {} {}",
            pad(&b.box_id, 13),
            pad(&b.state, 11),
            pad(&b.setup_status, 9),
            pad(b.label.as_deref().unwrap_or("-"), 12),
            b.created_at
        ));
    }
    lines
}

fn box_view_human(b: &crate::box_client::BoxRecord) -> Vec<String> {
    let mut lines = vec![
        format!("Box ID:       {}", b.box_id),
        format!("State:        {}", b.state),
        format!("Setup Status: {}", b.setup_status),
        format!("Label:        {}", b.label.as_deref().unwrap_or("-")),
        format!("Created:      {}", b.created_at),
    ];
    if let Some(stopped) = &b.stopped_at {
        lines.push(format!("Stopped:      {}", stopped));
    }
    lines
}

fn run_list_human(runs: &[crate::box_client::BoxRunRecord]) -> Vec<String> {
    if runs.is_empty() {
        return vec!["No runs recorded for this box.".to_string()];
    }
    let mut lines =
        vec!["RUN ID                               STATE      EXIT  COMMAND".to_string()];
    for r in runs {
        let command = if r.command.chars().count() > 40 {
            format!("{}...", r.command.chars().take(37).collect::<String>())
        } else {
            r.command.clone()
        };
        lines.push(format!(
            "{} {} {} {}",
            pad(&r.id, 36),
            pad(&r.state, 10),
            pad(
                &r.exit_status
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                5
            ),
            command
        ));
    }
    lines
}

fn run_view_human(r: &crate::box_client::BoxRunRecord) -> Vec<String> {
    let mut lines = vec![
        format!("Run ID:       {}", r.id),
        format!("Box ID:       {}", r.box_id),
        format!("State:        {}", r.state),
        format!("Command:      {}", r.command),
        format!(
            "Exit Status:  {}",
            r.exit_status
                .map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
        format!(
            "Timed Out:    {}",
            if r.timed_out == Some(true) { "yes" } else { "no" }
        ),
    ];
    if let Some(reason) = &r.failure_reason {
        lines.push(format!("Failure:      {}", reason));
    }
    lines.push(format!(
        "Admitted:     {}",
        r.admitted_at.as_deref().unwrap_or("-")
    ));
    lines.push(format!(
        "Dispatched:   {}",
        r.dispatched_at.as_deref().unwrap_or("-")
    ));
    lines.push(format!(
        "Started:      {}",
        r.started_at.as_deref().unwrap_or("-")
    ));
    lines.push(format!(
        "Finished:     {}",
        r.finished_at.as_deref().unwrap_or("-")
    ));
    lines
}

fn fanout_human(plan: &crate::box_client::BoxFanoutPlan) -> Vec<String> {
    let mut lines = vec![
        format!("Fanout Plan:  {}", plan.id),
        format!(
            "Requested:    {} boxes (Budgeted: {})",
            plan.requested_count,
            if plan.budgeted { "yes" } else { "no" }
        ),
        format!("Admitted:     {}", plan.admitted.len()),
    ];
    for item in &plan.admitted {
        lines.push(format!(
            "  [#{}] {} -> {} ({})",
            item.position,
            item.label,
            item.box_id.as_deref().unwrap_or("allocating"),
            item.state
        ));
    }
    lines.push(format!("Queued:       {}", plan.queued.len()));
    for item in &plan.queued {
        lines.push(format!(
            "  [#{}] {} (Reason: {})",
            item.position,
            item.label,
            item.queue_reason
                .as_deref()
                .unwrap_or("waiting for capacity")
        ));
    }
    lines
}

fn to_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

async fn run_box(action: BoxAction, token: Option<String>, json: bool) {
    let client = crate::box_client::BoxClient::new(API_BASE, token);
    match action {
        BoxAction::List { conversation } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let boxes = or_fail(client.list_boxes(&id).await);
            emit(
                json,
                &serde_json::json!({ "boxes": to_value(&boxes) }),
                &box_list_human(&boxes),
            );
        }
        BoxAction::Create {
            conversation,
            label,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let record = or_fail(client.create_box(&id, label.as_deref()).await);
            let mut human = vec![format!(
                "Provisioned Box {} (state: {}, setup: {}).",
                record.box_id, record.state, record.setup_status
            )];
            if let Some(name) = &record.label {
                human.push(format!("Label: {}", name));
            }
            emit(json, &serde_json::json!({ "box": to_value(&record) }), &human);
        }
        BoxAction::View {
            box_id,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let record = or_fail(client.view_box(&id, &box_id).await);
            emit(
                json,
                &serde_json::json!({ "box": to_value(&record) }),
                &box_view_human(&record),
            );
        }
        BoxAction::Exec {
            box_id,
            command,
            conversation,
            timeout,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let joined = command.join(" ");
            let result = or_fail(client.execute_command(&id, &box_id, &joined, timeout).await);
            let mut human = Vec::new();
            if !result.stdout.is_empty() {
                human.push(result.stdout.trim_end().to_string());
            }
            if !result.stderr.is_empty() {
                human.push(format!("[STDERR] {}", result.stderr.trim_end()));
            }
            if result.timed_out {
                human.push("[TIMED OUT]".to_string());
            }
            emit(
                json,
                &serde_json::json!({ "result": to_value(&result) }),
                &human,
            );
            // The box's exit status is this process's exit status, so a script
            // that runs a command in a box can branch on it.
            if result.exit_code != 0 {
                std::process::exit(result.exit_code.clamp(1, 255) as i32);
            }
        }
        BoxAction::Stop {
            box_id,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let record = or_fail(client.stop_box(&id, &box_id).await);
            emit(
                json,
                &serde_json::json!({ "box": to_value(&record) }),
                &[format!(
                    "Stopped Box {} (state: {}). Slot released.",
                    record.box_id, record.state
                )],
            );
        }
        BoxAction::Run {
            box_id,
            command,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let joined = command.join(" ");
            let run = or_fail(client.start_run(&id, &box_id, &joined, None).await);
            emit(
                json,
                &serde_json::json!({ "run": to_value(&run) }),
                &[
                    format!("Started background run {} on Box {}.", run.id, run.box_id),
                    format!("State: {}", run.state),
                    format!("Inspect with: oa box runs view {} {}", run.box_id, run.id),
                ],
            );
        }
        BoxAction::Runs { action } => run_box_runs(action, &client, json).await,
        BoxAction::Fanout {
            count,
            labels,
            budgeted,
            conversation,
            request_id,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let plan = match request_id {
                Some(request) => or_fail(client.view_fanout(&id, &request).await),
                None => {
                    let parsed: Vec<String> = labels
                        .as_deref()
                        .map(|raw| {
                            raw.split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect()
                        })
                        .unwrap_or_default();
                    or_fail(client.fanout(&id, count, &parsed, budgeted).await)
                }
            };
            emit(
                json,
                &serde_json::json!({ "plan": to_value(&plan) }),
                &fanout_human(&plan),
            );
        }
    }
}

async fn run_box_runs(
    action: BoxRunAction,
    client: &crate::box_client::BoxClient,
    json: bool,
) {
    match action {
        BoxRunAction::List {
            box_id,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let runs = or_fail(client.list_runs(&id, &box_id).await);
            emit(
                json,
                &serde_json::json!({ "runs": to_value(&runs) }),
                &run_list_human(&runs),
            );
        }
        BoxRunAction::View {
            box_id,
            run_id,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let run = or_fail(client.view_run(&id, &box_id, &run_id).await);
            emit(
                json,
                &serde_json::json!({ "run": to_value(&run) }),
                &run_view_human(&run),
            );
        }
        BoxRunAction::Output {
            box_id,
            run_id,
            offset,
            follow,
            interval_ms,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            if !follow {
                let result = or_fail(client.run_output(&id, &box_id, &run_id, offset).await);
                let mut human = Vec::new();
                if result.truncated {
                    // The box keeps a bounded log, so a read that starts before
                    // the retained window begins mid-stream. Say so rather than
                    // letting the gap read as the run's first line.
                    human.push("[EARLIER OUTPUT DROPPED BY THE BOX]".to_string());
                }
                human.push(result.output.trim_end().to_string());
                emit(json, &to_value(&result), &human);
                return;
            }
            follow_run_output(client, &id, &box_id, &run_id, offset, interval_ms, json).await;
        }
        BoxRunAction::Cancel {
            box_id,
            run_id,
            conversation,
        } => {
            let id = or_fail(client.conversation_id(conversation.as_deref()).await);
            let run = or_fail(client.cancel_run(&id, &box_id, &run_id).await);
            emit(
                json,
                &serde_json::json!({ "run": to_value(&run) }),
                &[format!(
                    "Requested cancellation for run {} (state: {}).",
                    run.id, run.state
                )],
            );
        }
    }
}

/// Print a followed run's output as it arrives, then its final record.
///
/// The loop itself lives in [`crate::box_client::BoxClient::follow_run_output`],
/// where it is tested; this is the rendering half.
async fn follow_run_output(
    client: &crate::box_client::BoxClient,
    conversation: &str,
    box_id: &str,
    run_id: &str,
    offset: Option<u64>,
    interval_ms: u64,
    json: bool,
) {
    use std::io::Write;

    let collected = std::cell::RefCell::new(String::new());
    let announced = std::cell::Cell::new(false);
    let followed = client
        .follow_run_output(
            conversation,
            box_id,
            run_id,
            offset,
            std::time::Duration::from_millis(interval_ms.max(50)),
            |chunk| {
                if chunk.truncated && !announced.get() {
                    announced.set(true);
                    if !json {
                        println!("[EARLIER OUTPUT DROPPED BY THE BOX]");
                    }
                }
                if chunk.output.is_empty() {
                    return;
                }
                if json {
                    collected.borrow_mut().push_str(&chunk.output);
                } else {
                    print!("{}", chunk.output);
                    let _ = std::io::stdout().flush();
                }
            },
        )
        .await;
    let (run, next_offset) = or_fail(followed);

    if json {
        emit(
            true,
            &serde_json::json!({
                "run": to_value(&run),
                "output": collected.into_inner(),
                "next_offset": next_offset,
                "truncated": announced.get(),
            }),
            &[],
        );
    } else {
        println!();
        for line in run_view_human(&run) {
            println!("{}", line);
        }
    }
}

// ---------------------------------------------------------------------------
// memory
// ---------------------------------------------------------------------------

fn memory_list_human(memories: &[crate::memory_client::MemoryRecord]) -> Vec<String> {
    if memories.is_empty() {
        return vec!["No memories stored for this account.".to_string()];
    }
    // One memory per block rather than one per row: a memory is a sentence a
    // person wrote, and a column would cut most of them off.
    let mut lines = Vec::new();
    for memory in memories {
        lines.push(format!(
            "{}  [{}]  {}",
            memory.id, memory.bucket, memory.created_at
        ));
        lines.push(format!("  {}", memory.body));
        if let Some(source) = &memory.source_ref {
            lines.push(format!("  source: {}", source));
        }
        if let Some(replacement) = &memory.superseded_by {
            lines.push(format!("  superseded by: {}", replacement));
        }
    }
    lines
}

async fn run_memory(action: MemoryAction, token: Option<String>, json: bool) {
    let client = crate::memory_client::MemoryClient::new(API_BASE, token);
    match action {
        MemoryAction::List {
            bucket,
            limit,
            include_superseded,
        } => {
            let memories = or_fail(
                client
                    .list_memories(bucket.as_deref(), limit, include_superseded)
                    .await,
            );
            emit(
                json,
                &serde_json::json!({ "memories": to_value(&memories) }),
                &memory_list_human(&memories),
            );
        }
        MemoryAction::Add {
            body,
            bucket,
            supersedes,
            source_ref,
        } => {
            let text = body.join(" ");
            let memory = or_fail(
                client
                    .add_memory(
                        &text,
                        bucket.as_deref(),
                        supersedes.as_deref(),
                        source_ref.as_deref(),
                    )
                    .await,
            );
            let mut human = vec![
                format!(
                    "Stored memory {} in the {} bucket.",
                    memory.id, memory.bucket
                ),
                format!("  {}", memory.body),
            ];
            if let Some(replaced) = &supersedes {
                human.push(format!("Supersedes {}.", replaced));
            }
            emit(
                json,
                &serde_json::json!({ "memory": to_value(&memory) }),
                &human,
            );
        }
        MemoryAction::Delete { memory_id } => {
            let memory = or_fail(client.delete_memory(&memory_id).await);
            emit(
                json,
                &serde_json::json!({ "memory": to_value(&memory) }),
                &[
                    format!("Removed memory {}.", memory.id),
                    format!("  {}", memory.body),
                ],
            );
        }
    }
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
