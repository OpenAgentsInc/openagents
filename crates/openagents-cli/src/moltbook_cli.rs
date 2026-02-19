use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use moltbook::{
    ClaimStatus, CommentSort, CreateCommentRequest, CreatePostRequest, CreateSubmoltRequest,
    ModeratorRequest, MoltbookClient, MoltbookError, PostSort, RegisterRequest,
    SubmoltSettingsRequest, UpdateProfileRequest,
};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser)]
pub struct MoltbookArgs {
    /// Moltbook API key (overrides env/config).
    #[arg(long)]
    pub api_key: Option<String>,

    /// Path to Moltbook credentials JSON (defaults to ~/.config/moltbook/credentials.json).
    #[arg(long)]
    pub credentials_file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: MoltbookCommand,
}

#[derive(Subcommand)]
pub enum MoltbookCommand {
    /// Register a new Moltbook agent (no auth).
    Register(RegisterArgs),

    /// Agent/profile operations.
    Agents(AgentsArgs),

    /// Post operations.
    Posts(PostsArgs),

    /// Comment operations.
    Comments(CommentsArgs),

    /// Personalized feed (followed agents + subscribed submolts).
    Feed(FeedArgs),

    /// Search posts/agents/submolts.
    Search(SearchArgs),

    /// Submolt (community) operations.
    Submolts(SubmoltsArgs),

    /// Watch a feed and stream new posts to stdout.
    Watch(WatchArgs),
}

#[derive(Args)]
pub struct RegisterArgs {
    /// Agent name (e.g. "OpenAgents")
    #[arg(long)]
    pub name: String,

    /// Agent description
    #[arg(long)]
    pub description: String,

    /// Save the returned API key to the credentials file.
    #[arg(long)]
    pub save: bool,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct AgentsArgs {
    #[command(subcommand)]
    pub command: AgentsCommand,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Subcommand)]
pub enum AgentsCommand {
    /// Get the current agent profile (agents/me).
    Me,

    /// Get claim status (agents/status).
    Status,

    /// View another agent profile by name.
    Profile(AgentsProfileArgs),

    /// Update the current agent profile (PATCH agents/me).
    Update(AgentsUpdateArgs),

    /// Upload avatar image for current agent.
    AvatarUpload(AvatarUploadArgs),

    /// Remove current agent avatar.
    AvatarRemove,

    /// Follow an agent.
    Follow(FollowArgs),

    /// Unfollow an agent.
    Unfollow(FollowArgs),
}

#[derive(Args)]
pub struct AgentsProfileArgs {
    /// Agent name (without @)
    pub name: String,
}

#[derive(Args)]
pub struct AgentsUpdateArgs {
    /// New description
    #[arg(long)]
    pub description: Option<String>,

    /// JSON metadata (string) to attach to profile
    #[arg(long)]
    pub metadata_json: Option<String>,
}

#[derive(Args)]
pub struct AvatarUploadArgs {
    /// Path to image (PNG/JPEG/GIF/WebP)
    pub file: PathBuf,
}

#[derive(Args)]
pub struct FollowArgs {
    /// Agent name to follow/unfollow
    pub name: String,
}

#[derive(Args)]
pub struct PostsArgs {
    #[command(subcommand)]
    pub command: PostsCommand,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Subcommand)]
pub enum PostsCommand {
    /// Create a post.
    Create(PostsCreateArgs),

    /// Get a post by id.
    Get(PostIdArgs),

    /// Delete a post by id.
    Delete(PostIdArgs),

    /// Upvote a post by id.
    Upvote(PostIdArgs),

    /// Downvote a post by id.
    Downvote(PostIdArgs),

    /// Pin a post (mod-only).
    Pin(PostIdArgs),

    /// Unpin a post (mod-only).
    Unpin(PostIdArgs),

    /// Read the global posts feed.
    Feed(PostsFeedArgs),
}

#[derive(Args)]
pub struct PostIdArgs {
    pub post_id: String,
}

#[derive(Args)]
pub struct PostsCreateArgs {
    /// Submolt name (community)
    #[arg(long, default_value = "general")]
    pub submolt: String,

    /// Title for the post. If omitted, derived from content (first line / first 80 chars).
    #[arg(long)]
    pub title: Option<String>,

    /// Post content. Use --stdin for multi-line content.
    #[arg(long, conflicts_with = "stdin")]
    pub content: Option<String>,

    /// Read content from stdin.
    #[arg(long)]
    pub stdin: bool,

    /// Optional URL (creates a link post).
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(Args)]
pub struct PostsFeedArgs {
    /// Feed sort
    #[arg(long, default_value = "new")]
    pub sort: FeedSort,

    /// Limit (max items)
    #[arg(long, default_value = "25")]
    pub limit: u32,

    /// Filter by submolt
    #[arg(long)]
    pub submolt: Option<String>,

    /// Preview chars (non-JSON output only)
    #[arg(long, default_value = "0")]
    pub preview_chars: usize,
}

#[derive(Args)]
pub struct CommentsArgs {
    #[command(subcommand)]
    pub command: CommentsCommand,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Subcommand)]
pub enum CommentsCommand {
    /// Create a comment on a post (or reply with --parent-id).
    Create(CommentsCreateArgs),

    /// List comments for a post.
    List(CommentsListArgs),

    /// Upvote a comment by id.
    Upvote(CommentIdArgs),
}

#[derive(Args)]
pub struct CommentsCreateArgs {
    /// Post id
    pub post_id: String,

    /// Comment content (use --stdin for multi-line).
    #[arg(long, conflicts_with = "stdin")]
    pub content: Option<String>,

    /// Read content from stdin.
    #[arg(long)]
    pub stdin: bool,

    /// Parent comment id (to reply to an existing comment).
    #[arg(long)]
    pub parent_id: Option<String>,
}

#[derive(Args)]
pub struct CommentsListArgs {
    /// Post id
    pub post_id: String,

    /// Comment sort
    #[arg(long, default_value = "top")]
    pub sort: CommentSortArg,

    /// Limit (max items)
    #[arg(long, default_value = "50")]
    pub limit: u32,
}

#[derive(Args)]
pub struct CommentIdArgs {
    pub comment_id: String,
}

#[derive(Args)]
pub struct FeedArgs {
    /// Feed sort
    #[arg(long, default_value = "new")]
    pub sort: FeedSort,

    /// Limit (max items)
    #[arg(long, default_value = "25")]
    pub limit: u32,

    /// Preview chars (non-JSON output only)
    #[arg(long, default_value = "0")]
    pub preview_chars: usize,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub q: String,

    /// Limit (max items)
    #[arg(long, default_value = "25")]
    pub limit: u32,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Args)]
pub struct SubmoltsArgs {
    #[command(subcommand)]
    pub command: SubmoltsCommand,

    #[command(flatten)]
    pub output: OutputArgs,
}

#[derive(Subcommand)]
pub enum SubmoltsCommand {
    /// Create a submolt.
    Create(SubmoltsCreateArgs),

    /// List submolts.
    List,

    /// Get submolt details by name.
    Get(SubmoltNameArgs),

    /// Subscribe to a submolt.
    Subscribe(SubmoltNameArgs),

    /// Unsubscribe from a submolt.
    Unsubscribe(SubmoltNameArgs),

    /// Update submolt settings.
    UpdateSettings(SubmoltsUpdateSettingsArgs),

    /// Upload submolt avatar image.
    UploadAvatar(SubmoltsUploadImageArgs),

    /// Upload submolt banner image.
    UploadBanner(SubmoltsUploadImageArgs),

    /// Add a moderator (mod-only).
    AddModerator(SubmoltsModeratorArgs),

    /// Remove a moderator (mod-only).
    RemoveModerator(SubmoltsModeratorArgs),

    /// List moderators.
    ListModerators(SubmoltNameArgs),
}

#[derive(Args)]
pub struct SubmoltNameArgs {
    pub name: String,
}

#[derive(Args)]
pub struct SubmoltsCreateArgs {
    /// Submolt name (slug)
    #[arg(long)]
    pub name: String,

    /// Display name
    #[arg(long)]
    pub display_name: String,

    /// Description
    #[arg(long)]
    pub description: String,
}

#[derive(Args)]
pub struct SubmoltsUpdateSettingsArgs {
    /// Submolt name
    pub name: String,

    /// New description
    #[arg(long)]
    pub description: Option<String>,

    /// New banner color (hex) e.g. "#000000"
    #[arg(long)]
    pub banner_color: Option<String>,

    /// New theme color (hex) e.g. "#00ff00"
    #[arg(long)]
    pub theme_color: Option<String>,
}

#[derive(Args)]
pub struct SubmoltsUploadImageArgs {
    /// Submolt name
    pub name: String,

    /// Path to image
    pub file: PathBuf,
}

#[derive(Args)]
pub struct SubmoltsModeratorArgs {
    /// Submolt name
    pub name: String,

    /// Moderator agent name
    pub moderator: String,
}

#[derive(Args)]
pub struct WatchArgs {
    /// Watch global posts feed (default) or personalized feed.
    #[arg(long)]
    pub personal: bool,

    /// Filter by submolt (global feed only).
    #[arg(long)]
    pub submolt: Option<String>,

    /// Sort order.
    #[arg(long, default_value = "new")]
    pub sort: FeedSort,

    /// Max items per poll.
    #[arg(long, default_value = "25")]
    pub limit: u32,

    /// Poll interval (seconds).
    #[arg(long, default_value = "15")]
    pub interval_secs: u64,

    /// Print the initial feed items before tailing.
    #[arg(long)]
    pub include_existing: bool,

    /// Print content preview (chars).
    #[arg(long, default_value = "0")]
    pub preview_chars: usize,

    /// Emit JSON lines (one object per new post).
    #[arg(long)]
    pub jsonl: bool,

    /// Exit after N poll cycles (useful for scripting/tests).
    #[arg(long)]
    pub max_iterations: Option<u32>,
}

#[derive(Args, Clone, Copy)]
pub struct OutputArgs {
    /// Output JSON instead of human-friendly text.
    #[arg(long)]
    pub json: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum FeedSort {
    Hot,
    New,
    Top,
    Rising,
}

impl FeedSort {
    fn to_post_sort(self) -> PostSort {
        match self {
            FeedSort::Hot => PostSort::Hot,
            FeedSort::New => PostSort::New,
            FeedSort::Top => PostSort::Top,
            FeedSort::Rising => PostSort::Rising,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum CommentSortArg {
    Top,
    New,
    Controversial,
}

impl CommentSortArg {
    fn to_comment_sort(self) -> CommentSort {
        match self {
            CommentSortArg::Top => CommentSort::Top,
            CommentSortArg::New => CommentSort::New,
            CommentSortArg::Controversial => CommentSort::Controversial,
        }
    }
}

#[derive(Serialize)]
struct RegisterOutput {
    api_key: String,
    claim_url: String,
    verification_code: String,
}

#[derive(Serialize)]
struct AgentOutput {
    id: Option<String>,
    name: String,
    description: Option<String>,
    karma: Option<i64>,
    follower_count: Option<u64>,
    following_count: Option<u64>,
    is_claimed: Option<bool>,
    is_active: Option<bool>,
    created_at: Option<String>,
    last_active: Option<String>,
    avatar_url: Option<String>,
    stats_posts: Option<u64>,
    stats_comments: Option<u64>,
    stats_subscriptions: Option<u64>,
}

#[derive(Serialize)]
struct PostSummaryOutput {
    id: String,
    submolt: Option<String>,
    title: Option<String>,
    author: Option<String>,
    score: Option<i64>,
    comment_count: Option<u64>,
    created_at: Option<String>,
    content_preview: Option<String>,
}

#[derive(Serialize)]
struct CommentOutput {
    id: String,
    post_id: Option<String>,
    parent_id: Option<String>,
    author: Option<String>,
    score: Option<i64>,
    created_at: Option<String>,
    content_preview: Option<String>,
}

#[derive(Serialize)]
struct SearchOutput {
    posts: Vec<PostSummaryOutput>,
    agents: Vec<String>,
    submolts: Vec<String>,
}

#[derive(Serialize)]
struct SubmoltOutput {
    name: String,
    display_name: Option<String>,
    description: Option<String>,
    subscriber_count: Option<u64>,
    your_role: Option<String>,
    avatar_url: Option<String>,
    banner_url: Option<String>,
}

pub fn run(args: MoltbookArgs) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("Failed to start Tokio runtime")?;
    runtime.block_on(run_async(args))
}

async fn run_async(args: MoltbookArgs) -> Result<()> {
    match args.command {
        MoltbookCommand::Register(cmd) => {
            register_command(args.api_key, args.credentials_file, cmd).await
        }
        MoltbookCommand::Agents(cmd) => {
            agents_command(args.api_key, args.credentials_file, cmd).await
        }
        MoltbookCommand::Posts(cmd) => {
            posts_command(args.api_key, args.credentials_file, cmd).await
        }
        MoltbookCommand::Comments(cmd) => {
            comments_command(args.api_key, args.credentials_file, cmd).await
        }
        MoltbookCommand::Feed(cmd) => feed_command(args.api_key, args.credentials_file, cmd).await,
        MoltbookCommand::Search(cmd) => {
            search_command(args.api_key, args.credentials_file, cmd).await
        }
        MoltbookCommand::Submolts(cmd) => {
            submolts_command(args.api_key, args.credentials_file, cmd).await
        }
        MoltbookCommand::Watch(cmd) => {
            watch_command(args.api_key, args.credentials_file, cmd).await
        }
    }
}

async fn register_command(
    _api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: RegisterArgs,
) -> Result<()> {
    let client = MoltbookClient::unauthenticated().context("Failed to create Moltbook client")?;
    let res = client
        .register(RegisterRequest {
            name: args.name.clone(),
            description: args.description,
        })
        .await
        .context("Moltbook register failed")?;

    if args.save {
        let path = credentials_file.unwrap_or_else(default_credentials_path);
        write_credentials(&path, &res.agent.api_key, &args.name)?;
        eprintln!("Saved credentials to {}", path.display());
    }

    if args.output.json {
        let out = RegisterOutput {
            api_key: res.agent.api_key,
            claim_url: res.agent.claim_url,
            verification_code: res.agent.verification_code,
        };
        print_json(&out)
    } else {
        println!("Registered '{}'", args.name);
        println!("API key: {}", res.agent.api_key);
        println!("Claim URL: {}", res.agent.claim_url);
        println!("Verification code: {}", res.agent.verification_code);
        Ok(())
    }
}

async fn agents_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: AgentsArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    match args.command {
        AgentsCommand::Me => {
            let agent = client.agents_me().await.context("agents/me failed")?;
            if args.output.json {
                print_json(&agent_to_output(agent))
            } else {
                print_agent_human(&agent_to_output(agent))
            }
        }
        AgentsCommand::Status => {
            let status = client
                .agents_status()
                .await
                .context("agents/status failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct StatusOut {
                    status: String,
                }
                let status_str = match status.status {
                    ClaimStatus::PendingClaim => "pending_claim",
                    ClaimStatus::Claimed => "claimed",
                };
                print_json(&StatusOut {
                    status: status_str.to_string(),
                })
            } else {
                println!("Claim status: {:?}", status.status);
                Ok(())
            }
        }
        AgentsCommand::Profile(p) => {
            let profile = client
                .agents_profile(&p.name)
                .await
                .context("agents/profile failed")?;
            if args.output.json {
                let posts = profile
                    .recent_posts
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| post_to_summary(p, 0))
                    .collect::<Vec<_>>();
                #[derive(Serialize)]
                struct ProfileOut {
                    agent: AgentOutput,
                    recent_posts: Vec<PostSummaryOutput>,
                }
                print_json(&ProfileOut {
                    agent: agent_to_output(profile.agent),
                    recent_posts: posts,
                })
            } else {
                let agent = agent_to_output(profile.agent);
                print_agent_human(&agent)?;
                if let Some(recent) = profile.recent_posts {
                    if !recent.is_empty() {
                        println!();
                        println!("Recent posts:");
                        for p in recent {
                            println!("  {}", format_post_line(&post_to_summary(p, 80), 80));
                        }
                    }
                }
                Ok(())
            }
        }
        AgentsCommand::Update(u) => {
            let metadata = match u.metadata_json {
                Some(s) => Some(
                    serde_json::from_str(&s).context("Invalid --metadata-json (must be JSON)")?,
                ),
                None => None,
            };
            let updated = client
                .agents_me_update(UpdateProfileRequest {
                    description: u.description,
                    metadata,
                })
                .await
                .context("agents/me update failed")?;
            if args.output.json {
                print_json(&agent_to_output(updated))
            } else {
                println!("Updated profile.");
                Ok(())
            }
        }
        AgentsCommand::AvatarUpload(u) => {
            let (bytes, filename) = read_file_bytes(&u.file)?;
            let updated = client
                .agents_me_avatar_upload(&bytes, &filename)
                .await
                .context("avatar upload failed")?;
            if args.output.json {
                print_json(&agent_to_output(updated))
            } else {
                println!("Avatar uploaded.");
                Ok(())
            }
        }
        AgentsCommand::AvatarRemove => {
            let updated = client
                .agents_me_avatar_remove()
                .await
                .context("avatar remove failed")?;
            if args.output.json {
                print_json(&agent_to_output(updated))
            } else {
                println!("Avatar removed.");
                Ok(())
            }
        }
        AgentsCommand::Follow(f) => {
            client
                .agents_follow(&f.name)
                .await
                .context("follow failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Followed {}", f.name);
                Ok(())
            }
        }
        AgentsCommand::Unfollow(f) => {
            client
                .agents_unfollow(&f.name)
                .await
                .context("unfollow failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Unfollowed {}", f.name);
                Ok(())
            }
        }
    }
}

async fn posts_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: PostsArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    match args.command {
        PostsCommand::Create(c) => {
            let raw_content = if c.stdin {
                Some(read_stdin()?)
            } else {
                c.content
            };
            let (title, content) = derive_title_and_content(c.title, raw_content);
            let created = client
                .posts_create(CreatePostRequest {
                    submolt: c.submolt,
                    title,
                    content,
                    url: c.url,
                })
                .await
                .context("post create failed")?;

            if args.output.json {
                print_json(&post_to_summary(created, 280))
            } else {
                println!("Posted: {}", created.id);
                Ok(())
            }
        }
        PostsCommand::Get(p) => {
            let post = client
                .posts_get(&p.post_id)
                .await
                .context("post get failed")?;
            if args.output.json {
                print_json(&post_to_summary(post, 500))
            } else {
                println!("{}", format_post_line(&post_to_summary(post, 240), 240));
                Ok(())
            }
        }
        PostsCommand::Delete(p) => {
            client
                .posts_delete(&p.post_id)
                .await
                .context("post delete failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Deleted post {}", p.post_id);
                Ok(())
            }
        }
        PostsCommand::Upvote(p) => {
            client
                .posts_upvote(&p.post_id)
                .await
                .context("post upvote failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Upvoted post {}", p.post_id);
                Ok(())
            }
        }
        PostsCommand::Downvote(p) => {
            client
                .posts_downvote(&p.post_id)
                .await
                .context("post downvote failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Downvoted post {}", p.post_id);
                Ok(())
            }
        }
        PostsCommand::Pin(p) => {
            client
                .posts_pin(&p.post_id)
                .await
                .context("post pin failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Pinned post {}", p.post_id);
                Ok(())
            }
        }
        PostsCommand::Unpin(p) => {
            client
                .posts_unpin(&p.post_id)
                .await
                .context("post unpin failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Unpinned post {}", p.post_id);
                Ok(())
            }
        }
        PostsCommand::Feed(f) => {
            let posts = client
                .posts_feed(f.sort.to_post_sort(), Some(f.limit), f.submolt.as_deref())
                .await
                .context("posts feed failed")?;
            if args.output.json {
                let out = posts
                    .into_iter()
                    .map(|p| post_to_summary(p, 0))
                    .collect::<Vec<_>>();
                print_json(&out)
            } else {
                let out = posts
                    .into_iter()
                    .map(|p| post_to_summary(p, f.preview_chars))
                    .collect::<Vec<_>>();
                for p in out {
                    println!("{}", format_post_line(&p, f.preview_chars));
                }
                Ok(())
            }
        }
    }
}

async fn comments_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: CommentsArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    match args.command {
        CommentsCommand::Create(c) => {
            let content = if c.stdin {
                read_stdin()?
            } else {
                c.content.unwrap_or_default()
            };
            if content.trim().is_empty() {
                return Err(anyhow::anyhow!("Comment content is empty"));
            }
            let comment = client
                .comments_create(
                    &c.post_id,
                    CreateCommentRequest {
                        content,
                        parent_id: c.parent_id,
                    },
                )
                .await
                .context("comment create failed")?;
            if args.output.json {
                print_json(&comment_to_output(comment, 280))
            } else {
                println!("Commented: {}", comment.id);
                Ok(())
            }
        }
        CommentsCommand::List(l) => {
            let comments = client
                .comments_list(&l.post_id, l.sort.to_comment_sort(), Some(l.limit))
                .await
                .context("comments list failed")?;
            if args.output.json {
                let out = comments
                    .into_iter()
                    .map(|c| comment_to_output(c, 0))
                    .collect::<Vec<_>>();
                print_json(&out)
            } else {
                for c in comments {
                    let out = comment_to_output(c, 200);
                    println!(
                        "[{}] {} {}",
                        out.created_at.as_deref().unwrap_or("-"),
                        out.author.as_deref().unwrap_or("?"),
                        out.content_preview.as_deref().unwrap_or("")
                    );
                }
                Ok(())
            }
        }
        CommentsCommand::Upvote(c) => {
            client
                .comments_upvote(&c.comment_id)
                .await
                .context("comment upvote failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Upvoted comment {}", c.comment_id);
                Ok(())
            }
        }
    }
}

async fn feed_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: FeedArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    let posts = client
        .feed(args.sort.to_post_sort(), Some(args.limit))
        .await
        .context("feed failed")?;
    if args.output.json {
        let out = posts
            .into_iter()
            .map(|p| post_to_summary(p, 0))
            .collect::<Vec<_>>();
        print_json(&out)
    } else {
        let out = posts
            .into_iter()
            .map(|p| post_to_summary(p, args.preview_chars))
            .collect::<Vec<_>>();
        for p in out {
            println!("{}", format_post_line(&p, args.preview_chars));
        }
        Ok(())
    }
}

async fn search_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: SearchArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    let res = client
        .search(&args.q, Some(args.limit))
        .await
        .context("search failed")?;
    let posts = res
        .posts
        .unwrap_or_default()
        .into_iter()
        .map(|p| post_to_summary(p, 0))
        .collect::<Vec<_>>();
    let agents = res
        .agents
        .unwrap_or_default()
        .into_iter()
        .map(|a| a.name)
        .collect::<Vec<_>>();
    let submolts = res
        .submolts
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.name)
        .collect::<Vec<_>>();
    let out = SearchOutput {
        posts,
        agents,
        submolts,
    };
    if args.output.json {
        print_json(&out)
    } else {
        if !out.posts.is_empty() {
            println!("Posts:");
            for p in out.posts {
                println!("  {}", format_post_line(&p, 80));
            }
        }
        if !out.agents.is_empty() {
            println!();
            println!("Agents:");
            for a in out.agents {
                println!("  {}", a);
            }
        }
        if !out.submolts.is_empty() {
            println!();
            println!("Submolts:");
            for s in out.submolts {
                println!("  {}", s);
            }
        }
        Ok(())
    }
}

async fn submolts_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: SubmoltsArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    match args.command {
        SubmoltsCommand::Create(c) => {
            let s = client
                .submolts_create(CreateSubmoltRequest {
                    name: c.name,
                    display_name: c.display_name,
                    description: c.description,
                })
                .await
                .context("submolt create failed")?;
            if args.output.json {
                print_json(&submolt_to_output(s))
            } else {
                println!("Created submolt {}", s.name);
                Ok(())
            }
        }
        SubmoltsCommand::List => {
            let list = client
                .submolts_list()
                .await
                .context("submolt list failed")?;
            if args.output.json {
                let out = list.into_iter().map(submolt_to_output).collect::<Vec<_>>();
                print_json(&out)
            } else {
                for s in list {
                    let out = submolt_to_output(s);
                    println!(
                        "{}{}",
                        out.name,
                        out.description
                            .as_deref()
                            .map(|d| format!(" — {}", d))
                            .unwrap_or_default()
                    );
                }
                Ok(())
            }
        }
        SubmoltsCommand::Get(g) => {
            let s = client
                .submolts_get(&g.name)
                .await
                .context("submolt get failed")?;
            if args.output.json {
                print_json(&submolt_to_output(s))
            } else {
                let out = submolt_to_output(s);
                println!("m/{}", out.name);
                if let Some(display) = out.display_name.as_deref() {
                    println!("Display: {}", display);
                }
                if let Some(desc) = out.description.as_deref() {
                    println!("About: {}", desc);
                }
                Ok(())
            }
        }
        SubmoltsCommand::Subscribe(s) => {
            client
                .submolts_subscribe(&s.name)
                .await
                .context("subscribe failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Subscribed to m/{}", s.name);
                Ok(())
            }
        }
        SubmoltsCommand::Unsubscribe(s) => {
            client
                .submolts_unsubscribe(&s.name)
                .await
                .context("unsubscribe failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Unsubscribed from m/{}", s.name);
                Ok(())
            }
        }
        SubmoltsCommand::UpdateSettings(u) => {
            client
                .submolts_settings_update(
                    &u.name,
                    SubmoltSettingsRequest {
                        description: u.description,
                        banner_color: u.banner_color,
                        theme_color: u.theme_color,
                    },
                )
                .await
                .context("submolt settings update failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Updated settings for m/{}", u.name);
                Ok(())
            }
        }
        SubmoltsCommand::UploadAvatar(u) => {
            let (bytes, filename) = read_file_bytes(&u.file)?;
            client
                .submolts_settings_upload_avatar(&u.name, &bytes, &filename)
                .await
                .context("upload avatar failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Uploaded avatar for m/{}", u.name);
                Ok(())
            }
        }
        SubmoltsCommand::UploadBanner(u) => {
            let (bytes, filename) = read_file_bytes(&u.file)?;
            client
                .submolts_settings_upload_banner(&u.name, &bytes, &filename)
                .await
                .context("upload banner failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Uploaded banner for m/{}", u.name);
                Ok(())
            }
        }
        SubmoltsCommand::AddModerator(m) => {
            client
                .submolts_moderators_add(
                    &m.name,
                    ModeratorRequest {
                        agent_name: m.moderator.clone(),
                        role: None,
                    },
                )
                .await
                .context("add moderator failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Added moderator {} to m/{}", m.moderator, m.name);
                Ok(())
            }
        }
        SubmoltsCommand::RemoveModerator(m) => {
            client
                .submolts_moderators_remove(&m.name, &m.moderator)
                .await
                .context("remove moderator failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct OkOut {
                    ok: bool,
                }
                print_json(&OkOut { ok: true })
            } else {
                println!("Removed moderator {} from m/{}", m.moderator, m.name);
                Ok(())
            }
        }
        SubmoltsCommand::ListModerators(s) => {
            let mods = client
                .submolts_moderators_list(&s.name)
                .await
                .context("list moderators failed")?;
            if args.output.json {
                #[derive(Serialize)]
                struct ModOut {
                    agent_name: Option<String>,
                    role: Option<String>,
                }
                let out = mods
                    .into_iter()
                    .map(|m| ModOut {
                        agent_name: m.agent_name,
                        role: m.role,
                    })
                    .collect::<Vec<_>>();
                print_json(&out)
            } else {
                println!("Moderators for m/{}:", s.name);
                for m in mods {
                    let name = m.agent_name.as_deref().unwrap_or("?");
                    let role = m.role.as_deref().unwrap_or("moderator");
                    println!("  {} ({})", name, role);
                }
                Ok(())
            }
        }
    }
}

async fn watch_command(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
    args: WatchArgs,
) -> Result<()> {
    let client = authenticated_client(api_key, credentials_file)?;
    let mut seen: HashSet<String> = HashSet::new();
    let mut order: VecDeque<String> = VecDeque::new();
    let mut iterations: u32 = 0;

    let mut interval = tokio::time::interval(Duration::from_secs(args.interval_secs.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    println!("Watching Moltbook… (ctrl-c to stop)");
    if args.personal {
        println!("Source: personalized feed");
    } else if let Some(s) = args.submolt.as_deref() {
        println!("Source: global feed (m/{})", s);
    } else {
        println!("Source: global feed");
    }

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Stopped.");
                return Ok(());
            }
            _ = interval.tick() => {}
        }

        let posts_res = if args.personal {
            client
                .feed(args.sort.to_post_sort(), Some(args.limit))
                .await
        } else {
            client
                .posts_feed(
                    args.sort.to_post_sort(),
                    Some(args.limit),
                    args.submolt.as_deref(),
                )
                .await
        };

        match posts_res {
            Ok(posts) => {
                let mut printed_any = false;
                if seen.is_empty() {
                    // First poll.
                    if args.include_existing {
                        for p in posts.iter().rev() {
                            let out = post_to_summary(p.clone(), args.preview_chars);
                            println!("{}", format_post_line(&out, args.preview_chars));
                            printed_any = true;
                        }
                    }
                    for p in posts {
                        remember_seen(&mut seen, &mut order, p.id, 2000);
                    }
                } else {
                    for p in posts.iter().rev() {
                        if !seen.contains(&p.id) {
                            let out = post_to_summary(p.clone(), args.preview_chars);
                            if args.jsonl {
                                print_json_line(&out)?;
                            } else {
                                println!("{}", format_post_line(&out, args.preview_chars));
                            }
                            printed_any = true;
                        }
                    }
                    for p in posts {
                        remember_seen(&mut seen, &mut order, p.id, 2000);
                    }
                }
                if printed_any {
                    io::Write::flush(&mut io::stdout()).ok();
                }
            }
            Err(MoltbookError::RateLimited {
                retry_after_minutes,
            }) => {
                let delay = Duration::from_secs(retry_after_minutes.saturating_mul(60) as u64);
                eprintln!("Rate limited; sleeping {}s", delay.as_secs());
                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                eprintln!("Watch poll error: {e}");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }

        iterations = iterations.saturating_add(1);
        if let Some(max) = args.max_iterations {
            if iterations >= max {
                return Ok(());
            }
        }
    }
}

fn authenticated_client(
    api_key: Option<String>,
    credentials_file: Option<PathBuf>,
) -> Result<MoltbookClient> {
    let api_key = resolve_api_key(api_key, credentials_file)?;
    MoltbookClient::new(api_key).context("Failed to create authenticated Moltbook client")
}

fn resolve_api_key(explicit: Option<String>, credentials_file: Option<PathBuf>) -> Result<String> {
    if let Some(k) = explicit {
        let trimmed = k.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Ok(k) = env::var("MOLTBOOK_API_KEY") {
        let trimmed = k.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let path = credentials_file.unwrap_or_else(default_credentials_path);
    let data = fs::read_to_string(&path)
        .with_context(|| format!("Read credentials {}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&data).context("Parse credentials JSON")?;
    let key = json
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing api_key in {}", path.display()))?;
    Ok(key)
}

fn default_credentials_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".config/moltbook/credentials.json")
}

fn write_credentials(path: &Path, api_key: &str, agent_name: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Create {}", parent.display()))?;
    }
    let obj = serde_json::json!({
        "api_key": api_key,
        "agent_name": agent_name
    });
    let pretty = serde_json::to_string_pretty(&obj).context("Serialize credentials")?;
    fs::write(path, pretty).with_context(|| format!("Write {}", path.display()))?;
    Ok(())
}

fn read_stdin() -> Result<String> {
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf).context("Read stdin")?;
    Ok(buf)
}

fn derive_title_and_content(
    title: Option<String>,
    content: Option<String>,
) -> (String, Option<String>) {
    if let Some(t) = title {
        let t = t.trim().to_string();
        if !t.is_empty() {
            let content = content.and_then(|c| {
                let trimmed = c.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });
            return (t, content);
        }
    }

    let raw = content.unwrap_or_default();
    let raw = raw.trim().to_string();
    if raw.is_empty() {
        return ("Update".to_string(), None);
    }

    if let Some((first, rest)) = raw.split_once('\n') {
        let first = first.trim();
        if !first.is_empty() {
            let rest = rest.trim();
            let content = if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };
            return (first.to_string(), content);
        }
    }

    let title = raw.chars().take(80).collect::<String>().trim().to_string();
    (
        if title.is_empty() {
            "Update".to_string()
        } else {
            title
        },
        None,
    )
}

fn read_file_bytes(path: &Path) -> Result<(Vec<u8>, String)> {
    let bytes = fs::read(path).with_context(|| format!("Read {}", path.display()))?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();
    Ok((bytes, filename))
}

fn remember_seen(seen: &mut HashSet<String>, order: &mut VecDeque<String>, id: String, cap: usize) {
    if seen.insert(id.clone()) {
        order.push_back(id);
    }
    while order.len() > cap {
        if let Some(old) = order.pop_front() {
            seen.remove(&old);
        }
    }
}

fn agent_to_output(agent: moltbook::Agent) -> AgentOutput {
    let stats_posts = agent.stats.as_ref().and_then(|s| s.posts);
    let stats_comments = agent.stats.as_ref().and_then(|s| s.comments);
    let stats_subscriptions = agent.stats.as_ref().and_then(|s| s.subscriptions);
    AgentOutput {
        id: agent.id,
        name: agent.name,
        description: agent.description,
        karma: agent.karma,
        follower_count: agent.follower_count,
        following_count: agent.following_count,
        is_claimed: agent.is_claimed,
        is_active: agent.is_active,
        created_at: agent.created_at,
        last_active: agent.last_active,
        avatar_url: agent.avatar_url,
        stats_posts,
        stats_comments,
        stats_subscriptions,
    }
}

fn post_to_summary(post: moltbook::Post, preview_chars: usize) -> PostSummaryOutput {
    let author = post.author.as_ref().map(|a| a.name.clone());
    let content_preview = post
        .content
        .as_ref()
        .and_then(|c| preview_text(c, preview_chars));
    PostSummaryOutput {
        id: post.id,
        submolt: post.submolt,
        title: post.title,
        author,
        score: post.score,
        comment_count: post.comment_count,
        created_at: post.created_at,
        content_preview,
    }
}

fn comment_to_output(comment: moltbook::Comment, preview_chars: usize) -> CommentOutput {
    let content_preview = comment
        .content
        .as_ref()
        .and_then(|c| preview_text(c, preview_chars));
    CommentOutput {
        id: comment.id,
        post_id: comment.post_id,
        parent_id: comment.parent_id,
        author: comment.author.map(|a| a.name),
        score: comment.score,
        created_at: comment.created_at,
        content_preview,
    }
}

fn submolt_to_output(s: moltbook::Submolt) -> SubmoltOutput {
    let your_role = s.your_role.map(|r| match r {
        moltbook::SubmoltRole::Owner => "owner".to_string(),
        moltbook::SubmoltRole::Moderator => "moderator".to_string(),
    });
    SubmoltOutput {
        name: s.name,
        display_name: s.display_name,
        description: s.description,
        subscriber_count: s.subscriber_count,
        your_role,
        avatar_url: s.avatar_url,
        banner_url: s.banner_url,
    }
}

fn preview_text(input: &str, preview_chars: usize) -> Option<String> {
    if preview_chars == 0 {
        return None;
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let collapsed = trimmed.replace('\n', " ");
    Some(collapsed.chars().take(preview_chars).collect::<String>())
}

fn format_post_line(p: &PostSummaryOutput, preview_chars: usize) -> String {
    let created = p.created_at.as_deref().unwrap_or("-");
    let submolt = p.submolt.as_deref().unwrap_or("-");
    let author = p.author.as_deref().unwrap_or("?");
    let score = p
        .score
        .map(|s| s.to_string())
        .unwrap_or_else(|| "—".to_string());
    let comments = p
        .comment_count
        .map(|c| c.to_string())
        .unwrap_or_else(|| "—".to_string());
    let title = p.title.as_deref().unwrap_or("(no title)");
    let mut line = format!(
        "[{created}] {submolt} · {author} · {score}↑ · {comments}c · {title} ({})",
        p.id
    );
    if preview_chars > 0 {
        if let Some(prev) = p.content_preview.as_deref() {
            if !prev.is_empty() {
                line.push_str(" — ");
                line.push_str(prev);
            }
        }
    }
    line
}

fn print_agent_human(agent: &AgentOutput) -> Result<()> {
    println!("Agent: {}", agent.name);
    if let Some(desc) = agent.description.as_deref() {
        println!("About: {}", desc);
    }
    if let Some(karma) = agent.karma {
        println!("Karma: {}", karma);
    }
    if let Some(posts) = agent.stats_posts {
        println!("Posts: {}", posts);
    }
    if let Some(comments) = agent.stats_comments {
        println!("Comments: {}", comments);
    }
    if let Some(subs) = agent.stats_subscriptions {
        println!("Subscriptions: {}", subs);
    }
    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let out = serde_json::to_string_pretty(value).context("Serialize JSON")?;
    println!("{}", out);
    Ok(())
}

fn print_json_line<T: Serialize>(value: &T) -> Result<()> {
    let out = serde_json::to_string(value).context("Serialize JSONL")?;
    println!("{}", out);
    Ok(())
}
