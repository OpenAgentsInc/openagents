use crate::nostr::event::Event;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use std::time::Duration;
use tracing::{error, info};

pub struct Database {
    pool: Pool<Postgres>,
}

#[derive(Default)]
pub struct EventFilter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<i32>>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<u64>,
    pub tag_filters: Vec<(char, HashSet<String>)>,
}

impl Database {
    pub async fn new_with_options(options: PgConnectOptions) -> Result<Self, anyhow::Error> {
        info!("Attempting to connect to database...");

        match PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(30))
            .connect_with(options.clone())
            .await
        {
            Ok(pool) => {
                info!("Database connection successful, running migrations...");
                match sqlx::migrate!("./migrations").run(&pool).await {
                    Ok(_) => {
                        info!("Migrations completed successfully");
                        Ok(Self { pool })
                    }
                    Err(e) => {
                        error!("Migration failed: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Database connection failed: {}", e);
                Err(e.into())
            }
        }
    }

    pub async fn save_event(&self, event: &Event) -> Result<(), anyhow::Error> {
        sqlx::query_as::<_, Event>(
            r#"
            INSERT INTO events (id, pubkey, created_at, kind, content, sig, tags)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, pubkey, created_at, kind, content, sig, tags
            "#,
        )
        .bind(&event.id)
        .bind(&event.pubkey)
        .bind(event.created_at)
        .bind(event.kind)
        .bind(&event.content)
        .bind(&event.sig)
        .bind(serde_json::to_value(&event.tags)?)
        .fetch_one(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_events_by_filter(
        &self,
        filter: EventFilter,
    ) -> Result<Vec<Event>, anyhow::Error> {
        let mut query = String::from(
            "SELECT id, pubkey, created_at, kind, content, sig, tags 
             FROM events 
             WHERE 1=1",
        );
        let mut params = vec![];

        if let Some(ids) = filter.ids {
            query.push_str(" AND id = ANY($1)");
            params.push(serde_json::to_value(ids)?);
        }

        if let Some(authors) = filter.authors {
            query.push_str(" AND pubkey = ANY($2)");
            params.push(serde_json::to_value(authors)?);
        }

        if let Some(kinds) = filter.kinds {
            query.push_str(" AND kind = ANY($3)");
            params.push(serde_json::to_value(kinds)?);
        }

        if let Some(since) = filter.since {
            query.push_str(" AND created_at >= $4");
            params.push(serde_json::to_value(since)?);
        }

        if let Some(until) = filter.until {
            query.push_str(" AND created_at <= $5");
            params.push(serde_json::to_value(until)?);
        }

        // Add tag filters
        for (tag_char, values) in filter.tag_filters.iter() {
            query.push_str(&format!(" AND tags @> ${}", params.len() + 1));

            // Create JSONB array of tag values
            let tag_array: Vec<Vec<String>> = values
                .iter()
                .map(|v| vec![tag_char.to_string(), v.clone()])
                .collect();

            params.push(serde_json::to_value(tag_array)?);
        }

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let events = sqlx::query_as::<_, Event>(&query)
            .fetch_all(&self.pool)
            .await?;

        Ok(events)
    }
}