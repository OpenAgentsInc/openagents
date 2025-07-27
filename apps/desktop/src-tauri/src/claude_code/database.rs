use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use super::models::{ConvexSession, ConvexMessage};

/// Database abstraction for Convex operations
#[async_trait]
pub trait ConvexDatabase: Send + Sync {
    /// Execute a query against the Convex database
    async fn query<T>(&mut self, function_name: &str, args: serde_json::Value) -> Result<T, AppError>
    where
        T: for<'de> Deserialize<'de>;

    /// Execute a mutation against the Convex database
    async fn mutation<T>(&mut self, function_name: &str, args: serde_json::Value) -> Result<T, AppError>
    where
        T: for<'de> Deserialize<'de>;

    /// Subscribe to real-time updates
    async fn subscribe<T>(&mut self, function_name: &str, args: serde_json::Value) -> Result<T, AppError>
    where
        T: for<'de> Deserialize<'de>;
}

/// Session management operations
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Get sessions with optional filtering
    async fn get_sessions(&mut self, limit: Option<usize>, user_id: Option<String>) -> Result<Vec<ConvexSession>, AppError>;
    
    /// Create a new session
    async fn create_session(&mut self, session: CreateSessionRequest) -> Result<String, AppError>;
    
    /// Update an existing session
    async fn update_session(&mut self, session_id: &str, updates: UpdateSessionRequest) -> Result<(), AppError>;
    
    /// Delete a session
    async fn delete_session(&mut self, session_id: &str) -> Result<(), AppError>;
    
    /// Get session by ID
    async fn get_session_by_id(&mut self, session_id: &str) -> Result<Option<ConvexSession>, AppError>;
}

/// Message management operations  
#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// Get messages for a session
    async fn get_messages(&mut self, session_id: &str, limit: Option<usize>) -> Result<Vec<ConvexMessage>, AppError>;
    
    /// Add a new message to a session
    async fn add_message(&mut self, message: CreateMessageRequest) -> Result<String, AppError>;
    
    /// Update a message
    async fn update_message(&mut self, message_id: &str, updates: UpdateMessageRequest) -> Result<(), AppError>;
    
    /// Delete a message
    async fn delete_message(&mut self, message_id: &str) -> Result<(), AppError>;
    
    /// Get message by ID
    async fn get_message_by_id(&mut self, message_id: &str) -> Result<Option<ConvexMessage>, AppError>;
}

/// APM (Application Performance Monitoring) operations
#[async_trait]
pub trait ApmRepository: Send + Sync {
    /// Get APM statistics
    async fn get_apm_stats(&mut self, time_range: ApmTimeRange) -> Result<ApmStats, AppError>;
    
    /// Record APM event
    async fn record_apm_event(&mut self, event: ApmEvent) -> Result<(), AppError>;
    
    /// Get APM events with filtering
    async fn get_apm_events(&mut self, filters: ApmFilters) -> Result<Vec<ApmEvent>, AppError>;
}

/// User management operations
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Get or create user from authentication data
    async fn get_or_create_user(&mut self, user_data: CreateUserRequest) -> Result<String, AppError>;
    
    /// Get current authenticated user
    async fn get_current_user(&mut self) -> Result<Option<ConvexUser>, AppError>;
    
    /// Get user by ID
    async fn get_user_by_id(&mut self, user_id: &str) -> Result<Option<ConvexUser>, AppError>;
}

// Request/Response structures for operations

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
    pub user_id: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionRequest {
    pub title: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub session_id: String,
    pub content: String,
    pub role: String, // "user", "assistant", "system"
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMessageRequest {
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub github_id: String,
    pub github_username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvexUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub github_id: String,
    pub github_username: String,
    pub created_at: i64,
    pub last_login: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApmTimeRange {
    pub start: i64, // Unix timestamp
    pub end: i64,   // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApmStats {
    pub total_requests: u64,
    pub avg_response_time: f64,
    pub error_rate: f64,
    pub throughput: f64,
    pub active_sessions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApmEvent {
    pub event_type: String,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub timestamp: i64,
    pub duration_ms: Option<f64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApmFilters {
    pub event_types: Option<Vec<String>>,
    pub session_id: Option<String>,
    pub user_id: Option<String>,
    pub time_range: Option<ApmTimeRange>,
    pub limit: Option<usize>,
}

/// Batch operations for performance
#[async_trait]
pub trait BatchOperations: Send + Sync {
    /// Execute multiple queries in batch
    async fn batch_query(&mut self, queries: Vec<BatchQuery>) -> Result<Vec<serde_json::Value>, AppError>;
    
    /// Execute multiple mutations in batch  
    async fn batch_mutation(&mut self, mutations: Vec<BatchMutation>) -> Result<Vec<serde_json::Value>, AppError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQuery {
    pub function_name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]  
pub struct BatchMutation {
    pub function_name: String,
    pub args: serde_json::Value,
}