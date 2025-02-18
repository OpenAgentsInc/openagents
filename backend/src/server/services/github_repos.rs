use anyhow::Result;
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub description: Option<String>,
    pub html_url: String,
    pub updated_at: String,
}

pub struct GitHubReposService {
    client: Octocrab,
}

impl GitHubReposService {
    pub fn new(token: String) -> Result<Self> {
        let client = Octocrab::builder().personal_token(token).build()?;

        Ok(Self { client })
    }

    pub async fn get_user_repos(&self) -> Result<Vec<RepoInfo>> {
        let repos = self
            .client
            .current()
            .list_repos_for_authenticated_user()
            .sort("updated")
            .direction("desc")
            .per_page(100)
            .send()
            .await?;

        let repos = repos
            .into_iter()
            .map(|repo| RepoInfo {
                name: repo.name,
                description: repo.description,
                html_url: repo
                    .html_url
                    .map_or_else(|| "unknown".to_string(), |url| url.to_string()),
                updated_at: repo
                    .updated_at
                    .map_or_else(|| "unknown".to_string(), |dt| dt.to_rfc3339()),
            })
            .collect();

        Ok(repos)
    }
}
