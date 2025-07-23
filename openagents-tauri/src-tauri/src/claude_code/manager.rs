use crate::claude_code::models::*;
use chrono::Utc;
use log::{debug, error, info, warn};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex, RwLock};
use uuid::Uuid;

pub struct ClaudeManager {
    sessions: Arc<RwLock<HashMap<String, Arc<Mutex<ClaudeSession>>>>>,
    binary_path: Option<PathBuf>,
}

impl ClaudeManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            binary_path: None,
        }
    }

    pub fn set_binary_path(&mut self, path: PathBuf) {
        self.binary_path = Some(path);
    }

    pub async fn create_session(&self, project_path: String) -> Result<String, ClaudeError> {
        let binary_path = self.binary_path.as_ref()
            .ok_or(ClaudeError::BinaryNotFound)?;

        let session_id = Uuid::new_v4().to_string();
        let session = Arc::new(Mutex::new(ClaudeSession::new(
            session_id.clone(),
            project_path,
            binary_path.clone(),
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
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?;

        let mut session_lock = session.lock().await;
        session_lock.send_message(message).await
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

    pub async fn subscribe_to_session(&self, session_id: &str) -> Result<mpsc::Receiver<Message>, ClaudeError> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or_else(|| ClaudeError::SessionNotFound(session_id.to_string()))?;

        let mut session_lock = session.lock().await;
        Ok(session_lock.subscribe())
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
    process: Option<Child>,
    stdin: Option<tokio::process::ChildStdin>,
    pending_tool_uses: HashMap<String, (String, HashMap<String, serde_json::Value>)>,
    message_subscribers: Vec<mpsc::Sender<Message>>,
    partial_buffer: String,
    output_receiver: Option<mpsc::Receiver<String>>,
}

impl ClaudeSession {
    fn new(id: String, project_path: String, binary_path: PathBuf) -> Self {
        Self {
            id,
            project_path,
            binary_path,
            messages: Vec::new(),
            is_active: true,
            first_message: None,
            summary: None,
            process: None,
            stdin: None,
            pending_tool_uses: HashMap::new(),
            message_subscribers: Vec::new(),
            partial_buffer: String::new(),
            output_receiver: None,
        }
    }

    async fn start(&mut self) -> Result<(), ClaudeError> {
        info!("Starting Claude Code session for: {}", self.project_path);

        // Build the command
        let claude_command = format!(
            "cd \"{}\" && MAX_THINKING_TOKENS=31999 \"{}\" -p --output-format stream-json --input-format stream-json --verbose --dangerously-skip-permissions",
            self.project_path, self.binary_path.display()
        );

        let mut cmd = Command::new("/bin/bash");
        cmd.args(&["-l", "-c", &claude_command]);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| ClaudeError::ProcessStartError(e.to_string()))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| ClaudeError::ProcessStartError("Failed to capture stdin".to_string()))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| ClaudeError::ProcessStartError("Failed to capture stdout".to_string()))?;

        let stderr = child.stderr.take()
            .ok_or_else(|| ClaudeError::ProcessStartError("Failed to capture stderr".to_string()))?;

        self.stdin = Some(stdin);
        self.process = Some(child);

        // Start reading stdout
        let session_id = self.id.clone();
        let mut stdout_reader = BufReader::new(stdout);
        
        // Create a channel for sending lines back to this session
        let (tx, rx) = mpsc::channel::<String>(100);
        self.output_receiver = Some(rx);

        // Spawn task to read stdout
        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                match stdout_reader.read_line(&mut line).await {
                    Ok(0) => {
                        info!("Claude Code stdout closed for session {}", session_id);
                        break;
                    }
                    Ok(_) => {
                        if !line.trim().is_empty() {
                            let _ = tx.send(line.clone()).await;
                        }
                        line.clear();
                    }
                    Err(e) => {
                        error!("Error reading stdout for session {}: {}", session_id, e);
                        break;
                    }
                }
            }
        });

        // Start reading stderr
        let session_id = self.id.clone();
        let mut stderr_reader = BufReader::new(stderr);
        let _stderr_sender = self.create_internal_sender();

        tokio::spawn(async move {
            let mut line = String::new();
            loop {
                match stderr_reader.read_line(&mut line).await {
                    Ok(0) => {
                        info!("Claude Code stderr closed for session {}", session_id);
                        break;
                    }
                    Ok(_) => {
                        if !line.trim().is_empty() {
                            warn!("STDERR for session {}: {}", session_id, line.trim());
                        }
                        line.clear();
                    }
                    Err(e) => {
                        error!("Error reading stderr for session {}: {}", session_id, e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn send_message(&mut self, message: String) -> Result<(), ClaudeError> {
        info!("Sending message to Claude Code: {}", message);

        // Add user message to our history
        let user_msg = Message {
            id: Uuid::new_v4(),
            message_type: MessageType::User,
            content: message.clone(),
            timestamp: Utc::now(),
            tool_info: None,
        };
        self.add_message(user_msg).await;

        // Store first message for preview
        if self.first_message.is_none() {
            self.first_message = Some(message.clone());
        }

        // Create the JSON message
        let msg_json = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": message
                }]
            }
        });

        // Send to Claude Code
        if let Some(ref mut stdin) = self.stdin {
            let json_str = serde_json::to_string(&msg_json)?;
            stdin.write_all(json_str.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        } else {
            return Err(ClaudeError::Other("No stdin available".to_string()));
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ClaudeError> {
        info!("Stopping Claude Code session: {}", self.id);
        self.is_active = false;

        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
        }

        Ok(())
    }

    fn subscribe(&mut self) -> mpsc::Receiver<Message> {
        let (tx, rx) = mpsc::channel(100);
        self.message_subscribers.push(tx);
        rx
    }

    async fn add_message(&mut self, message: Message) {
        self.messages.push(message.clone());

        // Notify all subscribers
        self.message_subscribers.retain(|tx| {
            tx.try_send(message.clone()).is_ok()
        });
    }

    fn create_internal_sender(&self) -> mpsc::Sender<(String, String)> {
        let (tx, mut rx) = mpsc::channel::<(String, String)>(100);

        tokio::spawn(async move {
            while let Some((_session_id, line)) = rx.recv().await {
                // This would be connected to the actual session processing
                // For now, we'll just log it
                debug!("Received line: {}", line);
            }
        });

        tx
    }

    async fn process_pending_output(&mut self) {
        // Take the receiver temporarily to avoid borrow issues
        if let Some(mut receiver) = self.output_receiver.take() {
            let mut lines = Vec::new();
            
            // Collect all available messages without blocking
            while let Ok(line) = receiver.try_recv() {
                lines.push(line);
            }
            
            // Put the receiver back
            self.output_receiver = Some(receiver);
            
            // Now process all the lines
            for line in lines {
                self.process_output_line(&line).await;
            }
        }
    }

    async fn process_output_line(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        
        debug!("Processing line from Claude: {}", trimmed);
        
        if !trimmed.starts_with('{') {
            info!("Non-JSON line from Claude: {}", trimmed);
            return;
        }

        match serde_json::from_str::<IncomingMessage>(trimmed) {
            Ok(msg) => {
                info!("Parsed message type: {}", msg.msg_type);
                self.handle_message(msg).await;
            }
            Err(e) => {
                warn!("Failed to parse JSON message: {} - Line: {}", e, trimmed);
            }
        }
    }

    async fn handle_message(&mut self, msg: IncomingMessage) {
        match msg.msg_type.as_str() {
            "system" => {
                if let Some(subtype) = msg.data.get("subtype").and_then(|v| v.as_str()) {
                    if subtype == "init" {
                        info!("Claude Code initialized for session {}", self.id);
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
                                    "tool_result" => self.handle_tool_result(item).await,
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
            if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                let mut text_content = String::new();
                let mut _has_thinking = false;

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
                                _has_thinking = true;
                                if let Some(thinking) = item.get("thinking").and_then(|v| v.as_str()) {
                                    let thinking_msg = Message {
                                        id: Uuid::new_v4(),
                                        message_type: MessageType::Thinking,
                                        content: thinking.to_string(),
                                        timestamp: Utc::now(),
                                        tool_info: None,
                                    };
                                    self.add_message(thinking_msg).await;
                                }
                            }
                            "tool_use" => {
                                self.handle_tool_use(item).await;
                            }
                            _ => {}
                        }
                    }
                }

                // Add text message if there's content
                if !text_content.is_empty() {
                    let assistant_msg = Message {
                        id: Uuid::new_v4(),
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

// Global manager instance
lazy_static::lazy_static! {
    static ref CLAUDE_MANAGER: Arc<Mutex<Option<ClaudeManager>>> = Arc::new(Mutex::new(None));
}

pub async fn get_claude_manager() -> Option<ClaudeManager> {
    let manager = CLAUDE_MANAGER.lock().await;
    manager.as_ref().map(|m| ClaudeManager {
        sessions: m.sessions.clone(),
        binary_path: m.binary_path.clone(),
    })
}

pub async fn set_claude_manager(manager: ClaudeManager) {
    let mut global_manager = CLAUDE_MANAGER.lock().await;
    *global_manager = Some(manager);
}