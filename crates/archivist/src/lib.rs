//! Archivist - Trajectory Analysis and Pattern Extraction
//!
//! The Archivist subagent reviews trajectories and extracts reusable patterns
//! into the skill/memory library.
//!
//! It runs periodically (or on-demand) to:
//! 1. Review completed task trajectories
//! 2. Identify successful patterns worth preserving
//! 3. Extract skills from repeated solutions
//! 4. Build semantic memories from lessons learned
//! 5. Prune low-value or outdated entries

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Trajectory Types
// ============================================================================

/// Action type in a trajectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    ToolCall,
    Thinking,
    Output,
    Error,
}

/// A recorded action in a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryAction {
    /// Action type (tool call, thinking, etc.)
    #[serde(rename = "type")]
    pub action_type: ActionType,
    /// Tool name if tool_call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Input/content
    pub content: String,
    /// Result if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Success flag
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    /// Duration in ms
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Timestamp
    pub timestamp: String,
}

/// Outcome of a trajectory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrajectoryOutcome {
    Success,
    Failure,
    Partial,
    Timeout,
}

/// Token usage for a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

/// A complete trajectory of a task attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Unique trajectory ID
    pub id: String,
    /// Task that was attempted
    #[serde(rename = "taskId")]
    pub task_id: String,
    /// Task description
    #[serde(rename = "taskDescription")]
    pub task_description: String,
    /// All actions taken
    pub actions: Vec<TrajectoryAction>,
    /// Overall outcome
    pub outcome: TrajectoryOutcome,
    /// Error message if failed
    #[serde(rename = "errorMessage", skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Skills that were used
    #[serde(rename = "skillsUsed")]
    pub skills_used: Vec<String>,
    /// Files that were modified
    #[serde(rename = "filesModified")]
    pub files_modified: Vec<String>,
    /// Total duration
    #[serde(rename = "totalDurationMs")]
    pub total_duration_ms: u64,
    /// Model used
    pub model: String,
    /// Token usage
    pub tokens: TokenUsage,
    /// Timestamp
    pub timestamp: String,
    /// Project ID
    #[serde(rename = "projectId", skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Whether this trajectory has been archived
    pub archived: bool,
}

// ============================================================================
// Pattern Types
// ============================================================================

/// Pattern type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternType {
    Skill,
    Convention,
    Antipattern,
    Optimization,
}

/// A pattern identified from trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedPattern {
    /// Pattern ID
    pub id: String,
    /// Pattern type
    #[serde(rename = "type")]
    pub pattern_type: PatternType,
    /// Name for the pattern
    pub name: String,
    /// Description of what this pattern does
    pub description: String,
    /// The pattern/code/approach
    pub content: String,
    /// When to use this pattern
    #[serde(rename = "triggerContext")]
    pub trigger_context: Vec<String>,
    /// Success rate from trajectories
    #[serde(rename = "successRate")]
    pub success_rate: f64,
    /// Number of trajectories this was seen in
    pub occurrences: u32,
    /// Source trajectory IDs
    #[serde(rename = "sourceTrajectoryIds")]
    pub source_trajectory_ids: Vec<String>,
    /// Confidence in this pattern (0-1)
    pub confidence: f64,
    /// Category for organization
    pub category: String,
    /// Tags
    pub tags: Vec<String>,
    /// Timestamp
    #[serde(rename = "extractedAt")]
    pub extracted_at: String,
}

// ============================================================================
// Lesson Types
// ============================================================================

/// Source of a lesson.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LessonSource {
    TerminalBench,
    Mechacoder,
    Manual,
}

/// A lesson learned from task executions.
/// Lessons are higher-level insights extracted from patterns and trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivistLesson {
    /// Unique lesson ID
    pub id: String,
    /// Source of the lesson
    pub source: LessonSource,
    /// Related task ID (if from a specific task)
    #[serde(rename = "taskId", skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Suite name (if from TB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<String>,
    /// Model that generated this lesson
    pub model: String,
    /// Human-readable summary of the lesson
    pub summary: String,
    /// Patterns that lead to failure
    #[serde(rename = "failurePatterns", skip_serializing_if = "Option::is_none")]
    pub failure_patterns: Option<Vec<String>>,
    /// Patterns that lead to success
    #[serde(rename = "successPatterns", skip_serializing_if = "Option::is_none")]
    pub success_patterns: Option<Vec<String>>,
    /// Skills mentioned or used in this lesson
    #[serde(rename = "skillsMentioned", skip_serializing_if = "Option::is_none")]
    pub skills_mentioned: Option<Vec<String>>,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Creation timestamp
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

// ============================================================================
// Archive Types
// ============================================================================

/// Result of an archiving run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveResult {
    /// Archive run ID
    pub id: String,
    /// Trajectories processed
    #[serde(rename = "trajectoriesProcessed")]
    pub trajectories_processed: u32,
    /// Patterns extracted
    #[serde(rename = "patternsExtracted")]
    pub patterns_extracted: u32,
    /// Skills created
    #[serde(rename = "skillsCreated")]
    pub skills_created: u32,
    /// Memories created
    #[serde(rename = "memoriesCreated")]
    pub memories_created: u32,
    /// Items pruned (low quality)
    #[serde(rename = "itemsPruned")]
    pub items_pruned: u32,
    /// Duration of archive run
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    /// Timestamp
    pub timestamp: String,
}

/// Archive run mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveMode {
    Full,
    Quick,
}

/// Configuration for archive runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConfig {
    /// Minimum success rate for skill extraction
    #[serde(rename = "minSuccessRate")]
    pub min_success_rate: f64,
    /// Minimum occurrences before extracting pattern
    #[serde(rename = "minOccurrences")]
    pub min_occurrences: u32,
    /// Maximum age in days for trajectories to process
    #[serde(rename = "maxTrajectoryAgeDays")]
    pub max_trajectory_age_days: u32,
    /// Whether to auto-prune low-performing skills
    #[serde(rename = "autoPrune")]
    pub auto_prune: bool,
    /// Minimum success rate before pruning
    #[serde(rename = "pruneThreshold")]
    pub prune_threshold: f64,
    /// Project root for file operations
    #[serde(rename = "projectRoot")]
    pub project_root: String,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            min_success_rate: 0.7,
            min_occurrences: 2,
            max_trajectory_age_days: 30,
            auto_prune: true,
            prune_threshold: 0.3,
            project_root: ".".to_string(),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a unique trajectory ID.
pub fn generate_trajectory_id() -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let timestamp_b36 = base36_encode(timestamp as u64);
    let random = base36_random(6);
    format!("traj-{}-{}", timestamp_b36, random)
}

/// Generate a unique pattern ID.
pub fn generate_pattern_id(pattern_type: &str) -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let timestamp_b36 = base36_encode(timestamp as u64);
    let random = base36_random(6);
    let prefix = if pattern_type.len() >= 3 {
        &pattern_type[..3]
    } else {
        pattern_type
    };
    format!("pat-{}-{}-{}", prefix, timestamp_b36, random)
}

/// Generate a unique archive ID.
pub fn generate_archive_id() -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let timestamp_b36 = base36_encode(timestamp as u64);
    let random = base36_random(4);
    format!("arch-{}-{}", timestamp_b36, random)
}

/// Generate a unique lesson ID.
pub fn generate_lesson_id() -> String {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let timestamp_b36 = base36_encode(timestamp as u64);
    let random = base36_random(6);
    format!("lesson-{}-{}", timestamp_b36, random)
}

/// Base36 encode a number.
fn base36_encode(mut n: u64) -> String {
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut result = Vec::new();
    while n > 0 {
        result.push(CHARS[(n % 36) as usize]);
        n /= 36;
    }
    result.reverse();
    String::from_utf8(result).unwrap()
}

/// Generate random base36 string.
fn base36_random(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    // Simple random using system time nanoseconds
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u64;

    let mut seed = nanos;
    let mut result = Vec::with_capacity(len);
    for _ in 0..len {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        result.push(CHARS[(seed % 36) as usize]);
    }
    String::from_utf8(result).unwrap()
}

/// Create a trajectory action.
pub fn create_action(
    action_type: ActionType,
    content: String,
    tool: Option<String>,
    result: Option<String>,
    success: Option<bool>,
    duration_ms: Option<u64>,
) -> TrajectoryAction {
    TrajectoryAction {
        action_type,
        tool,
        content,
        result,
        success,
        duration_ms,
        timestamp: Utc::now().to_rfc3339(),
    }
}

/// Create trajectory builder input.
pub struct CreateTrajectoryInput {
    pub actions: Vec<TrajectoryAction>,
    pub outcome: TrajectoryOutcome,
    pub error_message: Option<String>,
    pub skills_used: Vec<String>,
    pub files_modified: Vec<String>,
    pub total_duration_ms: u64,
    pub model: String,
    pub tokens: TokenUsage,
    pub project_id: Option<String>,
}

/// Create a trajectory from task execution data.
pub fn create_trajectory(
    task_id: String,
    task_description: String,
    input: CreateTrajectoryInput,
) -> Trajectory {
    Trajectory {
        id: generate_trajectory_id(),
        task_id,
        task_description,
        actions: input.actions,
        outcome: input.outcome,
        error_message: input.error_message,
        skills_used: input.skills_used,
        files_modified: input.files_modified,
        total_duration_ms: input.total_duration_ms,
        model: input.model,
        tokens: input.tokens,
        timestamp: Utc::now().to_rfc3339(),
        project_id: input.project_id,
        archived: false,
    }
}

/// Create lesson builder input.
pub struct CreateLessonInput {
    pub source: LessonSource,
    pub model: String,
    pub task_id: Option<String>,
    pub suite: Option<String>,
    pub failure_patterns: Option<Vec<String>>,
    pub success_patterns: Option<Vec<String>>,
    pub skills_mentioned: Option<Vec<String>>,
    pub confidence: Option<f64>,
    pub tags: Vec<String>,
}

/// Create a lesson from task execution data.
pub fn create_lesson(summary: String, input: CreateLessonInput) -> ArchivistLesson {
    ArchivistLesson {
        id: generate_lesson_id(),
        source: input.source,
        task_id: input.task_id,
        suite: input.suite,
        model: input.model,
        summary,
        failure_patterns: input.failure_patterns,
        success_patterns: input.success_patterns,
        skills_mentioned: input.skills_mentioned,
        confidence: input.confidence.unwrap_or(0.5),
        tags: input.tags,
        created_at: Utc::now().to_rfc3339(),
    }
}

/// Build a prompt for extracting patterns from trajectories.
pub fn build_pattern_extraction_prompt(trajectories: &[Trajectory]) -> String {
    let successful_trajs: Vec<_> = trajectories
        .iter()
        .filter(|t| t.outcome == TrajectoryOutcome::Success)
        .collect();
    let failed_trajs: Vec<_> = trajectories
        .iter()
        .filter(|t| t.outcome == TrajectoryOutcome::Failure)
        .collect();

    let mut parts = vec![
        "You are an Archivist analyzing task trajectories to extract reusable patterns.".to_string(),
        String::new(),
        "## Successful Trajectories".to_string(),
        String::new(),
    ];

    for traj in successful_trajs.iter().take(5) {
        parts.push(format!("### Task: {}", traj.task_description));
        let skills = if traj.skills_used.is_empty() {
            "none".to_string()
        } else {
            traj.skills_used.join(", ")
        };
        parts.push(format!(
            "Duration: {}ms, Skills: {}",
            traj.total_duration_ms, skills
        ));
        parts.push("Actions:".to_string());
        for action in traj.actions.iter().take(10) {
            if action.action_type == ActionType::ToolCall {
                let tool = action.tool.as_deref().unwrap_or("unknown");
                let content_preview = if action.content.len() > 100 {
                    format!("{}...", &action.content[..100])
                } else {
                    action.content.clone()
                };
                parts.push(format!("  - {}: {}", tool, content_preview));
            }
        }
        parts.push(String::new());
    }

    if !failed_trajs.is_empty() {
        parts.push("## Failed Trajectories (antipatterns to avoid)".to_string());
        parts.push(String::new());
        for traj in failed_trajs.iter().take(3) {
            parts.push(format!("### Task: {}", traj.task_description));
            let error = traj
                .error_message
                .as_ref()
                .map(|e| {
                    if e.len() > 200 {
                        format!("{}...", &e[..200])
                    } else {
                        e.clone()
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());
            parts.push(format!("Error: {}", error));
            parts.push(String::new());
        }
    }

    parts.extend([
        "## Extract Patterns".to_string(),
        String::new(),
        "Identify reusable patterns from these trajectories. For each pattern, provide:".to_string(),
        "1. **name**: Short descriptive name".to_string(),
        "2. **type**: skill | convention | antipattern | optimization".to_string(),
        "3. **description**: What the pattern does".to_string(),
        "4. **content**: The code/approach to use".to_string(),
        "5. **triggerContext**: When to use this pattern".to_string(),
        "6. **category**: Category for organization".to_string(),
        String::new(),
        "Output as a JSON array of patterns.".to_string(),
    ]);

    parts.join("\n")
}

/// Raw pattern from JSON response.
#[derive(Debug, Deserialize)]
struct RawPattern {
    name: Option<String>,
    #[serde(rename = "type")]
    pattern_type: Option<String>,
    description: Option<String>,
    content: Option<String>,
    #[serde(rename = "triggerContext")]
    trigger_context: Option<Vec<String>>,
    category: Option<String>,
    tags: Option<Vec<String>>,
}

/// Parse patterns from FM response.
pub fn parse_patterns_from_response(
    response: &str,
    source_trajectory_ids: Vec<String>,
) -> Vec<ExtractedPattern> {
    // Extract JSON array from response
    let json_start = response.find('[');
    let json_end = response.rfind(']');

    let (json_start, json_end) = match (json_start, json_end) {
        (Some(s), Some(e)) if e > s => (s, e),
        _ => return Vec::new(),
    };

    let json_str = &response[json_start..=json_end];

    let parsed: Vec<RawPattern> = match serde_json::from_str(json_str) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    parsed
        .into_iter()
        .map(|p| {
            let pattern_type_str = p.pattern_type.as_deref().unwrap_or("skill");
            let pattern_type = match pattern_type_str {
                "convention" => PatternType::Convention,
                "antipattern" => PatternType::Antipattern,
                "optimization" => PatternType::Optimization,
                _ => PatternType::Skill,
            };
            ExtractedPattern {
                id: generate_pattern_id(pattern_type_str),
                pattern_type,
                name: p.name.unwrap_or_else(|| "Unnamed Pattern".to_string()),
                description: p.description.unwrap_or_default(),
                content: p.content.unwrap_or_default(),
                trigger_context: p.trigger_context.unwrap_or_default(),
                success_rate: 1.0, // Will be updated from trajectory stats
                occurrences: source_trajectory_ids.len() as u32,
                source_trajectory_ids: source_trajectory_ids.clone(),
                confidence: 0.7,
                category: p.category.unwrap_or_else(|| "general".to_string()),
                tags: p.tags.unwrap_or_default(),
                extracted_at: Utc::now().to_rfc3339(),
            }
        })
        .collect()
}

/// Calculate success rate from trajectories.
pub fn calculate_success_rate(trajectories: &[Trajectory]) -> f64 {
    if trajectories.is_empty() {
        return 0.0;
    }
    let successful = trajectories
        .iter()
        .filter(|t| t.outcome == TrajectoryOutcome::Success)
        .count();
    successful as f64 / trajectories.len() as f64
}

/// Group trajectories by similarity (for pattern detection).
pub fn group_similar_trajectories(trajectories: &[Trajectory]) -> HashMap<String, Vec<&Trajectory>> {
    let mut groups: HashMap<String, Vec<&Trajectory>> = HashMap::new();

    for traj in trajectories {
        // Simple grouping by first tool used + outcome
        let first_tool = traj
            .actions
            .iter()
            .find(|a| a.action_type == ActionType::ToolCall)
            .and_then(|a| a.tool.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        let outcome_str = match traj.outcome {
            TrajectoryOutcome::Success => "success",
            TrajectoryOutcome::Failure => "failure",
            TrajectoryOutcome::Partial => "partial",
            TrajectoryOutcome::Timeout => "timeout",
        };

        let key = format!("{}-{}", first_tool, outcome_str);
        groups.entry(key).or_default().push(traj);
    }

    groups
}

/// Check if a trajectory is within the age limit.
pub fn is_within_age_limit(trajectory: &Trajectory, max_age_days: u32) -> bool {
    let timestamp = match DateTime::parse_from_rfc3339(&trajectory.timestamp) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => return false,
    };

    let cutoff = Utc::now() - chrono::Duration::days(max_age_days as i64);
    timestamp > cutoff
}

// ============================================================================
// Trajectory Store
// ============================================================================

/// Error type for trajectory store operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrajectoryStoreError {
    IoError(String),
    ParseError(String),
    NotFound(String),
}

impl std::fmt::Display for TrajectoryStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrajectoryStoreError::IoError(msg) => write!(f, "IO error: {}", msg),
            TrajectoryStoreError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TrajectoryStoreError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for TrajectoryStoreError {}

/// In-memory trajectory store.
#[derive(Debug, Default)]
pub struct TrajectoryStore {
    trajectories: HashMap<String, Trajectory>,
}

impl TrajectoryStore {
    /// Create a new empty trajectory store.
    pub fn new() -> Self {
        Self {
            trajectories: HashMap::new(),
        }
    }

    /// Save a trajectory.
    pub fn save(&mut self, trajectory: Trajectory) {
        self.trajectories.insert(trajectory.id.clone(), trajectory);
    }

    /// Get a trajectory by ID.
    pub fn get(&self, id: &str) -> Option<&Trajectory> {
        self.trajectories.get(id)
    }

    /// Get all trajectories.
    pub fn get_all(&self) -> Vec<&Trajectory> {
        self.trajectories.values().collect()
    }

    /// Get unarchived trajectories.
    pub fn get_unarchived(&self) -> Vec<&Trajectory> {
        self.trajectories
            .values()
            .filter(|t| !t.archived)
            .collect()
    }

    /// Get trajectories by outcome.
    pub fn get_by_outcome(&self, outcome: &TrajectoryOutcome) -> Vec<&Trajectory> {
        self.trajectories
            .values()
            .filter(|t| &t.outcome == outcome)
            .collect()
    }

    /// Get trajectories within age limit.
    pub fn get_recent(&self, max_age_days: u32) -> Vec<&Trajectory> {
        self.trajectories
            .values()
            .filter(|t| is_within_age_limit(t, max_age_days))
            .collect()
    }

    /// Mark trajectories as archived.
    pub fn mark_archived(&mut self, ids: &[String]) {
        for id in ids {
            if let Some(traj) = self.trajectories.get_mut(id) {
                traj.archived = true;
            }
        }
    }

    /// Prune old trajectories.
    pub fn prune(&mut self, max_age_days: u32) -> usize {
        let ids_to_remove: Vec<String> = self
            .trajectories
            .values()
            .filter(|t| !is_within_age_limit(t, max_age_days))
            .map(|t| t.id.clone())
            .collect();

        let count = ids_to_remove.len();
        for id in ids_to_remove {
            self.trajectories.remove(&id);
        }
        count
    }

    /// Get trajectory count.
    pub fn count(&self) -> usize {
        self.trajectories.len()
    }
}

// ============================================================================
// Lesson Store
// ============================================================================

/// Error type for lesson store operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LessonStoreError {
    IoError(String),
    ParseError(String),
    NotFound(String),
}

impl std::fmt::Display for LessonStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LessonStoreError::IoError(msg) => write!(f, "IO error: {}", msg),
            LessonStoreError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            LessonStoreError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for LessonStoreError {}

/// In-memory lesson store.
#[derive(Debug, Default)]
pub struct LessonStore {
    lessons: HashMap<String, ArchivistLesson>,
}

impl LessonStore {
    /// Create a new empty lesson store.
    pub fn new() -> Self {
        Self {
            lessons: HashMap::new(),
        }
    }

    /// Save a lesson.
    pub fn save(&mut self, lesson: ArchivistLesson) {
        self.lessons.insert(lesson.id.clone(), lesson);
    }

    /// Get a lesson by ID.
    pub fn get(&self, id: &str) -> Option<&ArchivistLesson> {
        self.lessons.get(id)
    }

    /// Get all lessons.
    pub fn get_all(&self) -> Vec<&ArchivistLesson> {
        self.lessons.values().collect()
    }

    /// Get lessons by source.
    pub fn get_by_source(&self, source: &LessonSource) -> Vec<&ArchivistLesson> {
        self.lessons
            .values()
            .filter(|l| &l.source == source)
            .collect()
    }

    /// Get lessons by model.
    pub fn get_by_model(&self, model: &str) -> Vec<&ArchivistLesson> {
        self.lessons
            .values()
            .filter(|l| l.model == model)
            .collect()
    }

    /// Get recent lessons.
    pub fn get_recent(&self, limit: usize) -> Vec<&ArchivistLesson> {
        let mut lessons: Vec<_> = self.lessons.values().collect();
        lessons.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        lessons.into_iter().take(limit).collect()
    }

    /// Delete a lesson.
    pub fn delete(&mut self, id: &str) -> bool {
        self.lessons.remove(id).is_some()
    }

    /// Get lesson count.
    pub fn count(&self) -> usize {
        self.lessons.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_trajectory_id() {
        let id = generate_trajectory_id();
        assert!(id.starts_with("traj-"));
        assert!(id.len() > 10);
    }

    #[test]
    fn test_generate_pattern_id() {
        let id = generate_pattern_id("skill");
        assert!(id.starts_with("pat-ski-"));

        let id2 = generate_pattern_id("antipattern");
        assert!(id2.starts_with("pat-ant-"));
    }

    #[test]
    fn test_generate_archive_id() {
        let id = generate_archive_id();
        assert!(id.starts_with("arch-"));
    }

    #[test]
    fn test_generate_lesson_id() {
        let id = generate_lesson_id();
        assert!(id.starts_with("lesson-"));
    }

    #[test]
    fn test_create_action() {
        let action = create_action(
            ActionType::ToolCall,
            "echo hello".to_string(),
            Some("bash".to_string()),
            Some("hello".to_string()),
            Some(true),
            Some(100),
        );

        assert_eq!(action.action_type, ActionType::ToolCall);
        assert_eq!(action.tool, Some("bash".to_string()));
        assert_eq!(action.content, "echo hello");
        assert_eq!(action.result, Some("hello".to_string()));
        assert_eq!(action.success, Some(true));
        assert_eq!(action.duration_ms, Some(100));
    }

    #[test]
    fn test_create_trajectory() {
        let traj = create_trajectory(
            "task-123".to_string(),
            "Fix the bug".to_string(),
            CreateTrajectoryInput {
                actions: vec![],
                outcome: TrajectoryOutcome::Success,
                error_message: None,
                skills_used: vec!["debugging".to_string()],
                files_modified: vec!["src/lib.rs".to_string()],
                total_duration_ms: 5000,
                model: "fm".to_string(),
                tokens: TokenUsage {
                    input: 100,
                    output: 50,
                    total: 150,
                },
                project_id: None,
            },
        );

        assert!(traj.id.starts_with("traj-"));
        assert_eq!(traj.task_id, "task-123");
        assert_eq!(traj.task_description, "Fix the bug");
        assert_eq!(traj.outcome, TrajectoryOutcome::Success);
        assert_eq!(traj.skills_used, vec!["debugging"]);
        assert!(!traj.archived);
    }

    #[test]
    fn test_create_lesson() {
        let lesson = create_lesson(
            "Always validate input before processing".to_string(),
            CreateLessonInput {
                source: LessonSource::TerminalBench,
                model: "fm".to_string(),
                task_id: Some("regex-log".to_string()),
                suite: Some("regex".to_string()),
                failure_patterns: Some(vec!["Not checking edge cases".to_string()]),
                success_patterns: Some(vec!["Thorough input validation".to_string()]),
                skills_mentioned: None,
                confidence: Some(0.8),
                tags: vec!["validation".to_string()],
            },
        );

        assert!(lesson.id.starts_with("lesson-"));
        assert_eq!(lesson.source, LessonSource::TerminalBench);
        assert_eq!(lesson.summary, "Always validate input before processing");
        assert_eq!(lesson.confidence, 0.8);
    }

    #[test]
    fn test_calculate_success_rate() {
        let mut store = TrajectoryStore::new();

        // Empty store
        assert_eq!(calculate_success_rate(&[]), 0.0);

        // Add some trajectories
        for i in 0..10 {
            let outcome = if i < 7 {
                TrajectoryOutcome::Success
            } else {
                TrajectoryOutcome::Failure
            };
            let traj = create_trajectory(
                format!("task-{}", i),
                "Test task".to_string(),
                CreateTrajectoryInput {
                    actions: vec![],
                    outcome,
                    error_message: None,
                    skills_used: vec![],
                    files_modified: vec![],
                    total_duration_ms: 1000,
                    model: "fm".to_string(),
                    tokens: TokenUsage { input: 10, output: 5, total: 15 },
                    project_id: None,
                },
            );
            store.save(traj);
        }

        let all: Vec<_> = store.get_all().into_iter().cloned().collect();
        let rate = calculate_success_rate(&all);
        assert!((rate - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_group_similar_trajectories() {
        let trajs: Vec<Trajectory> = (0..4)
            .map(|i| {
                let tool = if i % 2 == 0 { "bash" } else { "edit" };
                let outcome = if i < 2 {
                    TrajectoryOutcome::Success
                } else {
                    TrajectoryOutcome::Failure
                };

                create_trajectory(
                    format!("task-{}", i),
                    "Test".to_string(),
                    CreateTrajectoryInput {
                        actions: vec![create_action(
                            ActionType::ToolCall,
                            "content".to_string(),
                            Some(tool.to_string()),
                            None,
                            None,
                            None,
                        )],
                        outcome,
                        error_message: None,
                        skills_used: vec![],
                        files_modified: vec![],
                        total_duration_ms: 1000,
                        model: "fm".to_string(),
                        tokens: TokenUsage { input: 10, output: 5, total: 15 },
                        project_id: None,
                    },
                )
            })
            .collect();

        let groups = group_similar_trajectories(&trajs);
        assert!(groups.contains_key("bash-success"));
        assert!(groups.contains_key("edit-success"));
        assert!(groups.contains_key("bash-failure"));
        assert!(groups.contains_key("edit-failure"));
    }

    #[test]
    fn test_build_pattern_extraction_prompt() {
        let trajs = vec![
            create_trajectory(
                "task-1".to_string(),
                "Add feature".to_string(),
                CreateTrajectoryInput {
                    actions: vec![create_action(
                        ActionType::ToolCall,
                        "content".to_string(),
                        Some("edit".to_string()),
                        None,
                        Some(true),
                        None,
                    )],
                    outcome: TrajectoryOutcome::Success,
                    error_message: None,
                    skills_used: vec!["coding".to_string()],
                    files_modified: vec![],
                    total_duration_ms: 5000,
                    model: "fm".to_string(),
                    tokens: TokenUsage { input: 100, output: 50, total: 150 },
                    project_id: None,
                },
            ),
            create_trajectory(
                "task-2".to_string(),
                "Fix bug".to_string(),
                CreateTrajectoryInput {
                    actions: vec![],
                    outcome: TrajectoryOutcome::Failure,
                    error_message: Some("Type error".to_string()),
                    skills_used: vec![],
                    files_modified: vec![],
                    total_duration_ms: 3000,
                    model: "fm".to_string(),
                    tokens: TokenUsage { input: 80, output: 40, total: 120 },
                    project_id: None,
                },
            ),
        ];

        let prompt = build_pattern_extraction_prompt(&trajs);
        assert!(prompt.contains("Archivist analyzing task trajectories"));
        assert!(prompt.contains("Add feature"));
        assert!(prompt.contains("Fix bug"));
        assert!(prompt.contains("Failed Trajectories"));
        assert!(prompt.contains("Type error"));
    }

    #[test]
    fn test_parse_patterns_from_response() {
        let response = r#"
        Here are the patterns:
        [
            {
                "name": "Test First",
                "type": "convention",
                "description": "Write tests before code",
                "content": "Always write failing test first",
                "triggerContext": ["new feature", "bug fix"],
                "category": "testing"
            }
        ]
        "#;

        let patterns = parse_patterns_from_response(
            response,
            vec!["traj-1".to_string(), "traj-2".to_string()],
        );

        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].name, "Test First");
        assert_eq!(patterns[0].pattern_type, PatternType::Convention);
        assert_eq!(patterns[0].occurrences, 2);
        assert_eq!(patterns[0].source_trajectory_ids, vec!["traj-1", "traj-2"]);
    }

    #[test]
    fn test_parse_patterns_invalid_json() {
        let response = "No valid JSON here";
        let patterns = parse_patterns_from_response(response, vec!["traj-1".to_string()]);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_trajectory_store() {
        let mut store = TrajectoryStore::new();
        assert_eq!(store.count(), 0);

        let traj = create_trajectory(
            "task-1".to_string(),
            "Test task".to_string(),
            CreateTrajectoryInput {
                actions: vec![],
                outcome: TrajectoryOutcome::Success,
                error_message: None,
                skills_used: vec![],
                files_modified: vec![],
                total_duration_ms: 1000,
                model: "fm".to_string(),
                tokens: TokenUsage { input: 10, output: 5, total: 15 },
                project_id: None,
            },
        );
        let traj_id = traj.id.clone();

        store.save(traj);
        assert_eq!(store.count(), 1);

        let retrieved = store.get(&traj_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().task_id, "task-1");
    }

    #[test]
    fn test_trajectory_store_get_unarchived() {
        let mut store = TrajectoryStore::new();

        for i in 0..5 {
            let mut traj = create_trajectory(
                format!("task-{}", i),
                "Test".to_string(),
                CreateTrajectoryInput {
                    actions: vec![],
                    outcome: TrajectoryOutcome::Success,
                    error_message: None,
                    skills_used: vec![],
                    files_modified: vec![],
                    total_duration_ms: 1000,
                    model: "fm".to_string(),
                    tokens: TokenUsage { input: 10, output: 5, total: 15 },
                    project_id: None,
                },
            );
            if i < 2 {
                traj.archived = true;
            }
            store.save(traj);
        }

        let unarchived = store.get_unarchived();
        assert_eq!(unarchived.len(), 3);
    }

    #[test]
    fn test_trajectory_store_mark_archived() {
        let mut store = TrajectoryStore::new();

        let traj1 = create_trajectory(
            "task-1".to_string(),
            "Test".to_string(),
            CreateTrajectoryInput {
                actions: vec![],
                outcome: TrajectoryOutcome::Success,
                error_message: None,
                skills_used: vec![],
                files_modified: vec![],
                total_duration_ms: 1000,
                model: "fm".to_string(),
                tokens: TokenUsage { input: 10, output: 5, total: 15 },
                project_id: None,
            },
        );
        let id = traj1.id.clone();
        store.save(traj1);

        assert!(!store.get(&id).unwrap().archived);
        store.mark_archived(&[id.clone()]);
        assert!(store.get(&id).unwrap().archived);
    }

    #[test]
    fn test_lesson_store() {
        let mut store = LessonStore::new();
        assert_eq!(store.count(), 0);

        let lesson = create_lesson(
            "Test lesson".to_string(),
            CreateLessonInput {
                source: LessonSource::Mechacoder,
                model: "fm".to_string(),
                task_id: None,
                suite: None,
                failure_patterns: None,
                success_patterns: None,
                skills_mentioned: None,
                confidence: None,
                tags: vec![],
            },
        );
        let lesson_id = lesson.id.clone();

        store.save(lesson);
        assert_eq!(store.count(), 1);

        let retrieved = store.get(&lesson_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().summary, "Test lesson");
    }

    #[test]
    fn test_lesson_store_get_by_source() {
        let mut store = LessonStore::new();

        for i in 0..6 {
            let source = match i % 3 {
                0 => LessonSource::TerminalBench,
                1 => LessonSource::Mechacoder,
                _ => LessonSource::Manual,
            };
            let lesson = create_lesson(
                format!("Lesson {}", i),
                CreateLessonInput {
                    source,
                    model: "fm".to_string(),
                    task_id: None,
                    suite: None,
                    failure_patterns: None,
                    success_patterns: None,
                    skills_mentioned: None,
                    confidence: None,
                    tags: vec![],
                },
            );
            store.save(lesson);
        }

        let tb_lessons = store.get_by_source(&LessonSource::TerminalBench);
        assert_eq!(tb_lessons.len(), 2);

        let mc_lessons = store.get_by_source(&LessonSource::Mechacoder);
        assert_eq!(mc_lessons.len(), 2);
    }

    #[test]
    fn test_lesson_store_delete() {
        let mut store = LessonStore::new();

        let lesson = create_lesson(
            "To be deleted".to_string(),
            CreateLessonInput {
                source: LessonSource::Manual,
                model: "fm".to_string(),
                task_id: None,
                suite: None,
                failure_patterns: None,
                success_patterns: None,
                skills_mentioned: None,
                confidence: None,
                tags: vec![],
            },
        );
        let id = lesson.id.clone();
        store.save(lesson);

        assert_eq!(store.count(), 1);
        assert!(store.delete(&id));
        assert_eq!(store.count(), 0);
        assert!(!store.delete(&id)); // Already deleted
    }

    #[test]
    fn test_trajectory_outcome_serialization() {
        let json = serde_json::to_string(&TrajectoryOutcome::Success).unwrap();
        assert_eq!(json, "\"success\"");

        let json = serde_json::to_string(&TrajectoryOutcome::Failure).unwrap();
        assert_eq!(json, "\"failure\"");
    }

    #[test]
    fn test_pattern_type_serialization() {
        let json = serde_json::to_string(&PatternType::Skill).unwrap();
        assert_eq!(json, "\"skill\"");

        let json = serde_json::to_string(&PatternType::Antipattern).unwrap();
        assert_eq!(json, "\"antipattern\"");
    }

    #[test]
    fn test_lesson_source_serialization() {
        let json = serde_json::to_string(&LessonSource::TerminalBench).unwrap();
        assert_eq!(json, "\"terminal-bench\"");

        let json = serde_json::to_string(&LessonSource::Mechacoder).unwrap();
        assert_eq!(json, "\"mechacoder\"");
    }

    #[test]
    fn test_archive_config_default() {
        let config = ArchiveConfig::default();
        assert_eq!(config.min_success_rate, 0.7);
        assert_eq!(config.min_occurrences, 2);
        assert_eq!(config.max_trajectory_age_days, 30);
        assert!(config.auto_prune);
        assert_eq!(config.prune_threshold, 0.3);
    }
}
