use sqlx::{Pool, Postgres};
use sqlx::postgres::PgPoolOptions;
use crate::event::Event;
use std::collections::HashSet;
use std::error::Error;

pub struct Database {
    pool: Pool<Postgres>
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, Box<dyn Error>> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn save_event(&self, event: &Event) -> Result<(), Box<dyn Error>> {
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

    pub async fn get_events_by_filter(&self, 
        ids: &Option<Vec<String>>,
        authors: &Option<Vec<String>>,
        kinds: &Option<Vec<u64>>,
        since: &Option<u64>,
        until: &Option<u64>,
        limit: &Option<u64>,
        tag_filters: &[(char, HashSet<String>)]
    ) -> Result<Vec<Event>, Box<dyn Error>> {
        let mut query = String::from(
            "SELECT id, pubkey, created_at, kind, content, sig, tags 
             FROM events 
             WHERE 1=1"
        );
        let mut params = vec![];

        if let Some(ids) = ids {
            query.push_str(" AND id = ANY($1)");
            params.push(serde_json::to_value(ids)?);
        }

        if let Some(authors) = authors {
            query.push_str(" AND pubkey = ANY($2)");
            params.push(serde_json::to_value(authors)?);
        }

        if let Some(kinds) = kinds {
            query.push_str(" AND kind = ANY($3)");
            params.push(serde_json::to_value(kinds)?);
        }

        if let Some(since) = since {
            query.push_str(" AND created_at >= $4");
            params.push(serde_json::to_value(since)?);
        }

        if let Some(until) = until {
            query.push_str(" AND created_at <= $5");
            params.push(serde_json::to_value(until)?);
        }

        // Add tag filters
        for (_i, (tag_char, values)) in tag_filters.iter().enumerate() {
            query.push_str(&format!(
                " AND tags @> ${}",
                params.len() + 1
            ));
            
            // Create JSONB array of tag values
            let tag_array: Vec<Vec<String>> = values
                .iter()
                .map(|v| vec![tag_char.to_string(), v.clone()])
                .collect();
            
            params.push(serde_json::to_value(tag_array)?);
        }

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let events = sqlx::query_as::<_, Event>(&query)
            .fetch_all(&self.pool)
            .await?;

        Ok(events)
    }
}
