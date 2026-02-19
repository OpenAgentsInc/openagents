use anyhow::{Context, Result};
use octocrab::{Octocrab, OctocrabBuilder};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// GitHub OAuth application credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

/// Stored GitHub access token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubToken {
    pub access_token: String,
    pub token_type: String,
    pub scope: String,
}

/// Connected repository metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedRepo {
    pub owner: String,
    pub name: String,
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
    pub language: Option<String>,
    pub has_issues: bool,
    pub permissions: RepoPermissions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoPermissions {
    pub admin: bool,
    pub push: bool,
    pub pull: bool,
}

/// GitHub client for autopilot operations
pub struct GitHubClient {
    octocrab: Octocrab,
}

impl GitHubClient {
    /// Create a new GitHub client with a personal access token
    pub fn new(token: &str) -> Result<Self> {
        let octocrab = OctocrabBuilder::new()
            .personal_token(token.to_string())
            .build()
            .context("Failed to build GitHub client")?;

        Ok(Self { octocrab })
    }

    /// List repositories accessible to the authenticated user
    pub async fn list_repos(&self) -> Result<Vec<ConnectedRepo>> {
        info!("Fetching user repositories from GitHub");

        let page = self
            .octocrab
            .current()
            .list_repos_for_authenticated_user()
            .per_page(100)
            .send()
            .await
            .context("Failed to list repositories")?;

        let repos: Vec<ConnectedRepo> = page
            .items
            .into_iter()
            .filter_map(|repo| {
                Some(ConnectedRepo {
                    owner: repo.owner?.login,
                    name: repo.name,
                    full_name: repo.full_name?,
                    clone_url: repo.clone_url.map(|u| u.to_string())?,
                    default_branch: repo.default_branch.unwrap_or_else(|| "main".to_string()),
                    language: repo.language.and_then(|v| v.as_str().map(String::from)),
                    has_issues: repo.has_issues.unwrap_or(false),
                    permissions: RepoPermissions {
                        admin: repo.permissions.as_ref()?.admin,
                        push: repo.permissions.as_ref()?.push,
                        pull: repo.permissions.as_ref()?.pull,
                    },
                })
            })
            .collect();

        info!("Found {} repositories", repos.len());
        Ok(repos)
    }

    /// Get a specific repository by owner/name
    pub async fn get_repo(&self, owner: &str, name: &str) -> Result<ConnectedRepo> {
        debug!("Fetching repository {}/{}", owner, name);

        let repo = self
            .octocrab
            .repos(owner, name)
            .get()
            .await
            .context("Failed to get repository")?;

        Ok(ConnectedRepo {
            owner: repo
                .owner
                .ok_or_else(|| anyhow::anyhow!("Missing owner"))?
                .login,
            name: repo.name,
            full_name: repo
                .full_name
                .ok_or_else(|| anyhow::anyhow!("Missing full_name"))?,
            clone_url: repo
                .clone_url
                .ok_or_else(|| anyhow::anyhow!("Missing clone_url"))?
                .to_string(),
            default_branch: repo.default_branch.unwrap_or_else(|| "main".to_string()),
            language: repo.language.and_then(|v| v.as_str().map(String::from)),
            has_issues: repo.has_issues.unwrap_or(false),
            permissions: RepoPermissions {
                admin: repo.permissions.as_ref().map(|p| p.admin).unwrap_or(false),
                push: repo.permissions.as_ref().map(|p| p.push).unwrap_or(false),
                pull: repo.permissions.as_ref().map(|p| p.pull).unwrap_or(false),
            },
        })
    }

    /// Claim an issue by posting a comment
    pub async fn claim_issue(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        agent_identity: &str,
    ) -> Result<()> {
        info!("Claiming issue #{} in {}/{}", issue_number, owner, repo);

        let comment = format!(
            "I'll take this issue.\n\n🤖 Claimed by: {}\n\n_This is an automated response from OpenAgents Autopilot._",
            agent_identity
        );

        self.octocrab
            .issues(owner, repo)
            .create_comment(issue_number, comment)
            .await
            .context("Failed to post claiming comment")?;

        info!("Successfully claimed issue #{}", issue_number);
        Ok(())
    }

    /// Add a label to an issue
    pub async fn add_label(
        &self,
        owner: &str,
        repo: &str,
        issue_number: u64,
        label: &str,
    ) -> Result<()> {
        debug!(
            "Adding label '{}' to issue #{} in {}/{}",
            label, issue_number, owner, repo
        );

        self.octocrab
            .issues(owner, repo)
            .add_labels(issue_number, &[label.to_string()])
            .await
            .context("Failed to add label")?;

        Ok(())
    }

    /// Create a feature branch for an issue
    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch_name: &str,
        base_sha: &str,
    ) -> Result<()> {
        info!(
            "Creating branch '{}' in {}/{} from {}",
            branch_name, owner, repo, base_sha
        );

        let _: serde_json::Value = self
            .octocrab
            .post(
                format!("/repos/{}/{}/git/refs", owner, repo),
                Some(&serde_json::json!({
                    "ref": format!("refs/heads/{}", branch_name),
                    "sha": base_sha
                })),
            )
            .await
            .context("Failed to create branch")?;

        info!("Successfully created branch '{}'", branch_name);
        Ok(())
    }

    /// Create a pull request
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<u64> {
        info!("Creating PR in {}/{}: {} -> {}", owner, repo, head, base);

        let pr = self
            .octocrab
            .pulls(owner, repo)
            .create(title, head, base)
            .body(body)
            .send()
            .await
            .context("Failed to create pull request")?;

        info!("Successfully created PR #{}", pr.number);
        Ok(pr.number)
    }

    /// Post a comment on a pull request
    pub async fn comment_on_pr(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        comment: &str,
    ) -> Result<()> {
        debug!("Posting comment on PR #{} in {}/{}", pr_number, owner, repo);

        self.octocrab
            .issues(owner, repo)
            .create_comment(pr_number, comment)
            .await
            .context("Failed to post PR comment")?;

        Ok(())
    }
}

/// Get the GitHub token storage path
pub fn github_token_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".openagents")
        .join("github_token.json")
}

/// Load GitHub token from disk
pub fn load_github_token() -> Result<GitHubToken> {
    let path = github_token_path();
    let contents = std::fs::read_to_string(&path).context("Failed to read GitHub token file")?;

    let token: GitHubToken =
        serde_json::from_str(&contents).context("Failed to parse GitHub token")?;

    debug!("Loaded GitHub token from {:?}", path);
    Ok(token)
}

/// Save GitHub token to disk
pub fn save_github_token(token: &GitHubToken) -> Result<()> {
    let path = github_token_path();

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create .openagents directory")?;
    }

    let contents = serde_json::to_string_pretty(token).context("Failed to serialize token")?;

    std::fs::write(&path, contents).context("Failed to write GitHub token file")?;

    info!("Saved GitHub token to {:?}", path);
    Ok(())
}

/// Check if GitHub token exists and is valid
pub async fn check_github_auth() -> Result<bool> {
    match load_github_token() {
        Ok(token) => match GitHubClient::new(&token.access_token) {
            Ok(client) => match client.octocrab.current().user().await {
                Ok(user) => {
                    info!("GitHub authenticated as: {}", user.login);
                    Ok(true)
                }
                Err(e) => {
                    warn!("GitHub token is invalid: {}", e);
                    Ok(false)
                }
            },
            Err(e) => {
                warn!("Failed to create GitHub client: {}", e);
                Ok(false)
            }
        },
        Err(_) => {
            debug!("No GitHub token found");
            Ok(false)
        }
    }
}

/// Generate a branch name for an issue
pub fn branch_name_for_issue(issue_number: u64, title: &str) -> String {
    let slug = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join("-");

    format!("autopilot/{}-{}", issue_number, slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branch_name_for_issue() {
        let name = branch_name_for_issue(42, "Fix login bug");
        assert_eq!(name, "autopilot/42-fix-login-bug");

        let name = branch_name_for_issue(123, "Add support for OAuth 2.0 authentication");
        assert_eq!(name, "autopilot/123-add-support-for-oauth");
    }
}
