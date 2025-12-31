//! Smart checklist generation for PR reviews

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Review template types
///
/// Determines the type of checklist generated for a PR review.
/// Each template adds specific checks relevant to that type of change.
///
/// # Examples
///
/// ```
/// use gitafter::review::{ReviewTemplate, ChecklistGenerator};
///
/// // Generate a bug fix review checklist
/// let files = vec!["src/auth.rs".to_string()];
/// let checklist = ChecklistGenerator::generate(&files, ReviewTemplate::BugFix);
///
/// // Will include items like "Regression test added for the bug"
/// assert!(checklist.iter().any(|item| item.id == "bug-regression-test"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewTemplate {
    /// Bug fix review
    BugFix,
    /// New feature review
    Feature,
    /// Code refactoring review
    Refactor,
    /// NIP implementation review
    NipImplementation,
    /// General review (default)
    General,
}

/// Checklist item for PR review
///
/// Represents a single item in a code review checklist. Items can be
/// required or optional, manually checked or auto-checkable, and belong
/// to categories like "tests", "docs", "security", etc.
///
/// # Examples
///
/// ```
/// use gitafter::review::ChecklistItem;
///
/// // Create a required, auto-checkable item
/// let mut item = ChecklistItem::new(
///     "rust-clippy",
///     "Clippy warnings addressed",
///     "code",
///     true,   // required
///     true,   // auto-checkable
/// );
///
/// assert!(!item.checked);
/// assert!(item.required);
/// assert!(item.auto_checkable);
///
/// // Mark as checked
/// item.check();
/// assert!(item.checked);
///
/// // Set auto-check result
/// item.set_auto_result("0 warnings found");
/// assert_eq!(item.auto_result, Some("0 warnings found".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    /// Unique identifier
    pub id: String,
    /// Description of the check
    pub description: String,
    /// Category (code, tests, docs, security, etc)
    pub category: String,
    /// Whether this is required or optional
    pub required: bool,
    /// Whether it can be auto-checked
    pub auto_checkable: bool,
    /// Current check status
    pub checked: bool,
    /// Auto-check result if available
    pub auto_result: Option<String>,
}

impl ChecklistItem {
    /// Create a new checklist item
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the item (e.g., "rust-clippy")
    /// * `description` - Human-readable description of the check
    /// * `category` - Category tag ("code", "tests", "docs", "security", etc.)
    /// * `required` - Whether this check must pass before merge
    /// * `auto_checkable` - Whether this can be automatically validated
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::ChecklistItem;
    ///
    /// // Required security check that can be auto-verified
    /// let security_check = ChecklistItem::new(
    ///     "sql-injection",
    ///     "No SQL injection vulnerabilities",
    ///     "security",
    ///     true,
    ///     true,
    /// );
    ///
    /// // Optional documentation check that needs manual review
    /// let doc_check = ChecklistItem::new(
    ///     "api-docs",
    ///     "Public API documented",
    ///     "docs",
    ///     false,
    ///     false,
    /// );
    /// ```
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        required: bool,
        auto_checkable: bool,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            category: category.into(),
            required,
            auto_checkable,
            checked: false,
            auto_result: None,
        }
    }

    /// Mark as checked
    ///
    /// Marks this checklist item as manually verified by a reviewer.
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::ChecklistItem;
    ///
    /// let mut item = ChecklistItem::new("test", "Test item", "code", true, false);
    /// assert!(!item.checked);
    ///
    /// item.check();
    /// assert!(item.checked);
    /// ```
    pub fn check(&mut self) {
        self.checked = true;
    }

    /// Set auto-check result
    ///
    /// Records the result of an automated validation for this item.
    ///
    /// # Arguments
    ///
    /// * `result` - The auto-check result message (e.g., "PASS: 0 warnings", "FAIL: 3 errors")
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::ChecklistItem;
    ///
    /// let mut item = ChecklistItem::new(
    ///     "clippy",
    ///     "Clippy warnings addressed",
    ///     "code",
    ///     true,
    ///     true,
    /// );
    ///
    /// item.set_auto_result("PASS: 0 warnings found");
    /// assert_eq!(item.auto_result, Some("PASS: 0 warnings found".to_string()));
    /// ```
    pub fn set_auto_result(&mut self, result: impl Into<String>) {
        self.auto_result = Some(result.into());
    }
}

/// Generator for PR review checklists
///
/// Analyzes changed files and generates an appropriate checklist based on:
/// - Review template (bug fix, feature, refactor, NIP implementation)
/// - File types (Rust, TOML, Markdown, SQL)
/// - OpenAgents directives (d-012 no stubs, d-013 testing)
///
/// # Examples
///
/// ```
/// use gitafter::review::{ChecklistGenerator, ReviewTemplate};
///
/// // Generate checklist for a Rust feature PR
/// let changed_files = vec![
///     "src/auth.rs".to_string(),
///     "src/auth/mod.rs".to_string(),
///     "Cargo.toml".to_string(),
/// ];
///
/// let checklist = ChecklistGenerator::generate(&changed_files, ReviewTemplate::Feature);
///
/// // Will include feature-specific items
/// assert!(checklist.iter().any(|item| item.id == "feature-tests"));
/// assert!(checklist.iter().any(|item| item.id == "feature-docs"));
///
/// // Will include Rust-specific items
/// assert!(checklist.iter().any(|item| item.id == "rust-clippy"));
/// assert!(checklist.iter().any(|item| item.id == "rust-tests"));
///
/// // Will include directive checks
/// assert!(checklist.iter().any(|item| item.id == "d012-no-stubs"));
/// assert!(checklist.iter().any(|item| item.id == "d013-tests"));
///
/// // Will include TOML checks
/// assert!(checklist.iter().any(|item| item.id == "toml-deps"));
/// ```
///
/// # File Type Detection
///
/// The generator automatically detects file types and adds relevant checks:
///
/// - `.rs` files → Rust checks (clippy, unsafe, error handling, tests)
/// - `.toml` files → Dependency and version checks
/// - `.md` files → Documentation link and example checks
/// - `.sql` files → Migration and security checks
pub struct ChecklistGenerator;

impl ChecklistGenerator {
    /// Generate checklist for a PR based on changed files
    ///
    /// # Arguments
    ///
    /// * `changed_files` - List of file paths modified in the PR
    /// * `template` - Review template to use (determines template-specific checks)
    ///
    /// # Returns
    ///
    /// A vector of checklist items combining:
    /// - Template-specific items (based on review type)
    /// - File-type-specific items (based on extensions)
    /// - Directive items (d-012, d-013 for Rust files)
    ///
    /// # Examples
    ///
    /// ```
    /// use gitafter::review::{ChecklistGenerator, ReviewTemplate};
    ///
    /// // Bug fix review for a Rust file
    /// let files = vec!["src/parser.rs".to_string()];
    /// let checklist = ChecklistGenerator::generate(&files, ReviewTemplate::BugFix);
    ///
    /// // Should include bug-specific checks
    /// assert!(checklist.iter().any(|item| item.id == "bug-regression-test"));
    /// assert!(checklist.iter().any(|item| item.id == "bug-root-cause"));
    ///
    /// // NIP implementation review
    /// let nip_files = vec!["src/nip09.rs".to_string()];
    /// let nip_checklist = ChecklistGenerator::generate(&nip_files, ReviewTemplate::NipImplementation);
    ///
    /// // Should include NIP-specific checks
    /// assert!(nip_checklist.iter().any(|item| item.id == "nip-spec-compliance"));
    /// assert!(nip_checklist.iter().any(|item| item.id == "nip-interop-tests"));
    /// ```
    pub fn generate(changed_files: &[String], template: ReviewTemplate) -> Vec<ChecklistItem> {
        let mut items = Vec::new();

        // Detect file types
        let file_types = Self::detect_file_types(changed_files);

        // Add template-specific items
        items.extend(Self::template_items(template));

        // Add file-type-specific items
        if file_types.contains("rust") {
            items.extend(Self::rust_items());
        }

        if file_types.contains("toml") {
            items.extend(Self::toml_items());
        }

        if file_types.contains("markdown") {
            items.extend(Self::markdown_items());
        }

        if file_types.contains("sql") {
            items.extend(Self::sql_items());
        }

        // Add directive-specific items (d-012, d-013)
        items.extend(Self::directive_items(&file_types));

        items
    }

    /// Detect file types from changed files
    fn detect_file_types(files: &[String]) -> HashSet<&'static str> {
        let mut types = HashSet::new();

        for file in files {
            let path = Path::new(file);
            if let Some(ext) = path.extension() {
                match ext.to_str() {
                    Some("rs") => {
                        types.insert("rust");
                    }
                    Some("toml") => {
                        types.insert("toml");
                    }
                    Some("md") => {
                        types.insert("markdown");
                    }
                    Some("sql") => {
                        types.insert("sql");
                    }
                    Some("yaml") | Some("yml") => {
                        types.insert("yaml");
                    }
                    _ => {}
                }
            }
        }

        types
    }

    /// Template-specific checklist items
    fn template_items(template: ReviewTemplate) -> Vec<ChecklistItem> {
        match template {
            ReviewTemplate::BugFix => vec![
                ChecklistItem::new(
                    "bug-regression-test",
                    "Regression test added for the bug",
                    "tests",
                    true,
                    false,
                ),
                ChecklistItem::new(
                    "bug-root-cause",
                    "Root cause documented in commit message or issue",
                    "docs",
                    true,
                    false,
                ),
                ChecklistItem::new(
                    "bug-related-code",
                    "Related code paths checked for similar issues",
                    "code",
                    false,
                    false,
                ),
            ],
            ReviewTemplate::Feature => vec![
                ChecklistItem::new(
                    "feature-tests",
                    "Unit and integration tests added",
                    "tests",
                    true,
                    true,
                ),
                ChecklistItem::new(
                    "feature-docs",
                    "Documentation updated (README, inline docs)",
                    "docs",
                    true,
                    false,
                ),
                ChecklistItem::new(
                    "feature-benchmarks",
                    "Benchmarks added if performance-critical",
                    "performance",
                    false,
                    false,
                ),
                ChecklistItem::new(
                    "feature-backward-compat",
                    "Backward compatibility maintained or migration provided",
                    "code",
                    true,
                    false,
                ),
            ],
            ReviewTemplate::Refactor => vec![
                ChecklistItem::new(
                    "refactor-behavior",
                    "No behavior changes (or explicitly documented)",
                    "code",
                    true,
                    false,
                ),
                ChecklistItem::new(
                    "refactor-tests",
                    "Existing tests still pass",
                    "tests",
                    true,
                    true,
                ),
                ChecklistItem::new(
                    "refactor-performance",
                    "Performance impact measured if relevant",
                    "performance",
                    false,
                    false,
                ),
            ],
            ReviewTemplate::NipImplementation => vec![
                ChecklistItem::new(
                    "nip-spec-compliance",
                    "Implementation matches NIP specification",
                    "code",
                    true,
                    false,
                ),
                ChecklistItem::new(
                    "nip-interop-tests",
                    "Interoperability tests with other implementations",
                    "tests",
                    true,
                    false,
                ),
                ChecklistItem::new(
                    "nip-docs",
                    "NIP number and link documented in code",
                    "docs",
                    true,
                    true,
                ),
            ],
            ReviewTemplate::General => vec![],
        }
    }

    /// Rust-specific checklist items
    fn rust_items() -> Vec<ChecklistItem> {
        vec![
            ChecklistItem::new(
                "rust-clippy",
                "Clippy warnings addressed",
                "code",
                true,
                true,
            ),
            ChecklistItem::new(
                "rust-unsafe",
                "Unsafe code justified and documented",
                "security",
                true,
                true,
            ),
            ChecklistItem::new(
                "rust-unwrap",
                "No unwrap/expect in production code paths",
                "code",
                true,
                true,
            ),
            ChecklistItem::new(
                "rust-error-handling",
                "Proper error handling (Result, not panic)",
                "code",
                true,
                false,
            ),
            ChecklistItem::new(
                "rust-tests",
                "Tests for new functions and modules",
                "tests",
                true,
                true,
            ),
        ]
    }

    /// TOML-specific checklist items
    fn toml_items() -> Vec<ChecklistItem> {
        vec![
            ChecklistItem::new(
                "toml-deps",
                "New dependencies justified",
                "code",
                true,
                false,
            ),
            ChecklistItem::new(
                "toml-versions",
                "Version constraints appropriate (not too loose)",
                "code",
                true,
                true,
            ),
            ChecklistItem::new(
                "toml-features",
                "Feature flags properly configured",
                "code",
                false,
                true,
            ),
        ]
    }

    /// Markdown-specific checklist items
    fn markdown_items() -> Vec<ChecklistItem> {
        vec![
            ChecklistItem::new(
                "md-links",
                "All links valid and accessible",
                "docs",
                true,
                true,
            ),
            ChecklistItem::new(
                "md-examples",
                "Code examples compile and run",
                "docs",
                true,
                false,
            ),
        ]
    }

    /// SQL-specific checklist items
    fn sql_items() -> Vec<ChecklistItem> {
        vec![
            ChecklistItem::new(
                "sql-migration",
                "Migration includes up and down paths",
                "code",
                true,
                false,
            ),
            ChecklistItem::new(
                "sql-indexes",
                "Indexes added for new query patterns",
                "performance",
                true,
                false,
            ),
            ChecklistItem::new(
                "sql-injection",
                "No SQL injection vulnerabilities",
                "security",
                true,
                true,
            ),
        ]
    }

    /// Directive-specific checklist items (d-012, d-013)
    fn directive_items(file_types: &HashSet<&'static str>) -> Vec<ChecklistItem> {
        let mut items = Vec::new();

        if file_types.contains("rust") {
            // d-012: No stubs
            items.push(ChecklistItem::new(
                "d012-no-stubs",
                "No stub patterns (todo!, unimplemented!, NotImplemented)",
                "code",
                true,
                true,
            ));

            // d-013: Tests required
            items.push(ChecklistItem::new(
                "d013-tests",
                "Tests cover new functionality",
                "tests",
                true,
                true,
            ));
        }

        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_file_types() {
        let files = vec![
            "src/main.rs".to_string(),
            "Cargo.toml".to_string(),
            "README.md".to_string(),
            "migrations/001.sql".to_string(),
        ];

        let types = ChecklistGenerator::detect_file_types(&files);

        assert!(types.contains("rust"));
        assert!(types.contains("toml"));
        assert!(types.contains("markdown"));
        assert!(types.contains("sql"));
    }

    #[test]
    fn test_generate_bugfix_checklist() {
        let files = vec!["src/main.rs".to_string()];
        let checklist = ChecklistGenerator::generate(&files, ReviewTemplate::BugFix);

        // Should have bug-specific + rust-specific + directive items
        assert!(!checklist.is_empty());

        // Check for bug-specific items
        assert!(
            checklist
                .iter()
                .any(|item| item.id == "bug-regression-test")
        );
    }

    #[test]
    fn test_generate_feature_checklist() {
        let files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];
        let checklist = ChecklistGenerator::generate(&files, ReviewTemplate::Feature);

        // Should have feature + rust + toml + directive items
        assert!(!checklist.is_empty());

        // Check for feature-specific items
        assert!(checklist.iter().any(|item| item.id == "feature-tests"));
        assert!(checklist.iter().any(|item| item.id == "feature-docs"));
    }

    #[test]
    fn test_generate_nip_checklist() {
        let files = vec!["src/nip09.rs".to_string()];
        let checklist = ChecklistGenerator::generate(&files, ReviewTemplate::NipImplementation);

        // Should have NIP-specific items
        assert!(
            checklist
                .iter()
                .any(|item| item.id == "nip-spec-compliance")
        );
        assert!(checklist.iter().any(|item| item.id == "nip-interop-tests"));
    }

    #[test]
    fn test_rust_items_include_d012() {
        let files = vec!["src/main.rs".to_string()];
        let checklist = ChecklistGenerator::generate(&files, ReviewTemplate::General);

        // Should have d-012 no stubs check
        assert!(checklist.iter().any(|item| item.id == "d012-no-stubs"));
        assert!(checklist.iter().any(|item| item.id == "d013-tests"));
    }

    #[test]
    fn test_checklist_item_operations() {
        let mut item = ChecklistItem::new("test", "Test item", "code", true, true);

        assert!(!item.checked);
        assert!(item.auto_result.is_none());

        item.check();
        assert!(item.checked);

        item.set_auto_result("PASS");
        assert_eq!(item.auto_result, Some("PASS".to_string()));
    }
}
