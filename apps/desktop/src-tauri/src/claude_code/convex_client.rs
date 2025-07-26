use crate::claude_code::models::{ClaudeError, ConvexSession};
use convex::{ConvexClient as OfficialConvexClient, Value, FunctionResult};
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
        
        // Add limit parameter (Convex expects float64)
        args.insert("limit".to_string(), (limit.unwrap_or(50) as f64).into());
        
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
        
        // Extract data from FunctionResult using proper API
        let sessions = match result {
            FunctionResult::Value(value) => {
                match value {
                    Value::Array(items) => {
                        info!("Found {} sessions in Convex response", items.len());
                        let mut sessions = Vec::new();
                        
                        for item in items {
                            if let Value::Object(map) = item {
                                // Extract session data from the object
                                let session = ConvexSession {
                                    // Convex returns _id as the document ID  
                                    id: map.get("_id")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                                        .unwrap_or_else(|| "unknown".to_string()),
                                    
                                    // _creationTime is the timestamp
                                    creation_time: map.get("_creationTime")
                                        .and_then(|v| if let Value::Float64(f) = v { Some(*f) } else { None })
                                        .unwrap_or(0.0),
                                    
                                    // sessionId field
                                    session_id: map.get("sessionId")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                                        .unwrap_or_else(|| "unknown".to_string()),
                                    
                                    // projectPath field
                                    project_path: map.get("projectPath")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }),
                                    
                                    // title field  
                                    title: map.get("title")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }),
                                    
                                    // status field
                                    status: map.get("status")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }),
                                    
                                    // createdBy field
                                    created_by: map.get("createdBy")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }),
                                    
                                    // lastActivity field
                                    last_activity: map.get("lastActivity")
                                        .and_then(|v| if let Value::Float64(f) = v { Some(*f) } else { None }),
                                    
                                    // userId field  
                                    user_id: map.get("userId")
                                        .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None }),
                                };
                                
                                sessions.push(session);
                            } else {
                                error!("Expected session object, got: {:?}", item);
                            }
                        }
                        
                        info!("Successfully parsed {} sessions from Convex response", sessions.len());
                        sessions
                    }
                    _ => {
                        error!("Expected array from Convex, got: {:?}", value);
                        vec![]
                    }
                }
            }
            FunctionResult::ErrorMessage(err) => {
                error!("Convex query returned error message: {}", err);
                vec![]
            }
            FunctionResult::ConvexError(err) => {
                error!("Convex query returned error: {:?}", err);
                vec![]
            }
        };

        info!("Successfully fetched {} sessions from Convex", sessions.len());
        Ok(sessions)
    }

}