use crate::claude_code::models::{ClaudeError, ConvexSession};
use convex::{ConvexClient as OfficialConvexClient, Value};
use log::{error, info};
use std::collections::BTreeMap;

pub struct ConvexClient {
    client: OfficialConvexClient,
}

impl ConvexClient {
    pub async fn new(convex_url: &str) -> Result<Self, ClaudeError> {
        info!("Creating Convex client for URL: {}", convex_url);
        
        let client = OfficialConvexClient::new(convex_url)
            .await
            .map_err(|e| {
                error!("Failed to create Convex client: {}", e);
                ClaudeError::Other(format!("Failed to create Convex client: {}", e))
            })?;
        
        info!("Convex client created successfully");
        Ok(Self { client })
    }

    /// Fetch sessions from Convex database using official client
    pub async fn get_sessions(&mut self, limit: Option<usize>, user_id: Option<String>) -> Result<Vec<ConvexSession>, ClaudeError> {
        // Prepare the query arguments as BTreeMap (following quickstart pattern)
        let mut args = BTreeMap::new();
        
        // Add limit parameter
        args.insert("limit".to_string(), (limit.unwrap_or(50) as i64).into());
        
        if let Some(uid) = user_id {
            args.insert("userId".to_string(), uid.into());
        }

        info!("Calling Convex query 'claude:getSessions' with args: {:?}", args);

        // Call the query using official client
        let result = self.client
            .query("claude:getSessions", args)
            .await
            .map_err(|e| {
                error!("Convex query failed: {}", e);
                ClaudeError::Other(format!("Convex query failed: {}", e))
            })?;

        info!("Convex query successful!");
        
        // Following quickstart pattern: println!("{result:#?}");
        info!("Query result: {result:#?}");
        
        // For now, return empty sessions until we figure out the result parsing
        // We need to understand how to extract data from the FunctionResult
        info!("Returning empty sessions list temporarily - need to parse FunctionResult");
        let sessions: Vec<ConvexSession> = vec![];

        info!("Successfully fetched {} sessions from Convex", sessions.len());
        Ok(sessions)
    }

    /// Test connection to Convex
    pub async fn test_connection(&mut self) -> Result<bool, ClaudeError> {
        info!("Testing Convex connection...");

        let mut args = BTreeMap::new();
        args.insert("limit".to_string(), Value::from(1i64));

        match self.client.query("claude:getSessions", args).await {
            Ok(_) => {
                info!("Convex connection test successful");
                Ok(true)
            }
            Err(e) => {
                error!("Convex connection test failed: {}", e);
                Ok(false)
            }
        }
    }
}