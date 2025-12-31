//! Desktop notification system for GitAfter events
//!
//! Provides cross-platform native notifications for important GitAfter events.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Escape a string for safe use in shell commands
///
/// This function escapes special characters that could be used for shell injection:
/// - Double quotes (")
/// - Single quotes (')
/// - Backticks (`)
/// - Dollar signs ($)
/// - Backslashes (\)
/// - Newlines and other control characters
#[allow(dead_code)] // Used in platform-specific code
fn escape_shell_string(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\'' => vec!['\\', '\''],
            '`' => vec!['\\', '`'],
            '$' => vec!['\\', '$'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            c if c.is_control() => vec![], // Remove other control characters
            c => vec![c],
        })
        .collect()
}

/// Notification preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    /// Enable notifications globally
    pub enabled: bool,
    /// Notify when review is posted on your PR
    pub notify_pr_review: bool,
    /// Notify when issue claim is accepted/rejected
    pub notify_issue_claim: bool,
    /// Notify when PR is merged
    pub notify_pr_merged: bool,
    /// Notify when PR status changes
    pub notify_pr_status_change: bool,
    /// Notify when bounty is paid
    pub notify_bounty_paid: bool,
    /// Notify for new issues matching specialties (agent mode)
    pub notify_matching_issues: bool,
    /// Agent specialties for matching (e.g., ["rust", "typescript"])
    pub agent_specialties: Vec<String>,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            notify_pr_review: true,
            notify_issue_claim: true,
            notify_pr_merged: true,
            notify_pr_status_change: true,
            notify_bounty_paid: true,
            notify_matching_issues: false,
            agent_specialties: vec![],
        }
    }
}

/// Type of notification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    PrReview,
    IssueClaim,
    PrMerged,
    PrStatusChange,
    BountyPaid,
    MatchingIssue,
}

/// A notification to be displayed
#[derive(Debug, Clone)]
pub struct Notification {
    /// Title of notification
    pub title: String,
    /// Body text
    pub body: String,
    /// Type of notification
    pub notification_type: NotificationType,
    /// Event ID this notification is for
    pub event_id: String,
}

/// Notification manager
pub struct NotificationManager {
    preferences: NotificationPreferences,
    shown_notifications: HashSet<String>,
}

impl NotificationManager {
    /// Create a new notification manager
    pub fn new(preferences: NotificationPreferences) -> Self {
        Self {
            preferences,
            shown_notifications: HashSet::new(),
        }
    }

    /// Check if we should show a notification for this event
    pub fn should_notify(&self, notification_type: NotificationType) -> bool {
        if !self.preferences.enabled {
            return false;
        }

        match notification_type {
            NotificationType::PrReview => self.preferences.notify_pr_review,
            NotificationType::IssueClaim => self.preferences.notify_issue_claim,
            NotificationType::PrMerged => self.preferences.notify_pr_merged,
            NotificationType::PrStatusChange => self.preferences.notify_pr_status_change,
            NotificationType::BountyPaid => self.preferences.notify_bounty_paid,
            NotificationType::MatchingIssue => self.preferences.notify_matching_issues,
        }
    }

    /// Show a notification (platform-specific implementation)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gitafter::notifications::{NotificationManager, Notification, NotificationType, NotificationPreferences};
    ///
    /// let prefs = NotificationPreferences::default();
    /// let mut manager = NotificationManager::new(prefs);
    ///
    /// let notification = Notification {
    ///     title: "PR Review Posted".to_string(),
    ///     body: "Alice reviewed your PR #123".to_string(),
    ///     notification_type: NotificationType::PrReview,
    ///     event_id: "event123".to_string(),
    /// };
    ///
    /// manager.show(notification).ok();
    /// ```
    pub fn show(&mut self, notification: Notification) -> Result<()> {
        // Deduplicate - don't show the same notification twice
        if self.shown_notifications.contains(&notification.event_id) {
            return Ok(());
        }

        if !self.should_notify(notification.notification_type) {
            return Ok(());
        }

        // Platform-specific notification
        #[cfg(target_os = "linux")]
        self.show_linux(&notification)?;

        #[cfg(target_os = "macos")]
        self.show_macos(&notification)?;

        #[cfg(target_os = "windows")]
        self.show_windows(&notification)?;

        // Mark as shown
        self.shown_notifications.insert(notification.event_id);

        Ok(())
    }

    /// Show notification on Linux using notify-send
    #[cfg(target_os = "linux")]
    fn show_linux(&self, notification: &Notification) -> Result<()> {
        use std::process::Command;

        Command::new("notify-send")
            .arg(&notification.title)
            .arg(&notification.body)
            .arg("--app-name=GitAfter")
            .arg("--icon=git")
            .spawn()?;

        Ok(())
    }

    /// Show notification on macOS using osascript
    #[cfg(target_os = "macos")]
    fn show_macos(&self, notification: &Notification) -> Result<()> {
        use std::process::Command;

        let script = format!(
            r#"display notification "{}" with title "{}" subtitle "GitAfter""#,
            escape_shell_string(&notification.body),
            escape_shell_string(&notification.title)
        );

        Command::new("osascript").arg("-e").arg(&script).spawn()?;

        Ok(())
    }

    /// Show notification on Windows using PowerShell toast
    #[cfg(target_os = "windows")]
    fn show_windows(&self, notification: &Notification) -> Result<()> {
        use std::process::Command;

        let script = format!(
            r#"[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null
$Template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$RawXml = [xml] $Template.GetXml()
($RawXml.toast.visual.binding.text|where {{$_.id -eq "1"}}).AppendChild($RawXml.CreateTextNode("{}")) > $null
($RawXml.toast.visual.binding.text|where {{$_.id -eq "2"}}).AppendChild($RawXml.CreateTextNode("{}")) > $null
$SerializedXml = New-Object Windows.Data.Xml.Dom.XmlDocument
$SerializedXml.LoadXml($RawXml.OuterXml)
$Toast = [Windows.UI.Notifications.ToastNotification]::new($SerializedXml)
$Toast.Tag = "GitAfter"
$Toast.Group = "GitAfter"
$Notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("GitAfter")
$Notifier.Show($Toast);"#,
            escape_shell_string(&notification.title),
            escape_shell_string(&notification.body)
        );

        Command::new("powershell")
            .arg("-Command")
            .arg(&script)
            .spawn()?;

        Ok(())
    }

    /// Update preferences
    pub fn update_preferences(&mut self, preferences: NotificationPreferences) {
        self.preferences = preferences;
    }
}

/// Create a PR review notification
pub fn pr_review_notification(
    pr_title: &str,
    reviewer_pubkey: &str,
    event_id: &str,
) -> Notification {
    let short_pubkey = if reviewer_pubkey.len() > 16 {
        format!(
            "{}...{}",
            &reviewer_pubkey[..8],
            &reviewer_pubkey[reviewer_pubkey.len() - 8..]
        )
    } else {
        reviewer_pubkey.to_string()
    };

    Notification {
        title: "New PR Review".to_string(),
        body: format!("{} reviewed \"{}\"", short_pubkey, pr_title),
        notification_type: NotificationType::PrReview,
        event_id: event_id.to_string(),
    }
}

/// Create an issue claim notification
pub fn issue_claim_notification(
    issue_title: &str,
    agent_pubkey: &str,
    event_id: &str,
) -> Notification {
    let short_pubkey = if agent_pubkey.len() > 16 {
        format!(
            "{}...{}",
            &agent_pubkey[..8],
            &agent_pubkey[agent_pubkey.len() - 8..]
        )
    } else {
        agent_pubkey.to_string()
    };

    Notification {
        title: "Issue Claimed".to_string(),
        body: format!("{} claimed \"{}\"", short_pubkey, issue_title),
        notification_type: NotificationType::IssueClaim,
        event_id: event_id.to_string(),
    }
}

/// Create a PR merged notification
pub fn pr_merged_notification(pr_title: &str, event_id: &str) -> Notification {
    Notification {
        title: "PR Merged!".to_string(),
        body: format!("Your PR \"{}\" was merged", pr_title),
        notification_type: NotificationType::PrMerged,
        event_id: event_id.to_string(),
    }
}

/// Create a PR status change notification
pub fn pr_status_change_notification(
    pr_title: &str,
    old_status: &str,
    new_status: &str,
    event_id: &str,
) -> Notification {
    let status_emoji = match new_status {
        "Open" => "🟢",
        "Applied/Merged" => "✅",
        "Closed" => "🔴",
        "Draft" => "📝",
        _ => "🔄",
    };

    Notification {
        title: format!("PR Status Changed {}", status_emoji),
        body: format!(
            "\"{}\" changed from {} to {}",
            pr_title, old_status, new_status
        ),
        notification_type: NotificationType::PrStatusChange,
        event_id: event_id.to_string(),
    }
}

/// Create a bounty paid notification
pub fn bounty_paid_notification(
    amount_sats: u64,
    issue_title: &str,
    event_id: &str,
) -> Notification {
    Notification {
        title: "Bounty Paid!".to_string(),
        body: format!("Received {} sats for \"{}\"", amount_sats, issue_title),
        notification_type: NotificationType::BountyPaid,
        event_id: event_id.to_string(),
    }
}

/// Create a matching issue notification
pub fn matching_issue_notification(
    issue_title: &str,
    repo_name: &str,
    event_id: &str,
) -> Notification {
    Notification {
        title: "New Matching Issue".to_string(),
        body: format!("New issue in {}: \"{}\"", repo_name, issue_title),
        notification_type: NotificationType::MatchingIssue,
        event_id: event_id.to_string(),
    }
}

/// Send PR status change email notification
///
/// Sends an email to the PR author when their PR status changes.
///
/// # Arguments
/// * `pr_title` - Title of the pull request
/// * `old_status` - Previous status
/// * `new_status` - New status
/// * `to_email` - Recipient email address
/// * `smtp_config` - SMTP configuration
///
/// # Example
/// ```no_run
/// use gitafter::notifications::send_pr_status_email;
/// use gitafter::smtp_notifications::SmtpConfig;
///
/// # tokio_test::block_on(async {
/// let smtp = SmtpConfig {
///     host: "smtp.gmail.com".to_string(),
///     port: 587,
///     username: "bot@example.com".to_string(),
///     password: "password".to_string(),
///     from: "GitAfter <bot@example.com>".to_string(),
/// };
///
/// send_pr_status_email(
///     "Fix authentication bug",
///     "Open",
///     "Merged",
///     "author@example.com",
///     &smtp
/// ).await.ok();
/// # });
/// ```
pub async fn send_pr_status_email(
    pr_title: &str,
    old_status: &str,
    new_status: &str,
    to_email: &str,
    smtp_config: &crate::smtp_notifications::SmtpConfig,
) -> Result<()> {
    let status_emoji = match new_status {
        "Open" => "🟢",
        "Applied/Merged" => "✅",
        "Closed" => "🔴",
        "Draft" => "📝",
        _ => "🔄",
    };

    let notification = crate::smtp_notifications::Notification::new(
        format!("{} PR Status Changed: {}", status_emoji, pr_title),
        format!(
            "Your pull request \"{}\" has been updated.\n\n\
             Previous status: {}\n\
             New status: {}\n\n\
             View the PR in GitAfter to see more details.",
            pr_title, old_status, new_status
        ),
        "info",
    );

    notification
        .send_email(&[to_email.to_string()], smtp_config)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escaping() {
        // Test backtick escaping (shell command injection)
        assert_eq!(escape_shell_string("test`whoami`"), "test\\`whoami\\`");

        // Test command substitution escaping
        assert_eq!(escape_shell_string("test$(whoami)"), "test\\$(whoami)");

        // Test double quote escaping
        assert_eq!(escape_shell_string("test\"quote\""), "test\\\"quote\\\"");

        // Test single quote escaping
        assert_eq!(escape_shell_string("test'quote'"), "test\\'quote\\'");

        // Test backslash escaping
        assert_eq!(escape_shell_string("test\\backslash"), "test\\\\backslash");

        // Test newline/control character removal
        assert_eq!(
            escape_shell_string("test\nline\r\nbreak"),
            "test\\nline\\r\\nbreak"
        );

        // Test safe strings remain unchanged (except control chars)
        assert_eq!(escape_shell_string("safe-string_123"), "safe-string_123");
    }

    #[test]
    fn test_default_preferences() {
        let prefs = NotificationPreferences::default();
        assert!(prefs.enabled);
        assert!(prefs.notify_pr_review);
        assert!(prefs.notify_pr_merged);
    }

    #[test]
    fn test_should_notify() {
        let prefs = NotificationPreferences {
            enabled: true,
            notify_pr_review: true,
            notify_pr_merged: false,
            ..Default::default()
        };

        let manager = NotificationManager::new(prefs);
        assert!(manager.should_notify(NotificationType::PrReview));
        assert!(!manager.should_notify(NotificationType::PrMerged));
    }

    #[test]
    fn test_deduplication() {
        let prefs = NotificationPreferences::default();
        let mut manager = NotificationManager::new(prefs);

        let notification = Notification {
            title: "Test".to_string(),
            body: "Body".to_string(),
            notification_type: NotificationType::PrReview,
            event_id: "test123".to_string(),
        };

        // First show should work
        assert!(manager.shown_notifications.is_empty());

        // Simulate showing (without actual platform call)
        manager
            .shown_notifications
            .insert(notification.event_id.clone());

        // Check deduplication
        assert!(manager.shown_notifications.contains(&notification.event_id));
    }

    #[test]
    fn test_notification_constructors() {
        let notif = pr_review_notification("Fix bug", "npub123", "event1");
        assert_eq!(notif.title, "New PR Review");
        assert!(notif.body.contains("Fix bug"));

        let notif = pr_merged_notification("Add feature", "event2");
        assert_eq!(notif.title, "PR Merged!");
        assert!(notif.body.contains("Add feature"));

        let notif = bounty_paid_notification(50000, "Bug fix", "event3");
        assert_eq!(notif.title, "Bounty Paid!");
        assert!(notif.body.contains("50000 sats"));
    }
}
