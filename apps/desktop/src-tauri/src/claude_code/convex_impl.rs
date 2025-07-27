use async_trait::async_trait;
use convex::{ConvexClient as OfficialConvexClient, Value as ConvexValue, FunctionResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use reqwest::Client as HttpClient;
use crate::error::AppError;
use super::auth::{AuthService, AuthContext};
use super::token_storage::{TokenStorage, TokenInfo};
use super::database::{
    ConvexDatabase, SessionRepository, MessageRepository, ApmRepository, UserRepository, BatchOperations,
    CreateSessionRequest, UpdateSessionRequest, CreateMessageRequest, UpdateMessageRequest,
    CreateUserRequest, ConvexUser, ApmTimeRange, ApmStats, ApmEvent, ApmFilters,
    BatchQuery, BatchMutation,
};
use super::models::{ConvexSession, ConvexMessage};
use std::time::{SystemTime, UNIX_EPOCH};

/// Enhanced Convex client implementation with database abstractions and authentication
/// 
/// Phase 3: Updated with HTTP client, token storage, and refresh logic
pub struct EnhancedConvexClient {
    client: OfficialConvexClient,
    http_client: HttpClient,
    convex_url: String,
    auth_service: Option<AuthService>,
    auth_token: Option<String>, // JWT token for Authorization header
    token_storage: TokenStorage,
    openauth_domain: String,
}

impl EnhancedConvexClient {
    /// Create a new enhanced Convex client
    /// 
    /// Phase 3: Updated with HTTP client, token storage, and refresh logic
    pub async fn new(convex_url: &str, auth_token: Option<String>) -> Result<Self, AppError> {
        let client = OfficialConvexClient::new(convex_url)
            .await
            .map_err(|e| AppError::ConvexConnectionError(format!("Failed to create Convex client: {}", e)))?;
        
        let http_client = HttpClient::new();
        let mut token_storage = TokenStorage::new();
        token_storage.load_from_storage()?;
        
        let openauth_domain = std::env::var("OPENAUTH_DOMAIN")
            .unwrap_or_else(|_| "https://auth.openagents.com".to_string());
        
        Ok(Self { 
            client, 
            http_client,
            convex_url: convex_url.to_string(),
            auth_service: None,
            auth_token,
            token_storage,
            openauth_domain,
        })
    }

    /// Create a new enhanced Convex client with authentication service
    /// 
    /// Phase 3: Updated with HTTP client, token storage, and refresh logic
    pub async fn new_with_auth(convex_url: &str, auth_service: AuthService) -> Result<Self, AppError> {
        let client = OfficialConvexClient::new(convex_url)
            .await
            .map_err(|e| AppError::ConvexConnectionError(format!("Failed to create Convex client: {}", e)))?;
        
        let http_client = HttpClient::new();
        let mut token_storage = TokenStorage::new();
        token_storage.load_from_storage()?;
        
        let openauth_domain = std::env::var("OPENAUTH_DOMAIN")
            .unwrap_or_else(|_| "https://auth.openagents.com".to_string());
        
        // Extract token from auth service if available
        let auth_token = auth_service.get_auth_context()
            .map(|ctx| ctx.token.clone());
        
        Ok(Self { 
            client, 
            http_client,
            convex_url: convex_url.to_string(),
            auth_service: Some(auth_service),
            auth_token,
            token_storage,
            openauth_domain,
        })
    }

    /// Set authentication service
    pub fn set_auth_service(&mut self, auth_service: AuthService) {
        self.auth_service = Some(auth_service);
    }

    /// Set authentication token for Authorization header
    /// 
    /// Phase 2: Updated method name and purpose - now sets JWT token for Authorization header
    pub fn set_auth_token(&mut self, token: String) {
        self.auth_token = Some(token);
    }

    /// Clear authentication
    /// 
    /// Phase 2: Updated to clear auth_token instead of manual_auth_token
    pub fn clear_auth(&mut self) {
        self.auth_service = None;
        self.auth_token = None;
    }

    /// Get current authentication context
    pub fn get_auth_context(&self) -> Option<&AuthContext> {
        self.auth_service.as_ref()?.get_auth_context()
    }

    /// Check if authenticated
    /// 
    /// Phase 2: Updated to check auth_token instead of manual_auth_token
    pub fn is_authenticated(&self) -> bool {
        if let Some(auth_service) = &self.auth_service {
            auth_service.is_authenticated()
        } else {
            self.auth_token.is_some()
        }
    }

    /// Store authentication token securely
    /// 
    /// Phase 3: Secure token storage with automatic expiration handling
    pub fn store_auth_token(&mut self, token: String, expires_at: Option<u64>) -> Result<(), AppError> {
        // Store token in both places for compatibility
        self.auth_token = Some(token.clone());
        self.token_storage.store_token("access_token", token, expires_at)?;
        log::info!("Stored authentication token");
        Ok(())
    }

    /// Get valid authentication token (with automatic refresh if needed)
    /// 
    /// Phase 3: Automatic token validation and refresh
    pub async fn get_valid_token(&mut self) -> Result<Option<String>, AppError> {
        // Check if token needs refresh
        if self.token_needs_refresh()? {
            log::info!("Token needs refresh, attempting to refresh");
            self.refresh_token().await?;
        }

        // Try to get token from storage first
        if let Some(token) = self.token_storage.get_token("access_token")? {
            self.auth_token = Some(token.clone());
            return Ok(Some(token));
        }

        // Fallback to auth service token
        if let Some(auth_service) = &self.auth_service {
            if let Some(auth_context) = auth_service.get_auth_context() {
                return Ok(Some(auth_context.token.clone()));
            }
        }

        // Fallback to stored token
        Ok(self.auth_token.clone())
    }

    /// Check if current token needs refresh
    /// 
    /// Phase 3: Token expiration checking with configurable buffer
    pub fn token_needs_refresh(&self) -> Result<bool, AppError> {
        let needing_refresh = self.token_storage.get_tokens_needing_refresh(300); // 5 minute buffer
        Ok(needing_refresh.contains(&"access_token".to_string()))
    }

    /// Refresh authentication token
    /// 
    /// Phase 3: Automatic token refresh from OpenAuth
    pub async fn refresh_token(&mut self) -> Result<(), AppError> {
        // TODO: Phase 3 - Implement actual token refresh with OpenAuth
        // For now, this is a placeholder that would:
        // 1. Check if refresh token is available
        // 2. Make request to OpenAuth token endpoint
        // 3. Store new access token
        // 4. Update internal token state
        
        log::warn!("Token refresh not yet implemented - OpenAuth server issues prevent full implementation");
        
        // Placeholder implementation
        // In real implementation, this would make an HTTP request to:
        // POST https://auth.openagents.com/token
        // with refresh_token grant_type
        
        Ok(())
    }

    /// Clear all stored authentication tokens
    /// 
    /// Phase 3: Complete logout with secure token cleanup
    pub fn logout(&mut self) -> Result<(), AppError> {
        self.auth_token = None;
        self.auth_service = None;
        self.token_storage.clear_all_tokens()?;
        log::info!("Logged out - cleared all stored tokens");
        Ok(())
    }

    /// Get token information without exposing the token
    /// 
    /// Phase 3: Token metadata for UI and debugging
    pub fn get_token_info(&self) -> Option<TokenInfo> {
        self.token_storage.get_token_info("access_token")
    }

    /// Get Authorization header value for HTTP requests
    /// 
    /// Phase 3: Enhanced with automatic token refresh
    /// Returns "Bearer {token}" format for use in HTTP requests to Convex
    pub async fn get_authorization_header(&mut self) -> Result<Option<String>, AppError> {
        if let Some(token) = self.get_valid_token().await? {
            Ok(Some(format!("Bearer {}", token)))
        } else {
            Ok(None)
        }
    }

    /// Execute Convex function with HTTP client and Authorization header
    /// 
    /// Phase 3: Direct HTTP API calls to Convex with proper JWT authentication
    /// This bypasses the limitations of the convex Rust client v0.9 for header support
    async fn execute_with_http_auth<T>(&mut self, operation_type: &str, function_name: &str, args: Value) -> Result<T, AppError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let convex_args = self.convert_args(args)?;
        
        // Convert ConvexValue arguments back to JSON for HTTP API
        let json_args = self.convex_args_to_json(convex_args)?;
        
        // Construct Convex HTTP API URL
        let api_url = format!("{}/api/{}/{}", self.convex_url, operation_type, function_name);
        
        // Prepare HTTP request
        let mut request = self.http_client
            .post(&api_url)
            .header("Content-Type", "application/json")
            .json(&json_args);
        
        // Add Authorization header if available
        if let Some(auth_header) = self.get_authorization_header().await? {
            request = request.header("Authorization", auth_header);
            log::debug!("Making authenticated {} request to {}", operation_type, function_name);
        } else {
            log::debug!("Making unauthenticated {} request to {}", operation_type, function_name);
        }
        
        // Execute request
        let response = request
            .send()
            .await
            .map_err(|e| AppError::ConvexDatabaseError(format!("HTTP request failed: {}", e)))?;
        
        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AppError::ConvexDatabaseError(
                format!("Convex HTTP API error {}: {}", status, error_text)
            ));
        }
        
        // Parse response
        let json_response: Value = response
            .json()
            .await
            .map_err(|e| AppError::ConvexDatabaseError(format!("Failed to parse response: {}", e)))?;
        
        // Deserialize to target type
        serde_json::from_value(json_response)
            .map_err(|e| AppError::ConvexDatabaseError(format!("Failed to deserialize response: {}", e)))
    }

    /// Convert ConvexValue arguments back to JSON for HTTP API
    /// 
    /// Phase 3: Helper method for HTTP API compatibility
    fn convex_args_to_json(&self, convex_args: BTreeMap<String, ConvexValue>) -> Result<Value, AppError> {
        let mut json_obj = serde_json::Map::new();
        
        for (key, convex_value) in convex_args {
            let json_value = self.convex_value_to_json(convex_value)?;
            json_obj.insert(key, json_value);
        }
        
        Ok(Value::Object(json_obj))
    }

    /// Convert serde_json::Value to BTreeMap<String, ConvexValue>
    /// 
    /// Phase 2: Removed manual auth injection - authentication now handled via Authorization headers
    /// This method now only converts business logic arguments, letting Convex handle authentication
    fn convert_args(&self, args: Value) -> Result<BTreeMap<String, ConvexValue>, AppError> {
        let mut result = BTreeMap::new();
        
        if let Value::Object(map) = args {
            for (key, value) in map {
                let convex_value = self.json_to_convex_value(value)?;
                result.insert(key, convex_value);
            }
        }
        
        // REMOVED: Manual auth injection (Phase 2)
        // Previously added auth_userId, auth_githubId, auth_token to function arguments
        // Now using proper Authorization header approach where Convex handles JWT validation
        // and ctx.auth.getUserIdentity() provides user context automatically
        
        Ok(result)
    }

    /// Convert serde_json::Value to convex::Value
    fn json_to_convex_value(&self, value: Value) -> Result<ConvexValue, AppError> {
        match value {
            Value::Null => Ok(ConvexValue::Null),
            Value::Bool(b) => Ok(ConvexValue::Boolean(b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ConvexValue::Int64(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(ConvexValue::Float64(f))
                } else {
                    Err(AppError::ConvexDatabaseError("Invalid number format".to_string()))
                }
            }
            Value::String(s) => Ok(ConvexValue::String(s)),
            Value::Array(arr) => {
                let mut convex_arr = Vec::new();
                for item in arr {
                    convex_arr.push(self.json_to_convex_value(item)?);
                }
                Ok(ConvexValue::Array(convex_arr))
            }
            Value::Object(obj) => {
                let mut convex_obj = BTreeMap::new();
                for (key, value) in obj {
                    convex_obj.insert(key, self.json_to_convex_value(value)?);
                }
                Ok(ConvexValue::Object(convex_obj))
            }
        }
    }

    /// Convert FunctionResult to serde_json::Value
    fn function_result_to_json(&self, result: FunctionResult) -> Result<Value, AppError> {
        match result {
            FunctionResult::Value(convex_value) => self.convex_value_to_json(convex_value),
            FunctionResult::ErrorMessage(msg) => Err(AppError::ConvexDatabaseError(msg)),
            FunctionResult::ConvexError(err) => Err(AppError::ConvexDatabaseError(format!("Convex error: {:?}", err))),
        }
    }

    /// Convert convex::Value to serde_json::Value
    fn convex_value_to_json(&self, value: ConvexValue) -> Result<Value, AppError> {
        match value {
            ConvexValue::Null => Ok(Value::Null),
            ConvexValue::Boolean(b) => Ok(Value::Bool(b)),
            ConvexValue::Int64(i) => Ok(Value::Number(i.into())),
            ConvexValue::Float64(f) => {
                if f.is_finite() {
                    serde_json::Number::from_f64(f)
                        .map(Value::Number)
                        .ok_or_else(|| AppError::ConvexDatabaseError(format!("Invalid float64 value: {}", f)))
                } else if f.is_nan() {
                    Err(AppError::ConvexDatabaseError("Cannot convert NaN to JSON number".to_string()))
                } else {
                    Err(AppError::ConvexDatabaseError(format!("Cannot convert infinite value to JSON number: {}", f)))
                }
            }
            ConvexValue::String(s) => Ok(Value::String(s)),
            ConvexValue::Array(arr) => {
                let mut json_arr = Vec::new();
                for item in arr {
                    json_arr.push(self.convex_value_to_json(item)?);
                }
                Ok(Value::Array(json_arr))
            }
            ConvexValue::Object(obj) => {
                let mut json_obj = serde_json::Map::new();
                for (key, value) in obj {
                    json_obj.insert(key, self.convex_value_to_json(value)?);
                }
                Ok(Value::Object(json_obj))
            }
            ConvexValue::Bytes(_) => Err(AppError::ConvexDatabaseError("Bytes type not supported".to_string())),
        }
    }

    /// Execute a Convex operation with error handling
    /// 
    /// Phase 3: Uses HTTP client with Authorization headers when available
    /// Falls back to standard Convex client for non-authenticated requests
    async fn execute_operation<T>(&mut self, operation_type: &str, function_name: &str, args: Value) -> Result<T, AppError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Phase 3: Use HTTP client with Authorization headers when we have a token
        if self.get_authorization_header().await?.is_some() {
            log::debug!("Using HTTP client with Authorization header for {} '{}'", operation_type, function_name);
            return self.execute_with_http_auth(operation_type, function_name, args).await;
        }

        // Fallback to standard Convex client for non-authenticated requests
        log::debug!("Using standard Convex client for {} '{}' (no auth)", operation_type, function_name);
        
        let convex_args = self.convert_args(args)?;

        let result = match operation_type {
            "query" => {
                self.client.query(function_name, convex_args)
                    .await
                    .map_err(|e| AppError::ConvexDatabaseError(format!("Query '{}' failed: {}", function_name, e)))?
            }
            "mutation" => {
                self.client.mutation(function_name, convex_args)
                    .await
                    .map_err(|e| AppError::ConvexDatabaseError(format!("Mutation '{}' failed: {}", function_name, e)))?
            }
            _ => return Err(AppError::ConvexDatabaseError("Invalid operation type".to_string())),
        };

        let json_result = self.function_result_to_json(result)?;
        serde_json::from_value(json_result)
            .map_err(|e| AppError::ConvexDatabaseError(format!("Failed to deserialize response: {}", e)))
    }
}

#[async_trait]
impl ConvexDatabase for EnhancedConvexClient {
    async fn query<T>(&mut self, function_name: &str, args: Value) -> Result<T, AppError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        self.execute_operation("query", function_name, args).await
    }

    async fn mutation<T>(&mut self, function_name: &str, args: Value) -> Result<T, AppError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        self.execute_operation("mutation", function_name, args).await
    }

    async fn subscribe<T>(&mut self, function_name: &str, args: Value) -> Result<T, AppError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Note: Subscription implementation would require WebSocket support
        // For now, we'll treat it as a one-time query
        // TODO: Implement proper real-time subscriptions
        self.query(function_name, args).await
    }
}

#[async_trait]
impl SessionRepository for EnhancedConvexClient {
    async fn get_sessions(&mut self, limit: Option<usize>, user_id: Option<String>) -> Result<Vec<ConvexSession>, AppError> {
        let args = json!({
            "limit": limit,
            "userId": user_id
        });
        
        self.query("claude:getSessions", args).await
    }

    async fn create_session(&mut self, session: CreateSessionRequest) -> Result<String, AppError> {
        let args = json!({
            "title": session.title,
            "userId": session.user_id,
            "metadata": session.metadata
        });
        
        self.mutation("claude:createSession", args).await
    }

    async fn update_session(&mut self, session_id: &str, updates: UpdateSessionRequest) -> Result<(), AppError> {
        let args = json!({
            "sessionId": session_id,
            "updates": {
                "title": updates.title,
                "metadata": updates.metadata,
                "status": updates.status
            }
        });
        
        self.mutation("claude:updateSession", args).await
    }

    async fn delete_session(&mut self, session_id: &str) -> Result<(), AppError> {
        let args = json!({ "sessionId": session_id });
        self.mutation("claude:deleteSession", args).await
    }

    async fn get_session_by_id(&mut self, session_id: &str) -> Result<Option<ConvexSession>, AppError> {
        let args = json!({ "sessionId": session_id });
        self.query("claude:getSessionById", args).await
    }
}

#[async_trait]
impl MessageRepository for EnhancedConvexClient {
    async fn get_messages(&mut self, session_id: &str, limit: Option<usize>) -> Result<Vec<ConvexMessage>, AppError> {
        let args = json!({
            "sessionId": session_id,
            "limit": limit
        });
        
        self.query("claude:getMessages", args).await
    }

    async fn add_message(&mut self, message: CreateMessageRequest) -> Result<String, AppError> {
        let args = json!({
            "sessionId": message.session_id,
            "content": message.content,
            "role": message.role,
            "metadata": message.metadata
        });
        
        self.mutation("claude:addMessage", args).await
    }

    async fn update_message(&mut self, message_id: &str, updates: UpdateMessageRequest) -> Result<(), AppError> {
        let args = json!({
            "messageId": message_id,
            "updates": {
                "content": updates.content,
                "metadata": updates.metadata,
                "status": updates.status
            }
        });
        
        self.mutation("claude:updateMessage", args).await
    }

    async fn delete_message(&mut self, message_id: &str) -> Result<(), AppError> {
        let args = json!({ "messageId": message_id });
        self.mutation("claude:deleteMessage", args).await
    }

    async fn get_message_by_id(&mut self, message_id: &str) -> Result<Option<ConvexMessage>, AppError> {
        let args = json!({ "messageId": message_id });
        self.query("claude:getMessageById", args).await
    }
}

#[async_trait]
impl ApmRepository for EnhancedConvexClient {
    async fn get_apm_stats(&mut self, time_range: ApmTimeRange) -> Result<ApmStats, AppError> {
        let args = json!({
            "startTime": time_range.start,
            "endTime": time_range.end
        });
        
        self.query("apm:getStats", args).await
    }

    async fn record_apm_event(&mut self, event: ApmEvent) -> Result<(), AppError> {
        let args = json!({
            "eventType": event.event_type,
            "sessionId": event.session_id,
            "userId": event.user_id,
            "timestamp": event.timestamp,
            "durationMs": event.duration_ms,
            "metadata": event.metadata
        });
        
        self.mutation("apm:recordEvent", args).await
    }

    async fn get_apm_events(&mut self, filters: ApmFilters) -> Result<Vec<ApmEvent>, AppError> {
        let args = json!({
            "eventTypes": filters.event_types,
            "sessionId": filters.session_id,
            "userId": filters.user_id,
            "timeRange": filters.time_range.map(|tr| json!({
                "start": tr.start,
                "end": tr.end
            })),
            "limit": filters.limit
        });
        
        self.query("apm:getEvents", args).await
    }
}

#[async_trait]
impl UserRepository for EnhancedConvexClient {
    async fn get_or_create_user(&mut self, user_data: CreateUserRequest) -> Result<String, AppError> {
        let args = json!({
            "email": user_data.email,
            "name": user_data.name,
            "avatar": user_data.avatar,
            "githubId": user_data.github_id,
            "githubUsername": user_data.github_username
        });
        
        self.mutation("users:getOrCreateUser", args).await
    }

    async fn get_current_user(&mut self) -> Result<Option<ConvexUser>, AppError> {
        let args = json!({});
        self.query("users:getCurrentUser", args).await
    }

    async fn get_user_by_id(&mut self, user_id: &str) -> Result<Option<ConvexUser>, AppError> {
        let args = json!({ "userId": user_id });
        self.query("users:getUserById", args).await
    }
}

#[async_trait]
impl BatchOperations for EnhancedConvexClient {
    async fn batch_query(&mut self, queries: Vec<BatchQuery>) -> Result<Vec<Value>, AppError> {
        if queries.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::with_capacity(queries.len());
        
        // LIMITATION: Currently executing queries sequentially due to Convex client constraints
        // This provides atomicity but sacrifices performance for large batches
        // TODO: Implement true parallel execution when Convex client supports concurrent operations
        // or when we can safely parallelize without affecting data consistency
        
        log::info!("Executing batch of {} queries sequentially", queries.len());
        
        for (index, query) in queries.into_iter().enumerate() {
            match self.query(&query.function_name, query.args).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    log::error!("Batch query failed at index {}: {}", index, e);
                    return Err(AppError::ConvexDatabaseError(
                        format!("Batch query failed at operation {}: {}", index, e)
                    ));
                }
            }
        }
        
        log::info!("Completed batch query execution with {} results", results.len());
        Ok(results)
    }

    async fn batch_mutation(&mut self, mutations: Vec<BatchMutation>) -> Result<Vec<Value>, AppError> {
        if mutations.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::with_capacity(mutations.len());
        
        // DESIGN DECISION: Executing mutations sequentially to maintain data consistency
        // Mutations often have dependencies and side effects that require ordered execution
        // Parallel execution would require careful analysis of dependencies and rollback mechanisms
        // TODO: Consider implementing dependency analysis for safe parallelization
        
        log::info!("Executing batch of {} mutations sequentially for consistency", mutations.len());
        
        for (index, mutation) in mutations.into_iter().enumerate() {
            match self.mutation(&mutation.function_name, mutation.args).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    log::error!("Batch mutation failed at index {}: {}", index, e);
                    // Note: Previous mutations have already been committed
                    // Consider implementing transaction rollback in future versions
                    return Err(AppError::ConvexDatabaseError(
                        format!("Batch mutation failed at operation {} (previous operations committed): {}", index, e)
                    ));
                }
            }
        }
        
        log::info!("Completed batch mutation execution with {} results", results.len());
        Ok(results)
    }
}