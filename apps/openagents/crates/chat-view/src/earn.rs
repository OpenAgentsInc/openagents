//! The Earn page's plain words (#12294): what `openagents pylon status`,
//! `earnings`, and `settings` report, read the same way on the desktop's
//! Earn page and the web's Earn card for a paired computer.

use serde_json::Value;

/// The machine's state in plain words, from `pylon status`'s data.
/// `setting_up` while this page's `up` is still running.
pub fn status_line(data: &Value, setting_up: bool) -> String {
    let state = data["state"].as_str().unwrap_or("off");
    let why = data["why"].as_str().unwrap_or_default();
    let idle = state == "off" || why.starts_with("not running");
    match state {
        "serving" => "Serving".into(),
        "starting" => format!("Setting up: {}", starting(why)),
        _ if setting_up && idle => {
            if data["model"].is_null() {
                "Setting up: downloading the model (6.5 GB)".into()
            } else {
                "Setting up: starting the model".into()
            }
        }
        "paused" => paused(why).into(),
        _ => "Off".into(),
    }
}

fn starting(why: &str) -> &'static str {
    if why.starts_with("checking") {
        "checking this computer's answers"
    } else if why.contains("15 minutes") {
        "the model hasn't started after 15 minutes"
    } else {
        "starting the model"
    }
}

fn paused(why: &str) -> &'static str {
    let word = |w: &str| why.split(|c: char| !c.is_alphanumeric()).any(|x| x == w);
    if why.starts_with("owner busy") {
        "Paused: you're using this computer"
    } else if why.starts_with("no network") {
        "Paused: no internet connection"
    } else if why.starts_with("an engine stopped") {
        "Paused: restarting the model"
    } else if why.starts_with("finishing jobs") {
        "Stopping: finishing the work it took"
    } else if why.starts_with("not running") {
        "Paused: not running. Turn it off and on again to start it."
    } else if why.starts_with("could not start") {
        "Can't serve: the model couldn't start"
    } else if word("battery") {
        "Paused: on battery"
    } else if word("hot") || word("heat") || word("thermal") {
        "Paused: this computer is too hot"
    } else if why.contains("self-check") || why.contains("answers") {
        "Can't serve: this computer's answers didn't match the expected ones"
    } else {
        "Paused"
    }
}

/// What `pylon earnings` reports, read leniently: whole sats or msat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Earnings {
    pub decisions_today: u64,
    pub earned_sats: u64,
    pub paid_sats: u64,
    /// Where payouts go when it is not this computer's wallet: a Lightning
    /// address.
    pub elsewhere: Option<String>,
}

impl Earnings {
    pub fn from(data: &Value) -> Self {
        let sats = |key: &str| {
            data[format!("{key}_sats")]
                .as_u64()
                .or_else(|| data[format!("{key}_msat")].as_u64().map(|msat| msat / 1000))
        };
        let destination = match &data["destination"] {
            Value::String(text) => Some(text.as_str()),
            Value::Object(o) => ["address", "value", "lightning_address"]
                .iter()
                .find_map(|key| o.get(*key).and_then(Value::as_str)),
            _ => None,
        };
        Self {
            decisions_today: ["checked_today", "verified_today", "decisions_today"]
                .iter()
                .find_map(|key| data[*key].as_u64())
                .unwrap_or(0),
            earned_sats: sats("earned").unwrap_or(0),
            paid_sats: sats("paid").or_else(|| sats("sent")).unwrap_or(0),
            elsewhere: destination.filter(|d| d.contains('@')).map(str::to_string),
        }
    }

    /// The title and the line under it.
    pub fn lines(&self) -> (String, String) {
        let title = format!(
            "{} checked {} today",
            group(self.decisions_today),
            if self.decisions_today == 1 {
                "decision"
            } else {
                "decisions"
            }
        );
        let line = if self.earned_sats == 0 && self.paid_sats == 0 {
            "Nothing earned so far. Earnings follow demand and can be zero.".into()
        } else {
            format!(
                "{} earned so far · {} paid to you",
                sats_text(self.earned_sats),
                sats_text(self.paid_sats)
            )
        };
        (title, line)
    }
}

/// An amount the way the wallet writes it (BIP 177): `₿12,345`.
pub fn sats_text(sats: u64) -> String {
    format!("₿{}", group(sats))
}

fn group(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The user's own pause choices from `pylon settings`.
pub fn pauses(data: &Value) -> (bool, bool) {
    let on = |key: &str| data[key].as_bool().unwrap_or(data[key] == "on");
    (on("pause_on_battery"), on("pause_when_hot"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_status_and_earnings_read_in_plain_words() {
        assert_eq!(status_line(&json!({"state": "serving"}), false), "Serving");
        assert_eq!(
            status_line(
                &json!({"state": "paused", "why": "on battery power"}),
                false
            ),
            "Paused: on battery"
        );
        assert_eq!(
            status_line(&json!({"state": "off", "model": null}), true),
            "Setting up: downloading the model (6.5 GB)"
        );
        let earned = Earnings::from(&json!({
            "checked_today": 1234, "earned_msat": 2_500_000, "paid_sats": 0,
            "destination": {"address": "name@example.com"},
        }));
        assert_eq!(
            earned.lines(),
            (
                "1,234 checked decisions today".to_string(),
                "₿2,500 earned so far · ₿0 paid to you".to_string()
            )
        );
        assert_eq!(earned.elsewhere.as_deref(), Some("name@example.com"));
        assert_eq!(
            pauses(&json!({"pause_on_battery": "on", "pause_when_hot": false})),
            (true, false)
        );
        for line in [earned.lines().0, earned.lines().1] {
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        }
    }
}
