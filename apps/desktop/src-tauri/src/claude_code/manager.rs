use crate::claude_code::models::*;
use chrono::Utc;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

pub struct ClaudeManager {
    sessions: Arc<RwLock<HashMap<String, Arc<Mutex<ClaudeSession>>>>>,
    binary_path: Option<PathBuf>,
    app_handle: Option<AppHandle>,
}

impl ClaudeManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            binary_path: None,
            app_handle: None,
        }
    }

    pub fn set_binary_path(&mut self, path: PathBuf) {
        self.binary_path = Some(path);
    }

    pub fn set_app_handle(&mut self, handle: AppHandle) {
        self.app_handle = Some(handle);
    }

    pub async fn create_session(&self, project_path: String) -> Result<String, ClaudeError> {
        let binary_path = self.binary_path.as_ref()
            .ok_or(ClaudeError::BinaryNotFound)?;

        let session_id = Uuid::new_v4().to_string();
        let session = Arc::new(Mutex::new(ClaudeSession::new(
            session_id.clone(),
            project_path,
            binary_path.clone(),
            self.app_handle.clone(),
        )));

        // Start the session
        {
            let mut session_lock = session.lock().await;
            session_lock.start().await?;
        }

        // Store the session
        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id.clone(), session.clone());
        }

        Ok(session_id)
    }

    pub async fn send_message(&self, session_id: &str, message: String) -> Result<(), ClaudeError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?
            .clone();

        // Send message without holding the session lock for the entire duration
        ClaudeSession::send_message_static(session, message).await
    }

    pub async fn trigger_response(&self, session_id: &str, message: String) -> Result<(), ClaudeError> {
        info!("🎯 [MANAGER] trigger_response called - will NOT create user message");
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?
            .clone();

        // Trigger response without creating a user message
        ClaudeSession::trigger_response_static(session, message).await
    }

    pub async fn get_messages(&self, session_id: &str) -> Result<Vec<Message>, ClaudeError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?;

        let mut session_lock = session.lock().await;
        
        // Process any pending output lines
        session_lock.process_pending_output().await;
        
        Ok(session_lock.messages.clone())
    }

    pub async fn stop_session(&self, session_id: &str) -> Result<(), ClaudeError> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.remove(session_id) {
            let mut session_lock = session.lock().await;
            session_lock.stop().await?;
        }
        Ok(())
    }

    pub async fn get_active_sessions(&self) -> Vec<(String, String)> {
        let sessions = self.sessions.read().await;
        let mut result = Vec::new();

        for (id, session) in sessions.iter() {
            let session_lock = session.lock().await;
            if session_lock.is_active {
                result.push((id.clone(), session_lock.project_path.clone()));
            }
        }

        result
    }

}

pub struct ClaudeSession {
    pub id: String,
    pub project_path: String,
    pub binary_path: PathBuf,
    pub messages: Vec<Message>,
    pub is_active: bool,
    pub first_message: Option<String>,
    pub summary: Option<String>,
    pub version: Option<String>, // Claude Code version
    claude_session_id: Option<String>, // Claude's internal session ID
    pending_tool_uses: HashMap<String, (String, HashMap<String, serde_json::Value>)>,
    message_subscribers: Vec<mpsc::Sender<Message>>,
    message_id_map: HashMap<String, Uuid>, // Maps Claude message IDs to our internal UUIDs
    app_handle: Option<AppHandle>, // For emitting events
}

impl ClaudeSession {
    fn new(id: String, project_path: String, binary_path: PathBuf, app_handle: Option<AppHandle>) -> Self {
        Self {
            id,
            project_path,
            binary_path,
            messages: Vec::new(),
            is_active: true,
            first_message: None,
            summary: None,
            version: None,
            claude_session_id: None,
            pending_tool_uses: HashMap::new(),
            message_subscribers: Vec::new(),
            message_id_map: HashMap::new(),
            app_handle,
        }
    }

    async fn send_message_static(
        session: Arc<Mutex<ClaudeSession>>, 
        message: String
    ) -> Result<(), ClaudeError> {
        // Get session info we need
        let (project_path, binary_path, claude_session_id) = {
            let session_lock = session.lock().await;
            (
                session_lock.project_path.clone(),
                session_lock.binary_path.clone(),
                session_lock.claude_session_id.clone(),
            )
        };

        // Add user message
        {
            let mut session_lock = session.lock().await;
            let user_msg_id = Uuid::new_v4();
            info!("🚨 [RUST] Creating USER MESSAGE in send_message with ID: {}", user_msg_id);
            info!("🚨 [RUST] Message content: {}", message.clone());
            info!("🚨 [RUST] This is what creates the DUPLICATE!");
            
            let user_msg = Message {
                id: user_msg_id,
                message_type: MessageType::User,
                content: message.clone(),
                timestamp: Utc::now(),
                tool_info: None,
            };
            session_lock.add_message(user_msg).await;

            // Store first message for preview
            if session_lock.first_message.is_none() {
                session_lock.first_message = Some(message.clone());
            }
        }

        // Build the command
        let claude_command = if claude_session_id.is_some() {
            format!(
                "cd \"{}\" && MAX_THINKING_TOKENS=31999 \"{}\" -p --continue \"{}\" --output-format stream-json --verbose --dangerously-skip-permissions",
                project_path, 
                binary_path.display(), 
                message.replace("\"", "\\\"")
            )
        } else {
            format!(
                "cd \"{}\" && MAX_THINKING_TOKENS=31999 \"{}\" -p \"{}\" --output-format stream-json --verbose --dangerously-skip-permissions",
                project_path,
                binary_path.display(), 
                message.replace("\"", "\\\"")
            )
        };

        info!("Running command: {}", claude_command);

        // Execute the command with streaming output
        let mut child = Command::new("/bin/bash")
            .args(&["-l", "-c", &claude_command])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ClaudeError::Other(format!("Failed to execute Claude Code: {}", e)))?;

        // Get stdout and stderr handles
        let stdout = child.stdout.take()
            .ok_or_else(|| ClaudeError::Other("Failed to capture stdout".to_string()))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| ClaudeError::Other("Failed to capture stderr".to_string()))?;

        // Create readers for streaming
        use tokio::io::{AsyncBufReadExt, BufReader};
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        // Spawn the child process and read output in separate tasks
        let mut stdout_lines = stdout_reader.lines();
        let mut stderr_lines = stderr_reader.lines();
        
        // Create a channel for the child process status
        let (status_tx, mut status_rx) = mpsc::channel::<std::process::ExitStatus>(1);
        
        // Spawn task to wait for child process
        tokio::spawn(async move {
            if let Ok(status) = child.wait().await {
                let _ = status_tx.send(status).await;
            }
        });

        let mut error_buffer = String::new();

        // Process stdout and stderr as they come in
        loop {
            tokio::select! {
                // Read from stdout
                Ok(Some(line)) = stdout_lines.next_line() => {
                    if !line.trim().is_empty() {
                        let mut session_lock = session.lock().await;
                        session_lock.process_output_line(&line).await;
                    }
                }
                // Read from stderr
                Ok(Some(line)) = stderr_lines.next_line() => {
                    warn!("Claude Code stderr: {}", line);
                    error_buffer.push_str(&line);
                    error_buffer.push('\n');
                }
                // Check if process has exited
                Some(status) = status_rx.recv() => {
                    // Continue reading any remaining output
                    
                    // Read any remaining stdout
                    while let Ok(Some(line)) = stdout_lines.next_line().await {
                        if !line.trim().is_empty() {
                            let mut session_lock = session.lock().await;
                            session_lock.process_output_line(&line).await;
                        }
                    }
                    
                    // Read any remaining stderr
                    while let Ok(Some(line)) = stderr_lines.next_line().await {
                        warn!("Claude Code stderr: {}", line);
                        error_buffer.push_str(&line);
                        error_buffer.push('\n');
                    }
                    
                    // Check exit status
                    if !status.success() {
                        return Err(ClaudeError::Other(format!(
                            "Claude Code exited with status: {}",
                            status
                        )));
                    }
                    
                    break;
                }
                // If all channels are closed, we're done
                else => {
                    break;
                }
            }
        }

        // If there were errors, add an error message
        if !error_buffer.trim().is_empty() {
            let mut session_lock = session.lock().await;
            let error_msg = Message {
                id: Uuid::new_v4(),
                message_type: MessageType::Error,
                content: format!("Error: {}", error_buffer.trim()),
                timestamp: Utc::now(),
                tool_info: None,
            };
            session_lock.add_message(error_msg).await;
        }

        Ok(())
    }

    async fn trigger_response_static(
        session: Arc<Mutex<ClaudeSession>>, 
        message: String
    ) -> Result<(), ClaudeError> {
        info!("🎯 [SESSION] trigger_response_static - Triggering Claude WITHOUT creating user message");
        
        // Get session info we need
        let (project_path, binary_path, claude_session_id) = {
            let session_lock = session.lock().await;
            (
                session_lock.project_path.clone(),
                session_lock.binary_path.clone(),
                session_lock.claude_session_id.clone(),
            )
        };

        // DO NOT ADD USER MESSAGE - just trigger Claude Code

        // Build the command
        let claude_command = if claude_session_id.is_some() {
            format!(
                "cd \"{}\" && MAX_THINKING_TOKENS=31999 \"{}\" -p --continue \"{}\" --output-format stream-json --verbose --dangerously-skip-permissions",
                project_path, 
                binary_path.display(), 
                message.replace("\"", "\\\"")
            )
        } else {
            format!(
                "cd \"{}\" && MAX_THINKING_TOKENS=31999 \"{}\" -p \"{}\" --output-format stream-json --verbose --dangerously-skip-permissions",
                project_path,
                binary_path.display(), 
                message.replace("\"", "\\\"")
            )
        };

        info!("Running command: {}", claude_command);

        // Execute the command with streaming output
        let mut child = Command::new("/bin/bash")
            .args(&["-l", "-c", &claude_command])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| ClaudeError::Other(format!("Failed to execute Claude Code: {}", e)))?;

        // Get stdout and stderr handles
        let stdout = child.stdout.take()
            .ok_or_else(|| ClaudeError::Other("Failed to capture stdout".to_string()))?;
        let stderr = child.stderr.take()
            .ok_or_else(|| ClaudeError::Other("Failed to capture stderr".to_string()))?;

        // Create readers for streaming
        use tokio::io::{AsyncBufReadExt, BufReader};
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        // Spawn the child process and read output in separate tasks
        let mut stdout_lines = stdout_reader.lines();
        let mut stderr_lines = stderr_reader.lines();
        
        // Create a channel for the child process status
        let (status_tx, mut status_rx) = mpsc::channel::<std::process::ExitStatus>(1);
        
        // Spawn task to wait for child process
        tokio::spawn(async move {
            if let Ok(status) = child.wait().await {
                let _ = status_tx.send(status).await;
            }
        });

        let mut error_buffer = String::new();

        // Process stdout and stderr as they come in
        loop {
            tokio::select! {
                // Read from stdout
                Ok(Some(line)) = stdout_lines.next_line() => {
                    if !line.trim().is_empty() {
                        let mut session_lock = session.lock().await;
                        session_lock.process_output_line(&line).await;
                    }
                }
                // Read from stderr
                Ok(Some(line)) = stderr_lines.next_line() => {
                    warn!("Claude Code stderr: {}", line);
                    error_buffer.push_str(&line);
                    error_buffer.push('\n');
                }
                // Check if process has exited
                Some(status) = status_rx.recv() => {
                    // Continue reading any remaining output
                    
                    // Read any remaining stdout
                    while let Ok(Some(line)) = stdout_lines.next_line().await {
                        if !line.trim().is_empty() {
                            let mut session_lock = session.lock().await;
                            session_lock.process_output_line(&line).await;
                        }
                    }
                    
                    // Read any remaining stderr
                    while let Ok(Some(line)) = stderr_lines.next_line().await {
                        warn!("Claude Code stderr: {}", line);
                        error_buffer.push_str(&line);
                        error_buffer.push('\n');
                    }
                    
                    // Check exit status
                    if !status.success() {
                        return Err(ClaudeError::Other(format!(
                            "Claude Code exited with status: {}",
                            status
                        )));
                    }
                    
                    break;
                }
                // If all channels are closed, we're done
                else => {
                    break;
                }
            }
        }

        // If there were errors, add an error message
        if !error_buffer.trim().is_empty() {
            let mut session_lock = session.lock().await;
            let error_msg = Message {
                id: Uuid::new_v4(),
                message_type: MessageType::Error,
                content: format!("Error: {}", error_buffer.trim()),
                timestamp: Utc::now(),
                tool_info: None,
            };
            session_lock.add_message(error_msg).await;
        }

        Ok(())
    }

    async fn start(&mut self) -> Result<(), ClaudeError> {
        info!("Starting Claude Code session for: {}", self.project_path);
        
        // Check Claude version first
        let version_command = format!("\"{}\" --version", self.binary_path.display());
        if let Ok(output) = Command::new("/bin/bash")
            .args(&["-l", "-c", &version_command])
            .output()
            .await
        {
            let version_str = String::from_utf8_lossy(&output.stdout);
            info!("Claude Code version: {}", version_str.trim());
            
            // Store version for later reference
            self.version = Some(version_str.trim().to_string());
            
            // Check for old version
            if version_str.contains("0.2.109") || version_str.contains("0.2.") {
                warn!("You are using an old version of Claude Code. Please update with: claude update");
                
                // Send a warning message to the UI
                let warning_msg = Message {
                    id: Uuid::new_v4(),
                    message_type: MessageType::System,
                    content: format!(
                        "⚠️ Old Claude Code version detected: {}\n\
                        Please update Claude Code by running: claude update",
                        version_str.trim()
                    ),
                    timestamp: Utc::now(),
                    tool_info: None,
                };
                self.add_message(warning_msg).await;
            }
        }
        
        info!("Session initialized - will run Claude Code commands on demand");
        
        Ok(())
    }


    async fn stop(&mut self) -> Result<(), ClaudeError> {
        info!("Stopping Claude Code session: {}", self.id);
        self.is_active = false;
        Ok(())
    }


    async fn add_message(&mut self, message: Message) {
        self.messages.push(message.clone());

        // Notify all subscribers
        self.message_subscribers.retain(|tx| {
            tx.try_send(message.clone()).is_ok()
        });

        // Emit event to frontend
        if let Some(ref app_handle) = self.app_handle {
            let event_name = format!("claude:{}:message", self.id);
            if let Err(e) = app_handle.emit(&event_name, &message) {
                warn!("Failed to emit message event: {}", e);
            }
        }
    }


    async fn process_pending_output(&mut self) {
        // No longer needed since we're using synchronous command execution
    }

    async fn process_output_line(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        
        if !trimmed.starts_with('{') {
            info!("Non-JSON line from Claude: {}", trimmed);
            return;
        }

        match serde_json::from_str::<IncomingMessage>(trimmed) {
            Ok(msg) => {
                // Log concise info about the message instead of the full JSON
                let log_info = self.extract_message_info(&msg);
                info!("Processing line from Claude: {}", log_info);
                
                self.handle_message(msg).await;
            }
            Err(e) => {
                // Only show first 100 chars of the problematic line
                let truncated = if trimmed.len() > 100 {
                    format!("{}...", &trimmed[..100])
                } else {
                    trimmed.to_string()
                };
                warn!("Failed to parse JSON message: {} - Line: {}", e, truncated);
            }
        }
    }

    fn extract_message_info(&self, msg: &IncomingMessage) -> String {
        match msg.msg_type.as_str() {
            "system" => {
                if let Some(subtype) = msg.data.get("subtype").and_then(|v| v.as_str()) {
                    if subtype == "init" {
                        if let Some(session_id) = msg.data.get("session_id").and_then(|v| v.as_str()) {
                            format!("system(init, session: {})", &session_id[..8])
                        } else {
                            "system(init)".to_string()
                        }
                    } else {
                        format!("system({})", subtype)
                    }
                } else {
                    "system".to_string()
                }
            }
            "assistant" => {
                if let Some(message) = msg.data.get("message").and_then(|v| v.as_object()) {
                    if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                        let content_types: Vec<String> = content_array.iter()
                            .filter_map(|item| {
                                item.get("type").and_then(|v| v.as_str()).map(|t| {
                                    match t {
                                        "text" => "text".to_string(),
                                        "thinking" => "thinking".to_string(),
                                        "tool_use" => {
                                            if let Some(tool_name) = item.get("name").and_then(|v| v.as_str()) {
                                                format!("tool_use({})", tool_name)
                                            } else {
                                                "tool_use".to_string()
                                            }
                                        }
                                        _ => t.to_string()
                                    }
                                })
                            })
                            .collect();
                        
                        if content_types.is_empty() {
                            "assistant".to_string()
                        } else {
                            format!("assistant({})", content_types.join(", "))
                        }
                    } else {
                        "assistant".to_string()
                    }
                } else {
                    "assistant".to_string()
                }
            }
            "user" => {
                if let Some(message) = msg.data.get("message").and_then(|v| v.as_object()) {
                    if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                        let has_tool_result = content_array.iter()
                            .any(|item| item.get("type").and_then(|v| v.as_str()) == Some("tool_result"));
                        
                        if has_tool_result {
                            "user(tool_result)".to_string()
                        } else {
                            "user".to_string()
                        }
                    } else {
                        "user".to_string()
                    }
                } else {
                    "user".to_string()
                }
            }
            "result" => {
                let is_error = msg.data.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                let subtype = msg.data.get("subtype").and_then(|v| v.as_str()).unwrap_or("unknown");
                if is_error {
                    format!("result(error, {})", subtype)
                } else {
                    format!("result(success, {})", subtype)
                }
            }
            "error" => {
                if let Some(message) = msg.data.get("message").and_then(|v| v.as_str()) {
                    let preview = if message.len() > 50 {
                        format!("{}...", &message[..50])
                    } else {
                        message.to_string()
                    };
                    format!("error({})", preview)
                } else {
                    "error".to_string()
                }
            }
            _ => {
                format!("{}(?)", msg.msg_type)
            }
        }
    }

    async fn handle_message(&mut self, msg: IncomingMessage) {
        match msg.msg_type.as_str() {
            "system" => {
                if let Some(subtype) = msg.data.get("subtype").and_then(|v| v.as_str()) {
                    if subtype == "init" {
                        // Capture the Claude session ID
                        if let Some(session_id) = msg.data.get("session_id").and_then(|v| v.as_str()) {
                            self.claude_session_id = Some(session_id.to_string());
                            info!("Claude Code initialized with session ID: {}", session_id);
                        } else {
                            info!("Claude Code initialized for session {}", self.id);
                        }
                        
                        // Send initialization message with version info
                        let version_info = self.version.as_ref()
                            .map(|v| format!(" (version: {})", v))
                            .unwrap_or_default();
                        
                        let init_msg = Message {
                            id: Uuid::new_v4(),
                            message_type: MessageType::System,
                            content: format!("Claude Code initialized{}", version_info),
                            timestamp: Utc::now(),
                            tool_info: None,
                        };
                        self.add_message(init_msg).await;
                    }
                }
            }
            "assistant" => {
                self.handle_assistant_message(&msg.data).await;
            }
            "tool_use" => {
                self.handle_tool_use(&msg.data).await;
            }
            "user" => {
                // Handle tool results that come in user messages
                if let Some(message) = msg.data.get("message").and_then(|v| v.as_object()) {
                    if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                        for item in content_array {
                            if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                match item_type {
                                    "tool_use" => self.handle_tool_use(item).await,
                                    "tool_result" => {
                                        // Check if this is a permission error
                                        if let Some(content) = item.get("content").and_then(|v| v.as_str()) {
                                            if content.contains("Claude requested permissions") {
                                                let error_msg = Message {
                                                    id: Uuid::new_v4(),
                                                    message_type: MessageType::Error,
                                                    content: format!(
                                                        "⚠️ Permission Error: {}\n\n\
                                                        This shouldn't happen with --dangerously-skip-permissions flag. \
                                                        Try one of the following:\n\
                                                        1. Restart Claude Code\n\
                                                        2. Update Claude Code: claude update\n\
                                                        3. Check if Claude Code is running with correct permissions",
                                                        content
                                                    ),
                                                    timestamp: Utc::now(),
                                                    tool_info: None,
                                                };
                                                self.add_message(error_msg).await;
                                            }
                                        }
                                        self.handle_tool_result(item).await;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            "error" => {
                if let Some(message) = msg.data.get("message").and_then(|v| v.as_str()) {
                    let error_msg = Message {
                        id: Uuid::new_v4(),
                        message_type: MessageType::Error,
                        content: message.to_string(),
                        timestamp: Utc::now(),
                        tool_info: None,
                    };
                    self.add_message(error_msg).await;
                }
            }
            "summary" => {
                if let Some(summary) = msg.data.get("content").and_then(|v| v.as_str()) {
                    self.summary = Some(summary.to_string());
                    let summary_msg = Message {
                        id: Uuid::new_v4(),
                        message_type: MessageType::Summary,
                        content: format!("Summary: {}", summary),
                        timestamp: Utc::now(),
                        tool_info: None,
                    };
                    self.add_message(summary_msg).await;
                }
            }
            _ => {
                debug!("Unhandled message type: {} - Data: {:?}", msg.msg_type, msg.data);
            }
        }
    }

    async fn handle_assistant_message(&mut self, data: &serde_json::Value) {
        if let Some(message) = data.get("message").and_then(|v| v.as_object()) {
            // Extract the message ID from Claude's response
            let message_id = message.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            
            if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                let mut text_content = String::new();
                let mut has_thinking = false;
                let mut thinking_content = String::new();

                // Process all content items
                for item in content_array {
                    if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                        match item_type {
                            "text" => {
                                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                    text_content.push_str(text);
                                }
                            }
                            "thinking" => {
                                has_thinking = true;
                                if let Some(thinking) = item.get("thinking").and_then(|v| v.as_str()) {
                                    thinking_content = thinking.to_string();
                                }
                            }
                            "tool_use" => {
                                self.handle_tool_use(item).await;
                            }
                            _ => {}
                        }
                    }
                }

                // Handle thinking message (only create once)
                if has_thinking && !thinking_content.is_empty() {
                    let thinking_key = format!("{}-thinking", message_id);
                    
                    if !self.message_id_map.contains_key(&thinking_key) {
                        let thinking_uuid = Uuid::new_v4();
                        self.message_id_map.insert(thinking_key, thinking_uuid);
                        
                        let thinking_msg = Message {
                            id: thinking_uuid,
                            message_type: MessageType::Thinking,
                            content: thinking_content,
                            timestamp: Utc::now(),
                            tool_info: None,
                        };
                        self.add_message(thinking_msg).await;
                    }
                }

                // Handle assistant message - update if exists, create if new
                if !text_content.is_empty() {
                    // Check if we already have a UUID for this Claude message ID
                    if let Some(&existing_uuid) = self.message_id_map.get(message_id) {
                        // Update existing message
                        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == existing_uuid) {
                            msg.content = text_content;
                            
                            // Notify subscribers of the update
                            let updated_msg = msg.clone();
                            self.message_subscribers.retain(|tx| {
                                tx.try_send(updated_msg.clone()).is_ok()
                            });
                            
                            // Emit event to frontend
                            if let Some(ref app_handle) = self.app_handle {
                                let event_name = format!("claude:{}:message", self.id);
                                if let Err(e) = app_handle.emit(&event_name, &updated_msg) {
                                    warn!("Failed to emit message update event: {}", e);
                                }
                            }
                        }
                    } else {
                        // Create new message
                        let new_uuid = Uuid::new_v4();
                        self.message_id_map.insert(message_id.to_string(), new_uuid);
                        
                        let assistant_msg = Message {
                            id: new_uuid,
                            message_type: MessageType::Assistant,
                            content: text_content,
                            timestamp: Utc::now(),
                            tool_info: None,
                        };
                        self.add_message(assistant_msg).await;
                    }
                }
            }
        }
    }

    async fn handle_tool_use(&mut self, data: &serde_json::Value) {
        if let (Some(tool_name), Some(tool_id), Some(input)) = (
            data.get("name").and_then(|v| v.as_str()),
            data.get("id").and_then(|v| v.as_str()),
            data.get("input").and_then(|v| v.as_object()),
        ) {
            let input_map: HashMap<String, serde_json::Value> = input.clone().into_iter().collect();
            
            // Store pending tool use
            self.pending_tool_uses.insert(
                tool_id.to_string(),
                (tool_name.to_string(), input_map.clone()),
            );

            // Create human-readable description
            let description = self.describe_tool_use(tool_name, &input_map);

            let tool_msg = Message {
                id: Uuid::new_v4(),
                message_type: MessageType::ToolUse,
                content: description,
                timestamp: Utc::now(),
                tool_info: Some(ToolInfo {
                    tool_name: tool_name.to_string(),
                    tool_use_id: tool_id.to_string(),
                    input: input_map,
                    output: None,
                }),
            };
            self.add_message(tool_msg).await;
        }
    }

    async fn handle_tool_result(&mut self, data: &serde_json::Value) {
        if let (Some(tool_use_id), Some(content)) = (
            data.get("tool_use_id").and_then(|v| v.as_str()),
            data.get("content").and_then(|v| v.as_str()),
        ) {
            if let Some((_tool_name, _input)) = self.pending_tool_uses.remove(tool_use_id) {
                // Find and update the corresponding tool use message
                for msg in self.messages.iter_mut().rev() {
                    if let Some(ref mut tool_info) = msg.tool_info {
                        if tool_info.tool_use_id == tool_use_id && tool_info.output.is_none() {
                            tool_info.output = Some(content.to_string());
                            break;
                        }
                    }
                }
            }
        }
    }

    fn describe_tool_use(&self, tool_name: &str, input: &HashMap<String, serde_json::Value>) -> String {
        match tool_name {
            "Edit" | "MultiEdit" => {
                if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                    let path_buf = PathBuf::from(file_path);
                    let file_name = path_buf
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    format!("Editing {}", file_name)
                } else {
                    format!("Using {}", tool_name)
                }
            }
            "Write" => {
                if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                    let path_buf = PathBuf::from(file_path);
                    let file_name = path_buf
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    format!("Writing {}", file_name)
                } else {
                    "Writing file".to_string()
                }
            }
            "Read" => {
                if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                    let path_buf = PathBuf::from(file_path);
                    let file_name = path_buf
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    format!("Reading {}", file_name)
                } else {
                    "Reading file".to_string()
                }
            }
            "Bash" => {
                if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                    format!("Running: {}", command)
                } else {
                    "Running command".to_string()
                }
            }
            "Grep" => {
                if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                    format!("Searching for: {}", pattern)
                } else {
                    "Searching files".to_string()
                }
            }
            "Glob" => {
                if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                    format!("Finding files: {}", pattern)
                } else {
                    "Finding files".to_string()
                }
            }
            "LS" => {
                if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                    let path_buf = PathBuf::from(path);
                    let dir_name = path_buf
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("directory");
                    format!("Listing: {}", dir_name)
                } else {
                    "Listing directory".to_string()
                }
            }
            "TodoRead" => "Reading todo list".to_string(),
            "TodoWrite" => {
                if let Some(todos) = input.get("todos").and_then(|v| v.as_array()) {
                    let mut todo_list = "Updating todo list:\n".to_string();
                    for todo in todos {
                        if let (Some(content), Some(status)) = (
                            todo.get("content").and_then(|v| v.as_str()),
                            todo.get("status").and_then(|v| v.as_str()),
                        ) {
                            let status_icon = match status {
                                "completed" => "✓",
                                "in_progress" => "→",
                                _ => "○",
                            };
                            todo_list.push_str(&format!("{} {}\n", status_icon, content));
                        }
                    }
                    todo_list.trim_end().to_string()
                } else {
                    "Updating todo list".to_string()
                }
            }
            _ => format!("Using {}", tool_name),
        }
    }
}

