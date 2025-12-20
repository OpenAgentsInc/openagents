//! Core types for the marketplace

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Pricing model for marketplace skills
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillPricing {
    /// Free skill - no cost
    Free,
    /// Fixed cost per call
    PerCall {
        /// Credits charged per call
        credits: u64,
    },
    /// Cost based on token usage
    PerToken {
        /// Credits per 1K input tokens
        per_1k_input: u64,
        /// Credits per 1K output tokens
        per_1k_output: u64,
    },
    /// Combined fixed and token-based pricing
    Hybrid {
        /// Fixed credits per call
        per_call: u64,
        /// Credits per 1K input tokens
        per_1k_input: u64,
        /// Credits per 1K output tokens
        per_1k_output: u64,
    },
}

impl SkillPricing {
    /// Calculate the cost for a skill invocation
    ///
    /// # Arguments
    /// * `input_tokens` - Number of input tokens used
    /// * `output_tokens` - Number of output tokens generated
    ///
    /// # Returns
    /// Total cost in credits
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        match self {
            SkillPricing::Free => 0,
            SkillPricing::PerCall { credits } => *credits,
            SkillPricing::PerToken { per_1k_input, per_1k_output } => {
                let input_cost = (input_tokens * per_1k_input).div_ceil(1000); // Round up
                let output_cost = (output_tokens * per_1k_output).div_ceil(1000); // Round up
                input_cost + output_cost
            }
            SkillPricing::Hybrid { per_call, per_1k_input, per_1k_output } => {
                let input_cost = (input_tokens * per_1k_input).div_ceil(1000); // Round up
                let output_cost = (output_tokens * per_1k_output).div_ceil(1000); // Round up
                per_call + input_cost + output_cost
            }
        }
    }
}

/// Revenue split configuration for skill payments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevenueSplit {
    /// Percentage for skill creator (0-100)
    pub creator_pct: u8,
    /// Percentage for compute provider (0-100)
    pub compute_pct: u8,
    /// Percentage for platform (0-100)
    pub platform_pct: u8,
    /// Percentage for referrer (0-100)
    pub referrer_pct: u8,
}

impl RevenueSplit {
    /// Default revenue split
    pub const DEFAULT: Self = Self {
        creator_pct: 60,
        compute_pct: 25,
        platform_pct: 10,
        referrer_pct: 5,
    };

    /// Validate that the percentages sum to 100
    pub fn is_valid(&self) -> bool {
        self.creator_pct as u16
            + self.compute_pct as u16
            + self.platform_pct as u16
            + self.referrer_pct as u16
            == 100
    }

    /// Calculate the split amounts for a given total
    ///
    /// # Arguments
    /// * `total_credits` - Total credits to split
    ///
    /// # Returns
    /// Tuple of (creator, compute, platform, referrer) amounts
    pub fn split(&self, total_credits: u64) -> (u64, u64, u64, u64) {
        let creator = (total_credits * self.creator_pct as u64) / 100;
        let compute = (total_credits * self.compute_pct as u64) / 100;
        let platform = (total_credits * self.platform_pct as u64) / 100;
        let referrer = total_credits.saturating_sub(creator + compute + platform); // Remainder goes to referrer
        (creator, compute, platform, referrer)
    }
}

impl Default for RevenueSplit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Status of a skill submission in the publishing workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSubmissionStatus {
    /// Still being edited by creator
    Draft,
    /// Submitted and awaiting review assignment
    PendingReview,
    /// Currently being reviewed
    InReview,
    /// Reviewer requested changes
    ChangesRequested,
    /// Approved by reviewer
    Approved,
    /// Published to marketplace
    Published,
    /// No longer recommended for use
    Deprecated,
    /// Rejected and will not be published
    Rejected,
}

impl SkillSubmissionStatus {
    /// Get the status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            SkillSubmissionStatus::Draft => "draft",
            SkillSubmissionStatus::PendingReview => "pending_review",
            SkillSubmissionStatus::InReview => "in_review",
            SkillSubmissionStatus::ChangesRequested => "changes_requested",
            SkillSubmissionStatus::Approved => "approved",
            SkillSubmissionStatus::Published => "published",
            SkillSubmissionStatus::Deprecated => "deprecated",
            SkillSubmissionStatus::Rejected => "rejected",
        }
    }

    /// Check if the submission is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SkillSubmissionStatus::Published
                | SkillSubmissionStatus::Deprecated
                | SkillSubmissionStatus::Rejected
        )
    }

    /// Check if the submission can be edited
    pub fn is_editable(&self) -> bool {
        matches!(
            self,
            SkillSubmissionStatus::Draft | SkillSubmissionStatus::ChangesRequested
        )
    }
}

/// A skill submission for review and publishing
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSubmission {
    /// Unique submission ID
    pub id: String,
    /// Path to local skill directory
    pub skill_path: PathBuf,
    /// Creator's Nostr public key (hex format)
    pub creator: String,
    /// Current status
    pub status: SkillSubmissionStatus,
    /// When submitted
    pub submitted_at: DateTime<Utc>,
    /// When reviewed (if applicable)
    pub reviewed_at: Option<DateTime<Utc>>,
    /// Reviewer's notes
    pub reviewer_notes: Option<String>,
}

/// Type of quality check performed on a submission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityCheckType {
    /// Validates SKILL.md frontmatter schema
    SchemaValidation,
    /// Scans for malicious code in scripts
    SecurityScan,
    /// Tests execution in sandbox
    SandboxTest,
    /// Optional conformance benchmark
    BenchmarkTest,
}

impl QualityCheckType {
    /// Get the check type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityCheckType::SchemaValidation => "schema_validation",
            QualityCheckType::SecurityScan => "security_scan",
            QualityCheckType::SandboxTest => "sandbox_test",
            QualityCheckType::BenchmarkTest => "benchmark_test",
        }
    }

    /// Check if this is a required check
    pub fn is_required(&self) -> bool {
        !matches!(self, QualityCheckType::BenchmarkTest)
    }
}

/// Result of a quality check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityCheck {
    /// Type of check performed
    pub check_type: QualityCheckType,
    /// Whether the check passed
    pub passed: bool,
    /// Additional details about the check result
    pub details: Option<String>,
}

impl QualityCheck {
    /// Create a new quality check result
    pub fn new(check_type: QualityCheckType, passed: bool, details: Option<String>) -> Self {
        Self {
            check_type,
            passed,
            details,
        }
    }

    /// Create a passing check
    pub fn pass(check_type: QualityCheckType) -> Self {
        Self::new(check_type, true, None)
    }

    /// Create a failing check with details
    pub fn fail(check_type: QualityCheckType, details: impl Into<String>) -> Self {
        Self::new(check_type, false, Some(details.into()))
    }
}

/// Review decision for a skill submission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReviewDecision {
    /// Approve the submission for publishing
    Approve,
    /// Request changes before approval
    RequestChanges {
        /// Explanation of required changes
        reason: String,
    },
    /// Reject the submission
    Reject {
        /// Explanation of rejection
        reason: String,
    },
}

impl ReviewDecision {
    /// Check if the decision is approval
    pub fn is_approval(&self) -> bool {
        matches!(self, ReviewDecision::Approve)
    }

    /// Check if the decision is rejection
    pub fn is_rejection(&self) -> bool {
        matches!(self, ReviewDecision::Reject { .. })
    }

    /// Get the reason if applicable
    pub fn reason(&self) -> Option<&str> {
        match self {
            ReviewDecision::Approve => None,
            ReviewDecision::RequestChanges { reason } | ReviewDecision::Reject { reason } => {
                Some(reason)
            }
        }
    }
}

/// A review of a skill submission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionReview {
    /// ID of the submission being reviewed
    pub submission_id: String,
    /// Reviewer identifier
    pub reviewer: String,
    /// Review decision
    pub decision: ReviewDecision,
    /// Reviewer's notes
    pub notes: String,
    /// Quality checks performed
    pub checks: Vec<QualityCheck>,
}

impl SubmissionReview {
    /// Check if all required checks passed
    pub fn all_required_checks_passed(&self) -> bool {
        self.checks
            .iter()
            .filter(|c| c.check_type.is_required())
            .all(|c| c.passed)
    }

    /// Get failed checks
    pub fn failed_checks(&self) -> Vec<&QualityCheck> {
        self.checks.iter().filter(|c| !c.passed).collect()
    }
}

/// Status of a skill installation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallationStatus {
    /// Currently installing
    Installing,
    /// Successfully installed
    Installed,
    /// Update available with new version
    UpdateAvailable {
        /// New version available
        new_version: String,
    },
    /// Installation failed
    Failed {
        /// Error message
        error: String,
    },
    /// Previously installed but removed
    Uninstalled,
}

impl InstallationStatus {
    /// Check if installation is complete and working
    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            InstallationStatus::Installed | InstallationStatus::UpdateAvailable { .. }
        )
    }

    /// Check if installation is in progress
    pub fn is_installing(&self) -> bool {
        matches!(self, InstallationStatus::Installing)
    }

    /// Check if installation failed
    pub fn is_failed(&self) -> bool {
        matches!(self, InstallationStatus::Failed { .. })
    }

    /// Get error message if failed
    pub fn error(&self) -> Option<&str> {
        match self {
            InstallationStatus::Failed { error } => Some(error),
            _ => None,
        }
    }
}

/// A skill installation record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInstallation {
    /// Unique installation ID
    pub id: String,
    /// ID of the installed skill
    pub skill_id: String,
    /// User who installed the skill
    pub user_id: String,
    /// When installed
    pub installed_at: DateTime<Utc>,
    /// Installed version
    pub version: String,
    /// Local installation path
    pub path: PathBuf,
    /// Current installation status
    pub status: InstallationStatus,
}

/// An MCP dependency required by a skill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpDependency {
    /// MCP server name
    pub server_name: String,
    /// Optional version constraint (e.g., "^1.0.0", ">=2.1.0")
    pub version_constraint: Option<String>,
    /// Whether this dependency is required
    pub required: bool,
}

impl McpDependency {
    /// Create a new required dependency
    pub fn required(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            version_constraint: None,
            required: true,
        }
    }

    /// Create a new optional dependency
    pub fn optional(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
            version_constraint: None,
            required: false,
        }
    }

    /// Set version constraint
    pub fn with_version(mut self, constraint: impl Into<String>) -> Self {
        self.version_constraint = Some(constraint.into());
        self
    }
}

/// Information about a missing MCP dependency
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingDependency {
    /// Name of the missing server
    pub server_name: String,
    /// Required version constraint if any
    pub version_constraint: Option<String>,
    /// Whether this is a hard requirement
    pub required: bool,
    /// Suggested installation command or link
    pub install_hint: Option<String>,
}

impl MissingDependency {
    /// Create from an MCP dependency
    pub fn from_dependency(dep: &McpDependency) -> Self {
        Self {
            server_name: dep.server_name.clone(),
            version_constraint: dep.version_constraint.clone(),
            required: dep.required,
            install_hint: None,
        }
    }

    /// Add installation hint
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.install_hint = Some(hint.into());
        self
    }
}

/// Information about an available skill update
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillUpdate {
    /// Installation ID
    pub installation_id: String,
    /// Skill ID
    pub skill_id: String,
    /// Current version
    pub current_version: String,
    /// New version available
    pub new_version: String,
    /// Update description/changelog
    pub description: Option<String>,
    /// Whether this is a breaking change
    pub breaking: bool,
}

impl SkillUpdate {
    /// Create a new update notification
    pub fn new(
        installation_id: impl Into<String>,
        skill_id: impl Into<String>,
        current_version: impl Into<String>,
        new_version: impl Into<String>,
    ) -> Self {
        Self {
            installation_id: installation_id.into(),
            skill_id: skill_id.into(),
            current_version: current_version.into(),
            new_version: new_version.into(),
            description: None,
            breaking: false,
        }
    }

    /// Mark as breaking change
    pub fn as_breaking(mut self) -> Self {
        self.breaking = true;
        self
    }

    /// Add description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Type of marketplace item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceItemType {
    /// A plugin (extends functionality via MCP)
    Plugin,
    /// A skill (predefined task template)
    Skill,
    /// An agent (autonomous worker)
    Agent,
}

impl MarketplaceItemType {
    /// Get the item type as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            MarketplaceItemType::Plugin => "plugin",
            MarketplaceItemType::Skill => "skill",
            MarketplaceItemType::Agent => "agent",
        }
    }
}

/// Status of a marketplace item
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemStatus {
    /// Available for installation
    Available,
    /// Currently being installed
    Installing,
    /// Successfully installed
    Installed,
    /// Installation failed
    Failed,
    /// Update available
    UpdateAvailable,
}

impl ItemStatus {
    /// Get the status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Available => "available",
            ItemStatus::Installing => "installing",
            ItemStatus::Installed => "installed",
            ItemStatus::Failed => "failed",
            ItemStatus::UpdateAvailable => "update_available",
        }
    }

    /// Check if the item is installed
    pub fn is_installed(&self) -> bool {
        matches!(self, ItemStatus::Installed | ItemStatus::UpdateAvailable)
    }

    /// Check if the item is in a transitional state
    pub fn is_transitional(&self) -> bool {
        matches!(self, ItemStatus::Installing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_pricing_free() {
        let pricing = SkillPricing::Free;
        assert_eq!(pricing.calculate_cost(100, 200), 0);
        assert_eq!(pricing.calculate_cost(0, 0), 0);
    }

    #[test]
    fn test_skill_pricing_per_call() {
        let pricing = SkillPricing::PerCall { credits: 100 };
        assert_eq!(pricing.calculate_cost(100, 200), 100);
        assert_eq!(pricing.calculate_cost(0, 0), 100);
        assert_eq!(pricing.calculate_cost(1000, 1000), 100);
    }

    #[test]
    fn test_skill_pricing_per_token() {
        let pricing = SkillPricing::PerToken {
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // 1000 input tokens @ 10 credits/1k = 10 credits
        // 1000 output tokens @ 20 credits/1k = 20 credits
        // Total = 30 credits
        assert_eq!(pricing.calculate_cost(1000, 1000), 30);

        // 500 input tokens @ 10 credits/1k = 5 credits (rounds up to 5)
        // 500 output tokens @ 20 credits/1k = 10 credits (rounds up to 10)
        // Total = 15 credits
        assert_eq!(pricing.calculate_cost(500, 500), 15);

        // 1 input token @ 10 credits/1k = 1 credit (rounds up)
        // 1 output token @ 20 credits/1k = 1 credit (rounds up)
        // Total = 2 credits
        assert_eq!(pricing.calculate_cost(1, 1), 2);

        // 0 tokens = 0 credits
        assert_eq!(pricing.calculate_cost(0, 0), 0);
    }

    #[test]
    fn test_skill_pricing_hybrid() {
        let pricing = SkillPricing::Hybrid {
            per_call: 50,
            per_1k_input: 10,
            per_1k_output: 20,
        };

        // 50 (base) + 10 (1000 input tokens) + 20 (1000 output tokens) = 80 credits
        assert_eq!(pricing.calculate_cost(1000, 1000), 80);

        // 50 (base) + 5 (500 input tokens) + 10 (500 output tokens) = 65 credits
        assert_eq!(pricing.calculate_cost(500, 500), 65);

        // 50 (base) + 0 (0 tokens) = 50 credits
        assert_eq!(pricing.calculate_cost(0, 0), 50);
    }

    #[test]
    fn test_revenue_split_default() {
        let split = RevenueSplit::default();
        assert_eq!(split.creator_pct, 60);
        assert_eq!(split.compute_pct, 25);
        assert_eq!(split.platform_pct, 10);
        assert_eq!(split.referrer_pct, 5);
        assert!(split.is_valid());
    }

    #[test]
    fn test_revenue_split_validation() {
        let valid = RevenueSplit {
            creator_pct: 60,
            compute_pct: 25,
            platform_pct: 10,
            referrer_pct: 5,
        };
        assert!(valid.is_valid());

        let invalid = RevenueSplit {
            creator_pct: 60,
            compute_pct: 25,
            platform_pct: 10,
            referrer_pct: 10, // Sums to 105
        };
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_revenue_split_calculation() {
        let split = RevenueSplit::DEFAULT;

        // 100 credits total
        let (creator, compute, platform, referrer) = split.split(100);
        assert_eq!(creator, 60);
        assert_eq!(compute, 25);
        assert_eq!(platform, 10);
        assert_eq!(referrer, 5);

        // Verify total adds up
        assert_eq!(creator + compute + platform + referrer, 100);

        // 1000 credits total
        let (creator, compute, platform, referrer) = split.split(1000);
        assert_eq!(creator, 600);
        assert_eq!(compute, 250);
        assert_eq!(platform, 100);
        assert_eq!(referrer, 50);

        // Verify total adds up
        assert_eq!(creator + compute + platform + referrer, 1000);
    }

    #[test]
    fn test_revenue_split_rounding() {
        let split = RevenueSplit::DEFAULT;

        // With 99 credits, rounding might cause issues
        // 60% of 99 = 59.4 -> 59
        // 25% of 99 = 24.75 -> 24
        // 10% of 99 = 9.9 -> 9
        // Referrer gets remainder = 99 - 59 - 24 - 9 = 7
        let (creator, compute, platform, referrer) = split.split(99);
        assert_eq!(creator, 59);
        assert_eq!(compute, 24);
        assert_eq!(platform, 9);
        assert_eq!(referrer, 7); // Gets the rounding remainder

        // Verify total adds up exactly
        assert_eq!(creator + compute + platform + referrer, 99);
    }

    #[test]
    fn test_skill_pricing_serde() {
        let pricing = SkillPricing::Hybrid {
            per_call: 50,
            per_1k_input: 10,
            per_1k_output: 20,
        };

        let json = serde_json::to_string(&pricing).unwrap();
        let deserialized: SkillPricing = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, pricing);
    }

    #[test]
    fn test_revenue_split_serde() {
        let split = RevenueSplit::DEFAULT;
        let json = serde_json::to_string(&split).unwrap();
        let deserialized: RevenueSplit = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, split);
    }

    #[test]
    fn test_item_type_as_str() {
        assert_eq!(MarketplaceItemType::Plugin.as_str(), "plugin");
        assert_eq!(MarketplaceItemType::Skill.as_str(), "skill");
        assert_eq!(MarketplaceItemType::Agent.as_str(), "agent");
    }

    #[test]
    fn test_item_status_as_str() {
        assert_eq!(ItemStatus::Available.as_str(), "available");
        assert_eq!(ItemStatus::Installing.as_str(), "installing");
        assert_eq!(ItemStatus::Installed.as_str(), "installed");
        assert_eq!(ItemStatus::Failed.as_str(), "failed");
        assert_eq!(ItemStatus::UpdateAvailable.as_str(), "update_available");
    }

    #[test]
    fn test_item_status_is_installed() {
        assert!(!ItemStatus::Available.is_installed());
        assert!(!ItemStatus::Installing.is_installed());
        assert!(ItemStatus::Installed.is_installed());
        assert!(!ItemStatus::Failed.is_installed());
        assert!(ItemStatus::UpdateAvailable.is_installed());
    }

    #[test]
    fn test_item_status_is_transitional() {
        assert!(!ItemStatus::Available.is_transitional());
        assert!(ItemStatus::Installing.is_transitional());
        assert!(!ItemStatus::Installed.is_transitional());
        assert!(!ItemStatus::Failed.is_transitional());
        assert!(!ItemStatus::UpdateAvailable.is_transitional());
    }

    #[test]
    fn test_serde_roundtrip() {
        let item_type = MarketplaceItemType::Skill;
        let json = serde_json::to_string(&item_type).unwrap();
        assert_eq!(json, "\"skill\"");
        let deserialized: MarketplaceItemType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, item_type);

        let status = ItemStatus::Installed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"installed\"");
        let deserialized: ItemStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    #[test]
    fn test_submission_status_as_str() {
        assert_eq!(SkillSubmissionStatus::Draft.as_str(), "draft");
        assert_eq!(SkillSubmissionStatus::PendingReview.as_str(), "pending_review");
        assert_eq!(SkillSubmissionStatus::InReview.as_str(), "in_review");
        assert_eq!(SkillSubmissionStatus::ChangesRequested.as_str(), "changes_requested");
        assert_eq!(SkillSubmissionStatus::Approved.as_str(), "approved");
        assert_eq!(SkillSubmissionStatus::Published.as_str(), "published");
        assert_eq!(SkillSubmissionStatus::Deprecated.as_str(), "deprecated");
        assert_eq!(SkillSubmissionStatus::Rejected.as_str(), "rejected");
    }

    #[test]
    fn test_submission_status_is_terminal() {
        assert!(!SkillSubmissionStatus::Draft.is_terminal());
        assert!(!SkillSubmissionStatus::PendingReview.is_terminal());
        assert!(!SkillSubmissionStatus::InReview.is_terminal());
        assert!(!SkillSubmissionStatus::ChangesRequested.is_terminal());
        assert!(!SkillSubmissionStatus::Approved.is_terminal());
        assert!(SkillSubmissionStatus::Published.is_terminal());
        assert!(SkillSubmissionStatus::Deprecated.is_terminal());
        assert!(SkillSubmissionStatus::Rejected.is_terminal());
    }

    #[test]
    fn test_submission_status_is_editable() {
        assert!(SkillSubmissionStatus::Draft.is_editable());
        assert!(!SkillSubmissionStatus::PendingReview.is_editable());
        assert!(!SkillSubmissionStatus::InReview.is_editable());
        assert!(SkillSubmissionStatus::ChangesRequested.is_editable());
        assert!(!SkillSubmissionStatus::Approved.is_editable());
        assert!(!SkillSubmissionStatus::Published.is_editable());
        assert!(!SkillSubmissionStatus::Deprecated.is_editable());
        assert!(!SkillSubmissionStatus::Rejected.is_editable());
    }

    #[test]
    fn test_quality_check_type_as_str() {
        assert_eq!(QualityCheckType::SchemaValidation.as_str(), "schema_validation");
        assert_eq!(QualityCheckType::SecurityScan.as_str(), "security_scan");
        assert_eq!(QualityCheckType::SandboxTest.as_str(), "sandbox_test");
        assert_eq!(QualityCheckType::BenchmarkTest.as_str(), "benchmark_test");
    }

    #[test]
    fn test_quality_check_type_is_required() {
        assert!(QualityCheckType::SchemaValidation.is_required());
        assert!(QualityCheckType::SecurityScan.is_required());
        assert!(QualityCheckType::SandboxTest.is_required());
        assert!(!QualityCheckType::BenchmarkTest.is_required());
    }

    #[test]
    fn test_quality_check_constructors() {
        let pass = QualityCheck::pass(QualityCheckType::SchemaValidation);
        assert_eq!(pass.check_type, QualityCheckType::SchemaValidation);
        assert!(pass.passed);
        assert!(pass.details.is_none());

        let fail = QualityCheck::fail(QualityCheckType::SecurityScan, "Found malware");
        assert_eq!(fail.check_type, QualityCheckType::SecurityScan);
        assert!(!fail.passed);
        assert_eq!(fail.details, Some("Found malware".to_string()));
    }

    #[test]
    fn test_review_decision_is_approval() {
        assert!(ReviewDecision::Approve.is_approval());
        assert!(!ReviewDecision::RequestChanges {
            reason: "Fix typos".to_string()
        }
        .is_approval());
        assert!(!ReviewDecision::Reject {
            reason: "Spam".to_string()
        }
        .is_approval());
    }

    #[test]
    fn test_review_decision_is_rejection() {
        assert!(!ReviewDecision::Approve.is_rejection());
        assert!(!ReviewDecision::RequestChanges {
            reason: "Fix typos".to_string()
        }
        .is_rejection());
        assert!(ReviewDecision::Reject {
            reason: "Spam".to_string()
        }
        .is_rejection());
    }

    #[test]
    fn test_review_decision_reason() {
        assert_eq!(ReviewDecision::Approve.reason(), None);
        assert_eq!(
            ReviewDecision::RequestChanges {
                reason: "Fix typos".to_string()
            }
            .reason(),
            Some("Fix typos")
        );
        assert_eq!(
            ReviewDecision::Reject {
                reason: "Spam".to_string()
            }
            .reason(),
            Some("Spam")
        );
    }

    #[test]
    fn test_submission_review_all_required_checks_passed() {
        let review = SubmissionReview {
            submission_id: "sub1".to_string(),
            reviewer: "reviewer1".to_string(),
            decision: ReviewDecision::Approve,
            notes: "Looks good".to_string(),
            checks: vec![
                QualityCheck::pass(QualityCheckType::SchemaValidation),
                QualityCheck::pass(QualityCheckType::SecurityScan),
                QualityCheck::pass(QualityCheckType::SandboxTest),
                QualityCheck::fail(QualityCheckType::BenchmarkTest, "Slow"), // Optional check failure is OK
            ],
        };

        assert!(review.all_required_checks_passed());

        let review_with_failure = SubmissionReview {
            submission_id: "sub2".to_string(),
            reviewer: "reviewer1".to_string(),
            decision: ReviewDecision::RequestChanges {
                reason: "Fix security issues".to_string(),
            },
            notes: "Security problems found".to_string(),
            checks: vec![
                QualityCheck::pass(QualityCheckType::SchemaValidation),
                QualityCheck::fail(QualityCheckType::SecurityScan, "Malware detected"),
                QualityCheck::pass(QualityCheckType::SandboxTest),
            ],
        };

        assert!(!review_with_failure.all_required_checks_passed());
    }

    #[test]
    fn test_submission_review_failed_checks() {
        let review = SubmissionReview {
            submission_id: "sub1".to_string(),
            reviewer: "reviewer1".to_string(),
            decision: ReviewDecision::RequestChanges {
                reason: "Fix issues".to_string(),
            },
            notes: "Multiple issues".to_string(),
            checks: vec![
                QualityCheck::pass(QualityCheckType::SchemaValidation),
                QualityCheck::fail(QualityCheckType::SecurityScan, "Issue 1"),
                QualityCheck::fail(QualityCheckType::SandboxTest, "Issue 2"),
            ],
        };

        let failed = review.failed_checks();
        assert_eq!(failed.len(), 2);
        assert_eq!(failed[0].check_type, QualityCheckType::SecurityScan);
        assert_eq!(failed[1].check_type, QualityCheckType::SandboxTest);
    }

    #[test]
    fn test_skill_submission_serde() {
        let submission = SkillSubmission {
            id: "sub123".to_string(),
            skill_path: PathBuf::from("/path/to/skill"),
            creator: "npub1creator".to_string(),
            status: SkillSubmissionStatus::PendingReview,
            submitted_at: Utc::now(),
            reviewed_at: None,
            reviewer_notes: None,
        };

        let json = serde_json::to_string(&submission).unwrap();
        let deserialized: SkillSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, submission.id);
        assert_eq!(deserialized.status, submission.status);
    }

    #[test]
    fn test_review_decision_serde() {
        let approve = ReviewDecision::Approve;
        let json = serde_json::to_string(&approve).unwrap();
        let deserialized: ReviewDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, approve);

        let request_changes = ReviewDecision::RequestChanges {
            reason: "Fix formatting".to_string(),
        };
        let json = serde_json::to_string(&request_changes).unwrap();
        let deserialized: ReviewDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, request_changes);
    }

    #[test]
    fn test_installation_status_is_operational() {
        assert!(!InstallationStatus::Installing.is_operational());
        assert!(InstallationStatus::Installed.is_operational());
        assert!(InstallationStatus::UpdateAvailable {
            new_version: "2.0.0".to_string()
        }
        .is_operational());
        assert!(!InstallationStatus::Failed {
            error: "Error".to_string()
        }
        .is_operational());
        assert!(!InstallationStatus::Uninstalled.is_operational());
    }

    #[test]
    fn test_installation_status_is_installing() {
        assert!(InstallationStatus::Installing.is_installing());
        assert!(!InstallationStatus::Installed.is_installing());
        assert!(!InstallationStatus::Uninstalled.is_installing());
    }

    #[test]
    fn test_installation_status_is_failed() {
        assert!(!InstallationStatus::Installing.is_failed());
        assert!(!InstallationStatus::Installed.is_failed());
        assert!(InstallationStatus::Failed {
            error: "Error".to_string()
        }
        .is_failed());
    }

    #[test]
    fn test_installation_status_error() {
        assert_eq!(InstallationStatus::Installing.error(), None);
        assert_eq!(InstallationStatus::Installed.error(), None);
        assert_eq!(
            InstallationStatus::Failed {
                error: "Something broke".to_string()
            }
            .error(),
            Some("Something broke")
        );
    }

    #[test]
    fn test_mcp_dependency_builders() {
        let required = McpDependency::required("filesystem");
        assert_eq!(required.server_name, "filesystem");
        assert!(required.required);
        assert_eq!(required.version_constraint, None);

        let optional = McpDependency::optional("database");
        assert_eq!(optional.server_name, "database");
        assert!(!optional.required);

        let versioned = McpDependency::required("api").with_version("^1.0.0");
        assert_eq!(versioned.version_constraint, Some("^1.0.0".to_string()));
    }

    #[test]
    fn test_missing_dependency_from_dependency() {
        let dep = McpDependency::required("test-server").with_version(">=2.0.0");
        let missing = MissingDependency::from_dependency(&dep);

        assert_eq!(missing.server_name, "test-server");
        assert_eq!(missing.version_constraint, Some(">=2.0.0".to_string()));
        assert!(missing.required);
        assert_eq!(missing.install_hint, None);

        let with_hint = missing.with_hint("Run: npm install test-server");
        assert_eq!(
            with_hint.install_hint,
            Some("Run: npm install test-server".to_string())
        );
    }

    #[test]
    fn test_skill_update_builder() {
        let update = SkillUpdate::new("inst1", "skill1", "1.0.0", "2.0.0");
        assert_eq!(update.installation_id, "inst1");
        assert_eq!(update.skill_id, "skill1");
        assert_eq!(update.current_version, "1.0.0");
        assert_eq!(update.new_version, "2.0.0");
        assert!(!update.breaking);
        assert_eq!(update.description, None);

        let with_desc = update
            .clone()
            .with_description("Added new features");
        assert_eq!(
            with_desc.description,
            Some("Added new features".to_string())
        );

        let breaking = update.as_breaking();
        assert!(breaking.breaking);
    }

    #[test]
    fn test_skill_installation_serde() {
        let installation = SkillInstallation {
            id: "install123".to_string(),
            skill_id: "skill456".to_string(),
            user_id: "user789".to_string(),
            installed_at: Utc::now(),
            version: "1.0.0".to_string(),
            path: PathBuf::from("/home/user/.skills/skill456"),
            status: InstallationStatus::Installed,
        };

        let json = serde_json::to_string(&installation).unwrap();
        let deserialized: SkillInstallation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, installation.id);
        assert_eq!(deserialized.status, installation.status);
    }

    #[test]
    fn test_installation_status_serde() {
        let installed = InstallationStatus::Installed;
        let json = serde_json::to_string(&installed).unwrap();
        let deserialized: InstallationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, installed);

        let failed = InstallationStatus::Failed {
            error: "Network error".to_string(),
        };
        let json = serde_json::to_string(&failed).unwrap();
        let deserialized: InstallationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, failed);

        let update_available = InstallationStatus::UpdateAvailable {
            new_version: "3.0.0".to_string(),
        };
        let json = serde_json::to_string(&update_available).unwrap();
        let deserialized: InstallationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, update_available);
    }

    #[test]
    fn test_mcp_dependency_serde() {
        let dep = McpDependency::required("server").with_version("^1.0.0");
        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: McpDependency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.server_name, dep.server_name);
        assert_eq!(deserialized.version_constraint, dep.version_constraint);
        assert_eq!(deserialized.required, dep.required);
    }

    #[test]
    fn test_skill_update_serde() {
        let update = SkillUpdate::new("inst1", "skill1", "1.0.0", "2.0.0")
            .as_breaking()
            .with_description("Major update");

        let json = serde_json::to_string(&update).unwrap();
        let deserialized: SkillUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.installation_id, update.installation_id);
        assert_eq!(deserialized.breaking, update.breaking);
        assert_eq!(deserialized.description, update.description);
    }
}
