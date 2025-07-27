use async_trait::async_trait;
use convex::{ConvexClient as OfficialConvexClient, Value as ConvexValue, FunctionResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use crate::error::AppError;
use super::auth::{AuthService, AuthContext};
use super::database::{
    ConvexDatabase, SessionRepository, MessageRepository, ApmRepository, UserRepository, BatchOperations,
    CreateSessionRequest, UpdateSessionRequest, CreateMessageRequest, UpdateMessageRequest,
    CreateUserRequest, ConvexUser, ApmTimeRange, ApmStats, ApmEvent, ApmFilters,
    BatchQuery, BatchMutation,
};
use super::models::{ConvexSession, ConvexMessage};

/// Enhanced Convex client implementation with database abstractions and authentication
pub struct EnhancedConvexClient {
    client: OfficialConvexClient,
    auth_service: Option<AuthService>,
    manual_auth_token: Option<String>, // For backward compatibility
}

impl EnhancedConvexClient {
    /// Create a new enhanced Convex client
    pub async fn new(convex_url: &str, auth_token: Option<String>) -> Result<Self, AppError> {
        let client = OfficialConvexClient::new(convex_url)
            .await
            .map_err(|e| AppError::ConvexConnectionError(format!("Failed to create Convex client: {}", e)))?;
        
        Ok(Self { 
            client, 
            auth_service: None,
            manual_auth_token: auth_token,
        })
    }

    /// Create a new enhanced Convex client with authentication service
    pub async fn new_with_auth(convex_url: &str, auth_service: AuthService) -> Result<Self, AppError> {
        let client = OfficialConvexClient::new(convex_url)
            .await
            .map_err(|e| AppError::ConvexConnectionError(format!("Failed to create Convex client: {}", e)))?;
        
        Ok(Self { 
            client, 
            auth_service: Some(auth_service),
            manual_auth_token: None,
        })
    }

    /// Set authentication service
    pub fn set_auth_service(&mut self, auth_service: AuthService) {
        self.auth_service = Some(auth_service);
    }

    /// Set manual authentication token (for backward compatibility)
    pub fn set_auth_token(&mut self, token: String) {
        self.manual_auth_token = Some(token);
    }

    /// Clear authentication
    pub fn clear_auth(&mut self) {
        self.auth_service = None;
        self.manual_auth_token = None;
    }

    /// Get current authentication context
    pub fn get_auth_context(&self) -> Option<&AuthContext> {
        self.auth_service.as_ref()?.get_auth_context()
    }

    /// Check if authenticated
    pub fn is_authenticated(&self) -> bool {
        if let Some(auth_service) = &self.auth_service {
            auth_service.is_authenticated()
        } else {
            self.manual_auth_token.is_some()
        }
    }

    /// Convert serde_json::Value to BTreeMap<String, ConvexValue>
    fn convert_args(&self, args: Value) -> Result<BTreeMap<String, ConvexValue>, AppError> {
        let mut result = BTreeMap::new();
        
        if let Value::Object(map) = args {
            for (key, value) in map {
                let convex_value = self.json_to_convex_value(value)?;
                result.insert(key, convex_value);
            }
        }
        
        // Add authentication information if available
        if let Some(auth_service) = &self.auth_service {
            if let Some(auth_context) = auth_service.get_auth_context() {
                // Add user context to the arguments
                result.insert("auth_userId".to_string(), ConvexValue::String(auth_context.user_id.clone()));
                result.insert("auth_githubId".to_string(), ConvexValue::String(auth_context.github_id.clone()));
                result.insert("auth_token".to_string(), ConvexValue::String(auth_context.token.clone()));
                
                if let Some(email) = &auth_context.email {
                    result.insert("auth_email".to_string(), ConvexValue::String(email.clone()));
                }
            }
        } else if let Some(token) = &self.manual_auth_token {
            // Fallback to manual token for backward compatibility
            result.insert("auth_token".to_string(), ConvexValue::String(token.clone()));
        }
        
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
    async fn execute_operation<T>(&mut self, operation_type: &str, function_name: &str, args: Value) -> Result<T, AppError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
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