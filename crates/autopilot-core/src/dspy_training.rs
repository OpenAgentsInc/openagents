//! Training data extraction from session logs.
//!
//! Extracts DSPy Examples from autopilot session logs for optimization.
//! Session logs are stored as JSONL files in `~/.openagents/sessions/{date}/{session_id}.jsonl`.
//!
//! # Usage
//!
//! ```ignore
//! use autopilot_core::dspy_training::{TrainingExtractor, ExtractedExamples};
//! use autopilot_core::dspy_hub::DspyHub;
//!
//! let hub = DspyHub::new();
//! let extractor = TrainingExtractor::new(hub);
//!
//! // Extract from a specific session
//! let examples = extractor.extract_from_session("/path/to/session.jsonl").await?;
//!
//! // Extract from a date (YYYYMMDD)
//! let examples = extractor.extract_from_date("20250109").await?;
//!
//! // Save extracted training data
//! extractor.save_training_data(&examples)?;
//! ```

use crate::dspy_hub::DspyHub;
use crate::dspy_optimization::{ExecutionExample, PlanningExample, VerificationExample};
use crate::logger::LogEntry;
use anyhow::{Context, Result};
use dsrs::Example;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, create_dir_all};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Extracted examples from session logs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedExamples {
    /// Planning examples (issue -> plan).
    pub planning: Vec<PlanningExample>,

    /// Execution examples (step -> action).
    pub execution: Vec<ExecutionExample>,

    /// Verification examples (changes -> verdict).
    pub verification: Vec<VerificationExample>,

    /// Tool selection examples.
    pub tool_selection: Vec<Example>,

    /// Tool interpretation examples.
    pub tool_interpretation: Vec<Example>,

    /// Session IDs that were processed.
    pub session_ids: Vec<String>,

    /// Total log entries processed.
    pub entries_processed: usize,
}

impl ExtractedExamples {
    /// Check if we have enough examples for optimization.
    pub fn has_enough_examples(&self, min_per_type: usize) -> bool {
        self.planning.len() >= min_per_type
            || self.execution.len() >= min_per_type
            || self.verification.len() >= min_per_type
    }

    /// Get total example count.
    pub fn total_count(&self) -> usize {
        self.planning.len()
            + self.execution.len()
            + self.verification.len()
            + self.tool_selection.len()
            + self.tool_interpretation.len()
    }

    /// Merge with another set of extracted examples.
    pub fn merge(&mut self, other: ExtractedExamples) {
        self.planning.extend(other.planning);
        self.execution.extend(other.execution);
        self.verification.extend(other.verification);
        self.tool_selection.extend(other.tool_selection);
        self.tool_interpretation.extend(other.tool_interpretation);
        self.session_ids.extend(other.session_ids);
        self.entries_processed += other.entries_processed;
    }
}

/// Criteria for filtering successful examples.
#[derive(Debug, Clone)]
pub struct SuccessCriteria {
    /// Require phase to complete successfully.
    pub require_success: bool,

    /// Minimum confidence threshold for predictions.
    pub min_confidence: Option<f32>,

    /// Filter by specific phase.
    pub phase_filter: Option<String>,
}

impl Default for SuccessCriteria {
    fn default() -> Self {
        Self {
            require_success: true,
            min_confidence: Some(0.7),
            phase_filter: None,
        }
    }
}

/// Training data extractor for DSPy optimization.
pub struct TrainingExtractor {
    hub: DspyHub,
    criteria: SuccessCriteria,
}

impl TrainingExtractor {
    /// Create a new extractor with the given hub.
    pub fn new(hub: DspyHub) -> Self {
        Self {
            hub,
            criteria: SuccessCriteria::default(),
        }
    }

    /// Set success criteria for filtering.
    pub fn with_criteria(mut self, criteria: SuccessCriteria) -> Self {
        self.criteria = criteria;
        self
    }

    /// Get the sessions directory path.
    fn sessions_path(&self) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(&home).join(".openagents/sessions")
    }

    /// Extract examples from a specific session log file.
    pub fn extract_from_session(&self, session_path: &Path) -> Result<ExtractedExamples> {
        let mut examples = ExtractedExamples::default();

        if !session_path.exists() {
            return Ok(examples);
        }

        let file = File::open(session_path).context("Failed to open session log")?;
        let reader = BufReader::new(file);

        // Collect all entries
        let mut entries: Vec<LogEntry> = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<LogEntry>(&line) {
                entries.push(entry);
            }
        }

        examples.entries_processed = entries.len();

        // Extract session ID from filename
        if let Some(stem) = session_path.file_stem() {
            examples
                .session_ids
                .push(stem.to_string_lossy().to_string());
        }

        // Process entries by phase
        let mut phase_entries: HashMap<String, Vec<&LogEntry>> = HashMap::new();
        for entry in &entries {
            phase_entries
                .entry(entry.phase.clone())
                .or_default()
                .push(entry);
        }

        // Extract planning examples
        if let Some(planning_entries) = phase_entries.get("planning") {
            if let Some(example) = self.extract_planning_example(planning_entries) {
                examples.planning.push(example);
            }
        }

        // Extract execution examples
        if let Some(execution_entries) = phase_entries.get("execution") {
            examples
                .execution
                .extend(self.extract_execution_examples(execution_entries));
        }

        // Extract verification examples
        if let Some(verification_entries) = phase_entries.get("verification") {
            if let Some(example) = self.extract_verification_example(verification_entries) {
                examples.verification.push(example);
            }
        }

        // Extract tool examples from all phases
        for phase_entries in phase_entries.values() {
            examples
                .tool_selection
                .extend(self.extract_tool_selection_examples(phase_entries));
            examples
                .tool_interpretation
                .extend(self.extract_tool_interpretation_examples(phase_entries));
        }

        Ok(examples)
    }

    /// Extract examples from all sessions on a specific date (YYYYMMDD).
    pub fn extract_from_date(&self, date: &str) -> Result<ExtractedExamples> {
        let date_path = self.sessions_path().join(date);
        let mut all_examples = ExtractedExamples::default();

        if !date_path.exists() {
            return Ok(all_examples);
        }

        for entry in fs::read_dir(&date_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let examples = self.extract_from_session(&path)?;
                all_examples.merge(examples);
            }
        }

        Ok(all_examples)
    }

    /// Extract examples from all available sessions.
    pub fn extract_all(&self) -> Result<ExtractedExamples> {
        let sessions_path = self.sessions_path();
        let mut all_examples = ExtractedExamples::default();

        if !sessions_path.exists() {
            return Ok(all_examples);
        }

        for date_entry in fs::read_dir(&sessions_path)? {
            let date_entry = date_entry?;
            if date_entry.file_type()?.is_dir() {
                if let Some(date) = date_entry.file_name().to_str() {
                    let examples = self.extract_from_date(date)?;
                    all_examples.merge(examples);
                }
            }
        }

        Ok(all_examples)
    }

    /// Save extracted training data to the hub.
    pub fn save_training_data(&self, examples: &ExtractedExamples) -> Result<SavedTrainingPaths> {
        let training_path = self.hub.training_path();
        create_dir_all(&training_path)?;

        let date = chrono::Local::now().format("%Y%m%d").to_string();
        let mut paths = SavedTrainingPaths::default();

        // Save planning examples
        if !examples.planning.is_empty() {
            let dir = training_path.join("PlanningSignature");
            create_dir_all(&dir)?;
            let path = dir.join(format!("{}.jsonl", date));
            self.append_jsonl(&path, &examples.planning)?;
            paths.planning = Some(path);
        }

        // Save execution examples
        if !examples.execution.is_empty() {
            let dir = training_path.join("ExecutionSignature");
            create_dir_all(&dir)?;
            let path = dir.join(format!("{}.jsonl", date));
            self.append_jsonl(&path, &examples.execution)?;
            paths.execution = Some(path);
        }

        // Save verification examples
        if !examples.verification.is_empty() {
            let dir = training_path.join("VerificationSignature");
            create_dir_all(&dir)?;
            let path = dir.join(format!("{}.jsonl", date));
            self.append_jsonl(&path, &examples.verification)?;
            paths.verification = Some(path);
        }

        // Save tool selection examples
        if !examples.tool_selection.is_empty() {
            let dir = training_path.join("ToolSelectionSignature");
            create_dir_all(&dir)?;
            let path = dir.join(format!("{}.jsonl", date));
            self.append_jsonl(&path, &examples.tool_selection)?;
            paths.tool_selection = Some(path);
        }

        // Save tool interpretation examples
        if !examples.tool_interpretation.is_empty() {
            let dir = training_path.join("ToolResultInterpretationSignature");
            create_dir_all(&dir)?;
            let path = dir.join(format!("{}.jsonl", date));
            self.append_jsonl(&path, &examples.tool_interpretation)?;
            paths.tool_interpretation = Some(path);
        }

        Ok(paths)
    }

    /// Load training data for a specific signature.
    pub fn load_training_data(&self, signature_name: &str) -> Result<Vec<Example>> {
        let sig_path = self.hub.training_path().join(signature_name);
        let mut examples = Vec::new();

        if !sig_path.exists() {
            return Ok(examples);
        }

        for entry in fs::read_dir(&sig_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let file = File::open(&path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(example) = serde_json::from_str::<Example>(&line) {
                        examples.push(example);
                    }
                }
            }
        }

        Ok(examples)
    }

    /// Append items to a JSONL file.
    fn append_jsonl<T: Serialize>(&self, path: &Path, items: &[T]) -> Result<()> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let mut writer = BufWriter::new(file);
        for item in items {
            let json = serde_json::to_string(item)?;
            writeln!(writer, "{}", json)?;
        }
        writer.flush()?;
        Ok(())
    }

    /// Extract a planning example from planning phase entries.
    fn extract_planning_example(&self, entries: &[&LogEntry]) -> Option<PlanningExample> {
        // Find phase_start and result entries
        let mut repository_summary = String::new();
        let mut issue_description = String::new();
        let mut relevant_files = String::new();
        let mut analysis = String::new();
        let mut files: Vec<String> = Vec::new();
        let mut steps: Vec<String> = Vec::new();
        let mut test_strategy = String::new();
        let mut has_result = false;

        for entry in entries {
            match entry.event_type.as_str() {
                "phase_start" => {
                    // Extract inputs from phase_start data
                    if let Some(summary) = entry
                        .data
                        .get("repository_summary")
                        .and_then(|v| v.as_str())
                    {
                        repository_summary = summary.to_string();
                    }
                    if let Some(issue) =
                        entry.data.get("issue_description").and_then(|v| v.as_str())
                    {
                        issue_description = issue.to_string();
                    }
                    if let Some(files_str) =
                        entry.data.get("relevant_files").and_then(|v| v.as_str())
                    {
                        relevant_files = files_str.to_string();
                    }
                }
                "result" => {
                    has_result = true;
                    if let Some(a) = entry.data.get("analysis").and_then(|v| v.as_str()) {
                        analysis = a.to_string();
                    }
                    if let Some(f) = entry.data.get("files_to_modify") {
                        if let Some(arr) = f.as_array() {
                            files = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                        }
                    }
                    if let Some(s) = entry.data.get("implementation_steps") {
                        if let Some(arr) = s.as_array() {
                            steps = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                        }
                    }
                    if let Some(t) = entry.data.get("test_strategy").and_then(|v| v.as_str()) {
                        test_strategy = t.to_string();
                    }
                }
                _ => {}
            }
        }

        // Only return if we have a successful result
        if self.criteria.require_success && !has_result {
            return None;
        }

        if !issue_description.is_empty() && !steps.is_empty() {
            Some(PlanningExample {
                repository_summary,
                issue_description,
                relevant_files,
                expected_analysis: analysis,
                expected_files: files,
                expected_steps: steps,
                expected_test_strategy: test_strategy,
            })
        } else {
            None
        }
    }

    /// Extract execution examples from execution phase entries.
    fn extract_execution_examples(&self, entries: &[&LogEntry]) -> Vec<ExecutionExample> {
        let mut examples = Vec::new();
        let mut current_step = String::new();
        let mut file_state = String::new();
        let mut history = Vec::new();

        for entry in entries {
            match entry.event_type.as_str() {
                "phase_start" => {
                    if let Some(step) = entry.data.get("current_step").and_then(|v| v.as_str()) {
                        current_step = step.to_string();
                    }
                    if let Some(state) = entry.data.get("file_state").and_then(|v| v.as_str()) {
                        file_state = state.to_string();
                    }
                }
                "tool_use" => {
                    if let Some(tool) = entry.data.get("tool").and_then(|v| v.as_str()) {
                        let input = entry.data.get("input").cloned().unwrap_or_default();

                        // Create an execution example for this tool use
                        if !current_step.is_empty() {
                            examples.push(ExecutionExample {
                                plan_step: current_step.clone(),
                                current_file_state: file_state.clone(),
                                execution_history: history.join("\n"),
                                expected_action: tool.to_uppercase(),
                                expected_params: input,
                                expected_reasoning: format!(
                                    "Execute {} for step: {}",
                                    tool, current_step
                                ),
                            });

                            // Add to history
                            history.push(format!("{}: {}", tool, entry.timestamp));
                        }
                    }
                }
                _ => {}
            }
        }

        examples
    }

    /// Extract a verification example from verification phase entries.
    fn extract_verification_example(&self, entries: &[&LogEntry]) -> Option<VerificationExample> {
        let mut requirements: Vec<String> = Vec::new();
        let mut solution_summary = String::new();
        let mut code_changes = String::new();
        let mut build_output = String::new();
        let mut test_output = String::new();
        let mut verdict = String::new();
        let mut explanation = String::new();
        let mut has_result = false;

        for entry in entries {
            match entry.event_type.as_str() {
                "phase_start" => {
                    if let Some(reqs) = entry.data.get("requirements") {
                        if let Some(arr) = reqs.as_array() {
                            requirements = arr
                                .iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect();
                        }
                    }
                    if let Some(summary) =
                        entry.data.get("solution_summary").and_then(|v| v.as_str())
                    {
                        solution_summary = summary.to_string();
                    }
                    if let Some(changes) = entry.data.get("code_changes").and_then(|v| v.as_str()) {
                        code_changes = changes.to_string();
                    }
                }
                "tool_result" => {
                    if let Some(tool) = entry.data.get("tool").and_then(|v| v.as_str()) {
                        if tool.contains("build") || tool.contains("cargo") {
                            if let Some(result) = entry.data.get("result").and_then(|v| v.as_str())
                            {
                                build_output = result.to_string();
                            }
                        }
                        if tool.contains("test") {
                            if let Some(result) = entry.data.get("result").and_then(|v| v.as_str())
                            {
                                test_output = result.to_string();
                            }
                        }
                    }
                }
                "result" => {
                    has_result = true;
                    if let Some(v) = entry.data.get("verdict").and_then(|v| v.as_str()) {
                        verdict = v.to_string();
                    }
                    if let Some(e) = entry.data.get("explanation").and_then(|v| v.as_str()) {
                        explanation = e.to_string();
                    }
                }
                _ => {}
            }
        }

        if self.criteria.require_success && !has_result {
            return None;
        }

        if !verdict.is_empty() {
            Some(VerificationExample {
                requirements,
                solution_summary,
                code_changes,
                build_output,
                test_output,
                expected_verdict: verdict,
                expected_explanation: explanation,
            })
        } else {
            None
        }
    }

    /// Extract tool selection examples from entries.
    fn extract_tool_selection_examples(&self, entries: &[&LogEntry]) -> Vec<Example> {
        let mut examples = Vec::new();

        for entry in entries {
            if entry.event_type == "tool_use" {
                if let Some(tool) = entry.data.get("tool").and_then(|v| v.as_str()) {
                    // Extract context from previous entries if available
                    let context = entry
                        .data
                        .get("context")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let task = entry
                        .data
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    if !context.is_empty() || !task.is_empty() {
                        let mut data = std::collections::HashMap::new();
                        data.insert("context".to_string(), serde_json::Value::String(context));
                        data.insert("task".to_string(), serde_json::Value::String(task));
                        data.insert(
                            "available_tools".to_string(),
                            serde_json::Value::String("read,edit,bash,glob,grep".to_string()),
                        );
                        data.insert(
                            "selected_tool".to_string(),
                            serde_json::Value::String(tool.to_string()),
                        );
                        data.insert(
                            "confidence".to_string(),
                            serde_json::Value::String("0.9".to_string()),
                        );

                        let input_keys = vec![
                            "context".to_string(),
                            "task".to_string(),
                            "available_tools".to_string(),
                        ];
                        let output_keys =
                            vec!["selected_tool".to_string(), "confidence".to_string()];

                        examples.push(Example::new(data, input_keys, output_keys));
                    }
                }
            }
        }

        examples
    }

    /// Extract tool interpretation examples from entries.
    fn extract_tool_interpretation_examples(&self, entries: &[&LogEntry]) -> Vec<Example> {
        let mut examples = Vec::new();

        // Look for tool_use followed by tool_result pairs
        let mut i = 0;
        while i < entries.len() {
            if entries[i].event_type == "tool_use" {
                if let Some(tool) = entries[i].data.get("tool").and_then(|v| v.as_str()) {
                    // Look for the corresponding result
                    let mut j = i + 1;
                    while j < entries.len() && entries[j].event_type != "tool_result" {
                        j += 1;
                    }

                    if j < entries.len() && entries[j].event_type == "tool_result" {
                        let result_tool = entries[j].data.get("tool").and_then(|v| v.as_str());
                        if result_tool == Some(tool) {
                            // We have a matching pair
                            let input = entries[i].data.get("input").cloned().unwrap_or_default();
                            let result = entries[j].data.get("result").cloned().unwrap_or_default();

                            let mut data = std::collections::HashMap::new();
                            data.insert(
                                "tool_name".to_string(),
                                serde_json::Value::String(tool.to_string()),
                            );
                            data.insert("tool_input".to_string(), input);
                            data.insert("tool_output".to_string(), result.clone());
                            data.insert(
                                "success".to_string(),
                                serde_json::Value::String("YES".to_string()),
                            );
                            data.insert(
                                "interpretation".to_string(),
                                serde_json::Value::String(format!(
                                    "Tool {} completed successfully",
                                    tool
                                )),
                            );
                            data.insert(
                                "next_action".to_string(),
                                serde_json::Value::String("CONTINUE".to_string()),
                            );

                            let input_keys = vec![
                                "tool_name".to_string(),
                                "tool_input".to_string(),
                                "tool_output".to_string(),
                            ];
                            let output_keys = vec![
                                "success".to_string(),
                                "interpretation".to_string(),
                                "next_action".to_string(),
                            ];

                            examples.push(Example::new(data, input_keys, output_keys));
                        }
                    }
                }
            }
            i += 1;
        }

        examples
    }
}

/// Paths to saved training data files.
#[derive(Debug, Clone, Default)]
pub struct SavedTrainingPaths {
    pub planning: Option<PathBuf>,
    pub execution: Option<PathBuf>,
    pub verification: Option<PathBuf>,
    pub tool_selection: Option<PathBuf>,
    pub tool_interpretation: Option<PathBuf>,
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_extractor() -> (TrainingExtractor, TempDir) {
        let temp = TempDir::new().unwrap();
        let hub = DspyHub::with_base_path(temp.path().to_path_buf());
        let extractor = TrainingExtractor::new(hub);
        (extractor, temp)
    }

    #[test]
    fn test_extracted_examples_merge() {
        let mut examples1 = ExtractedExamples::default();
        examples1.planning.push(PlanningExample {
            repository_summary: "Test".to_string(),
            issue_description: "Issue 1".to_string(),
            relevant_files: "".to_string(),
            expected_analysis: "".to_string(),
            expected_files: vec![],
            expected_steps: vec!["Step 1".to_string()],
            expected_test_strategy: "".to_string(),
        });

        let mut examples2 = ExtractedExamples::default();
        examples2.planning.push(PlanningExample {
            repository_summary: "Test".to_string(),
            issue_description: "Issue 2".to_string(),
            relevant_files: "".to_string(),
            expected_analysis: "".to_string(),
            expected_files: vec![],
            expected_steps: vec!["Step 2".to_string()],
            expected_test_strategy: "".to_string(),
        });

        examples1.merge(examples2);
        assert_eq!(examples1.planning.len(), 2);
    }

    #[test]
    fn test_extracted_examples_has_enough() {
        let mut examples = ExtractedExamples::default();
        assert!(!examples.has_enough_examples(1));

        examples.planning.push(PlanningExample {
            repository_summary: "".to_string(),
            issue_description: "Test".to_string(),
            relevant_files: "".to_string(),
            expected_analysis: "".to_string(),
            expected_files: vec![],
            expected_steps: vec!["Step".to_string()],
            expected_test_strategy: "".to_string(),
        });

        assert!(examples.has_enough_examples(1));
        assert!(!examples.has_enough_examples(2));
    }

    #[test]
    fn test_extract_from_nonexistent_session() {
        let (extractor, _temp) = test_extractor();
        let result = extractor.extract_from_session(Path::new("/nonexistent/path.jsonl"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().total_count(), 0);
    }

    #[test]
    fn test_load_empty_training_data() {
        let (extractor, _temp) = test_extractor();
        let result = extractor.load_training_data("NonexistentSignature");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_success_criteria_defaults() {
        let criteria = SuccessCriteria::default();
        assert!(criteria.require_success);
        assert_eq!(criteria.min_confidence, Some(0.7));
        assert!(criteria.phase_filter.is_none());
    }
}
