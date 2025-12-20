//! Bounty system types for requesting specific data patterns

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::data_contribution::DataContributionType;

/// Bounty status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BountyStatus {
    /// Bounty is open for submissions
    Open,
    /// Bounty has submissions being reviewed
    InProgress,
    /// Bounty has been fulfilled
    Fulfilled,
    /// Bounty deadline passed without fulfillment
    Expired,
    /// Bounty was cancelled by creator
    Cancelled,
}

impl BountyStatus {
    /// Check if bounty can accept new submissions
    pub fn can_accept_submissions(&self) -> bool {
        matches!(self, BountyStatus::Open | BountyStatus::InProgress)
    }

    /// Check if bounty is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            BountyStatus::Fulfilled | BountyStatus::Expired | BountyStatus::Cancelled
        )
    }

    /// Get status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            BountyStatus::Open => "open",
            BountyStatus::InProgress => "in_progress",
            BountyStatus::Fulfilled => "fulfilled",
            BountyStatus::Expired => "expired",
            BountyStatus::Cancelled => "cancelled",
        }
    }
}

/// Bounty requirements specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BountyRequirements {
    /// Type of data contribution required
    pub data_type: DataContributionType,
    /// Minimum quality score (0.0-1.0)
    pub min_quality_score: f32,
    /// Optional language requirements
    pub languages: Option<Vec<String>>,
    /// Optional tool requirements
    pub tools: Option<Vec<String>>,
    /// Minimum duration in seconds (for sessions/trajectories)
    pub min_duration_secs: Option<u64>,
    /// Custom criteria description
    pub custom_criteria: Option<String>,
}

impl BountyRequirements {
    /// Create new requirements with data type
    pub fn new(data_type: DataContributionType) -> Self {
        Self {
            data_type,
            min_quality_score: 0.0,
            languages: None,
            tools: None,
            min_duration_secs: None,
            custom_criteria: None,
        }
    }

    /// Set minimum quality score
    pub fn with_min_quality(mut self, score: f32) -> Self {
        self.min_quality_score = score.clamp(0.0, 1.0);
        self
    }

    /// Set language requirements
    pub fn with_languages(mut self, languages: Vec<String>) -> Self {
        self.languages = Some(languages);
        self
    }

    /// Set tool requirements
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set minimum duration
    pub fn with_min_duration(mut self, secs: u64) -> Self {
        self.min_duration_secs = Some(secs);
        self
    }

    /// Set custom criteria
    pub fn with_custom_criteria(mut self, criteria: impl Into<String>) -> Self {
        self.custom_criteria = Some(criteria.into());
        self
    }
}

/// Submission status
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SubmissionStatus {
    /// Submission is pending review
    Pending,
    /// Submission was accepted
    Accepted,
    /// Submission was rejected with reason
    Rejected {
        /// Rejection reason
        reason: String,
    },
    /// Payment has been made
    Paid,
}

impl SubmissionStatus {
    /// Check if submission is pending
    pub fn is_pending(&self) -> bool {
        matches!(self, SubmissionStatus::Pending)
    }

    /// Check if submission was accepted
    pub fn is_accepted(&self) -> bool {
        matches!(self, SubmissionStatus::Accepted | SubmissionStatus::Paid)
    }

    /// Check if submission was rejected
    pub fn is_rejected(&self) -> bool {
        matches!(self, SubmissionStatus::Rejected { .. })
    }

    /// Get rejection reason if available
    pub fn rejection_reason(&self) -> Option<&str> {
        match self {
            SubmissionStatus::Rejected { reason } => Some(reason),
            _ => None,
        }
    }

    /// Get status as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            SubmissionStatus::Pending => "pending",
            SubmissionStatus::Accepted => "accepted",
            SubmissionStatus::Rejected { .. } => "rejected",
            SubmissionStatus::Paid => "paid",
        }
    }
}

/// Bounty submission
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BountySubmission {
    /// Unique submission ID
    pub id: String,
    /// Bounty ID this submission is for
    pub bounty_id: String,
    /// Contributor's Nostr public key
    pub contributor: String,
    /// Data contribution ID
    pub contribution_id: String,
    /// When submitted
    pub submitted_at: DateTime<Utc>,
    /// Current status
    pub status: SubmissionStatus,
    /// Reward earned in satoshis (if accepted)
    pub reward_earned: Option<u64>,
}

impl BountySubmission {
    /// Create a new submission
    pub fn new(
        id: impl Into<String>,
        bounty_id: impl Into<String>,
        contributor: impl Into<String>,
        contribution_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            bounty_id: bounty_id.into(),
            contributor: contributor.into(),
            contribution_id: contribution_id.into(),
            submitted_at: Utc::now(),
            status: SubmissionStatus::Pending,
            reward_earned: None,
        }
    }

    /// Accept the submission with reward
    pub fn accept(&mut self, reward_sats: u64) {
        self.status = SubmissionStatus::Accepted;
        self.reward_earned = Some(reward_sats);
    }

    /// Reject the submission
    pub fn reject(&mut self, reason: impl Into<String>) {
        self.status = SubmissionStatus::Rejected {
            reason: reason.into(),
        };
    }

    /// Mark as paid
    pub fn mark_paid(&mut self) -> Result<(), String> {
        if !self.status.is_accepted() {
            return Err("Cannot mark as paid: submission not accepted".to_string());
        }
        self.status = SubmissionStatus::Paid;
        Ok(())
    }
}

/// Data bounty
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataBounty {
    /// Unique bounty ID
    pub id: String,
    /// Creator's Nostr public key
    pub creator: String,
    /// Bounty title
    pub title: String,
    /// Bounty description
    pub description: String,
    /// Requirements for submissions
    pub requirements: BountyRequirements,
    /// Reward in satoshis
    pub reward_sats: u64,
    /// Current status
    pub status: BountyStatus,
    /// Optional deadline
    pub deadline: Option<DateTime<Utc>>,
    /// Submissions received
    pub submissions: Vec<BountySubmission>,
}

impl DataBounty {
    /// Create a new bounty
    pub fn new(
        id: impl Into<String>,
        creator: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        requirements: BountyRequirements,
        reward_sats: u64,
    ) -> Self {
        Self {
            id: id.into(),
            creator: creator.into(),
            title: title.into(),
            description: description.into(),
            requirements,
            reward_sats,
            status: BountyStatus::Open,
            deadline: None,
            submissions: Vec::new(),
        }
    }

    /// Set deadline
    pub fn with_deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Add a submission
    pub fn add_submission(&mut self, submission: BountySubmission) -> Result<(), String> {
        if !self.status.can_accept_submissions() {
            return Err(format!(
                "Bounty is {} and cannot accept new submissions",
                self.status.as_str()
            ));
        }

        // Check if contributor already submitted
        if self
            .submissions
            .iter()
            .any(|s| s.contributor == submission.contributor)
        {
            return Err("Contributor has already submitted to this bounty".to_string());
        }

        self.submissions.push(submission);

        // Update status to InProgress if this is the first submission
        if self.status == BountyStatus::Open && self.submissions.len() == 1 {
            self.status = BountyStatus::InProgress;
        }

        Ok(())
    }

    /// Accept a submission and fulfill the bounty
    pub fn accept_submission(&mut self, submission_id: &str) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err("Bounty is in a terminal state".to_string());
        }

        let submission = self
            .submissions
            .iter_mut()
            .find(|s| s.id == submission_id)
            .ok_or("Submission not found")?;

        submission.accept(self.reward_sats);
        self.status = BountyStatus::Fulfilled;

        Ok(())
    }

    /// Reject a submission
    pub fn reject_submission(
        &mut self,
        submission_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), String> {
        let submission = self
            .submissions
            .iter_mut()
            .find(|s| s.id == submission_id)
            .ok_or("Submission not found")?;

        submission.reject(reason);
        Ok(())
    }

    /// Cancel the bounty
    pub fn cancel(&mut self) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err("Bounty is already in a terminal state".to_string());
        }

        self.status = BountyStatus::Cancelled;
        Ok(())
    }

    /// Mark bounty as expired
    pub fn mark_expired(&mut self) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err("Bounty is already in a terminal state".to_string());
        }

        self.status = BountyStatus::Expired;
        Ok(())
    }

    /// Get accepted submission if any
    pub fn accepted_submission(&self) -> Option<&BountySubmission> {
        self.submissions.iter().find(|s| s.status.is_accepted())
    }

    /// Get pending submissions
    pub fn pending_submissions(&self) -> Vec<&BountySubmission> {
        self.submissions
            .iter()
            .filter(|s| s.status.is_pending())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounty_status_can_accept_submissions() {
        assert!(BountyStatus::Open.can_accept_submissions());
        assert!(BountyStatus::InProgress.can_accept_submissions());
        assert!(!BountyStatus::Fulfilled.can_accept_submissions());
        assert!(!BountyStatus::Expired.can_accept_submissions());
        assert!(!BountyStatus::Cancelled.can_accept_submissions());
    }

    #[test]
    fn test_bounty_status_is_terminal() {
        assert!(!BountyStatus::Open.is_terminal());
        assert!(!BountyStatus::InProgress.is_terminal());
        assert!(BountyStatus::Fulfilled.is_terminal());
        assert!(BountyStatus::Expired.is_terminal());
        assert!(BountyStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_bounty_requirements_builder() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace)
            .with_min_quality(0.8)
            .with_languages(vec!["rust".to_string(), "python".to_string()])
            .with_tools(vec!["vscode".to_string()])
            .with_min_duration(300)
            .with_custom_criteria("Must include error handling");

        assert_eq!(requirements.min_quality_score, 0.8);
        assert_eq!(requirements.languages.as_ref().unwrap().len(), 2);
        assert_eq!(requirements.tools.as_ref().unwrap().len(), 1);
        assert_eq!(requirements.min_duration_secs, Some(300));
        assert!(requirements.custom_criteria.is_some());
    }

    #[test]
    fn test_submission_status_methods() {
        assert!(SubmissionStatus::Pending.is_pending());
        assert!(!SubmissionStatus::Accepted.is_pending());

        assert!(SubmissionStatus::Accepted.is_accepted());
        assert!(SubmissionStatus::Paid.is_accepted());
        assert!(!SubmissionStatus::Pending.is_accepted());

        let rejected = SubmissionStatus::Rejected {
            reason: "Low quality".to_string(),
        };
        assert!(rejected.is_rejected());
        assert_eq!(rejected.rejection_reason(), Some("Low quality"));
    }

    #[test]
    fn test_bounty_submission_new() {
        let submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");
        assert_eq!(submission.id, "sub1");
        assert_eq!(submission.bounty_id, "bounty1");
        assert_eq!(submission.contributor, "contributor1");
        assert_eq!(submission.contribution_id, "contrib1");
        assert!(submission.status.is_pending());
        assert!(submission.reward_earned.is_none());
    }

    #[test]
    fn test_bounty_submission_accept() {
        let mut submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");
        submission.accept(10_000);

        assert!(submission.status.is_accepted());
        assert_eq!(submission.reward_earned, Some(10_000));
    }

    #[test]
    fn test_bounty_submission_reject() {
        let mut submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");
        submission.reject("Does not meet requirements");

        assert!(submission.status.is_rejected());
        assert_eq!(
            submission.status.rejection_reason(),
            Some("Does not meet requirements")
        );
    }

    #[test]
    fn test_bounty_submission_mark_paid() {
        let mut submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");

        // Cannot mark as paid if not accepted
        assert!(submission.mark_paid().is_err());

        submission.accept(10_000);
        assert!(submission.mark_paid().is_ok());
        assert_eq!(submission.status, SubmissionStatus::Paid);
    }

    #[test]
    fn test_data_bounty_new() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Need session data",
            "Looking for high-quality sessions",
            requirements,
            50_000,
        );

        assert_eq!(bounty.id, "bounty1");
        assert_eq!(bounty.creator, "creator1");
        assert_eq!(bounty.reward_sats, 50_000);
        assert_eq!(bounty.status, BountyStatus::Open);
        assert!(bounty.submissions.is_empty());
    }

    #[test]
    fn test_data_bounty_with_deadline() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let deadline = Utc::now();
        let bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        )
        .with_deadline(deadline);

        assert_eq!(bounty.deadline, Some(deadline));
    }

    #[test]
    fn test_data_bounty_add_submission() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let mut bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        let submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");
        assert!(bounty.add_submission(submission).is_ok());
        assert_eq!(bounty.submissions.len(), 1);
        assert_eq!(bounty.status, BountyStatus::InProgress);

        // Cannot add duplicate from same contributor
        let submission2 = BountySubmission::new("sub2", "bounty1", "contributor1", "contrib2");
        assert!(bounty.add_submission(submission2).is_err());
    }

    #[test]
    fn test_data_bounty_accept_submission() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let mut bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        let submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");
        bounty.add_submission(submission).unwrap();

        assert!(bounty.accept_submission("sub1").is_ok());
        assert_eq!(bounty.status, BountyStatus::Fulfilled);

        let accepted = bounty.accepted_submission().unwrap();
        assert_eq!(accepted.reward_earned, Some(50_000));
    }

    #[test]
    fn test_data_bounty_reject_submission() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let mut bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        let submission = BountySubmission::new("sub1", "bounty1", "contributor1", "contrib1");
        bounty.add_submission(submission).unwrap();

        assert!(bounty.reject_submission("sub1", "Low quality").is_ok());

        let rejected = &bounty.submissions[0];
        assert!(rejected.status.is_rejected());
        assert_eq!(rejected.status.rejection_reason(), Some("Low quality"));
    }

    #[test]
    fn test_data_bounty_cancel() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let mut bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        assert!(bounty.cancel().is_ok());
        assert_eq!(bounty.status, BountyStatus::Cancelled);

        // Cannot cancel again
        assert!(bounty.cancel().is_err());
    }

    #[test]
    fn test_data_bounty_mark_expired() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let mut bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        assert!(bounty.mark_expired().is_ok());
        assert_eq!(bounty.status, BountyStatus::Expired);
    }

    #[test]
    fn test_data_bounty_pending_submissions() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let mut bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        bounty
            .add_submission(BountySubmission::new(
                "sub1",
                "bounty1",
                "contributor1",
                "contrib1",
            ))
            .unwrap();
        bounty
            .add_submission(BountySubmission::new(
                "sub2",
                "bounty1",
                "contributor2",
                "contrib2",
            ))
            .unwrap();

        // Reject one
        bounty.reject_submission("sub1", "Low quality").unwrap();

        let pending = bounty.pending_submissions();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "sub2");
    }

    #[test]
    fn test_bounty_serde() {
        let requirements = BountyRequirements::new(DataContributionType::SessionTrace);
        let bounty = DataBounty::new(
            "bounty1",
            "creator1",
            "Test",
            "Description",
            requirements,
            50_000,
        );

        let json = serde_json::to_string(&bounty).unwrap();
        let deserialized: DataBounty = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, bounty.id);
        assert_eq!(deserialized.reward_sats, bounty.reward_sats);
    }
}
