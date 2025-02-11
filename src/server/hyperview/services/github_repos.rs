use anyhow::Result;
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub description: Option<String>,
    pub html_url: String,
}

pub struct GitHubReposService {
    client: Octocrab,
}

impl GitHubReposService {
    pub fn new(token: String) -> Result<Self> {
        let client = Octocrab::builder()
            .personal_token(token)
            .build()?;

        Ok(Self { client })
    }

    pub async fn get_user_repos(&self) -> Result<Vec<RepoInfo>> {
        let page = self.client
            .current()
            .list_repos_for_authenticated_user()
            .type_("owner")
            .sort("updated")
            .per_page(100)
            .send()
            .await?;

        let repos = page.items
            .into_iter()
            .map(|repo| RepoInfo {
                name: repo.name,
                description: repo.description,
                html_url: repo.html_url
                    .map(|url| url.to_string())
                    .unwrap_or_default(),
            })
            .collect();

        Ok(repos)
    }
}
