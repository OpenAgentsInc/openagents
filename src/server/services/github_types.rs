use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub title: String,
    pub body: String,
    pub number: i32,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
}