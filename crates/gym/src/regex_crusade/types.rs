//! RegexCrusade domain types
//!
//! Laser-focused types for solving the regex-log Terminal-Bench task.

/// The single task we care about - hardcoded for regex-log
pub const REGEX_LOG_TASK_ID: &str = "regex-log";
pub const REGEX_LOG_TASK_NAME: &str = "Regex Log Parser";
pub const REGEX_LOG_DESCRIPTION: &str = r#"Write a regex pattern that captures dates from log lines.

Requirements:
- Match dates in YYYY-MM-DD format
- Only match on lines containing an IPv4 address
- Capture the LAST date if multiple dates exist on a line
- Respect word boundaries (don't match abc2023-10-15)
- Allow February up to 29 days

Output: Write the regex to /app/regex.txt"#;

/// Status of the crusade session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrusadeStatus {
    #[default]
    Idle,
    GeneratingTests,
    RunningIteration,
    Validating,
    Completed,
    Failed,
}

impl CrusadeStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::GeneratingTests => "Generating Tests...",
            Self::RunningIteration => "Running Iteration...",
            Self::Validating => "Validating...",
            Self::Completed => "Complete!",
            Self::Failed => "Failed",
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::GeneratingTests | Self::RunningIteration | Self::Validating
        )
    }
}

/// Whether a test is a STUB (just `pass`) or REAL (has assertions)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestQuality {
    #[default]
    Unknown,
    /// Test body is just `pass` or empty assertions
    Stub,
    /// Test has actual input/output assertions
    Real,
    /// Has assertions but looks templated
    Suspicious,
}

impl TestQuality {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Unknown => "?",
            Self::Stub => "S",
            Self::Real => "R",
            Self::Suspicious => "!",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Stub => "STUB",
            Self::Real => "Real",
            Self::Suspicious => "Suspicious",
        }
    }
}

/// Detect if a test is a stub by analyzing its code
pub fn detect_stub(code: &str) -> TestQuality {
    let trimmed = code.trim();

    // Obvious stubs
    if trimmed.is_empty() || trimmed == "pass" {
        return TestQuality::Stub;
    }

    // Check for meaningful assertions
    let has_assert = trimmed.contains("assert");
    let has_real_check = trimmed.contains("==")
        || trimmed.contains("in ")
        || trimmed.contains(".match(")
        || trimmed.contains(".search(")
        || trimmed.contains(".findall(");

    if !has_assert {
        // No assertions at all
        return TestQuality::Stub;
    }

    if has_assert && !has_real_check {
        // Has assert but might be `assert True` or `assert result`
        if trimmed.contains("assert True")
            || trimmed.contains("assert result")
            || trimmed.contains("assert output")
        {
            return TestQuality::Suspicious;
        }
    }

    if has_assert && has_real_check {
        return TestQuality::Real;
    }

    TestQuality::Unknown
}

/// Test categories (subset relevant to regex-log)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrusadeCategory {
    AntiCheat,
    Existence,
    Correctness,
    Boundary,
    Integration,
}

impl CrusadeCategory {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::AntiCheat => "AC",
            Self::Existence => "EX",
            Self::Correctness => "CO",
            Self::Boundary => "BO",
            Self::Integration => "IN",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AntiCheat => "Anti-Cheat",
            Self::Existence => "Existence",
            Self::Correctness => "Correctness",
            Self::Boundary => "Boundary",
            Self::Integration => "Integration",
        }
    }
}

/// Test run status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestRunStatus {
    #[default]
    NotRun,
    Running,
    Passed,
    Failed,
    Error,
}

impl TestRunStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::NotRun => "-",
            Self::Running => "~",
            Self::Passed => "+",
            Self::Failed => "x",
            Self::Error => "!",
        }
    }
}

/// A test case with stub detection
#[derive(Debug, Clone)]
pub struct CrusadeTest {
    pub id: String,
    pub category: CrusadeCategory,
    pub quality: TestQuality,
    pub status: TestRunStatus,
    /// The input data for this test
    pub input: String,
    /// Expected output/match
    pub expected: Option<String>,
    /// Actual output when run (None if not run yet)
    pub actual: Option<String>,
    /// The Python test code
    pub code: String,
    /// Why this test exists
    pub reasoning: String,
    /// Confidence from TestGen (0.0-1.0)
    pub confidence: f32,
}

/// A single iteration/turn in the hill-climbing process
#[derive(Debug, Clone)]
pub struct Iteration {
    pub turn: u32,
    /// The regex pattern tried this turn
    pub regex_pattern: String,
    /// Tests passed / total
    pub passed: u32,
    pub total: u32,
    /// What changed from previous iteration
    pub change_description: String,
    /// Duration in ms
    pub duration_ms: u32,
}

impl Iteration {
    pub fn pass_rate(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f32 / self.total as f32
        }
    }
}

/// Session state for the crusade
#[derive(Debug, Clone, Default)]
pub struct CrusadeSession {
    pub status: CrusadeStatus,
    /// Current best regex pattern
    pub best_regex: Option<String>,
    /// Current pass rate
    pub tests_passed: u32,
    pub tests_total: u32,
    /// Number of stub tests (bad)
    pub stub_count: u32,
    /// Number of real tests (good)
    pub real_count: u32,
    /// Iteration history
    pub iterations: Vec<Iteration>,
}

/// Generate sample test data for MVP
pub fn sample_tests() -> Vec<CrusadeTest> {
    vec![
        CrusadeTest {
            id: "test_anti_cheat_no_hardcoded_dates".to_string(),
            category: CrusadeCategory::AntiCheat,
            quality: TestQuality::Real,
            status: TestRunStatus::Passed,
            input: "192.168.1.1 hello world".to_string(),
            expected: Some("[]".to_string()),
            actual: Some("[]".to_string()),
            code: r#"pattern = Path("/app/regex.txt").read_text().strip()
matches = re.findall(pattern, "192.168.1.1 hello world")
assert matches == [], f"Expected no matches, got {matches}""#
                .to_string(),
            reasoning: "Verify regex doesn't hardcode date values".to_string(),
            confidence: 0.95,
        },
        CrusadeTest {
            id: "test_existence_regex_file".to_string(),
            category: CrusadeCategory::Existence,
            quality: TestQuality::Real,
            status: TestRunStatus::Passed,
            input: "/app/regex.txt".to_string(),
            expected: Some("exists".to_string()),
            actual: Some("exists".to_string()),
            code: r#"assert Path("/app/regex.txt").exists(), "regex.txt must exist""#.to_string(),
            reasoning: "Output file must exist".to_string(),
            confidence: 1.0,
        },
        CrusadeTest {
            id: "test_correctness_simple_date".to_string(),
            category: CrusadeCategory::Correctness,
            quality: TestQuality::Stub,
            status: TestRunStatus::NotRun,
            input: "192.168.1.1 2024-01-15 some log".to_string(),
            expected: Some("[\"2024-01-15\"]".to_string()),
            actual: None,
            code: "pass  # TODO: Implement test logic".to_string(),
            reasoning: "Basic date extraction".to_string(),
            confidence: 0.8,
        },
        CrusadeTest {
            id: "test_correctness_last_date_only".to_string(),
            category: CrusadeCategory::Correctness,
            quality: TestQuality::Stub,
            status: TestRunStatus::NotRun,
            input: "192.168.1.1 2024-01-15 2024-02-28".to_string(),
            expected: Some("[\"2024-02-28\"]".to_string()),
            actual: None,
            code: "pass".to_string(),
            reasoning: "Must capture LAST date only".to_string(),
            confidence: 0.7,
        },
        CrusadeTest {
            id: "test_correctness_requires_ipv4".to_string(),
            category: CrusadeCategory::Correctness,
            quality: TestQuality::Stub,
            status: TestRunStatus::NotRun,
            input: "No IP here 2024-01-15".to_string(),
            expected: Some("[]".to_string()),
            actual: None,
            code: "pass".to_string(),
            reasoning: "No match without IPv4".to_string(),
            confidence: 0.9,
        },
        CrusadeTest {
            id: "test_boundary_word_boundary".to_string(),
            category: CrusadeCategory::Boundary,
            quality: TestQuality::Suspicious,
            status: TestRunStatus::Failed,
            input: "192.168.1.1 abc2024-01-15".to_string(),
            expected: Some("[]".to_string()),
            actual: Some("[\"2024-01-15\"]".to_string()),
            code: r#"assert result  # needs real assertion"#.to_string(),
            reasoning: "Word boundary prevents partial match".to_string(),
            confidence: 0.6,
        },
        CrusadeTest {
            id: "test_boundary_feb_29".to_string(),
            category: CrusadeCategory::Boundary,
            quality: TestQuality::Real,
            status: TestRunStatus::Passed,
            input: "10.0.0.1 2024-02-29".to_string(),
            expected: Some("[\"2024-02-29\"]".to_string()),
            actual: Some("[\"2024-02-29\"]".to_string()),
            code: r#"matches = re.findall(pattern, "10.0.0.1 2024-02-29")
assert matches == ["2024-02-29"]"#
                .to_string(),
            reasoning: "Feb 29 is valid in leap years".to_string(),
            confidence: 0.85,
        },
        CrusadeTest {
            id: "test_integration_multiline".to_string(),
            category: CrusadeCategory::Integration,
            quality: TestQuality::Stub,
            status: TestRunStatus::NotRun,
            input: "Line1: 192.168.1.1 2024-01-01\\nLine2: no ip 2024-02-02".to_string(),
            expected: Some("[\"2024-01-01\"]".to_string()),
            actual: None,
            code: "pass".to_string(),
            reasoning: "Only match lines with IP".to_string(),
            confidence: 0.75,
        },
    ]
}

/// Generate sample iteration data for MVP
pub fn sample_iterations() -> Vec<Iteration> {
    vec![
        Iteration {
            turn: 1,
            regex_pattern: r"\d{4}-\d{2}-\d{2}".to_string(),
            passed: 3,
            total: 8,
            change_description: "Initial naive pattern".to_string(),
            duration_ms: 1200,
        },
        Iteration {
            turn: 2,
            regex_pattern: r"\b\d{4}-\d{2}-\d{2}\b".to_string(),
            passed: 5,
            total: 8,
            change_description: "Added word boundaries".to_string(),
            duration_ms: 980,
        },
        Iteration {
            turn: 3,
            regex_pattern: r"(?=.*\d+\.\d+\.\d+\.\d+)\b\d{4}-\d{2}-\d{2}\b".to_string(),
            passed: 6,
            total: 8,
            change_description: "Added IPv4 lookahead".to_string(),
            duration_ms: 1450,
        },
    ]
}
