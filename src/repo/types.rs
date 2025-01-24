use std::path::PathBuf;

pub struct RepoContext {
    pub temp_dir: PathBuf,
    pub api_key: String,
    pub github_token: Option<String>,
}

impl RepoContext {
    pub fn new(temp_dir: PathBuf, api_key: String, github_token: Option<String>) -> Self {
        Self {
            temp_dir,
            api_key,
            github_token,
        }
    }
}
