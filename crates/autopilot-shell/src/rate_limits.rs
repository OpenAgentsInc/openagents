//! Rate limit fetching from Claude API
//!
//! Fetches live rate limit data from Claude API response headers.
//! Works with OAuth authentication using the oauth-2025-04-20 beta.
//!
//! Headers parsed:
//! - anthropic-ratelimit-unified-{claim}-utilization (0-1 usage)
//! - anthropic-ratelimit-unified-{claim}-reset (unix timestamp)
//! - anthropic-ratelimit-unified-status (allowed/allowed_warning/rejected)
//! Claims: 7d (weekly), 5h (session), 7ds (sonnet), 7do (opus)

use chrono::{DateTime, Utc};
use reqwest::header::HeaderMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// API endpoint for rate limit fetching (only works with API key, not OAuth)
const API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Rate limit type from API
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitType {
    SevenDay,      // 7d - weekly limit
    FiveHour,      // 5h - session limit
    SevenDaySonnet, // 7ds - Sonnet specific
    SevenDayOpus,   // 7do - Opus specific
    Overage,       // extra usage
    Unknown,
}

impl RateLimitType {
    #[allow(dead_code)]
    fn from_claim(claim: &str) -> Self {
        match claim {
            "7d" => Self::SevenDay,
            "5h" => Self::FiveHour,
            "7ds" => Self::SevenDaySonnet,
            "7do" => Self::SevenDayOpus,
            "overage" => Self::Overage,
            _ => Self::Unknown,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::SevenDay => "weekly limit",
            Self::FiveHour => "session limit",
            Self::SevenDaySonnet => "Sonnet limit",
            Self::SevenDayOpus => "Opus limit",
            Self::Overage => "extra usage",
            Self::Unknown => "usage limit",
        }
    }
}

/// Rate limit window data from API headers
#[derive(Debug, Clone)]
pub struct RateLimitWindow {
    /// Percentage used (0-100)
    pub used_percent: f64,
    /// Window duration in minutes
    pub window_minutes: Option<i64>,
    /// Unix timestamp when limit resets
    pub resets_at: Option<i64>,
    /// Type of rate limit
    pub limit_type: RateLimitType,
}

impl RateLimitWindow {
    /// Format reset time as human-readable string
    pub fn format_reset(&self) -> String {
        if let Some(ts) = self.resets_at {
            if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
                let now = Utc::now();
                let duration = dt.signed_duration_since(now);

                if duration.num_hours() > 24 {
                    return format!("in {} days", duration.num_days());
                } else if duration.num_hours() > 0 {
                    return format!("in {}h", duration.num_hours());
                } else if duration.num_minutes() > 0 {
                    return format!("in {}m", duration.num_minutes());
                } else {
                    return "soon".to_string();
                }
            }
        }

        // Fallback based on window minutes
        if let Some(mins) = self.window_minutes {
            if mins >= 10080 {
                return "weekly".to_string();
            } else if mins >= 1440 {
                return "daily".to_string();
            } else if mins >= 60 {
                return format!("{}h", mins / 60);
            }
        }

        "unknown".to_string()
    }

    /// Get window name based on duration
    pub fn window_name(&self) -> &'static str {
        if let Some(mins) = self.window_minutes {
            if mins >= 10080 {
                return "Weekly limit";
            } else if mins >= 1440 {
                return "Daily limit";
            }
        }
        "Usage limit"
    }
}

/// Credits info from API headers
#[derive(Debug, Clone, Default)]
pub struct CreditsSnapshot {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

/// Complete rate limit snapshot from API
#[derive(Debug, Clone, Default)]
pub struct RateLimitSnapshot {
    /// Primary rate limit (usually weekly)
    pub primary: Option<RateLimitWindow>,
    /// Secondary rate limit (usually session/daily)
    pub secondary: Option<RateLimitWindow>,
    /// Credits info
    pub credits: Option<CreditsSnapshot>,
}

/// Parse rate limits from HTTP response headers
/// Supports multiple header formats:
/// - x-codex-* headers (Claude Code legacy)
/// - anthropic-ratelimit-unified-* headers (Claude Code current)
/// - x-ratelimit-* headers (public Anthropic API)
pub fn parse_rate_limits(headers: &HeaderMap) -> RateLimitSnapshot {
    // Try anthropic-ratelimit-unified headers first (current Claude Code format)
    let unified = parse_unified_rate_limit(headers);

    // Then try x-codex headers (Claude Code legacy infrastructure)
    let primary = parse_rate_limit_window(
        headers,
        "x-codex-primary-used-percent",
        "x-codex-primary-window-minutes",
        "x-codex-primary-reset-at",
    );

    let secondary = parse_rate_limit_window(
        headers,
        "x-codex-secondary-used-percent",
        "x-codex-secondary-window-minutes",
        "x-codex-secondary-reset-at",
    );

    // Also try standard x-ratelimit headers (public Anthropic API)
    let standard_limit = parse_standard_rate_limit(headers);

    // Merge: prefer unified, then codex, then standard
    let final_primary = unified.or(primary).or(standard_limit);

    let credits = parse_credits_snapshot(headers);

    RateLimitSnapshot {
        primary: final_primary,
        secondary,
        credits,
    }
}

/// Claim abbreviations to check for rate limits (in order of priority)
const RATE_LIMIT_CLAIMS: &[(&str, RateLimitType, i64)] = &[
    ("7d", RateLimitType::SevenDay, 10080),      // weekly - 7 days in minutes
    ("7ds", RateLimitType::SevenDaySonnet, 10080), // sonnet weekly
    ("7do", RateLimitType::SevenDayOpus, 10080),   // opus weekly
    ("5h", RateLimitType::FiveHour, 300),         // session - 5 hours in minutes
];

/// Parse unified rate limit headers from current Claude Code
/// Headers: anthropic-ratelimit-unified-{claim}-utilization, anthropic-ratelimit-unified-{claim}-reset
fn parse_unified_rate_limit(headers: &HeaderMap) -> Option<RateLimitWindow> {
    // Check status first - if present, we have rate limit info
    let status = parse_header_str(headers, "anthropic-ratelimit-unified-status");

    // Get the representative claim if specified
    let representative = parse_header_str(headers, "anthropic-ratelimit-unified-representative-claim");

    // Try each claim type to find utilization data
    for (claim, limit_type, window_minutes) in RATE_LIMIT_CLAIMS {
        let utilization_header = format!("anthropic-ratelimit-unified-{}-utilization", claim);
        let reset_header = format!("anthropic-ratelimit-unified-{}-reset", claim);

        if let Some(utilization_str) = parse_header_str(headers, &utilization_header) {
            // Utilization is 0-1, convert to percentage
            let utilization = utilization_str.parse::<f64>().ok()?;
            let used_percent = utilization * 100.0;

            let resets_at = parse_header_str(headers, &reset_header)
                .and_then(|s| s.parse::<i64>().ok());

            debug!(
                "Found rate limit from claim {}: {:.1}% used, resets {:?}",
                claim, used_percent, resets_at
            );

            return Some(RateLimitWindow {
                used_percent,
                window_minutes: Some(*window_minutes),
                resets_at,
                limit_type: *limit_type,
            });
        }
    }

    // Fallback: try the old status-based parsing
    let status = status?;

    // Parse status - might be percentage or fraction
    let used_percent = if status.contains('%') {
        status.trim_end_matches('%').parse::<f64>().ok()?
    } else if status.contains('/') {
        // Format: "used/limit" like "32/100"
        let parts: Vec<&str> = status.split('/').collect();
        if parts.len() == 2 {
            let used = parts[0].parse::<f64>().ok()?;
            let limit = parts[1].parse::<f64>().ok()?;
            if limit > 0.0 { (used / limit) * 100.0 } else { 0.0 }
        } else {
            return None;
        }
    } else {
        // If it's just the status word (allowed, rejected), we don't have percentage
        return None;
    };

    // Parse reset time from generic reset header
    let reset_str = parse_header_str(headers, "anthropic-ratelimit-unified-reset");
    let resets_at = reset_str.and_then(|s| {
        s.parse::<i64>().ok()
            .or_else(|| chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp()))
    });

    Some(RateLimitWindow {
        used_percent,
        window_minutes: Some(10080), // Assume weekly
        resets_at,
        limit_type: RateLimitType::SevenDay,
    })
}

/// Parse standard x-ratelimit-* headers from Anthropic public API
fn parse_standard_rate_limit(headers: &HeaderMap) -> Option<RateLimitWindow> {
    // Standard headers: x-ratelimit-limit-requests, x-ratelimit-remaining-requests, etc.
    let limit = parse_header_i64(headers, "x-ratelimit-limit-requests")?;
    let remaining = parse_header_i64(headers, "x-ratelimit-remaining-requests")?;
    let reset_str = parse_header_str(headers, "x-ratelimit-reset-requests");

    let used = limit - remaining;
    let used_percent = if limit > 0 {
        (used as f64 / limit as f64) * 100.0
    } else {
        0.0
    };

    // Parse reset time if available (format: "2024-01-01T00:00:00Z" or "60s")
    let resets_at = reset_str.and_then(|s| {
        // Try parsing as duration first (e.g., "60s", "5m")
        if s.ends_with('s') {
            s.trim_end_matches('s').parse::<i64>().ok()
                .map(|secs| chrono::Utc::now().timestamp() + secs)
        } else if s.ends_with('m') {
            s.trim_end_matches('m').parse::<i64>().ok()
                .map(|mins| chrono::Utc::now().timestamp() + mins * 60)
        } else {
            // Try parsing as ISO timestamp
            chrono::DateTime::parse_from_rfc3339(s).ok()
                .map(|dt| dt.timestamp())
        }
    });

    Some(RateLimitWindow {
        used_percent,
        window_minutes: Some(1), // Standard API usually per-minute
        resets_at,
        limit_type: RateLimitType::Unknown,
    })
}

fn parse_rate_limit_window(
    headers: &HeaderMap,
    used_percent_header: &str,
    window_minutes_header: &str,
    resets_at_header: &str,
) -> Option<RateLimitWindow> {
    let used_percent = parse_header_f64(headers, used_percent_header)?;

    let window_minutes = parse_header_i64(headers, window_minutes_header);
    let resets_at = parse_header_i64(headers, resets_at_header);

    // Determine limit type from window minutes
    let limit_type = match window_minutes {
        Some(mins) if mins >= 10080 => RateLimitType::SevenDay,
        Some(mins) if mins >= 300 => RateLimitType::FiveHour,
        _ => RateLimitType::Unknown,
    };

    // Only return if we have meaningful data
    let has_data = used_percent != 0.0
        || window_minutes.is_some_and(|m| m != 0)
        || resets_at.is_some();

    has_data.then_some(RateLimitWindow {
        used_percent,
        window_minutes,
        resets_at,
        limit_type,
    })
}

fn parse_credits_snapshot(headers: &HeaderMap) -> Option<CreditsSnapshot> {
    let has_credits = parse_header_bool(headers, "x-codex-credits-has-credits")?;
    let unlimited = parse_header_bool(headers, "x-codex-credits-unlimited").unwrap_or(false);
    let balance = parse_header_str(headers, "x-codex-credits-balance")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from);

    Some(CreditsSnapshot {
        has_credits,
        unlimited,
        balance,
    })
}

fn parse_header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    parse_header_str(headers, name)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn parse_header_i64(headers: &HeaderMap, name: &str) -> Option<i64> {
    parse_header_str(headers, name)?.parse::<i64>().ok()
}

fn parse_header_bool(headers: &HeaderMap, name: &str) -> Option<bool> {
    let raw = parse_header_str(headers, name)?;
    if raw.eq_ignore_ascii_case("true") || raw == "1" {
        Some(true)
    } else if raw.eq_ignore_ascii_case("false") || raw == "0" {
        Some(false)
    } else {
        None
    }
}

fn parse_header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

/// Claude OAuth credentials from ~/.claude/.credentials
#[derive(Debug, Clone)]
struct ClaudeCredentials {
    #[allow(dead_code)]
    access_token: String,
    #[allow(dead_code)]
    refresh_token: Option<String>,
    subscription_type: Option<String>,
    rate_limit_tier: Option<String>,
}

fn load_claude_credentials() -> Option<ClaudeCredentials> {
    // Try macOS keychain first (current credentials)
    if let Some(creds) = load_from_keychain() {
        return Some(creds);
    }

    // Fall back to file-based credentials
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.claude/.credentials", home),
        format!("{}/.claude/.credentialsold", home),
    ];

    for path in &paths {
        if let Some(creds) = load_from_file(path) {
            return Some(creds);
        }
    }

    error!("No Claude credentials found");
    None
}

/// Load credentials from macOS keychain
fn load_from_keychain() -> Option<ClaudeCredentials> {
    #[cfg(target_os = "macos")]
    {
        // Get current username
        let username = std::env::var("USER").ok()?;

        // Run security command to get keychain password
        let output = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s", "Claude Code-credentials",
                "-a", &username,
                "-w",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            debug!("No keychain credentials found");
            return None;
        }

        let json_str = String::from_utf8(output.stdout).ok()?;
        parse_credentials_json(&json_str, "keychain")
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Load credentials from a file path
fn load_from_file(path: &str) -> Option<ClaudeCredentials> {
    let contents = std::fs::read_to_string(path).ok()?;
    parse_credentials_json(&contents, path)
}

/// Parse credentials JSON (shared between keychain and file)
fn parse_credentials_json(json_str: &str, source: &str) -> Option<ClaudeCredentials> {
    let json: serde_json::Value = serde_json::from_str(json_str).ok()?;

    let oauth = json.get("claudeAiOauth")?;

    let access_token = oauth.get("accessToken")
        .and_then(|v| v.as_str())
        .map(String::from)?;

    let refresh_token = oauth.get("refreshToken")
        .and_then(|v| v.as_str())
        .map(String::from);

    let subscription_type = oauth.get("subscriptionType")
        .and_then(|v| v.as_str())
        .map(String::from);

    let rate_limit_tier = oauth.get("rateLimitTier")
        .and_then(|v| v.as_str())
        .map(String::from);

    info!("Loaded Claude credentials from {}", source);
    Some(ClaudeCredentials {
        access_token,
        refresh_token,
        subscription_type,
        rate_limit_tier,
    })
}

/// Rate limit fetcher - uses OAuth credentials like Claude Code CLI does
#[derive(Clone)]
pub struct RateLimitFetcher {
    client: reqwest::Client,
    api_key: Option<String>,
    credentials: Option<ClaudeCredentials>,
    latest: Arc<RwLock<Option<RateLimitSnapshot>>>,
}

impl RateLimitFetcher {
    pub fn new() -> Self {
        // Try ANTHROPIC_API_KEY first (works with public API)
        let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        let credentials = load_claude_credentials();

        Self {
            client: reqwest::Client::new(),
            api_key,
            credentials,
            latest: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if we can actively fetch rate limits (have OAuth or API key)
    pub fn can_fetch(&self) -> bool {
        self.credentials.is_some() || self.api_key.is_some()
    }

    /// Check if we have credentials configured
    pub fn has_credentials(&self) -> bool {
        self.credentials.is_some()
    }

    /// Get subscription type from credentials (e.g., "max")
    pub fn subscription_type(&self) -> Option<&str> {
        self.credentials.as_ref()?.subscription_type.as_deref()
    }

    /// Get rate limit tier from credentials (e.g., "default_claude_max_20x")
    pub fn rate_limit_tier(&self) -> Option<&str> {
        self.credentials.as_ref()?.rate_limit_tier.as_deref()
    }

    /// Get the latest cached rate limits
    pub async fn get_latest(&self) -> Option<RateLimitSnapshot> {
        self.latest.read().await.clone()
    }

    /// Get latest rate limits synchronously
    pub fn get_latest_sync(&self) -> Option<RateLimitSnapshot> {
        // Try to get lock without blocking
        if let Ok(guard) = self.latest.try_read() {
            guard.clone()
        } else {
            None
        }
    }

    /// Fetch rate limits by making a minimal API call
    /// Works with OAuth tokens using the oauth-2025-04-20 beta header
    pub async fn fetch_rate_limits(&self) -> Result<RateLimitSnapshot, String> {
        // Try OAuth first (preferred), then fall back to API key
        let request = if let Some(ref creds) = self.credentials {
            // OAuth with beta header - use Haiku for minimal cost
            debug!("Fetching rate limits with OAuth token");
            self.client
                .post(API_URL)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01")
                .header("anthropic-beta", "oauth-2025-04-20")
                .header("authorization", format!("Bearer {}", creds.access_token))
        } else if let Some(ref api_key) = self.api_key {
            // Fall back to API key
            debug!("Fetching rate limits with API key");
            self.client
                .post(API_URL)
                .header("content-type", "application/json")
                .header("anthropic-version", "2023-06-01")
                .header("x-api-key", api_key)
        } else {
            return Err("No credentials available".to_string());
        };

        // Minimal request body - Haiku is cheapest, max_tokens=1 to minimize usage
        let body = serde_json::json!({
            "model": "claude-3-haiku-20240307",
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "x"}]
        });

        let request = request.body(body.to_string());

        // Send request
        let response = request.send().await
            .map_err(|e| format!("Request failed: {}", e))?;

        // Parse rate limits from headers
        let headers = response.headers().clone();
        let snapshot = parse_rate_limits(&headers);

        // Log what we got
        if let Some(ref primary) = snapshot.primary {
            info!(
                "Fetched rate limit: {:.1}% of {} used, resets {}",
                primary.used_percent,
                primary.limit_type.display_name(),
                primary.format_reset()
            );
        } else {
            warn!("No rate limit data in response headers");
            // Log all headers for debugging
            for (name, value) in headers.iter() {
                if name.as_str().contains("ratelimit") || name.as_str().contains("limit") {
                    debug!("Header {}: {:?}", name, value);
                }
            }
        }

        // Cache the result
        *self.latest.write().await = Some(snapshot.clone());

        Ok(snapshot)
    }

    /// Update rate limits from API response headers
    /// Call this when receiving responses from Claude API through autopilot
    pub async fn update_from_headers(&self, headers: &reqwest::header::HeaderMap) {
        let snapshot = parse_rate_limits(headers);

        // Only update if we got meaningful data
        if snapshot.primary.is_some() || snapshot.secondary.is_some() {
            info!("Updating rate limits from API response");
            if let Some(ref primary) = snapshot.primary {
                info!("Rate limit: {:.1}% of {} used, resets {}",
                    primary.used_percent, primary.limit_type.display_name(), primary.format_reset());
            }
            *self.latest.write().await = Some(snapshot);
        }
    }

    /// Update rate limits synchronously
    pub fn update_from_headers_sync(&self, headers: &reqwest::header::HeaderMap) {
        let snapshot = parse_rate_limits(headers);

        if snapshot.primary.is_some() || snapshot.secondary.is_some() {
            info!("Updating rate limits from API response");
            if let Ok(mut guard) = self.latest.try_write() {
                *guard = Some(snapshot);
            }
        }
    }
}

impl Default for RateLimitFetcher {
    fn default() -> Self {
        Self::new()
    }
}
