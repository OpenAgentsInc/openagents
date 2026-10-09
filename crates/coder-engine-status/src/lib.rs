//! An engine's sign-in status, as it leaves the user's computer.
//!
//! The bring-your-own-Claude policy (`docs/cloud/claude-code-byo.md`) lets
//! status text leave a computer, never a credential. So the projection a
//! computer answers with is [`Status`]: closed enums and Unix seconds, with
//! no string field anywhere. A Claude login, an email address, an
//! organization name, or any other text Claude Code prints cannot ride in
//! it, because nothing in it can hold text. Every web and native client
//! renders this one type with [`Status::summary`].
//!
//! [`claude`] runs the pinned, unmodified Claude Code binary's own
//! `claude auth status` inside the computer and folds its answer, plus the
//! last usage-limit or login notice Claude Code itself printed during a
//! Coder run ([`Notice`]), into a [`Status`]. OpenAgents keeps no usage
//! ledger for the user's plan: a rate limit is shown only with the reset
//! time Claude Code reported.

use serde::{Deserialize, Serialize};

pub mod claude;
pub mod limit;

/// The engine a status describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    /// Claude Code, the unmodified published binary.
    Claude,
}

/// What a computer's engine sign-in needs from the user, if anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// No login and no key: the user signs in through the engine's flow.
    SignedOut,
    /// Signed in with the user's own Claude account.
    SignedIn,
    /// Signed in, and the login expires soon.
    Expiring,
    /// The login expired or stopped working; the user signs in again.
    Expired,
    /// The engine reported a usage limit that has not reset yet.
    RateLimited,
    /// The engine uses the user's own API key or cloud credential.
    ApiKey,
    /// The engine did not answer: missing, failed, or timed out.
    Unavailable,
}

/// The credential type the engine reports. Never the credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// A Claude account login made through Anthropic's own flow.
    ClaudeAi,
    /// A long-lived token the user set in their own environment.
    OauthToken,
    /// The user's own Anthropic API key.
    ApiKey,
    /// An API key the user's own helper script supplies.
    ApiKeyHelper,
    /// The user's own Bedrock, Vertex, or Foundry credential.
    ThirdParty,
    /// A method this release does not name.
    Other,
}

/// The Claude plan the engine reports for an account login.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    Free,
    Pro,
    Max,
    Team,
    Enterprise,
    /// A plan this release does not name.
    Other,
}

/// A computer's engine sign-in status: the only thing that leaves the
/// computer. It holds no text, so it cannot carry credential bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub engine: Engine,
    pub state: State,
    /// The credential type, when the engine is signed in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<Method>,
    /// The plan, for an account login that reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Plan>,
    /// When the login expires, in Unix seconds, when the engine said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// When a usage limit resets, in Unix seconds, as the engine reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// When the computer checked, in Unix seconds.
    pub checked_at: u64,
}

/// What Claude Code itself printed during a Coder run on this computer
/// that the sign-in status should remember: a usage limit and its reset,
/// or a login that expired or is about to. Typed, like [`Status`]; the
/// message text is never kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
pub enum Notice {
    Limited {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resets_at: Option<u64>,
        at: u64,
    },
    LoginExpired {
        at: u64,
    },
    LoginExpiring {
        at: u64,
    },
}

/// Claude Code warns this long before a login expires.
pub const EXPIRING_WITHIN: u64 = 3 * 86_400;

impl Status {
    /// A status for an engine that did not answer.
    #[must_use]
    pub fn unavailable(engine: Engine, now: u64) -> Self {
        Status {
            engine,
            state: State::Unavailable,
            method: None,
            plan: None,
            expires_at: None,
            resets_at: None,
            checked_at: now,
        }
    }

    /// Whether the user should run the engine's sign-in again: the
    /// one-click renew opens the computer's terminal running the engine.
    #[must_use]
    pub fn renew(&self) -> bool {
        matches!(
            self.state,
            State::SignedOut | State::Expiring | State::Expired
        )
    }

    /// The one-click action a client offers, when the user should sign in
    /// again: it opens the computer's terminal running the engine's own
    /// sign-in. `None` when nothing is needed.
    #[must_use]
    pub fn action(&self) -> Option<&'static str> {
        match (self.engine, self.state) {
            (Engine::Claude, State::SignedOut) => Some("Sign in to Claude"),
            (Engine::Claude, State::Expiring | State::Expired) => Some("Renew Claude sign-in"),
            _ => None,
        }
    }

    /// What a native terminal client shows once, when the status needs
    /// the person: the summary and the same renew, typed into a terminal
    /// on that computer. `None` when nothing is needed.
    #[must_use]
    pub fn terminal_notice(&self) -> Option<String> {
        match self.state {
            State::SignedOut | State::Expiring | State::Expired => Some(format!(
                "{} Run `claude` in a terminal on this computer and type /login to sign in through Anthropic's own flow.",
                self.summary()
            )),
            State::RateLimited => Some(self.summary()),
            State::SignedIn | State::ApiKey | State::Unavailable => None,
        }
    }

    /// A short label for the state.
    #[must_use]
    pub fn headline(&self) -> &'static str {
        match self.state {
            State::SignedOut => "Not signed in",
            State::SignedIn => "Signed in",
            State::Expiring => "Login expiring soon",
            State::Expired => "Login expired",
            State::RateLimited => "Usage limit reached",
            State::ApiKey => "Using your own key",
            State::Unavailable => "Status unavailable",
        }
    }

    /// The plain-text status every client shows. No logos, and nothing
    /// beyond what the engine reported.
    #[must_use]
    pub fn summary(&self) -> String {
        let engine = match self.engine {
            Engine::Claude => "This computer runs Claude Code",
        };
        let account = match (self.method, self.plan) {
            (_, Some(Plan::Other)) | (Some(Method::ClaudeAi), None) => {
                "your Claude account".to_string()
            }
            (_, Some(plan)) => format!("your Claude {} plan", plan_name(plan)),
            (Some(Method::OauthToken), None) => "a long-lived Claude token you set".to_string(),
            (Some(Method::ApiKey), None) => "your own Anthropic API key".to_string(),
            (Some(Method::ApiKeyHelper), None) => "an API key from your own helper".to_string(),
            (Some(Method::ThirdParty), None) => "your own cloud provider credential".to_string(),
            (Some(Method::Other) | None, None) => "your own credential".to_string(),
        };
        match self.state {
            State::SignedOut => format!(
                "{engine}, and it is not signed in. Sign in to Claude to use your own plan or API key."
            ),
            State::SignedIn => format!("{engine}, signed in with {account}."),
            State::ApiKey => format!("{engine}, using {account}."),
            State::Expiring => match self.expires_at {
                Some(at) => format!(
                    "{engine}, signed in with {account}. The login expires {}. Renew the sign-in before then so background tasks keep running.",
                    utc(at)
                ),
                None => format!(
                    "{engine}, signed in with {account}. Claude Code says the login expires soon. Renew the sign-in so background tasks keep running."
                ),
            },
            State::Expired => format!(
                "{engine}, and its login has expired or stopped working. Renew the sign-in to keep using {account}."
            ),
            State::RateLimited => match self.resets_at {
                Some(at) => format!(
                    "{engine}, signed in with {account}. Claude Code reported a usage limit that resets {}. Tasks on this plan wait until then. OpenAgents keeps no usage ledger for your plan.",
                    utc(at)
                ),
                None => format!(
                    "{engine}, signed in with {account}. Claude Code reported a usage limit without a reset time. OpenAgents keeps no usage ledger for your plan."
                ),
            },
            State::Unavailable => format!(
                "{engine}, but it did not report a sign-in status. Check that the engine is installed on this computer."
            ),
        }
    }
}

fn plan_name(plan: Plan) -> &'static str {
    match plan {
        Plan::Free => "Free",
        Plan::Pro => "Pro",
        Plan::Max => "Max",
        Plan::Team => "Team",
        Plan::Enterprise => "Enterprise",
        Plan::Other => "",
    }
}

/// Unix seconds as `2026-09-23 11:50 UTC`.
#[must_use]
pub fn utc(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let rest = seconds % 86_400;
    // Civil date from days since 1970-01-01 (proleptic Gregorian).
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rest / 3_600,
        rest % 3_600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Every string a serialized status holds, at any depth.
    fn strings(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => out.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| strings(item, out)),
            Value::Object(map) => map.values().for_each(|item| strings(item, out)),
            _ => {}
        }
    }

    fn every_status() -> Vec<Status> {
        let states = [
            State::SignedOut,
            State::SignedIn,
            State::Expiring,
            State::Expired,
            State::RateLimited,
            State::ApiKey,
            State::Unavailable,
        ];
        let methods = [
            None,
            Some(Method::ClaudeAi),
            Some(Method::OauthToken),
            Some(Method::ApiKey),
            Some(Method::ApiKeyHelper),
            Some(Method::ThirdParty),
            Some(Method::Other),
        ];
        let plans = [
            None,
            Some(Plan::Free),
            Some(Plan::Pro),
            Some(Plan::Max),
            Some(Plan::Team),
            Some(Plan::Enterprise),
            Some(Plan::Other),
        ];
        let mut all = vec![];
        for state in states {
            for method in methods {
                for plan in plans {
                    all.push(Status {
                        engine: Engine::Claude,
                        state,
                        method,
                        plan,
                        expires_at: Some(u64::MAX),
                        resets_at: Some(u64::MAX),
                        checked_at: u64::MAX,
                    });
                }
            }
        }
        all
    }

    #[test]
    fn the_projection_holds_only_closed_vocabulary_and_numbers() {
        let vocabulary = [
            "claude",
            "signed_out",
            "signed_in",
            "expiring",
            "expired",
            "rate_limited",
            "api_key",
            "unavailable",
            "claude_ai",
            "oauth_token",
            "api_key_helper",
            "third_party",
            "other",
            "free",
            "pro",
            "max",
            "team",
            "enterprise",
        ];
        for status in every_status() {
            let value = serde_json::to_value(status).unwrap();
            let mut found = vec![];
            strings(&value, &mut found);
            for text in found {
                assert!(vocabulary.contains(&text.as_str()), "{text}");
            }
            assert_eq!(serde_json::from_value::<Status>(value).unwrap(), status);
            assert!(!status.summary().is_empty());
        }
    }

    #[test]
    fn the_projection_refuses_any_text_a_credential_could_ride_in() {
        let token = format!("sk-ant-oat01-{}", "q7".repeat(40));
        let base = serde_json::json!({"engine":"claude","state":"signed_in","checked_at":1});
        assert!(serde_json::from_value::<Status>(base.clone()).is_ok());
        for (field, value) in [
            ("token", Value::String(token.clone())),
            ("email", Value::String("someone@example.com".into())),
            ("method", Value::String(token.clone())),
            ("plan", Value::String(token.clone())),
            ("state", Value::String(token.clone())),
            ("engine", Value::String(token.clone())),
            ("expires_at", Value::String(token.clone())),
            ("resets_at", Value::String(token.clone())),
        ] {
            let mut forged = base.clone();
            forged[field] = value;
            assert!(
                serde_json::from_value::<Status>(forged).is_err(),
                "{field} carried text"
            );
        }
        let notice = serde_json::json!({"kind":"limited","at":1,"message":token});
        assert!(serde_json::from_value::<Notice>(notice).is_err());
    }

    #[test]
    fn renew_is_offered_for_signed_out_expiring_and_expired() {
        for status in every_status() {
            assert_eq!(status.action().is_some(), status.renew());
            assert_eq!(
                status
                    .terminal_notice()
                    .is_some_and(|text| text.contains("Run `claude`")),
                status.renew()
            );
            assert_eq!(
                status.renew(),
                matches!(
                    status.state,
                    State::SignedOut | State::Expiring | State::Expired
                )
            );
        }
    }

    #[test]
    fn copy_is_plain_text_and_names_the_reset_claude_code_reported() {
        let status = Status {
            engine: Engine::Claude,
            state: State::RateLimited,
            method: Some(Method::ClaudeAi),
            plan: Some(Plan::Max),
            expires_at: None,
            resets_at: Some(1_790_164_200),
            checked_at: 1_790_163_058,
        };
        let text = status.summary();
        assert!(text.contains("runs Claude Code"), "{text}");
        assert!(text.contains("2026-09-23 11:50 UTC"), "{text}");
        assert!(text.contains("Max plan"), "{text}");
        assert!(text.contains("no usage ledger"), "{text}");
        for status in every_status() {
            let text = status.summary();
            assert!(!text.contains('<') && !text.to_lowercase().contains("logo"));
        }
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00 UTC");
    }
}
