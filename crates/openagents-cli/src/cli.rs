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
}

#[derive(Args, Debug)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Sign in to OpenAgents
    Login,
    /// Ingest token from standard input
    TokenStdin,
    /// Check authentication status
    Status,
    /// Log out of OpenAgents
    Logout,
}

#[derive(Args, Debug)]
pub struct IdentityArgs {
    #[command(subcommand)]
    pub action: IdentityAction,
}

#[derive(Subcommand, Debug)]
pub enum IdentityAction {
    /// Show current identity
    Show,
    /// Create a new cryptographic identity
    Create {
        #[arg(long)]
        name: Option<String>,
    },
    /// Import identity seed or key
    Import {
        #[arg(long)]
        seed: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct IssueArgs {
    #[command(subcommand)]
    pub action: IssueAction,
}

#[derive(Subcommand, Debug)]
pub enum IssueAction {
    /// List repository issues
    List {
        #[arg(short = 'R', long, help = "Repository (e.g. OpenAgentsInc/openagents)")]
        repo: Option<String>,
    },
    /// View issue details
    View {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    /// Create a new issue
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    /// Close an issue
    Close {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    /// Post a comment on an issue
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
    /// List projects
    List {
        #[arg(short = 'R', long)]
        repo: Option<String>,
    },
    /// View project details
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
    /// List repositories
    List,
    /// View repository details
    View {
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

pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Auth(auth) => match auth.action {
            AuthAction::Login => println!("Auth login initialized"),
            AuthAction::TokenStdin => println!("Reading token from stdin"),
            AuthAction::Status => println!("Authenticated as local operator"),
            AuthAction::Logout => println!("Logged out successfully"),
        },
        Commands::Identity(identity) => match identity.action {
            IdentityAction::Show => println!("Identity: active"),
            IdentityAction::Create { name } => println!("Created identity {:?}", name),
            IdentityAction::Import { seed: _ } => println!("Imported identity"),
        },
        Commands::Issue(issue) => match issue.action {
            IssueAction::List { repo } => println!("Listing issues for repo: {:?}", repo),
            IssueAction::View { number, repo } => println!("Viewing issue #{} in repo {:?}", number, repo),
            IssueAction::Create { title, body: _, repo } => println!("Created issue: {} in {:?}", title, repo),
            IssueAction::Close { number, repo } => println!("Closed issue #{} in {:?}", number, repo),
            IssueAction::Comment { number, body: _, repo } => println!("Commented on #{} in {:?}", number, repo),
        },
        Commands::Project(project) => match project.action {
            ProjectAction::List { repo } => println!("Listing projects in {:?}", repo),
            ProjectAction::View { number, repo } => println!("Viewing project #{} in {:?}", number, repo),
        },
        Commands::Repo(repo) => match repo.action {
            RepoAction::List => println!("Listing repos"),
            RepoAction::View { slug } => println!("Viewing repo {}", slug),
        },
        Commands::Coder(coder) => {
            if coder.delegate {
                crate::delegate::run_delegation(coder).await?;
            } else if coder.headless {
                println!("Executing coder prompt headlessly: {:?}", coder.prompt);
            } else {
                crate::interactive::run_tui(coder).await?;
            }
        }
    }
    Ok(())
}
