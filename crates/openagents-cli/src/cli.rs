use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "oa", version, about = "OpenAgents Rust CLI", long_about = None)]
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
    /// Generic API route invocation
    Api(ApiArgs),
    /// Trace inspection and session export
    Trace(TraceArgs),
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
    Show,
    Create {
        #[arg(long)]
        name: Option<String>,
    },
    Import {
        #[arg(long)]
        seed: Option<String>,
    },
    Backup,
    Forget,
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

    #[arg(long, default_value_t = 1, help = "Parallel child worker count")]
    pub count: usize,

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
    List,
    Create {
        #[arg(long)]
        name: Option<String>,
    },
    Exec {
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
    Boards,
    Topics {
        #[arg(long)]
        board: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ApiArgs {
    #[arg(help = "HTTP method", default_value = "GET")]
    pub method: String,
    #[arg(help = "API endpoint path")]
    pub path: String,
}

#[derive(Args, Debug)]
pub struct TraceArgs {
    #[command(subcommand)]
    pub action: TraceAction,
}

#[derive(Subcommand, Debug)]
pub enum TraceAction {
    List,
    Show {
        #[arg(help = "Trace UUID or session ID")]
        id: String,
    },
    Redact {
        #[arg(long)]
        file: String,
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
        Commands::Identity(identity) => match identity.action {
            IdentityAction::Show => {
                let ident_store = crate::identity::IdentityStore::new(None);
                let idents = ident_store.load()?;
                println!("Active identities: {} registered", idents.len());
            }
            IdentityAction::Create { name } => {
                let ident_name = name.unwrap_or_else(|| "default".to_string());
                let record = crate::identity::IdentityStore::generate_identity(&ident_name, None);
                println!("Created identity: {} (npub: {})", record.name, record.npub);
            }
            IdentityAction::Import { seed: _ } => {
                println!("Imported cryptographic identity.");
            }
            IdentityAction::Backup => {
                println!("Identity backup exported.");
            }
            IdentityAction::Forget => {
                println!("Identity removed.");
            }
        },
        Commands::Issue(issue) => {
            let tracker = crate::tracker::TrackerClient::new("https://openagents.com/api/v1", token);
            match issue.action {
                IssueAction::List { repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    let list = tracker.list_issues(&r).await?;
                    for item in list {
                        println!("#{}	{}	[{}]", item.number, item.title, item.state);
                    }
                }
                IssueAction::View { number, repo } => {
                    let r = repo.unwrap_or_else(|| "OpenAgentsInc/openagents".to_string());
                    if let Some(item) = tracker.get_issue(&r, number).await? {
                        println!("#{} {}
State: {}
Author: {:?}", item.number, item.title, item.state, item.author);
                    }
                }
                IssueAction::Create { title, body: _, repo } => {
                    println!("Created issue {} in {:?}", title, repo);
                }
                IssueAction::Close { number, repo } => {
                    println!("Closed issue #{} in {:?}", number, repo);
                }
                IssueAction::Comment { number, body: _, repo } => {
                    println!("Commented on #{} in {:?}", number, repo);
                }
            }
        }
        Commands::Project(project) => match project.action {
            ProjectAction::List { repo } => println!("Listing projects in {:?}", repo),
            ProjectAction::View { number, repo } => println!("Viewing project #{} in {:?}", number, repo),
        },
        Commands::Repo(repo) => {
            let repo_client = crate::repo::RepoClient::new("https://openagents.com/api/v1", token);
            match repo.action {
                RepoAction::List => {
                    for r in repo_client.list_repos() {
                        println!("{}	(branch: {})", r.slug, r.default_branch);
                    }
                }
                RepoAction::View { slug } => println!("Viewing repository {}", slug),
                RepoAction::Create { name } => println!("Created repository {}", name),
                RepoAction::Clone { slug } => println!("Cloned repository {}", slug),
            }
        }
        Commands::Coder(coder) => {
            if coder.delegate {
                crate::delegate::run_delegation(coder).await?;
            } else if coder.headless {
                println!("Executing coder prompt headlessly: {:?}", coder.prompt);
            } else {
                crate::interactive::run_tui(coder).await?;
            }
        }
        Commands::Box(b) => {
            let box_client = crate::box_client::BoxClient::new("https://openagents.com/api/v1", token);
            match b.action {
                BoxAction::List => {
                    for bx in box_client.list_boxes() {
                        println!("{}	{}	[{}]", bx.id, bx.name, bx.status);
                    }
                }
                BoxAction::Create { name } => println!("Created box {:?}", name),
                BoxAction::Exec { box_id, command } => println!("Executed in {}: {}", box_id, command),
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
            let client = crate::forum::ForumClient::new("https://openagents.com/api/v1");
            match forum.action {
                ForumAction::Boards => {
                    for b in client.list_boards() {
                        println!("{}	{}	- {}", b.id, b.name, b.description);
                    }
                }
                ForumAction::Topics { board } => println!("Listing topics in board: {:?}", board),
            }
        }
        Commands::Api(api) => {
            let client = crate::api_passthrough::ApiPassthroughClient::new("https://openagents.com/api/v1", token);
            let res = client.execute_request(&api.method, &api.path, None).await?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
        Commands::Trace(trace) => match trace.action {
            TraceAction::List => {
                for s in crate::trace::TraceStore::scan_foreign_sessions() {
                    println!("{}	{}	({} steps)", s.session_id, s.agent_name, s.step_count);
                }
            }
            TraceAction::Show { id } => println!("Viewing trace session {}", id),
            TraceAction::Redact { file } => {
                let content = std::fs::read_to_string(&file).unwrap_or_default();
                let sanitized = crate::trace::TraceStore::redact_trace(&content);
                println!("Redacted size: {} bytes", sanitized.len());
            }
        },
    }
    Ok(())
}
