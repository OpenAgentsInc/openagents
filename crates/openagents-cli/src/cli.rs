use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};

/// The shells `--completions` can write a script for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Sh,
}

#[derive(Parser, Debug)]
#[command(
    name = "openagents",
    version = crate::VERSION,
    about = "Manage OpenAgents resources from your terminal",
    long_about = None
)]
#[command(args_conflicts_with_subcommands = false)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(long, global = true, help = "Output as JSON")]
    pub json: bool,

    #[arg(short, long, global = true, help = "Verbose logging output")]
    pub verbose: bool,

    #[arg(long, global = true, help = "Disable ANSI output")]
    pub no_color: bool,

    #[arg(
        long,
        value_enum,
        help = "Print a shell completion script and exit",
        value_name = "SHELL"
    )]
    pub completions: Option<CompletionShell>,

    #[arg(
        long,
        global = true,
        help = "API origin to talk to, such as https://openagents.com"
    )]
    pub api_url: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Named API endpoint: production, staging, or local"
    )]
    pub profile: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Forge repositories and deployment targets
    Forge(ForgeArgs),
    /// Authentication management
    Auth(AuthArgs),
    /// OpenAgents issue tracker operations
    Issue(IssueArgs),
    /// OpenAgents project management
    Project(ProjectArgs),
    /// Repository milestones
    Milestone(MilestoneArgs),
    /// Repository tracking and operations
    Repo(RepoArgs),
    /// OpenAgents interactive Coder agent session and autonomous tools
    Coder(CoderArgs),
    /// Run one prompt on many child coding agents at once and report each result
    Delegate(DelegateArgs),
    /// Operator deployment of the OpenAgents fleet
    Deploy(DeployArgs),
    /// Earn on verified work: decide what a leased job is owed
    Provider(ProviderArgs),
    /// Box sandbox management and fanout execution
    Box(BoxArgs),
    /// Computer agent daemon and local policy probe
    Computer(crate::computer::ComputerArgs),
    /// Forum boards and topics
    Forum(ForumArgs),
    /// Account-level system memory and knowledge management
    Memory(MemoryArgs),
    /// Generic API route invocation
    Api(crate::api_passthrough::ApiArgs),
    /// Sandboxed WebAssembly capability plugins: catalog, digests, and runs
    Plugin(crate::plugins::PluginArgs),
    /// Gym environment, suites, and corpus
    Gym(GymArgs),
    /// Trace inspection and session export
    Trace(TraceArgs),
    /// Local swarm: discover sessions and exchange messages between them
    Swarm(crate::swarm_args::SwarmArgs),
    /// Replace this binary with the release the channel names
    #[command(alias = "self-update")]
    Update(UpdateArgs),
}

#[derive(Args, Debug)]
pub struct ForgeArgs {
    #[command(subcommand)]
    pub action: ForgeAction,
}

#[derive(Subcommand, Debug)]
pub enum ForgeAction {
    /// Repository operations on the forge
    Repo(RepoArgs),
    /// Deployment target operations on the forge
    Deploy(DeployArgs),
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    #[arg(long, help = "Release channel to resolve (default: stable)")]
    pub channel: Option<String>,

    #[arg(
        long,
        help = "Install this exact version instead of resolving a channel"
    )]
    pub version: Option<String>,

    #[arg(
        long,
        help = "Report what the channel names without downloading anything"
    )]
    pub check: bool,

    #[arg(
        long,
        help = "Reinstall even when the channel names the running version"
    )]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Authorize this CLI in your browser and store the resulting token
    Login {
        #[arg(
            long,
            help = "Read and store a token from standard input instead of opening a browser"
        )]
        token_stdin: bool,
        #[arg(
            long,
            help = "Print an authorization URL and code without waiting for approval"
        )]
        headless: bool,
        #[arg(long, help = "Complete the pending device authorization")]
        resume: bool,
        #[arg(
            long,
            help = "Request a scope for the new token; repeatable. Omit to take the server's default"
        )]
        scope: Vec<String>,
    },
    /// Read a token from standard input and store it for the selected API
    TokenStdin,
    /// Show authentication status for the selected API
    Status,
    /// Remove the stored token for the selected API
    Logout,
    /// Configure git to obtain OpenAgents credentials from this CLI
    SetupGit {
        #[arg(long, help = "Configure the current git repository")]
        local: bool,
        #[arg(long, help = "Configure your global git settings")]
        global: bool,
        #[arg(long, help = "Confirm a global git credential-helper change")]
        yes: bool,
    },
    /// Internal git credential-helper protocol endpoint
    GitCredential {
        #[arg(default_value = "get")]
        operation: String,
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
    /// Put an existing issue on a milestone, or take it off one
    ///
    /// `issue create --milestone` was the only way to attach one, so an issue
    /// filed without a milestone could never be given one outside the browser.
    Milestone {
        #[arg(help = "Issue number")]
        number: u64,
        #[arg(
            long,
            value_name = "NUMBER",
            help = "Milestone number to put the issue on"
        )]
        set: Option<u64>,
        #[arg(long, help = "Take the issue off whatever milestone it is on")]
        clear: bool,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct MilestoneArgs {
    #[command(subcommand)]
    pub action: MilestoneAction,
}

#[derive(Subcommand, Debug)]
pub enum MilestoneAction {
    /// List the milestones of a repository
    List {
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Open a new milestone
    Create {
        #[arg(help = "Milestone title")]
        title: String,
        #[arg(long, help = "What the milestone is for")]
        description: Option<String>,
        #[arg(long, value_name = "DATE", help = "Due date, as the server stores it")]
        due_on: Option<String>,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Delete a milestone
    Delete {
        #[arg(help = "Milestone number")]
        number: u64,
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
    /// Edit a project board's title, description, state, or archive
    Edit {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(long, help = "New project title")]
        title: Option<String>,
        #[arg(long, help = "Markdown project description")]
        description: Option<String>,
        #[arg(long, help = "New project state: open or closed")]
        state: Option<String>,
        #[arg(long, help = "Move the board out of the working set")]
        archive: bool,
        #[arg(long, help = "Return the board to the working set")]
        unarchive: bool,
        #[arg(short = 'R', long, help = "Repository as owner/repo")]
        repo: Option<String>,
    },
    /// Permanently delete a project board (archive it first with project edit --archive)
    Delete {
        #[arg(help = "Project number")]
        number: u64,
        #[arg(long, help = "Confirm permanent project deletion")]
        yes: bool,
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
        #[arg(
            long = "set",
            required = true,
            help = "Set a field, as FIELD=VALUE; repeatable"
        )]
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
    /// List repositories available to you
    List {
        #[arg(long, help = "Filter by a GitHub-backed namespace")]
        namespace: Option<String>,
        #[arg(
            long,
            default_value_t = 30,
            help = "Return between 1 and 100 repositories"
        )]
        limit: u32,
        #[arg(long, help = "Continue from an opaque repository cursor")]
        after: Option<String>,
    },
    /// Show one repository, or infer it from this checkout's git remotes
    View {
        #[arg(help = "Repository in OWNER/REPO format")]
        repository: Option<String>,
        #[arg(
            short = 'R',
            long,
            help = "Select OWNER/REPO instead of inferring the remote"
        )]
        repo: Option<String>,
    },
    /// Create an empty OpenAgents repository
    Create {
        #[arg(help = "Repository name, or OWNER/NAME")]
        name: String,
        #[arg(long, help = "Set the repository description")]
        description: Option<String>,
        #[arg(long, help = "Create a public repository")]
        public: bool,
        #[arg(long, help = "Create a private repository")]
        private: bool,
        #[arg(long, default_value = "main", help = "Set the initial default branch")]
        default_branch: String,
        #[arg(
            long,
            default_value_t = 300,
            help = "Seconds to wait for durable provisioning (0 does not wait)"
        )]
        wait_timeout: u64,
        #[arg(long, help = "Attach the new repository to an existing git worktree")]
        source: Option<String>,
        #[arg(
            long,
            help = "Name the git remote attached with --source (defaults to origin)"
        )]
        remote: Option<String>,
    },
    /// Import a GitHub repository once
    Import {
        #[arg(help = "GitHub repository in OWNER/REPO format")]
        source: String,
        #[arg(long, help = "Override the destination repository name")]
        name: Option<String>,
        #[arg(long, help = "Import into an eligible GitHub organization namespace")]
        namespace: Option<String>,
        #[arg(long, help = "Import as a public repository")]
        public: bool,
        #[arg(long, help = "Import as a private repository")]
        private: bool,
        #[arg(
            long,
            default_value_t = 300,
            help = "Seconds to wait for the import (0 does not wait)"
        )]
        wait_timeout: u64,
    },
    /// Clone a repository with git
    Clone {
        #[arg(help = "Repository in OWNER/REPO format")]
        repository: Option<String>,
        #[arg(help = "Directory to clone into")]
        directory: Option<String>,
        #[arg(
            short = 'R',
            long,
            help = "Select OWNER/REPO instead of inferring the remote"
        )]
        repo: Option<String>,
    },
    /// Permanently delete a repository you own
    Delete {
        #[arg(help = "Repository in OWNER/REPO format")]
        repository: Option<String>,
        #[arg(
            short = 'R',
            long,
            help = "Select OWNER/REPO instead of inferring the remote"
        )]
        repo: Option<String>,
        #[arg(long, help = "Confirm permanent repository deletion")]
        yes: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct CoderArgs {
    #[arg(help = "Optional prompt to execute headlessly or start interactive session with")]
    pub prompt: Option<String>,

    #[arg(long, help = "Delegate prompt to parallel child agents")]
    pub delegate: bool,

    #[arg(
        long,
        default_value_t = 1,
        help = "How many child agents run the prompt"
    )]
    pub count: usize,

    /// `--concurrency` is the name the TypeScript CLI gives this, and the name
    /// `oa delegate` already gives it. An alias rather than a second field:
    /// two flags that both set one value can be written together and then
    /// disagree, and nothing would say which one won.
    #[arg(
        long,
        visible_alias = "concurrency",
        help = "How many children run at once. Defaults to all of them"
    )]
    pub max_parallel: Option<usize>,

    #[arg(
        long,
        help = "Working directory each child gets: worktree (default, a detached git worktree of HEAD), directory, or none"
    )]
    pub isolation: Option<String>,

    #[arg(
        long,
        help = "Leave the children's worktrees on disk so their work can be read"
    )]
    pub keep_workspaces: bool,

    // The four `--child-*` flags. `oa delegate` declared them and `oa coder
    // --delegate` did not, so a fan-out started from the coder command could
    // not be given a harness, a model, or a config file — and `--child-config`
    // is the only route a provider credential has to a child, because this CLI
    // deliberately never stores one. A lane that needed its own credential was
    // therefore unreachable from `oa coder` at all.
    #[arg(
        long,
        help = "Run children on this model instead of the lane's own, as `provider/model`. Defaults to OPENAGENTS_DELEGATE_MODEL"
    )]
    pub child_model: Option<String>,

    #[arg(
        long,
        help = "The harness that runs a child. Defaults to OPENAGENTS_DELEGATE_COMMAND, or the lane's own binary"
    )]
    pub child_command: Option<String>,

    #[arg(
        long,
        help = "A harness config file for children, passed as OPENCODE_CONFIG. This is how a provider credential reaches a child without being stored by the CLI"
    )]
    pub child_config: Option<String>,

    #[arg(
        long,
        help = "Make children ask before using a tool. A delegated child has nobody to ask, so this stops it at its first edit; it exists for a dry run over a directory you do not want touched"
    )]
    pub child_ask: bool,

    #[arg(
        long,
        help = "Target harness lane (e.g. flash, free, devin, claude, codex)"
    )]
    pub lane: Option<String>,

    /// Pick the model a turn runs on by its catalog id.
    ///
    /// `--lane` deals in tiers; this deals in ids, settled against `GET
    /// /api/v1/models` before a thread is opened, so an id this deployment
    /// does not serve is refused by name rather than quietly replaced with the
    /// default. `ollama:<model>` names a model on this machine.
    #[arg(
        long,
        help = "Model id to answer on, or ollama:<model> for one on this machine"
    )]
    pub model: Option<String>,

    /// Answer from an Ollama server on this machine.
    ///
    /// The same lane `--lane local` selects. Nothing in the conversation
    /// leaves the machine and nothing is metered.
    #[arg(
        long,
        help = "Answer from a model running on this machine through Ollama"
    )]
    pub local: bool,

    /// Answer from the built-in stand-in instead of reaching a model.
    ///
    /// A deliberate mode, not a failure path. The live path fails loudly when
    /// it cannot reach a model — this is the only way to get the stand-in, and
    /// it opens no thread, spends nothing, and says on every reply that it is
    /// not a model.
    #[arg(
        long,
        help = "Answer from the built-in stand-in instead of reaching a model"
    )]
    pub offline: bool,

    /// The effort recorded on the thread as its admitted execution shape.
    #[arg(
        long,
        value_parser = ["minimal", "low", "medium", "high", "max"],
        help = "Reasoning effort recorded on the thread as its admitted execution shape"
    )]
    pub reasoning: Option<String>,

    /// Continue a thread of the account's instead of opening a new one.
    ///
    /// Bare `--resume` shows a picker over this repository's recent threads;
    /// `--resume <id>` names one; `--resume --last` continues the most recent
    /// without asking.
    ///
    /// The id is the flag's own value rather than the positional argument.
    /// `openagents coder` reads the positional as the id under `--resume`,
    /// which costs it the ability to resume and say something in the same
    /// breath; here `oa coder --resume <id> "what did we conclude?"` is both.
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "ID",
        help = "Continue a thread instead of opening one. Bare: a picker over this repository's threads"
    )]
    pub resume: Option<String>,

    #[arg(
        long,
        help = "With --resume, continue the most recent thread without asking"
    )]
    pub last: bool,

    #[arg(
        long,
        help = "With --resume, list every thread on the account rather than this repository's"
    )]
    pub all: bool,

    #[arg(long, help = "Run in non-interactive headless mode")]
    pub headless: bool,

    #[arg(long, help = "Export conversation transcript to file")]
    pub export: Option<String>,

    /// Line-oriented output with no cursor control, even on a terminal.
    ///
    /// The full-screen session draws over the scrollback and cannot be piped
    /// or read back; `--plain` prints the prompt and the reply as lines, which
    /// is what a transcript, a pipe, and a screen reader can all use.
    #[arg(
        long,
        help = "Line-oriented output with no cursor control, even on a terminal"
    )]
    pub plain: bool,

    /// Talk to the local Rust coder API on this machine.
    ///
    /// Starts `openagents-coder-api` if none is running. Prefers port 4000;
    /// if Phoenix already owns that port, binds 4010. The global `--api-url`
    /// still wins when both are given. Production origin is unchanged
    /// without this flag.
    #[arg(long, help = "Talk to the local Rust coder API on this machine")]
    pub dev: bool,

    #[arg(
        long,
        default_value_t = 4000,
        help = "Port --dev talks to on this machine"
    )]
    pub dev_port: u16,
}

/// The lane name `oa coder` runs on when nothing names another.
///
/// A lane name, not a model id. It used to be `ox-alpha`, which pinned one
/// model directly and went stale the moment that model left the catalog; the
/// default is now the Flash lane, whose id is resolved against
/// `GET /api/v1/models` at open.
const DEFAULT_LANE: &str = "flash";

impl CoderArgs {
    /// The lane this invocation asked for, as a name [`crate::runtime::Lane`]
    /// understands.
    ///
    /// Three flags reach the same setting — `--lane` names a tier, `--model`
    /// names a catalog id, `--local` names this machine — so two of them
    /// naming different things is a refusal rather than a silent precedence
    /// order. A reader who wrote both meant one of them and cannot tell which
    /// one won.
    pub fn lane_name(&self) -> Result<String, String> {
        let mut asked: Vec<(&str, String)> = Vec::new();
        if let Some(lane) = self
            .lane
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            asked.push(("--lane", lane.to_string()));
        }
        if let Some(model) = self
            .model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            asked.push(("--model", model.to_string()));
        }
        if self.local {
            asked.push(("--local", "local".to_string()));
        }

        match asked.len() {
            0 => Ok(DEFAULT_LANE.to_string()),
            1 => Ok(asked.remove(0).1),
            _ => {
                let resolved: Vec<crate::runtime::Lane> = asked
                    .iter()
                    .map(|(_, value)| crate::runtime::Lane::from_str(value))
                    .collect();
                // `--local --model ollama:x` is one intent written twice, and
                // is the one combination that is not a contradiction: `--local`
                // names the lane and the other names the model on it, so the
                // more specific one is what was meant. Bare `--local` alone
                // means "whatever is installed", which contradicts nothing.
                if resolved.iter().all(crate::runtime::Lane::is_local) {
                    let named: Vec<&str> = resolved
                        .iter()
                        .filter_map(|lane| match lane {
                            crate::runtime::Lane::Local(model) if !model.is_empty() => {
                                Some(model.as_str())
                            }
                            _ => None,
                        })
                        .collect();
                    match named.as_slice() {
                        [] => return Ok("local".to_string()),
                        [only] => return Ok(format!("ollama:{only}")),
                        _ if named.windows(2).all(|pair| pair[0] == pair[1]) => {
                            return Ok(format!("ollama:{}", named[0]));
                        }
                        _ => {}
                    }
                }
                // Two names for one hosted lane is agreement, not a conflict:
                // `--lane pro` and `--model gpt-5.6-luna` are the same thing
                // said twice.
                if resolved.windows(2).all(|pair| pair[0] == pair[1]) {
                    return Ok(asked.remove(0).1);
                }
                let names: Vec<String> = asked
                    .iter()
                    .map(|(flag, value)| format!("{flag} {value}"))
                    .collect();
                Err(format!(
                    "{} name different lanes. Give one of them.",
                    names.join(" and ")
                ))
            }
        }
    }

    /// The four `--child-*` flags, resolved with their environment fallbacks.
    ///
    /// One place, so a session that delegates through `/delegate` or the
    /// `delegate` tool starts its children on exactly what `--delegate` would.
    pub fn child_options(&self) -> crate::delegate::ChildOptions {
        crate::delegate::ChildOptions::resolve(
            self.child_model.clone(),
            self.child_command.clone(),
            self.child_config.clone(),
            self.child_ask,
        )
    }

    /// Whether any of `--lane`, `--model` or `--local` was written.
    ///
    /// A resumed thread already holds the model its grant pins, so naming one
    /// on the same command line is a flag with nothing to do.
    pub fn named_a_lane(&self) -> bool {
        self.lane.is_some() || self.model.is_some() || self.local
    }
}

// ---------------------------------------------------------------------------
// coder: the offline stand-in
// ---------------------------------------------------------------------------

/// What `--offline` answers with.
///
/// A deliberate mode. This text is reachable only when someone asks for it:
/// the live path fails loudly when it cannot reach a model, and there is no
/// branch that falls back here. The reply names itself as a stand-in on every
/// turn, because a reply that reads like a model's and is not one is the
/// failure this whole flag exists to keep visible.
pub fn standin_reply(prompt: &str) -> String {
    let asked = prompt.trim();
    format!(
        "[stand-in] No model answered this. `--offline` asked for the built-in \
         stand-in, so nothing was sent anywhere, no thread was opened, and \
         nothing was spent.\n\n\
         You asked: {asked}\n\n\
         What works without a model: the composer, the transcript, `--plain`, \
         `--export`, and the tool registry. Drop `--offline` to reach a model, \
         or use `--local` for one on this machine."
    )
}

/// The label a stand-in session reports where a model id would go.
pub const STANDIN_MODEL: &str = "stand-in (no model attached)";

// ---------------------------------------------------------------------------
// delegate
// ---------------------------------------------------------------------------

/// `oa delegate`.
///
/// The same fan-out `oa coder --delegate` runs, raised to a command of its own
/// so the flags that configure a child are not buried under a coding session's
/// flags. Every flag below is read by [`crate::delegate::run_delegation`]; the
/// TypeScript command carries `--description` and the five `--child-*` flags
/// for the same reason, and they mean the same things here.
#[derive(Args, Debug)]
pub struct DelegateArgs {
    #[arg(help = "The task every child performs")]
    pub prompt: Option<String>,

    #[arg(long, default_value_t = 1, help = "How many children run this prompt")]
    pub agents: usize,

    #[arg(long, help = "Where children work. Defaults to the current directory")]
    pub dir: Option<String>,

    #[arg(
        long,
        help = "Three to five words naming the task. Defaults to the start of the prompt"
    )]
    pub description: Option<String>,

    #[arg(long, help = "How many children may run at once. The rest queue")]
    pub concurrency: Option<usize>,

    #[arg(
        long,
        help = "Target harness lane (for example openagents, gemini, opencode/<model>, devin, claude, or codex). Defaults to openagents, which spends this account's grant"
    )]
    pub lane: Option<String>,

    #[arg(
        long,
        help = "Working directory each child gets: worktree (default, a detached git worktree of HEAD), directory, or none"
    )]
    pub isolation: Option<String>,

    #[arg(
        long,
        help = "Leave the children's worktrees on disk so their work can be read"
    )]
    pub keep_workspaces: bool,

    #[arg(
        long,
        help = "Run children on this model instead of the lane's own, as `provider/model`. Defaults to OPENAGENTS_DELEGATE_MODEL"
    )]
    pub child_model: Option<String>,

    #[arg(
        long,
        help = "The harness that runs a child. Defaults to OPENAGENTS_DELEGATE_COMMAND, or the lane's own binary"
    )]
    pub child_command: Option<String>,

    #[arg(
        long,
        help = "A harness config file for children, passed as OPENCODE_CONFIG. This is how a provider credential reaches a child without being stored by the CLI"
    )]
    pub child_config: Option<String>,

    #[arg(
        long,
        help = "Make children ask before using a tool. A delegated child has nobody to ask, so this stops it at its first edit; it exists for a dry run over a directory you do not want touched"
    )]
    pub child_ask: bool,
}

// ---------------------------------------------------------------------------
// deploy
// ---------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct DeployArgs {
    #[command(subcommand)]
    pub action: DeployAction,
}

#[derive(Subcommand, Debug)]
pub enum DeployAction {
    /// Promote an exact pushed commit as the production fleet target (operator only)
    Promote {
        #[arg(
            long,
            help = "Canonical repository exactly as the server allows it, such as openagents.com"
        )]
        repo: Option<String>,
        #[arg(
            long,
            help = "Full 40-character commit SHA; branch names and abbreviations are refused"
        )]
        sha: Option<String>,
        #[arg(
            long,
            help = "Deployment environment, stated explicitly; the server admits production"
        )]
        environment: Option<String>,
        #[arg(
            long,
            help = "Caller-generated idempotency key for controlled automation; omitted, the CLI generates one and reuses it across automatic retries. Never printed"
        )]
        idempotency_key: Option<String>,
        #[arg(
            long,
            help = "Compare-and-set: refuse the promotion when the current target is no longer this ID"
        )]
        expected_current_target: Option<String>,
        #[arg(
            long,
            help = "Poll the status resource with bounded backoff until the target reaches live, failed, reverted, or needs_rolling_replace"
        )]
        wait: bool,
        #[arg(
            long,
            default_value_t = 1800,
            help = "Seconds --wait polls before reporting a timeout (the target keeps running)"
        )]
        wait_timeout: u64,
    },
    /// Show one fleet target; --wait follows it to a terminal state
    View {
        #[arg(help = "Fleet target ID returned by deploy promote or deploy list")]
        target_id: String,
        #[arg(
            long,
            help = "Poll the status resource with bounded backoff until the target reaches live, failed, reverted, or needs_rolling_replace"
        )]
        wait: bool,
        #[arg(
            long,
            default_value_t = 1800,
            help = "Seconds --wait polls before reporting a timeout (the target keeps running)"
        )]
        wait_timeout: u64,
    },
    /// List recent fleet targets, newest first
    List {
        #[arg(
            long,
            help = "Canonical repository exactly as the server allows it, such as openagents.com"
        )]
        repo: Option<String>,
        #[arg(long, help = "Return between 1 and 50 recent targets")]
        limit: Option<u32>,
    },
}

// ---------------------------------------------------------------------------
// provider
// ---------------------------------------------------------------------------

#[derive(Args, Debug)]
pub struct ProviderArgs {
    #[command(subcommand)]
    pub action: ProviderAction,
}

#[derive(Subcommand, Debug)]
pub enum ProviderAction {
    /// Decide what one leased job earned. Payment follows a NIP-LBR closeout
    /// receipt that names a verification command, its evidence, and the
    /// platform's own closeout; a lease, a submission, or time spent online
    /// earns nothing. The decision accrues and never pays: no key is held and
    /// no payout rail is connected.
    Settle {
        #[arg(
            long,
            help = "Path to the lease document the buyer granted for this job"
        )]
        lease: String,
        #[arg(
            long,
            help = "Path to the NIP-LBR closeout receipt covering this job. Omit it to see what an unverified job earns."
        )]
        closeout: Option<String>,
    },
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
        #[arg(
            required = true,
            trailing_var_arg = true,
            help = "Command to run in the background"
        )]
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
        // Optional, not required, because `--request-id` reads a plan that
        // already exists and has a count of its own. While this was
        // `count: u64`, clap refused every `--request-id` invocation for a
        // missing `--count`, and the only way to reach the read path was to
        // pass a number the command then ignored.
        #[arg(
            long,
            help = "Number of boxes to request; required without --request-id"
        )]
        count: Option<u64>,
        #[arg(long, help = "Comma-separated labels for the fanout boxes")]
        labels: Option<String>,
        #[arg(long, help = "Allow scaling up to the budgeted limit")]
        budgeted: bool,
        #[arg(long, help = "Conversation id override")]
        conversation: Option<String>,
        #[arg(
            long,
            help = "Read an existing plan by its request id instead of asking for one"
        )]
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
        #[arg(
            long,
            default_value_t = 1000,
            help = "Milliseconds between polls while following"
        )]
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
        // The server pages this route at 25 rows. Without the flag a caller
        // could read the first page and never learn the other four existed.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "One-based page number")]
        page: Option<u32>,
    },
    /// Search topics across boards
    Search {
        #[arg(help = "Search query")]
        query: String,
        #[arg(long, help = "Narrow the search to one board slug")]
        board: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "One-based page number")]
        page: Option<u32>,
    },
    /// Read one topic and its posts
    Topic {
        #[arg(help = "Topic id (the prefix a topic URL starts with works too)")]
        id: String,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "One-based page number")]
        page: Option<u32>,
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
pub struct TraceArgs {
    #[command(subcommand)]
    pub action: TraceAction,
}

#[derive(Subcommand, Debug)]
pub enum TraceAction {
    /// Discover trace files in the local agent stores
    List {
        #[arg(
            long,
            help = "Scan this directory instead of the default stores. Repeatable"
        )]
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
    /// Upload one ATIF document to openagents.com
    ///
    /// Stored at `dark` — nothing public — unless --visibility names a higher
    /// rung. Redact the trace first: what is uploaded is the file as it stands.
    Upload {
        #[arg(help = "A trace file path, or a file name inside ~/.openagents/exports")]
        trace: String,
        #[arg(
            long,
            default_value = crate::trace_client::DEFAULT_TRACE_VISIBILITY,
            help = "Transparency rung: dark (nothing public), pulse (metadata only), ledger (content and metadata), or glass (full access)"
        )]
        visibility: String,
        #[arg(long, help = "Bind the trace to the forge attempt with this id")]
        assignment: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct GymArgs {
    #[command(subcommand)]
    pub action: GymAction,
}

#[derive(Subcommand, Debug)]
pub enum GymAction {
    /// Discover and inspect benchmark suite manifests
    Suite(crate::gym::suite::SuiteArgs),
    /// Execute and manage Gym runs
    Run(crate::gym::run::RunArgs),
    /// Gym environment plumbing
    Env(crate::gym::env::EnvArgs),
    /// Walk the three local trace stores and write a corpus inventory
    Inventory {
        #[arg(
            long,
            help = "Scan this directory instead of the default stores. Repeatable"
        )]
        path: Vec<String>,
        #[arg(long, help = "Write the inventory to this file")]
        out: Option<String>,
    },
    /// Apply the plan.md qualification filters and write the exclusion report
    Qualify {
        #[arg(help = "Inventory file to qualify")]
        inventory: String,
    },
    /// Manage named, versioned datasets of traces and tasks
    Dataset(crate::gym::dataset::DatasetArgs),
    /// Score Harbor jobs and read the chained results store
    Results(crate::gym::results::ResultsArgs),
    /// Import, status, and verify the CoderBench trace corpus
    Corpus(crate::gym::corpus::CorpusArgs),
}

/// The completion script for one shell.
///
/// `sh` is not a shell clap generates for, and a POSIX shell has no completion
/// protocol of its own; the TypeScript CLI offers it because its generator
/// emits a bash-compatible script there, so this does the same rather than
/// refusing a shell the other binary accepts.
pub fn completion_script(shell: CompletionShell) -> String {
    use clap::CommandFactory;
    use clap_complete::{Shell, generate};
    let generated = match shell {
        CompletionShell::Bash | CompletionShell::Sh => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => Shell::Fish,
    };
    let mut command = Cli::command();
    let mut buffer: Vec<u8> = Vec::new();
    generate(generated, &mut command, "oa", &mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

async fn run_gym(
    action: GymAction,
    api_base: &str,
    token: Option<String>,
    json: bool,
) -> Result<(), crate::errors::CliError> {
    match action {
        GymAction::Suite(args) => crate::gym::suite::run_suite(args.action, json),
        GymAction::Run(run) => crate::gym::run::run(run.action, api_base, token, json).await,
        GymAction::Env(env) => {
            crate::gym::env::run(env.action, api_base, token, json).await;
            Ok(())
        }
        GymAction::Inventory { path, out } => {
            let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
                crate::errors::CliError::Input(
                    "could not determine the home directory; set HOME or pass --path".to_string(),
                )
            })?;
            let extra: Vec<PathBuf> = path.into_iter().map(PathBuf::from).collect();
            let out = out.map(PathBuf::from).unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join("docs/coderbench/inventory.json")
            });
            let document = crate::gym::corpus::inventory(
                &home,
                &extra,
                &out,
                crate::gym::corpus::INVENTORY_BOUNDS,
            )
            .map_err(|error| crate::errors::CliError::Input(error.to_string()))?;
            let human: Vec<String> = {
                let mut lines = vec![format!(
                    "Wrote {} rows to {}",
                    document.rows.len(),
                    out.display()
                )];
                for store in &document.stores {
                    lines.push(format!(
                        "{} matched={} listed={} qualified={} excluded={}",
                        store.source, store.matched, store.listed, store.qualified, store.excluded
                    ));
                }
                if let Some(counts) = &document.counts {
                    for (reason, count) in counts {
                        lines.push(format!("{}: {}", reason, count));
                    }
                }
                lines
            };
            emit(
                json,
                &schema_document(crate::gym::corpus::INVENTORY_SCHEMA, &document),
                &human,
            );
            Ok(())
        }
        GymAction::Qualify { inventory } => {
            let report = crate::gym::corpus::qualify(Path::new(&inventory))
                .map_err(|error| crate::errors::CliError::Input(error.to_string()))?;
            let human: Vec<String> = {
                let mut lines = vec![format!(
                    "qualified={} excluded={} total={}",
                    report.qualified_rows, report.excluded_rows, report.total_rows
                )];
                for (reason, count) in &report.by_reason {
                    lines.push(format!("{}: {}", reason, count));
                }
                lines
            };
            emit(
                json,
                &schema_document(crate::gym::corpus::QUALIFY_SCHEMA, &report),
                &human,
            );
            Ok(())
        }
        GymAction::Dataset(args) => crate::gym::dataset::run_dataset(args, json),
        GymAction::Results(args) => crate::gym::results::run_results(args, json),
        GymAction::Corpus(args) => {
            crate::gym::corpus::run_corpus(args, api_base, token, json).await
        }
    }
}

pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    crate::diag::set_verbose(cli.verbose);
    crate::diag::initialize_color_from_environment();
    if cli.no_color {
        crate::diag::set_color(false);
    }
    // The failure path is reached from several hundred sites that never took
    // the flag, so it is recorded once here and read there.
    crate::errors::set_json(cli.json);

    // `--completions` writes a script and stops. It reaches no endpoint and
    // needs no token, so it is answered before either is resolved.
    if let Some(shell) = cli.completions {
        // Written through the handle rather than `print!` so a reader piping
        // several thousand lines into `head` closes the pipe and gets nothing
        // worse than a short script.
        use std::io::Write;
        let _ = std::io::stdout().write_all(completion_script(shell).as_bytes());
        return Ok(());
    }

    let Some(command) = cli.command else {
        // Without a subcommand there is nothing to do. Clap's own help goes to
        // stdout on `--help`; a bare invocation is a usage error, so the same
        // text goes to stderr and the status is the usage status.
        use clap::CommandFactory;
        let _ = Cli::command().print_help();
        std::process::exit(2);
    };

    // `box exec`, `box run`, and `memory add` capture the rest of the line for
    // something else, which is right, and which also captured `oa`'s own
    // `--conversation` and sent the id to a remote shell without a word about
    // the flag it was written for (#109). A trailing token that names a flag of
    // the same subcommand is refused here, before an endpoint is resolved and
    // before anything is sent anywhere.
    if let Err(reason) = crate::trailing_args::check_command(&command) {
        fail(&reason);
    }

    let endpoint =
        match crate::auth::resolve_endpoint(cli.api_url.as_deref(), cli.profile.as_deref()) {
            Ok(endpoint) => endpoint,
            Err(error) => fail(&error.to_string()),
        };
    let cred_store = crate::auth::CredentialStore::for_origin(&endpoint.origin);
    let token = cred_store.get_token();
    // Every client below is built against the selected endpoint. It used to be
    // built against a literal production origin at each of seven call sites,
    // which meant `--profile` and `--api-url` parsed and changed nothing for
    // any command but `auth` and `repo`.
    let api_base = format!("{}/api/v1", endpoint.origin);

    match command {
        Commands::Forge(forge) => match forge.action {
            ForgeAction::Repo(repo) => {
                run_repo(repo.action, &endpoint, &cred_store, cli.json).await
            }
            ForgeAction::Deploy(deploy) => {
                run_deploy(deploy.action, &api_base, token, cli.json).await
            }
        },
        Commands::Auth(auth) => run_auth(auth.action, &endpoint, &cred_store, cli.json).await,
        Commands::Issue(issue) => run_issue(issue.action, &api_base, token, cli.json).await,
        Commands::Project(project) => run_project(project.action, &api_base, token, cli.json).await,
        Commands::Milestone(milestone) => {
            run_milestone(milestone.action, &api_base, token, cli.json).await
        }
        Commands::Repo(repo) => run_repo(repo.action, &endpoint, &cred_store, cli.json).await,
        Commands::Coder(coder) => {
            // The session talks to the selected endpoint like every other
            // command. `--dev` names a server on this machine, and the global
            // `--api-url`/`--profile` still wins when both are given.
            let session_base = if cli.api_url.is_some() || cli.profile.is_some() {
                api_base.clone()
            } else if coder.dev {
                match crate::coder_dev::ensure_running().await {
                    Ok(api) => api.api_v1(),
                    Err(error) => fail(&error.to_string()),
                }
            } else {
                api_base.clone()
            };
            // Flags that name the same setting differently are refused before
            // anything runs. Every one of these is a combination where one
            // flag would have to be ignored, and a flag that is ignored is a
            // flag that lied.
            if let Err(reason) = coder.lane_name() {
                fail(&reason);
            }
            let resuming = coder.resume.is_some();
            if coder.offline && resuming {
                fail("--resume reads the thread from the server; it cannot combine with --offline");
            }
            if coder.offline && coder.named_a_lane() {
                fail(
                    "--offline answers from the built-in stand-in and reaches no model, \
                     so it cannot combine with --lane, --model or --local",
                );
            }
            if coder.offline && coder.reasoning.is_some() {
                fail(
                    "--offline opens no thread, and --reasoning is recorded on a thread. \
                     Drop one of them.",
                );
            }
            if (coder.last || coder.all) && !resuming {
                fail("--last and --all say which thread to continue, so they need --resume");
            }
            if resuming && coder.named_a_lane() {
                fail(
                    "a resumed thread answers on the model its own grant pins, \
                     so --resume cannot combine with --lane, --model or --local",
                );
            }
            if resuming && coder.reasoning.is_some() {
                fail(
                    "a resumed thread already carries the effort it was opened with, \
                     so --resume cannot combine with --reasoning",
                );
            }

            if coder.delegate {
                crate::delegate::run_delegation(
                    crate::delegate::DelegationRequest::from_coder(coder),
                    token,
                    cli.json,
                )
                .await?;
            } else if coder.offline {
                run_offline_coder(coder);
            } else {
                // What a thread is recorded against, and what `--resume`
                // filters the picker to. A directory that is not an OpenAgents
                // checkout has none, which is not an error: the thread is
                // simply not attributable to a repository.
                let repository = crate::repo::infer_repository(&endpoint.origin, None).ok();
                // `--resume` is settled before anything draws: the picker
                // prints to the normal screen, and a refusal has to be
                // readable rather than painted over by the full-screen
                // session and wiped on exit.
                let resumed = if let Some(named) = coder.resume.as_deref() {
                    let interactive = !coder.plain
                        && !cli.json
                        && std::io::IsTerminal::is_terminal(&std::io::stdin());
                    let request = crate::resume::ResumeRequest {
                        thread_id: Some(named).filter(|id| !id.is_empty()),
                        last: coder.last,
                        all: coder.all,
                        repository: repository.clone(),
                        interactive,
                    };
                    match crate::resume::resolve(&session_base, token.as_deref(), request).await {
                        Ok(Some(resumption)) => Some(resumption),
                        // An empty answer at the picker cancels, and cancelling
                        // is not a failure.
                        Ok(None) => return Ok(()),
                        Err(reason) => fail(&reason),
                    }
                } else {
                    None
                };
                if coder.headless {
                    run_headless_coder(coder, &session_base, token, repository, resumed).await?;
                } else {
                    crate::interactive::run_tui(coder, session_base, token, repository, resumed)
                        .await?;
                }
            }
        }
        Commands::Delegate(args) => {
            crate::delegate::run_delegation(
                crate::delegate::DelegationRequest::from_delegate(args),
                token,
                cli.json,
            )
            .await?;
        }
        Commands::Deploy(deploy) => run_deploy(deploy.action, &api_base, token, cli.json).await,
        Commands::Provider(provider) => run_provider(provider.action, cli.json),
        Commands::Box(b) => run_box(b.action, &api_base, token, cli.json).await,
        Commands::Computer(comp) => crate::computer::run(comp, &endpoint, cli.json).await,
        Commands::Gym(gym) => {
            or_fail(run_gym(gym.action, &api_base, token.clone(), cli.json).await)
        }
        Commands::Forum(forum) => {
            let client = crate::forum::ForumClient::new(&api_base, token);
            match forum.action {
                ForumAction::Boards => {
                    // A refusal ends the command. The version this replaces answered
                    // a non-2xx with two hardcoded boards, one of which the server
                    // has never served.
                    let boards = or_fail(client.list_boards().await);
                    let human: Vec<String> = if boards.is_empty() {
                        vec!["No boards found.".to_string()]
                    } else {
                        boards
                            .iter()
                            .map(|b| format!("{} — {} ({} topics)", b.slug, b.title, b.topic_count))
                            .collect()
                    };
                    let value = serde_json::json!({
                        "boards": boards
                            .iter()
                            .map(|b| serde_json::json!({
                                "slug": b.slug,
                                "title": b.title,
                                "topic_count": b.topic_count,
                            }))
                            .collect::<Vec<_>>()
                    });
                    emit(cli.json, &value, &human);
                }
                ForumAction::Topics { board, page } => {
                    let list = or_fail(client.list_topics(&board, page).await);
                    emit(
                        cli.json,
                        &crate::forum::topic_list_value(&list),
                        &crate::forum::topic_rows(&list),
                    );
                }
                ForumAction::Search { query, board, page } => {
                    if query.trim().is_empty() {
                        fail("Pass the words to search for.");
                    }
                    let list = or_fail(client.search_topics(&query, board.as_deref(), page).await);
                    emit(
                        cli.json,
                        &crate::forum::topic_list_value(&list),
                        &crate::forum::search_rows(&list),
                    );
                }
                ForumAction::Topic { id, page } => {
                    let topic = or_fail(client.read_topic(&id, page).await);
                    emit(
                        cli.json,
                        &crate::forum::topic_page_value(&topic),
                        &crate::forum::topic_page_rows(&topic),
                    );
                }
            }
        }
        Commands::Memory(mem) => run_memory(mem.action, &api_base, token, cli.json).await,
        Commands::Api(api) => crate::api_passthrough::run(api, &endpoint, cli.json).await,
        Commands::Plugin(plugin) => crate::plugins::run(plugin, cli.json).await,
        Commands::Trace(trace) => run_trace(trace.action, &api_base, token, cli.json).await,
        Commands::Swarm(swarm) => crate::swarm_args::run_swarm(swarm.action, cli.json).await,
        Commands::Update(update) => {
            let outcome = crate::update::run(
                update.channel,
                update.version,
                update.check,
                update.force,
                cli.json,
            )
            .await?;
            if cli.json {
                print_json(&outcome.document());
            }
        }
    }
    Ok(())
}

/// Refuse an input the CLI will not act on: exit 2, the usage status.
///
/// Exit code 2 is what the TypeScript CLI returns for an input or configuration
/// error, and the point of this whole path: a command that cannot reach its data
/// says so and exits non-zero rather than returning something plausible.
///
/// This used to be the *only* refusal path, so a 404, an expired token, and a
/// misspelled flag all left here with the same status. A failure the server
/// caused goes through [`or_fail`] instead, which classifies it. Reserve this
/// for what the caller typed.
pub(crate) fn fail(message: &str) -> ! {
    crate::errors::fail(&crate::errors::CliError::Input(message.to_string()))
}

/// Refuse with a class of the caller's choosing.
///
/// The escape hatch for failures that are neither an input error nor a server
/// refusal — a deployment that reached `failed`, a document that would not
/// render — so each reaches its own rung of the ladder.
pub(crate) fn fail_as(error: crate::errors::CliError) -> ! {
    crate::errors::fail(&error)
}

// ---------------------------------------------------------------------------
// auth
// ---------------------------------------------------------------------------

use crate::auth::{
    CredentialStore, DeviceClient, Endpoint, PendingDeviceAuthorization, PendingStore, Secret,
};

/// Unwrap or refuse. Every auth, repository, tracker, box, and memory path
/// funnels through this, so a store that could not be read, or a server that
/// answered with anything other than success, ends the command instead of
/// continuing with a value nobody provided.
///
/// That is the whole difference between reporting what the server said and
/// printing an empty list that reads as "there is nothing".
/// Unwrap, or report the failure on the rung of the ladder it belongs to.
///
/// The bound is `Into<CliError>` rather than `Display` on purpose. `Display`
/// let every failure in the crate reach this function and leave it as exit 2,
/// which is how an expired token, a missing repository, and a misspelled flag
/// came to be indistinguishable to a caller. A type that wants a rung of its
/// own declares it with a `From` impl in `crate::errors`; a type with no impl
/// is an input error, which is what it exited as before.
fn or_fail<T, E: Into<crate::errors::CliError>>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => crate::errors::fail(&error.into()),
    }
}

/// Read a token from standard input, keeping it out of the process table and
/// the shell history.
fn read_token_from_stdin() -> Secret {
    use std::io::{BufRead, IsTerminal};
    if std::io::stdin().is_terminal() {
        fail(
            "refusing to read the token from a terminal — pipe it in instead, \
             e.g. `op read ... | openagents auth login --token-stdin`",
        );
    }
    let mut buffer = String::new();
    if std::io::stdin().lock().read_line(&mut buffer).is_err() {
        fail("could not read a token from standard input");
    }
    let trimmed = buffer.trim().to_string();
    if trimmed.is_empty() {
        fail("standard input carried no token");
    }
    Secret::new(trimmed)
}

async fn run_auth(action: AuthAction, endpoint: &Endpoint, store: &CredentialStore, json: bool) {
    match action {
        AuthAction::Login {
            token_stdin,
            headless,
            resume,
            scope,
        } => run_auth_login(endpoint, store, token_stdin, headless, resume, &scope, json).await,
        AuthAction::TokenStdin => {
            let token = read_token_from_stdin();
            let source = or_fail(store.store(&token));
            if json {
                print_json(&serde_json::json!({
                    "origin": endpoint.origin,
                    "stored": true,
                    "token_source": source.label(),
                }));
            } else {
                println!("Stored an OpenAgents token for {}.", endpoint.origin);
            }
        }
        AuthAction::Status => run_auth_status(endpoint, store, json).await,
        AuthAction::Logout => {
            let removed = or_fail(store.remove());
            if json {
                print_json(&serde_json::json!({
                    "origin": endpoint.origin,
                    "removed": removed,
                }));
            } else if removed {
                println!(
                    "Removed the stored OpenAgents token for {}.",
                    endpoint.origin
                );
            } else {
                println!("No OpenAgents token was stored for {}.", endpoint.origin);
            }
        }
        AuthAction::SetupGit { local, global, yes } => {
            if local == global {
                fail("choose exactly one of --local or --global");
            }
            if global && !yes {
                fail("global setup requires --yes confirmation");
            }
            let scope = if local { "local" } else { "global" };
            or_fail(crate::repo::configure_credential_helper(
                &endpoint.origin,
                scope,
                None,
            ));
            if json {
                print_json(&serde_json::json!({
                    "origin": endpoint.origin,
                    "scope": scope,
                    "configured": true,
                }));
            } else {
                println!(
                    "Configured the {scope} git credential helper for {}.",
                    endpoint.origin
                );
            }
        }
        AuthAction::GitCredential { operation } => {
            let input = or_fail(crate::repo::read_credential_stdin());
            let answer = or_fail(crate::repo::run_git_credential_helper(
                &endpoint.origin,
                &operation,
                &input,
                store,
            ));
            print!("{answer}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_auth_login(
    endpoint: &Endpoint,
    store: &CredentialStore,
    token_stdin: bool,
    headless: bool,
    resume: bool,
    scope: &[String],
    json: bool,
) {
    if [token_stdin, headless, resume]
        .iter()
        .filter(|f| **f)
        .count()
        > 1
    {
        fail("use only one of --token-stdin, --headless, or --resume");
    }

    let announce = |source: &str| {
        if json {
            print_json(&serde_json::json!({
                "origin": endpoint.origin,
                "authenticated": true,
                "token_source": source,
            }));
        } else {
            println!("Authenticated with {}.", endpoint.origin);
            println!("The token is stored in ~/.openagents/credentials.json.");
            println!("Run oa auth setup-git --local to configure git for this repository.");
        }
    };

    if token_stdin {
        let token = read_token_from_stdin();
        or_fail(store.store(&token));
        announce("token_stdin");
        return;
    }

    let pending_store = PendingStore::new();
    let devices = DeviceClient::new(&endpoint.origin);

    if resume {
        let pending = match or_fail(pending_store.get(&endpoint.origin)) {
            Some(pending) => pending,
            None => fail(&format!(
                "no pending authorization exists for {}. Run {} first",
                endpoint.origin,
                crate::auth::login_command_for(endpoint)
            )),
        };
        let remaining = (pending.expires_at_ms - crate::auth::now_ms()) / 1_000;
        if remaining <= 0 {
            let _ = pending_store.remove(&endpoint.origin);
            fail(&format!(
                "the pending authorization expired. Run {} again",
                crate::auth::login_command_for(endpoint)
            ));
        }
        let authorization = crate::auth::DeviceAuthorization {
            device_code: pending.device_code.clone(),
            user_code: pending.user_code.clone(),
            verification_uri: pending.verification_uri.clone(),
            verification_uri_complete: pending.verification_uri_complete.clone(),
            expires_in: remaining,
            interval: pending.interval,
            // The pending record predates the scope field and holds no scope.
            // The authorization the server already opened decides it, and this
            // path only waits for that one to be approved.
            scope: None,
        };
        let token = or_fail(devices.wait(&authorization).await);
        or_fail(store.store(&token));
        or_fail(pending_store.remove(&endpoint.origin));
        announce("device_authorization");
        return;
    }

    let authorization = or_fail(devices.start(scope).await);

    // A session with no terminal cannot wait for a person to click, and a
    // session asked for JSON cannot interleave a wait with its single object.
    // Both hand the approval back and record it for `--resume`.
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stderr());
    if headless || json || !interactive {
        let resume_command = crate::auth::resume_command_for(endpoint);
        or_fail(pending_store.set(&PendingDeviceAuthorization {
            origin: endpoint.origin.clone(),
            device_code: authorization.device_code.clone(),
            user_code: authorization.user_code.clone(),
            verification_uri: authorization.verification_uri.clone(),
            verification_uri_complete: authorization.verification_uri_complete.clone(),
            expires_at_ms: crate::auth::now_ms() + authorization.expires_in * 1_000,
            interval: authorization.interval,
            kind: Some("device".to_string()),
        }));
        if json {
            print_json(&serde_json::json!({
                "origin": endpoint.origin,
                "authenticated": false,
                "authorization_pending": true,
                "verification_url": authorization.verification_uri_complete,
                "user_code": authorization.user_code,
                "expires_in": authorization.expires_in,
                "scope": authorization.scope,
                "resume_command": resume_command,
            }));
        } else {
            println!("OpenAgents authorization is ready.");
            println!("Open this URL: {}", authorization.verification_uri_complete);
            println!("Authorization code: {}", authorization.user_code);
            // What the approval page will ask the reader to grant. The server
            // settles this: `--scope` asks and the deployment decides.
            if let Some(scope) = authorization.scope.as_deref().filter(|s| !s.is_empty()) {
                println!("Scope requested: {scope}");
            }
            println!("After you approve the request, run: {resume_command}");
        }
        return;
    }

    eprintln!(
        "OpenAgents authorization URL: {}",
        authorization.verification_uri_complete
    );
    eprintln!("OpenAgents authorization code: {}", authorization.user_code);
    if let Some(scope) = authorization.scope.as_deref().filter(|s| !s.is_empty()) {
        eprintln!("Scope requested: {scope}");
    }
    if !crate::auth::open_browser(&authorization.verification_uri_complete) {
        eprintln!("The browser did not open. Open the authorization URL above.");
    }
    eprintln!("Waiting for approval...");
    let token = or_fail(devices.wait(&authorization).await);
    or_fail(store.store(&token));
    announce("device_authorization");
}

async fn run_auth_status(endpoint: &Endpoint, store: &CredentialStore, json: bool) {
    let held = or_fail(store.find_token());
    let (local_helper, global_helper) =
        crate::repo::credential_helper_state(&endpoint.origin, None);

    let Some(held) = held else {
        if json {
            print_json(&serde_json::json!({
                "origin": endpoint.origin,
                "profile": endpoint.profile,
                "authenticated": false,
                "token_source": serde_json::Value::Null,
                "account": serde_json::Value::Null,
                "namespaces": [],
                "token_expires_at": serde_json::Value::Null,
                "git_helper": { "local": local_helper, "global": global_helper },
            }));
        } else {
            println!("API: {}", endpoint.origin);
            println!("No token is available.");
            println!(
                "Set OPENAGENTS_TOKEN or run {}.",
                crate::auth::login_command_for(endpoint)
            );
        }
        return;
    };

    // The token is only evidence that something is stored. Whether it still
    // authenticates anyone is a question only the server can answer, so ask it.
    // A revoked token reports as revoked here rather than at some later command.
    let client = crate::repo::RepoClient::new(&endpoint.origin, Some(held.token.clone()));
    let user = or_fail(client.authenticated_user().await);

    if json {
        print_json(&serde_json::json!({
            "origin": endpoint.origin,
            "profile": endpoint.profile,
            "authenticated": true,
            "token_source": held.source.label(),
            "account": { "id": user.id, "login": user.login },
            "namespaces": user.namespaces,
            "token_expires_at": user.token_expires_at,
            "git_helper": { "local": local_helper, "global": global_helper },
        }));
    } else {
        println!("API: {}", endpoint.origin);
        println!(
            "Authenticated as {} ({}) with a {} token.",
            user.login,
            user.id,
            held.source.label()
        );
        println!(
            "Eligible namespaces: {}.",
            user.namespaces
                .iter()
                .map(|namespace| namespace.login.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("Token expires: {}.", user.token_expires_at);
        println!(
            "Git helper: local {}; global {}.",
            if local_helper {
                "configured"
            } else {
                "not configured"
            },
            if global_helper {
                "configured"
            } else {
                "not configured"
            }
        );
    }
}

// ---------------------------------------------------------------------------
// repo
// ---------------------------------------------------------------------------

/// One `--json` document, on one line. See [`emit`] for why it is not
/// pretty-printed.
fn print_json(value: &serde_json::Value) {
    match serde_json::to_string(value) {
        Ok(text) => println!("{text}"),
        Err(error) => fail_as(crate::errors::CliError::Output(format!(
            "could not render JSON output: {error}"
        ))),
    }
}

/// The token every repository command needs, or a refusal naming the login.
fn require_token(endpoint: &Endpoint, store: &CredentialStore) -> Secret {
    match or_fail(store.find_token()) {
        Some(held) => held.token,
        None => fail(&format!(
            "no OpenAgents token for {}. Set OPENAGENTS_TOKEN or run {}",
            endpoint.origin,
            crate::auth::login_command_for(endpoint)
        )),
    }
}

/// Resolve `OWNER/REPO` from the positional argument, `--repo`, or this
/// checkout's git remotes. When none of the three answers, the command refuses
/// rather than picking a repository nobody named.
fn resolve_repository(
    positional: Option<String>,
    override_flag: Option<String>,
    origin: &str,
) -> (String, String) {
    if positional.is_some() && override_flag.is_some() {
        fail("pass a repository argument or --repo, not both");
    }
    let selected = match override_flag.or(positional) {
        Some(value) => value,
        None => or_fail(crate::repo::infer_repository(origin, None)),
    };
    or_fail(crate::repo::parse_repository_target(&selected))
}

fn visibility(public: bool, private: bool) -> Option<bool> {
    if public && private {
        fail("use either --public or --private, not both");
    }
    if public {
        return Some(false);
    }
    if private {
        return Some(true);
    }
    None
}

async fn run_repo(action: RepoAction, endpoint: &Endpoint, store: &CredentialStore, json: bool) {
    let token = require_token(endpoint, store);
    let client = crate::repo::RepoClient::new(&endpoint.origin, Some(token));

    match action {
        RepoAction::List {
            namespace,
            limit,
            after,
        } => {
            let listed = or_fail(
                client
                    .list(namespace.as_deref(), limit, after.as_deref())
                    .await,
            );
            if json {
                print_json(&serde_json::json!({
                    "repositories": listed.repositories,
                    "next_cursor": listed.next_cursor,
                }));
            } else if listed.repositories.is_empty() {
                println!("No repositories found.");
            } else {
                // The slug alone, as the TypeScript CLI prints it. This output
                // is piped, and a trailing `\t(branch: main)` makes every
                // consumer of it cut a field off first.
                for repository in &listed.repositories {
                    println!("{}", repository.full_name);
                }
                if let Some(cursor) = listed.next_cursor {
                    println!("Next cursor: {cursor}");
                }
            }
        }
        RepoAction::View { repository, repo } => {
            let (owner, name) = resolve_repository(repository, repo, &endpoint.origin);
            let value = or_fail(client.view(&owner, &name).await);
            if json {
                print_json(&serde_json::to_value(&value).unwrap_or(serde_json::Value::Null));
            } else {
                for line in value.human_lines() {
                    println!("{line}");
                }
            }
        }
        RepoAction::Create {
            name,
            description,
            public,
            private,
            default_branch,
            wait_timeout,
            source,
            remote,
        } => {
            // Both checked before the repository is created, not after: a
            // refusal that has already made a repository on the server is not
            // a refusal.
            //
            // `--remote` names the remote `--source` attaches; on its own it
            // has nothing to name. The TypeScript CLI ignores it silently,
            // which leaves a reader believing they configured something.
            if remote.is_some() && source.is_none() {
                fail("--remote names the remote --source attaches. Give --source too");
            }
            if let Some(remote) = remote.as_deref() {
                or_fail(crate::repo::validate_remote_name(remote));
            }
            // A `--source` that is not a worktree cannot be attached, and
            // finding that out after the create leaves a repository on the
            // server that the reader did not get told how to push to.
            if let Some(directory) = source.as_deref() {
                or_fail(crate::repo::require_worktree(std::path::Path::new(
                    directory,
                )));
            }

            // Naming neither flag used to create a *public* repository, a
            // default disclosed only in `--public`'s own help text. Publishing
            // a repository is not undoable by the reader who did not know they
            // asked for it, so the omission is refused here — before the
            // create, like the two checks above — rather than resolved in the
            // direction that exposes it.
            let Some(is_private) = visibility(public, private) else {
                fail(
                    "say whether the repository is public or private: \
                     `oa repo create <name> --private` or `--public`",
                );
            };
            let (owner, repository_name) = if name.contains('/') {
                let (owner, repository_name) = or_fail(crate::repo::parse_repository_target(&name));
                (Some(owner), repository_name)
            } else {
                (None, name.clone())
            };
            let created = or_fail(
                client
                    .create(
                        owner.as_deref(),
                        &repository_name,
                        is_private,
                        description.as_deref(),
                        &default_branch,
                        std::time::Duration::from_secs(wait_timeout),
                    )
                    .await,
            );
            // A repository still provisioning has no clone URL to attach yet,
            // so `--source` is reported as not done rather than done wrong.
            let attached = match source.as_deref() {
                Some(directory) if created.lifecycle_state == "ready" => {
                    let (_, clone_url) =
                        or_fail(client.clone_info(&created.owner.login, &created.name).await);
                    let path = std::path::PathBuf::from(directory);
                    let remote = remote.as_deref().unwrap_or("origin");
                    Some((
                        or_fail(crate::repo::attach_remote(
                            &endpoint.origin,
                            &clone_url,
                            &path,
                            remote,
                        )),
                        path,
                    ))
                }
                _ => None,
            };

            if json {
                let mut value = serde_json::to_value(&created).unwrap_or(serde_json::Value::Null);
                if let Some((attached, path)) = &attached {
                    value = serde_json::json!({
                        "repository": value,
                        "remote": attached.remote,
                        "next_push": attached.next_push_argv(path),
                    });
                }
                print_json(&value);
            } else {
                println!("Repository created.");
                for line in created.human_lines() {
                    println!("{line}");
                }
                match (&attached, source.as_deref()) {
                    (Some((attached, path)), Some(directory)) => {
                        println!("Configured remote {} in {directory}.", attached.remote);
                        println!("Next: {}", attached.next_push_command(path));
                    }
                    (None, Some(_)) => println!(
                        "The repository is still provisioning, so the CLI did not configure a remote."
                    ),
                    _ => {}
                }
            }
        }
        RepoAction::Import {
            source,
            name,
            namespace,
            public,
            private,
            wait_timeout,
        } => {
            let is_private = visibility(public, private);
            let (source_owner, source_repo) =
                or_fail(crate::repo::parse_repository_target(&source));
            let destination = namespace.clone().unwrap_or_else(|| source_owner.clone());
            if !destination.eq_ignore_ascii_case(&source_owner) {
                fail("--namespace must match the GitHub source owner");
            }
            // Eligibility is the server's fact, so read it rather than assume it.
            let user = or_fail(client.authenticated_user().await);
            let personal = destination.eq_ignore_ascii_case(&user.login);
            if !personal
                && !user.namespaces.iter().any(|candidate| {
                    candidate.r#type == "organization"
                        && candidate.login.eq_ignore_ascii_case(&destination)
                })
            {
                fail(&format!(
                    "{destination} is not an eligible GitHub namespace for this account"
                ));
            }
            let (repository, repository_import) = or_fail(
                client
                    .import(
                        if personal {
                            None
                        } else {
                            Some(destination.as_str())
                        },
                        &format!("{source_owner}/{source_repo}"),
                        name.as_deref(),
                        is_private,
                        std::time::Duration::from_secs(wait_timeout),
                    )
                    .await,
            );
            if json {
                print_json(&serde_json::json!({
                    "repository": repository,
                    "import": repository_import,
                }));
            } else {
                println!(
                    "Imported {source_owner}/{source_repo} into {}.",
                    repository.full_name
                );
                println!("Import state: {}", repository_import.state);
                println!("This is a one-time import. Later GitHub changes do not sync.");
            }
        }
        RepoAction::Clone {
            repository,
            directory,
            repo,
        } => {
            let (owner, name) = resolve_repository(repository, repo, &endpoint.origin);
            let (value, clone_url) = or_fail(client.clone_info(&owner, &name).await);
            or_fail(crate::repo::git_clone(&clone_url, directory.as_deref()).await);
            if json {
                print_json(&serde_json::json!({
                    "repository": value,
                    "clone_url": clone_url,
                    "cloned": true,
                }));
            } else {
                println!("Cloned {}.", value.full_name);
            }
        }
        RepoAction::Delete {
            repository,
            repo,
            yes,
        } => {
            if !yes {
                fail("repository deletion requires --yes confirmation");
            }
            let (owner, name) = resolve_repository(repository, repo, &endpoint.origin);
            or_fail(client.remove(&owner, &name).await);
            if json {
                print_json(&serde_json::json!({
                    "full_name": format!("{owner}/{name}"),
                    "deleted": true,
                }));
            } else {
                println!("Deleted {owner}/{name}.");
            }
        }
    }
}

fn home_directory() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

// ---------------------------------------------------------------------------
// tracker: issues, projects, milestones
// ---------------------------------------------------------------------------

/// Print the server's body verbatim under `--json`, or the human lines.
///
/// One line, not pretty-printed. The TypeScript CLI stringifies compactly
/// (`output.ts`), so a consumer reading `oa … --json` in a loop gets one
/// document per line; pretty-printing spread each document over dozens of
/// lines and broke every NDJSON reader that worked against `openagents`.
pub(crate) fn emit(json: bool, value: &serde_json::Value, human: &[String]) {
    if json {
        match serde_json::to_string(value) {
            Ok(text) => println!("{}", text),
            Err(error) => fail_as(crate::errors::CliError::Output(format!(
                "Could not render JSON: {}",
                error
            ))),
        }
    } else {
        for line in human {
            println!("{}", line);
        }
    }
}

/// A serializable value with a `schema` field in front of it.
///
/// The TypeScript commands publish `{ schema: "…", ...result }`, which is a
/// spread. Rust has no spread, so the fields are merged here rather than
/// restated field by field at each call — restating them is how `forum search
/// --json` came to drop five of them.
fn schema_document<T: serde::Serialize>(schema: &str, value: &T) -> serde_json::Value {
    let mut document = serde_json::Map::new();
    document.insert("schema".to_string(), schema.into());
    if let Ok(serde_json::Value::Object(fields)) = serde_json::to_value(value) {
        document.extend(fields);
    }
    serde_json::Value::Object(document)
}

fn trace_summary_document(summary: &crate::trace::TraceSummary) -> serde_json::Value {
    schema_document("openagents.trace_summary.v1", summary)
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
    let extension = issue
        .get("openagents")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
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
    let extension = issue
        .get("openagents")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let milestone = issue
        .get("milestone")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
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
        format!(
            "Labels:     {}",
            or_none(&names(issue.get("labels"), "name"))
        ),
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
        format!("Progress:   {}", {
            let progress = field(&extension, "progress");
            if progress.is_empty() {
                "unknown".to_string()
            } else {
                progress
            }
        }),
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
        format!(
            "Blocks:     {}",
            or_none(&issue_references(extension.get("blocks")))
        ),
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
///
/// Stdin reads are guarded so a caller that can never write stdin hangs the
/// CLI no more (#178): refuse a TTY, where paste-until-EOF is ambiguous and
/// Ctrl-D is folklore; refuse an empty EOF instead of creating an empty body;
/// cap the read at the same maximum body size a file path is subject to. A
/// piped `printf '%s' "$body" | openagents ...` still works, since it closes
/// stdin with content.
fn resolve_body(body: Option<String>, body_file: Option<String>) -> Option<String> {
    // Match the tool-output cap: an unbounded stdin read is the #178 hang
    // wearing a different hat.
    const MAX_BODY_BYTES: usize = crate::tools::OUTPUT_LIMIT;

    match (body, body_file) {
        (Some(_), Some(_)) => fail("Use either --body or --body-file, not both."),
        (Some(text), None) => Some(text),
        (None, Some(path)) => {
            if path == "-" {
                use std::io::IsTerminal;
                if std::io::stdin().is_terminal() {
                    fail(
                        "refusing to read the body from a terminal — pass a file path \
                         (`--body-file <file>`) or pipe the content in, e.g. \
                         `printf '%s' \"$(cat body.md)\" | openagents ...`",
                    );
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                if let Err(error) = std::io::stdin()
                    .lock()
                    .take((MAX_BODY_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                {
                    fail(&format!(
                        "Could not read the body from standard input: {}",
                        error
                    ));
                }
                if bytes.len() > MAX_BODY_BYTES {
                    fail(&format!(
                        "the body read from standard input exceeds the {}-byte maximum",
                        MAX_BODY_BYTES
                    ));
                }
                let buffer = String::from_utf8(bytes)
                    .map_err(|_| "the body read from standard input is not valid UTF-8".to_string())
                    .unwrap_or_else(|error| fail(&error));
                if buffer.trim().is_empty() {
                    fail(
                        "the body read from stdin is empty — pass `--body-file <file>` \
                         or pipe the content in",
                    );
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

async fn run_issue(action: IssueAction, api_base: &str, token: Option<String>, json: bool) {
    let tracker = crate::tracker::TrackerClient::new(api_base, token);
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
                human.push(
                    match result.pagination.get("total").and_then(|t| t.as_u64()) {
                        Some(total) => {
                            format!("Showing {} of {} issues.", result.issues.len(), total)
                        }
                        None => format!("Showing {} issues.", result.issues.len()),
                    },
                );
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
            // The body is resolved before the repository target: a stdin read
            // must be refused — and must be refused with the stdin guidance —
            // before anything else about the invocation is judged (#178). The
            // pty test spawns with no repository and expects the guard's
            // refusal, not a repo error.
            let text = resolve_body(body, body_file);
            let target = target_or_fail(repo);
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
            // Same order as Create: the stdin guard fires before the
            // repository target is judged (#178).
            let text = resolve_body(body, body_file);
            let target = target_or_fail(repo);
            match text {
                Some(text) => {
                    let comment = or_fail(tracker.comment_issue(&target, number, &text).await);
                    emit(json, &comment, &[format!("Commented on #{}.", number)]);
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
                value = Some(or_fail(
                    tracker.add_dependencies(&target, number, &add).await,
                ));
            }
            for blocked_by in &remove {
                value = Some(or_fail(
                    tracker
                        .remove_dependency(&target, number, *blocked_by)
                        .await,
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
            emit(json, &value, &milestone_listing(&value));
        }
        IssueAction::Milestone {
            number,
            set,
            clear,
            repo,
        } => {
            // Two ways to say what the milestone should become, and they
            // disagree. Guessing which one was meant is how an issue ends up on
            // a milestone the caller was trying to take it off.
            let milestone = match (set, clear) {
                (Some(_), true) => {
                    fail("Use either --set or --clear, not both.");
                }
                (None, false) => {
                    fail(
                        "Say what the milestone should become: --set <number> to put the issue on one, or --clear to take it off.",
                    );
                }
                (Some(number), false) => Some(number),
                (None, true) => None,
            };
            let target = target_or_fail(repo);
            let value = or_fail(
                tracker
                    .set_issue_milestone(&target, number, milestone)
                    .await,
            );
            // Report what came BACK, not what was asked for. A server that
            // accepted the request and stored something else is the case a
            // printed echo of the argument would hide.
            let stored = value
                .get("milestone")
                .filter(|milestone| !milestone.is_null());
            let human = match stored {
                Some(m) => format!(
                    "Issue #{} is on milestone #{} {}",
                    number,
                    number_or_question(m, "number"),
                    field(m, "title")
                ),
                None => format!("Issue #{} is on no milestone.", number),
            };
            emit(json, &value, &[human]);
        }
    }
}

/// The rows `milestone list` and `issue milestones` both print.
///
/// One renderer, so the two entry points cannot describe the same milestone
/// differently.
fn milestone_listing(value: &serde_json::Value) -> Vec<String> {
    let rows = value
        .get("milestones")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    if rows.is_empty() {
        return vec!["No milestones found.".to_string()];
    }
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
}

/// `oa milestone`: the write half the API has always had and neither CLI reached.
///
/// `create` and `delete` existed on the client and were wired to nothing, so a
/// milestone could only be opened or removed in a browser -- which made
/// milestones useless to agents, and agents file most of the issues here.
async fn run_milestone(action: MilestoneAction, api_base: &str, token: Option<String>, json: bool) {
    let tracker = crate::tracker::TrackerClient::new(api_base, token);
    match action {
        MilestoneAction::List { repo } => {
            let target = target_or_fail(repo);
            let value = or_fail(tracker.list_milestones(&target).await);
            emit(json, &value, &milestone_listing(&value));
        }
        MilestoneAction::Create {
            title,
            description,
            due_on,
            repo,
        } => {
            let target = target_or_fail(repo);
            let value = or_fail(
                tracker
                    .create_milestone(&target, &title, description.as_deref(), due_on.as_deref())
                    .await,
            );
            // The server assigns the number. Printing the one it returned is
            // the only way the caller learns what to pass to `--set`.
            let human = format!(
                "Opened milestone #{} {}",
                number_or_question(&value, "number"),
                field(&value, "title")
            );
            emit(json, &value, &[human]);
        }
        MilestoneAction::Delete { number, repo } => {
            let target = target_or_fail(repo);
            // A 204 carries no body, so there is nothing to report back but the
            // number that was asked for and the fact that the server accepted it.
            let value = or_fail(tracker.delete_milestone(&target, number).await);
            emit(json, &value, &[format!("Deleted milestone #{}.", number)]);
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
            let issue = item
                .get("issue")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
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

async fn run_project(action: ProjectAction, api_base: &str, token: Option<String>, json: bool) {
    let tracker = crate::tracker::TrackerClient::new(api_base, token);
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
                    if owner.is_empty() {
                        "unknown".to_string()
                    } else {
                        owner
                    }
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
        ProjectAction::Edit {
            number,
            title,
            description,
            state,
            archive,
            unarchive,
            repo,
        } => {
            if archive && unarchive {
                fail("Pass either --archive or --unarchive, not both.");
            }
            if let Some(value) = state.as_deref() {
                if value != "open" && value != "closed" {
                    fail("--state accepts open or closed.");
                }
            }
            let archived = if archive {
                Some(true)
            } else if unarchive {
                Some(false)
            } else {
                None
            };
            if title.is_none() && description.is_none() && state.is_none() && archived.is_none() {
                fail("Pass --title, --description, --state, --archive, or --unarchive.");
            }
            let target = target_or_fail(repo);
            let project = or_fail(
                tracker
                    .edit_project(
                        &target,
                        number,
                        title.as_deref(),
                        description.as_deref(),
                        state.as_deref(),
                        archived,
                    )
                    .await,
            );
            emit(
                json,
                &project,
                &[format!(
                    "Updated project #{} {}",
                    number_or_question(&project, "number"),
                    field(&project, "title")
                )],
            );
        }
        ProjectAction::Delete { number, yes, repo } => {
            if !yes {
                fail("Project deletion requires --yes confirmation.");
            }
            let target = target_or_fail(repo);
            or_fail(tracker.delete_project(&target, number).await);
            emit(
                json,
                &serde_json::json!({ "number": number, "deleted": true }),
                &[format!("Deleted project #{number}.")],
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
        ProjectAction::ItemRemove { number, item, repo } => {
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
            if r.timed_out == Some(true) {
                "yes"
            } else {
                "no"
            }
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

async fn run_box(action: BoxAction, api_base: &str, token: Option<String>, json: bool) {
    let client = crate::box_client::BoxClient::new(api_base, token);
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
            emit(
                json,
                &serde_json::json!({ "box": to_value(&record) }),
                &human,
            );
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
                    let Some(count) = count else {
                        fail(
                            "pass --count <n> to request a fanout, or --request-id <id> to read an existing plan",
                        );
                    };
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

async fn run_box_runs(action: BoxRunAction, client: &crate::box_client::BoxClient, json: bool) {
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

async fn run_memory(action: MemoryAction, api_base: &str, token: Option<String>, json: bool) {
    let client = crate::memory_client::MemoryClient::new(api_base, token);
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
///
/// It also carries the protection line. Whether the seed on this machine is
/// encrypted or is readable text is not something a person can infer from the
/// path, and the plaintext fallback is only honest if the surface that shows an
/// identity says so every time.

// ---------------------------------------------------------------------------
// trace
// ---------------------------------------------------------------------------

async fn run_trace(action: TraceAction, api_base: &str, token: Option<String>, json: bool) {
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

            let mut human: Vec<String> = Vec::new();
            for scan in &scans {
                if !scan.present {
                    human.push(format!(
                        "{}: {} (not present)",
                        scan.kind.as_str(),
                        scan.root.display()
                    ));
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
                human.push(line);
            }

            if candidates.is_empty() {
                human.push("No trace files found.".to_string());
            }
            for candidate in &candidates {
                human.push(format!(
                    "{}  {}  {}B  {}",
                    candidate.kind.as_str(),
                    candidate.modified_at,
                    candidate.bytes,
                    candidate.path.display()
                ));
            }

            // The document the TypeScript CLI publishes for this command
            // (`trace-command.ts:151`): the same schema name and the same two
            // arrays, so a consumer that reads one reads the other.
            emit(
                json,
                &serde_json::json!({
                    "schema": "openagents.trace_list.v1",
                    "stores": scans,
                    "traces": candidates,
                }),
                &human,
            );
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

            let mut human: Vec<String> = vec![format!("File: {}", summary.path.display())];
            if summary.format != "atif" {
                let described = if summary.format == "jsonl" {
                    "line-delimited session log (not ATIF)"
                } else {
                    "unknown"
                };
                human.push(format!("Format: {}", described));
                human.push(format!("Size: {} bytes", summary.bytes));
                if let Some(lines) = summary.lines {
                    human.push(format!("Lines: {}", lines));
                }
                human.push(
                    "This slice summarizes ATIF documents only; foreign logs get metadata."
                        .to_string(),
                );
                emit(json, &trace_summary_document(&summary), &human);
                return;
            }

            human.push(format!(
                "Schema: {}",
                summary
                    .schema_version
                    .as_deref()
                    .unwrap_or("(missing schema_version)")
            ));
            if let Some(session) = &summary.session_id {
                human.push(format!("Session: {}", session));
            }
            if summary.agent_name.is_some() || summary.agent_model.is_some() {
                human.push(format!(
                    "Agent: {} ({})",
                    summary.agent_name.as_deref().unwrap_or("unknown"),
                    summary.agent_model.as_deref().unwrap_or("unknown model")
                ));
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
            human.push(format!(
                "Steps: {} ({})",
                summary.steps.unwrap_or(0),
                sources
            ));
            let models = summary.models.clone().unwrap_or_default();
            human.push(format!(
                "Models: {}",
                if models.is_empty() {
                    "(none recorded)".to_string()
                } else {
                    models.join(", ")
                }
            ));
            human.push(format!("Tool calls: {}", summary.tool_calls.unwrap_or(0)));
            human.push(
                match (summary.total_prompt_tokens, summary.total_completion_tokens) {
                    (None, None) => "Tokens: not recorded".to_string(),
                    (prompt, completion) => format!(
                        "Tokens: {} prompt, {} completion",
                        prompt.unwrap_or(0),
                        completion.unwrap_or(0)
                    ),
                },
            );
            if let (Some(first), Some(last)) = (&summary.first_timestamp, &summary.last_timestamp) {
                human.push(format!("Span: {} to {}", first, last));
            }
            emit(json, &trace_summary_document(&summary), &human);
        }
        TraceAction::Redact {
            trace: argument,
            file,
        } => {
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

            let mut human: Vec<String> = vec![format!("Wrote {}", result.output.display())];
            if result.total == 0 {
                human.push("Nothing matched the redaction rules.".to_string());
            } else {
                // Counts per category, never the matched text.
                let detail = result
                    .counts
                    .iter()
                    .map(|(category, count)| format!("{} {}", category, count))
                    .collect::<Vec<_>>()
                    .join(", ");
                human.push(format!(
                    "Redacted {} match{}: {}",
                    result.total,
                    if result.total == 1 { "" } else { "es" },
                    detail
                ));
            }
            if result.valid_json == Some(false) {
                human.push(
                    "Warning: the redacted copy no longer parses as JSON; review it before sharing."
                        .to_string(),
                );
            }
            emit(
                json,
                &schema_document("openagents.trace_redaction.v1", &result),
                &human,
            );
        }
        TraceAction::Upload {
            trace: argument,
            visibility,
            assignment,
        } => {
            // Everything checkable against the file is checked before anything
            // leaves this machine, so a refusal names the file rather than
            // arriving as a status the caller then has to interpret.
            let visibility = or_fail(crate::trace_client::read_visibility(&visibility));
            let path = trace::resolve_trace_argument(&argument, &home)
                .unwrap_or_else(|message| fail(&message));

            let size = std::fs::metadata(&path)
                .map(|meta| meta.len())
                .unwrap_or_else(|error| {
                    fail(&format!(
                        "The trace file at {} could not be read: {}",
                        path.display(),
                        error
                    ))
                });
            if size > crate::trace_client::MAXIMUM_TRACE_BYTES {
                fail(&format!(
                    "{} is {} bytes; the ingest route accepts at most {}. Upload a redacted or trimmed copy instead.",
                    path.display(),
                    size,
                    crate::trace_client::MAXIMUM_TRACE_BYTES
                ));
            }

            let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                fail(&format!(
                    "The trace file at {} could not be read: {}",
                    path.display(),
                    error
                ))
            });
            let document: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| {
                fail(&format!(
                    "{} is not JSON. The ingest route takes one ATIF document; a line-delimited session log has to be converted first.",
                    path.display()
                ))
            });
            if !document.is_object() {
                fail(&format!(
                    "{} is JSON but not an object, so it is not an ATIF document.",
                    path.display()
                ));
            }
            // The server decides which schema versions it accepts. This only
            // catches a file that names none at all, which it can say something
            // more useful about the file than a 422 can.
            if document
                .get("schema_version")
                .and_then(|v| v.as_str())
                .is_none()
            {
                fail(&format!(
                    "{} carries no schema_version, so it is not an ATIF document. `oa trace show {}` reports what it is.",
                    path.display(),
                    argument
                ));
            }

            let client = crate::trace_client::TraceClient::new(api_base, token);
            let stored = or_fail(
                client
                    .upload(&document, visibility, assignment.as_deref())
                    .await,
            );

            let value = serde_json::json!({
                "schema": "openagents.trace_upload.v1",
                "input": path,
                "id": stored.id,
                "digest": stored.digest,
                "byte_size": stored.byte_size,
                "visibility": stored.visibility,
                "inserted_at": stored.inserted_at,
                "created": stored.created,
            });
            let human = vec![
                // A 200 means the server already held this digest. Calling that
                // an upload would report a write that did not happen.
                if stored.created {
                    format!("Uploaded {}", path.display())
                } else {
                    "Already stored: the server holds this trace under the same digest.".to_string()
                },
                format!("Trace: {}", stored.id),
                format!("Digest: {}", stored.digest),
                format!(
                    "Stored: {} bytes at visibility {}",
                    stored.byte_size, stored.visibility
                ),
                // No link. The response carries a url pointing at
                // GET /api/v1/traces/:id, and that route does not exist, so
                // printing it would hand the reader a 404 dressed as a receipt.
            ];
            emit(json, &value, &human);
        }
    }
}

// ---------------------------------------------------------------------------
// deploy
// ---------------------------------------------------------------------------

/// `oa deploy`.
///
/// Every path here is a real call to `/api/v1/admin/forge/targets`. A refusal
/// is the server's own, carried through with the command that obtains the
/// privileged scope appended; nothing here invents a target, a state, or a
/// list.
async fn run_deploy(action: DeployAction, api_base: &str, token: Option<String>, json: bool) {
    use crate::fleet;
    let client = fleet::FleetClient::new(api_base, token);

    match action {
        DeployAction::List { repo, limit } => {
            if let Some(limit) = limit {
                if !(1..=50).contains(&limit) {
                    fail("--limit must be between 1 and 50.");
                }
            }
            let value = or_fail(client.list(repo.as_deref(), limit).await);
            let targets = value
                .get("targets")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            // The TypeScript CLI leads with the repository the targets belong
            // to. Without it a list of bare SHAs does not say which fleet it
            // describes, which matters once more than one repository deploys.
            let mut human: Vec<String> = Vec::new();
            if let Some(repo) = value.get("repo").and_then(serde_json::Value::as_str) {
                human.push(format!("Repository: {repo}"));
            }
            if targets.is_empty() {
                human.push("No fleet targets found.".to_string());
            } else {
                human.extend(targets.iter().map(fleet::target_row));
            }
            emit(json, &value, &human);
        }
        DeployAction::View {
            target_id,
            wait,
            wait_timeout,
        } => {
            if wait_timeout < 1 {
                fail("--wait-timeout must be at least 1 second.");
            }
            if !wait {
                // A bare view is a read: it reports the state and exits zero
                // even for a failed target. Exit behaviour for terminal states
                // belongs to `--wait`.
                let target = or_fail(client.view(&target_id).await);
                emit(
                    json,
                    &fleet::target_document("openagents.fleet_target.v1", &target, "pending", &[]),
                    &fleet::target_human(&target),
                );
                return;
            }
            let target = or_fail_deploy_wait(
                client
                    .wait(&target_id, std::time::Duration::from_secs(wait_timeout))
                    .await,
            );
            let mut human = fleet::target_human(&target);
            human.push(fleet::terminal_human(&target));
            emit(
                json,
                &fleet::target_document("openagents.fleet_target.v1", &target, "pending", &[]),
                &human,
            );
            conclude_fleet_target(&target);
        }
        DeployAction::Promote {
            repo,
            sha,
            environment,
            idempotency_key,
            expected_current_target,
            wait,
            wait_timeout,
        } => {
            let Some(repo) = repo else {
                fail(
                    "Pass --repo with the canonical repository the server deploys, such as \
                     --repo openagents.com.",
                );
            };
            let Some(sha) = sha else {
                fail("Pass --sha with the full 40-character commit SHA you reviewed.");
            };
            let sha = sha.trim().to_lowercase();
            if !fleet::full_sha(&sha) {
                fail(
                    "--sha must be one full 40-character commit SHA. Branch names, tags, and \
                     abbreviations are refused; print the exact reviewed value with: \
                     git rev-parse HEAD",
                );
            }
            let Some(environment) = environment else {
                fail(
                    "Pass --environment production explicitly. Production promotion never \
                     assumes an environment.",
                );
            };
            if wait_timeout < 1 {
                fail("--wait-timeout must be at least 1 second.");
            }
            // Generated once and reused across automatic transport retries, so
            // a re-send can never deploy twice. Never printed.
            let key = idempotency_key.unwrap_or_else(idempotency_key_for_this_run);
            let result = or_fail(
                client
                    .promote(&crate::fleet::PromoteInput {
                        repo,
                        sha,
                        environment,
                        idempotency_key: key,
                        expected_current_target_id: expected_current_target,
                    })
                    .await,
            );
            let id = fleet::target_id(&result.target);
            let extra = [
                ("accepted", serde_json::Value::Bool(result.accepted)),
                ("replayed", serde_json::Value::Bool(result.replayed)),
            ];
            if !wait {
                let mut human = fleet::target_human(&result.target);
                human.push(if result.replayed {
                    "This idempotency key already named this promotion; the original target is \
                     returned."
                        .to_string()
                } else {
                    "Promotion accepted. Accepted means recorded, not live; the fleet deploys it \
                     now."
                        .to_string()
                });
                human.push(format!("Follow it with: oa deploy view {id} --wait"));
                emit(
                    json,
                    &fleet::target_document(
                        "openagents.fleet_promotion.v1",
                        &result.target,
                        "accepted",
                        &extra,
                    ),
                    &human,
                );
                return;
            }
            let target = or_fail_deploy_wait(
                client
                    .wait(&id, std::time::Duration::from_secs(wait_timeout))
                    .await,
            );
            let mut human = fleet::target_human(&target);
            human.push(fleet::terminal_human(&target));
            emit(
                json,
                &fleet::target_document(
                    "openagents.fleet_promotion.v1",
                    &target,
                    "accepted",
                    &extra,
                ),
                &human,
            );
            conclude_fleet_target(&target);
        }
    }
}

/// Turn a terminal target into the command's exit behaviour, after the full
/// document is already written.
///
/// `failed` and `reverted` are a deployment failure; `needs_rolling_replace`
/// is its own condition; `live` succeeds.
/// Unwrap a `--wait`, keeping "stopped watching" apart from "was refused".
///
/// A wait that runs out is not a failed deployment: the target keeps running.
/// The TypeScript CLI gives it rung 18 of its own so release automation can
/// resume rather than roll back, and this is where `oa` earns the same
/// distinction. Every other failure keeps the class it already had.
fn or_fail_deploy_wait<T>(result: Result<T, crate::tracker::ApiError>) -> T {
    match result {
        Ok(value) => value,
        Err(crate::tracker::ApiError::Timeout { message, .. }) => {
            fail_as(crate::errors::CliError::DeploymentWaitTimeout(message))
        }
        Err(other) => or_fail(Err(other)),
    }
}

fn conclude_fleet_target(target: &serde_json::Value) {
    let status = crate::fleet::target_status(target);
    let id = crate::fleet::target_id(target);
    match status.as_str() {
        // Three outcomes, three statuses. Release automation keys on 17, 18,
        // and 19 to tell "the fleet rejected these bytes" from "the CLI
        // stopped watching" from "an operator has to finish this by hand", and
        // it can only do that if they never share a status with each other or
        // with a transport failure.
        "failed" | "reverted" => {
            let code = crate::fleet::failure_code(target)
                .map(|code| format!(" ({code})"))
                .unwrap_or_default();
            fail_as(crate::errors::CliError::DeploymentFailed(format!(
                "The fleet target {id} reached {status}{code}."
            )));
        }
        "needs_rolling_replace" => {
            fail_as(crate::errors::CliError::DeploymentRollingReplaceRequired(
                format!("The fleet target {id} needs a rolling replacement before it can be live."),
            ))
        }
        _ => {}
    }
}

/// An idempotency key for one promotion.
///
/// A UUID would need a dependency this crate does not carry. What the key has
/// to be is unique per run and stable across this run's retries, so it is a
/// hash over the clock, the process, and this binary's own address space,
/// rendered in the UUID layout the server already accepts.
fn idempotency_key_for_this_run() -> String {
    use sha2::{Digest, Sha256};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let stack = &now as *const _ as usize;
    let mut hasher = Sha256::new();
    hasher.update(now.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    hasher.update(stack.to_le_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-4{}-a{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[13..16],
        &hex[17..20],
        &hex[20..32]
    )
}

// ---------------------------------------------------------------------------
// provider
// ---------------------------------------------------------------------------

/// `oa provider settle`.
///
/// The whole decision is local: a lease document and, when one exists, the
/// NIP-LBR closeout receipt that covers it. Nothing is fetched, because the
/// claim and lease transport is not wired and a command that pretended to
/// fetch a receipt would be inventing the one thing the gate exists to check.
fn run_provider(action: ProviderAction, json: bool) {
    match action {
        ProviderAction::Settle { lease, closeout } => {
            let lease_value = or_fail(crate::provider::read_json_file(&lease, "lease"));
            let lease_doc = or_fail(crate::provider::decode_lease(&lease_value, &lease));
            let closeout_doc = match closeout {
                Some(path) => {
                    let value = or_fail(crate::provider::read_json_file(&path, "closeout"));
                    Some(or_fail(crate::provider::decode_closeout(&value, &path)))
                }
                None => None,
            };
            let decision = crate::provider::settle_lease(&lease_doc, closeout_doc.as_ref());
            emit(json, &decision.to_json(), &decision.human());
        }
    }
}

// ---------------------------------------------------------------------------
// coder: the offline stand-in
// ---------------------------------------------------------------------------

/// `oa coder --offline`.
///
/// One turn, line-oriented, from [`standin_reply`]. It opens no socket at all,
/// so it answers with the network down — which is the whole point, and the
/// only reason the stand-in exists.
///
/// This is a mode someone asked for, not a fallback someone landed in. The
/// live path in [`crate::runtime`] fails loudly when it cannot reach a model
/// and there is no branch from there to here. The inversion this replaces was
/// the reverse: a rejected request answered with the sentence `Completed
/// autonomous reasoning turn (offline fallback).` and exit 0, with no flag
/// that could ask for it.
fn run_offline_coder(coder: CoderArgs) {
    let Some(prompt) = coder
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    else {
        fail(
            "--offline answers one prompt from the built-in stand-in. Give it one: \
             `oa coder --offline \"<prompt>\"`",
        );
    };
    let answer = standin_reply(prompt);
    println!("{answer}");
    println!("\nModel: {STANDIN_MODEL}");
    if let Some(path) = coder.export.as_deref() {
        let transcript = crate::interactive::transcript_of(prompt, &answer);
        if let Err(error) = std::fs::write(path, &transcript) {
            fail(&format!(
                "could not write the transcript to {path}: {error}"
            ));
        }
        println!("Transcript written to {path}");
    }
}

// ---------------------------------------------------------------------------
// coder: headless
// ---------------------------------------------------------------------------

/// `oa coder --headless`.
///
/// Lifted out of the dispatch so `--export` is written here too. It used to be
/// read only by the full-screen session, which meant a headless run that asked
/// for a transcript got none and was told nothing.
///
/// A missing prompt is a missing input, not a licence to invent one. This
/// substituted the literal `Analyze workspace and run tests`, opened a thread,
/// and spent the grant on an instruction nobody gave — one screen above
/// [`run_offline_coder`], which refuses the identical omission by name. The
/// same input is now handled the same way on both paths.
async fn run_headless_coder(
    coder: CoderArgs,
    api_base: &str,
    token: Option<String>,
    repository: Option<String>,
    resumed: Option<crate::resume::Resumption>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(prompt) = coder
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
    else {
        fail(
            "--headless runs one prompt and exits. Give it one: \
             `oa coder --headless \"<prompt>\"`",
        );
    };
    println!("Executing coder prompt headlessly: {}", prompt);
    let lane_name = coder.lane_name().unwrap_or_else(|reason| fail(&reason));
    // A headless session may start children. They run on the same lane and the
    // same credential, and they do not get the tool themselves. A headless
    // run also has an operator who typed the command, so the read-only mount
    // tier is granted: `capability` may load the foreign-session scanner and
    // conversation readers here too.
    let tools = crate::tools::HarnessToolRegistry::with_delegation(
        None,
        crate::tools::DelegationGate {
            lane: lane_name.clone(),
            user_token: token.clone(),
            api_base: Some(api_base.to_string()),
            max_count: crate::delegate::MAX_DELEGATE_COUNT,
            child: coder.child_options(),
            acp_agents: crate::coder::acp::find_agents().await.unwrap_or_default(),
            acp_spent: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
    )
    .allowing_plugin_mounts();
    let lane = crate::runtime::Lane::from_str(&lane_name);
    let mut runtime =
        crate::runtime::CoderRuntimeSession::new(lane, Some(api_base.to_string()), token, tools);
    runtime.reasoning = coder.reasoning.clone();
    runtime.repository = repository;
    if let Some(resumption) = &resumed {
        if let Err(reason) = crate::resume::apply(&mut runtime, resumption).await {
            fail(&reason);
        }
        println!(
            "{}",
            crate::resume::resumed_line(
                resumption,
                runtime.last_model.as_deref().unwrap_or("an unnamed model")
            )
        );
    }
    let result = runtime
        .execute_turn(&prompt, |chunk| {
            print!("{}", chunk);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        })
        .await
        .map_err(|e| e.to_string());
    // The thread ends whether the turn worked or not — a failed turn still
    // opened one, and one left open holds its grant's remaining budget — and it
    // ends by saying which of the two happened. `close()` here instead would
    // file this run as a cancellation however it went (issue #106).
    let revoked = runtime.finish().await;
    // Harbor copies the newest ~/.openagents/exports/*.json to trajectory.json.
    // Write it before any `fail`, so a turn that ran tools and then died still
    // leaves argv for T1/T2 scoring.
    {
        let (repo, branch) = crate::coder::export::git_info()
            .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
        let model = runtime.last_model.as_deref().unwrap_or("unknown");
        let exported =
            crate::coder::export::export_runtime_messages(&runtime.messages, model, &repo, &branch);
        println!(
            "exported {} steps to {} (copied: {})",
            exported.steps, exported.path, exported.copied
        );
    }
    // A turn that could not reach a model is a failure, and says so in the
    // shape every other refusal here uses.
    let result = match result {
        Ok(result) => result,
        Err(error) => fail(&error),
    };
    println!("\n\nTurn result:\n{}", result);
    if let Some(model) = &runtime.last_model {
        println!("Model: {model}");
    }
    if runtime.last_usage.reported() {
        println!("Usage: {}", runtime.last_usage.line());
    }
    // The revocation reply carries the grant's own spend, and dropping it is
    // how the CLI came to print a client-side count nothing could check.
    match revoked {
        Ok(spent) => {
            if let Some(line) = runtime.spend_line(spent) {
                println!("{line}");
            }
        }
        Err(error) => eprintln!("oa: the thread was not ended: {error}"),
    }
    for failure in &runtime.record_failures {
        eprintln!("oa: {failure}");
    }

    if let Some(path) = coder.export.as_deref() {
        let transcript = crate::interactive::transcript_of(&prompt, &result);
        std::fs::write(path, &transcript)
            .map_err(|error| format!("could not write the transcript to {path}: {error}"))?;
        println!("Transcript written to {path}");
    }
    Ok(())
}
