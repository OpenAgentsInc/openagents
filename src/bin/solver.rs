use anyhow::{Context as _, Result};
use chrono::Local;
use openagents::server::services::deepseek::DeepSeekService;
use openagents::server::services::github_issue::GitHubService;
use openagents::server::services::ollama::OllamaService;
use openagents::solver::state::SolverState;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use tracing::info;
use tracing_subscriber::fmt::MakeWriter;

mod solver_impl;
use solver_impl::{
    changes::{apply_file_changes, generate_changes},
    context::collect_context,
    files::identify_files,
};

const OLLAMA_URL: &str = "http://192.168.1.189:11434";
const DEEPSEEK_URL: &str = "http://localhost:8000";

// Custom writer to capture log output
#[derive(Clone)]
struct LogWriter {
    buffer: std::sync::Arc<Mutex<Vec<String>>>,
}

impl LogWriter {
    fn new() -> Self {
        Self {
            buffer: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_logs(&self) -> Vec<String> {
        self.buffer.lock().unwrap().clone()
    }
}

impl std::io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(s) = String::from_utf8(buf.to_vec()) {
            // Write to stdout as well
            print!("{}", s);
            self.buffer.lock().unwrap().push(s);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stdout().flush()?;
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with custom writer to capture output
    let writer = LogWriter::new();
    let writer_clone = writer.clone();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(move || writer_clone.clone())
        .init();

    info!("Starting solver...");

    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize state
    let mut state = SolverState::new("Initial solver state".to_string());

    // Configuration
    let owner = "OpenAgentsInc";
    let name = "openagents";
    let issue_num = 651;

    info!("Fetching issue #{}", issue_num);

    // Get GitHub token from environment
    let github_token = std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN not set")?;

    // Initialize services
    let github = GitHubService::new(Some(github_token.clone()))
        .context("Failed to initialize GitHub service")?;

    // Use hardcoded Ollama URL but allow override from environment
    let ollama_url = std::env::var("OLLAMA_URL").unwrap_or_else(|_| OLLAMA_URL.to_string());
    let mistral = OllamaService::with_config(&ollama_url, "mistral-small");

    // Initialize DeepSeek service
    let deepseek_url = std::env::var("DEEPSEEK_URL").unwrap_or_else(|_| DEEPSEEK_URL.to_string());
    let deepseek_api_key = std::env::var("DEEPSEEK_API_KEY").context("DEEPSEEK_API_KEY not set")?;
    let deepseek = DeepSeekService::with_base_url(deepseek_api_key, deepseek_url);

    info!("Initialized services");

    // Execute solver loop
    info!("Starting solver loop...");
    let (repo_dir, valid_paths) =
        collect_context(&mut state, &github, owner, name, issue_num).await?;
    info!("Context collected");

    identify_files(&mut state, &mistral, &deepseek, &valid_paths).await?;
    info!("Files identified");

    generate_changes(&mut state, &mistral, &repo_dir).await?;
    info!("Changes generated");

    apply_file_changes(&mut state, &repo_dir).await?;
    info!("Changes applied");

    // Print final state
    println!("\nFinal solver state:");
    println!("Status: {:?}", state.status);
    println!("Files to modify:");
    for file in &state.files {
        println!("- {} (score: {:.2})", file.path, file.relevance_score);
        println!("  Analysis: {}", file.analysis);
        for change in &file.changes {
            println!("  Change:");
            println!("    Search:  {}", change.search);
            println!("    Replace: {}", change.replace);
            println!("    Analysis: {}", change.analysis);
        }
    }

    // Create solve-runs directory if it doesn't exist
    let solve_runs_dir = Path::new("docs/solve-runs");
    fs::create_dir_all(solve_runs_dir)?;

    // Generate timestamp and create log file
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M");
    let log_file_path = solve_runs_dir.join(format!("{}.md", timestamp));

    // Write captured output to file
    let mut log_file = fs::File::create(&log_file_path)?;
    writeln!(log_file, "````bash")?;

    // Write the command that was run
    writeln!(
        log_file,
        "  openagents git:(solver/state-loop-651) cargo run --bin solver -- --issue {}",
        issue_num
    )?;

    // Write the captured output
    for line in writer.get_logs() {
        write!(log_file, "{}", line)?;
    }

    writeln!(log_file, "````")?;

    info!("\nSolver completed successfully.");
    Ok(())
}