use anyhow::Result;
use clap::{Parser, Subcommand};
use openagents::server::services::{
    deepseek::{ChatMessage, DeepSeekService, ToolChoice},
    github_issue::GitHubService,
};
use serde_json::json;
use std::io::{self, Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Chat {
        #[arg(short, long)]
        message: Option<String>,
    },
}

fn print_colored(role: &str, content: &str) -> Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    let color = match role {
        "user" => Color::Blue,
        "assistant" => Color::Green,
        "system" => Color::Yellow,
        _ => Color::Red,
    };
    stdout.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true))?;
    write!(stdout, "{}:", role)?;
    stdout.reset()?;
    writeln!(stdout, " {}", content)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize DeepSeek service
    let deepseek_api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set");
    let service = DeepSeekService::new(deepseek_api_key);

    // Initialize GitHub service
    let github_token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
    let github_service: GitHubService = GitHubService::new(Some(github_token))?;

    // Create GitHub issue tool
    let get_issue_tool = DeepSeekService::create_tool(
        "get_github_issue".to_string(),
        Some("Get a GitHub issue by number".to_string()),
        json!({
            "type": "object",
            "properties": {
                "owner": {
                    "type": "string",
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "issue_number": {
                    "type": "integer",
                    "description": "The issue number"
                }
            },
            "required": ["owner", "repo", "issue_number"]
        }),
    );

    match &cli.command {
        Some(Commands::Chat { message }) => {
            let mut messages = Vec::new();

            // Add system message
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: "You are a helpful assistant. When referring to GitHub issues, use the read_github_issue tool to fetch the details.".to_string(),
                tool_call_id: None,
                tool_calls: None,
            });

            // Process initial message if provided
            if let Some(msg) = message {
                print_colored("user", msg)?;
                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: msg.to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                });

                let (response, _, tool_calls) = service
                    .chat_with_tools_messages(
                        messages.clone(),
                        vec![get_issue_tool.clone()],
                        Some(ToolChoice::Auto("auto".to_string())),
                        false,
                    )
                    .await?;

                print_colored("assistant", &response)?;

                if let Some(tool_calls) = tool_calls {
                    for tool_call in tool_calls {
                        if tool_call.function.name == "get_github_issue" {
                            let args: serde_json::Value =
                                serde_json::from_str(&tool_call.function.arguments)?;
                            let owner = args["owner"].as_str().unwrap_or("OpenAgentsInc");
                            let repo = args["repo"].as_str().unwrap_or("openagents");
                            let issue_number = args["issue_number"].as_i64().unwrap_or(0) as i32;

                            print_colored(
                                "system",
                                &format!(
                                    "Fetching GitHub issue #{} from {}/{}",
                                    issue_number, owner, repo
                                ),
                            )?;

                            let issue = github_service.get_issue(owner, repo, issue_number).await?;

                            messages.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: format!(
                                    "Let me fetch GitHub issue #{} for you.",
                                    issue_number
                                ),
                                tool_call_id: None,
                                tool_calls: Some(vec![tool_call.clone()]),
                            });

                            messages.push(ChatMessage {
                                role: "tool".to_string(),
                                content: serde_json::to_string(&issue)?,
                                tool_call_id: Some(tool_call.id),
                                tool_calls: None,
                            });

                            let (response, _, _) = service
                                .chat_with_tools_messages(
                                    messages.clone(),
                                    vec![get_issue_tool.clone()],
                                    None,
                                    false,
                                )
                                .await?;

                            print_colored("assistant", &response)?;
                        }
                    }
                }
            }

            // Interactive chat loop
            loop {
                let mut stdout = StandardStream::stdout(ColorChoice::Always);
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Blue)).set_bold(true))?;
                write!(stdout, "User:")?;
                stdout.reset()?;
                write!(stdout, " ")?;
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let input = input.trim();

                if input.is_empty() {
                    break;
                }

                messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: input.to_string(),
                    tool_call_id: None,
                    tool_calls: None,
                });

                let (response, _, tool_calls) = service
                    .chat_with_tools_messages(
                        messages.clone(),
                        vec![get_issue_tool.clone()],
                        Some(ToolChoice::Auto("auto".to_string())),
                        false,
                    )
                    .await?;

                print_colored("assistant", &response)?;

                if let Some(tool_calls) = tool_calls {
                    for tool_call in tool_calls {
                        if tool_call.function.name == "get_github_issue" {
                            let args: serde_json::Value =
                                serde_json::from_str(&tool_call.function.arguments)?;
                            let owner = args["owner"].as_str().unwrap_or("OpenAgentsInc");
                            let repo = args["repo"].as_str().unwrap_or("openagents");
                            let issue_number = args["issue_number"].as_i64().unwrap_or(0) as i32;

                            print_colored(
                                "system",
                                &format!(
                                    "Fetching GitHub issue #{} from {}/{}",
                                    issue_number, owner, repo
                                ),
                            )?;

                            let issue = github_service.get_issue(owner, repo, issue_number).await?;

                            messages.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: format!(
                                    "Let me fetch GitHub issue #{} for you.",
                                    issue_number
                                ),
                                tool_call_id: None,
                                tool_calls: Some(vec![tool_call.clone()]),
                            });

                            messages.push(ChatMessage {
                                role: "tool".to_string(),
                                content: serde_json::to_string(&issue)?,
                                tool_call_id: Some(tool_call.id),
                                tool_calls: None,
                            });

                            let (response, _, _) = service
                                .chat_with_tools_messages(
                                    messages.clone(),
                                    vec![get_issue_tool.clone()],
                                    None,
                                    false,
                                )
                                .await?;

                            print_colored("assistant", &response)?;
                        }
                    }
                }
            }
        }
        None => {
            println!("No command specified. Use --help for usage information.");
        }
    }

    Ok(())
}
