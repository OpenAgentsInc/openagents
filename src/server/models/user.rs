use crate::server::models::timestamp::{DateTimeWrapper, Timestamp, TimestampExt};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::JsonValue;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct User {
    pub id: i32,
    pub scramble_id: Option<String>,
    pub github_id: Option<i64>,
    pub github_token: Option<String>,
    pub metadata: JsonValue,
    pub created_at: Timestamp,
    pub last_login_at: Option<Timestamp>,
    pub pseudonym: Option<String>,
}

#[derive(Default)]
pub struct UserBuilder {
    id: i32,
    scramble_id: Option<String>,
    github_id: Option<i64>,
    github_token: Option<String>,
    metadata: Option<JsonValue>,
    created_at: Option<DateTimeWrapper>,
    last_login_at: Option<DateTimeWrapper>,
    pseudonym: Option<String>,
}

impl UserBuilder {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn scramble_id(mut self, scramble_id: Option<String>) -> Self {
        self.scramble_id = scramble_id;
        self
    }

    pub fn github_id(mut self, github_id: Option<i64>) -> Self {
        self.github_id = github_id;
        self
    }

    pub fn github_token(mut self, github_token: Option<String>) -> Self {
        self.github_token = github_token;
        self
    }

    pub fn metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn created_at(mut self, created_at: DateTimeWrapper) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn last_login_at(mut self, last_login_at: Option<DateTimeWrapper>) -> Self {
        self.last_login_at = last_login_at;
        self
    }

    pub fn pseudonym(mut self, pseudonym: Option<String>) -> Self {
        self.pseudonym = pseudonym;
        self
    }

    pub fn build(self) -> User {
        User {
            id: self.id,
            scramble_id: self.scramble_id,
            github_id: self.github_id,
            github_token: self.github_token,
            metadata: self.metadata.expect("metadata should never be null"),
            created_at: self
                .created_at
                .expect("created_at should never be null")
                .into(),
            last_login_at: self.last_login_at.to_timestamp(),
            pseudonym: self.pseudonym,
        }
    }
}

impl User {
    pub fn builder(id: i32) -> UserBuilder {
        UserBuilder::new(id)
    }

    pub fn new(
        id: i32,
        scramble_id: Option<String>,
        github_id: Option<i64>,
        github_token: Option<String>,
        metadata: JsonValue,
        created_at: DateTimeWrapper,
        last_login_at: Option<DateTimeWrapper>,
        pseudonym: Option<String>,
    ) -> Self {
        Self {
            id,
            scramble_id,
            github_id,
            github_token,
            metadata,
            created_at: created_at.0.into(),
            last_login_at: last_login_at.map(|dt| dt.0.into()),
            pseudonym,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub scramble_id: String,
    pub metadata: Option<JsonValue>,
    #[serde(default)]
    pub github_id: Option<i64>,
    #[serde(default)]
    pub github_token: Option<String>,
}
