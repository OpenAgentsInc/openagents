//! The launch forms' choices and checks. Every problem is one plain
//! sentence the page shows over the form.

use super::store::{Profile, Survey, WaitlistEntry};

/// What someone does: value and label. Shared by the waitlist (optional)
/// and the profile (required).
pub(crate) const ROLES: [(&str, &str); 5] = [
    ("indie", "Indie developer"),
    ("hobbyist", "Hobbyist"),
    ("student", "Student"),
    ("player", "Player"),
    ("other", "Something else"),
];

pub(crate) const ENGINES: [(&str, &str); 8] = [
    ("unity", "Unity"),
    ("unreal", "Unreal"),
    ("godot", "Godot"),
    ("roblox", "Roblox"),
    ("gamemaker", "GameMaker"),
    ("web", "Web (Three.js, Babylon, and others)"),
    ("other", "Another engine"),
    ("none", "None yet"),
];

pub(crate) const EXPERIENCE: [(&str, &str); 4] = [
    ("new", "Just starting"),
    ("some", "A year or two"),
    ("several", "Several years"),
    ("work", "I make games for work"),
];

pub(crate) const FINISHED: [(&str, &str); 3] =
    [("yes", "Yes"), ("partial", "Part of it"), ("no", "No")];

/// The longest free-text answer.
pub(crate) const TEXT_MAX: usize = 1000;
const EMAIL_MAX: usize = 200;

/// A form body as name and value pairs, in order, so a field may repeat
/// (the engine checkboxes).
#[derive(Default, Debug)]
pub(crate) struct Fields(Vec<(String, String)>);

impl Fields {
    pub(crate) fn parse(body: &[u8]) -> Self {
        Fields(
            url::form_urlencoded::parse(body)
                .into_owned()
                .take(64)
                .collect(),
        )
    }

    /// The first value of `name`, trimmed; empty when absent.
    pub(crate) fn get(&self, name: &str) -> &str {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map_or("", |(_, value)| value.trim())
    }

    pub(crate) fn all(&self, name: &str) -> Vec<&str> {
        self.0
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.trim())
            .collect()
    }

    pub(crate) fn checked(&self, name: &str) -> bool {
        matches!(self.get(name), "on" | "yes" | "1" | "true")
    }
}

fn choice(options: &[(&'static str, &str)], value: &str) -> Option<&'static str> {
    options
        .iter()
        .find(|(option, _)| *option == value)
        .map(|(option, _)| *option)
}

/// `email`, trimmed and lowercased, when it looks like an address.
pub(crate) fn email(value: &str) -> Option<String> {
    let email = value.trim();
    let (local, domain) = email.split_once('@')?;
    let fine = !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
        && !email.contains(char::is_whitespace)
        && !email.chars().any(char::is_control)
        && email.len() <= EMAIL_MAX;
    fine.then(|| email.to_ascii_lowercase())
}

/// Free text, trimmed, with control characters other than newlines
/// removed; `None` when it is longer than [`TEXT_MAX`] characters.
pub(crate) fn text(value: &str) -> Option<String> {
    let clean: String = value
        .trim()
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    (clean.chars().count() <= TEXT_MAX).then_some(clean)
}

/// Where a visitor came from: `utm_source` or `ref`, lowercased, letters,
/// digits, `-`, `_` and `.` only, at most 40 characters.
pub(crate) fn source(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(40)
        .collect()
}

pub(crate) fn waitlist(fields: &Fields, now: u64) -> Result<WaitlistEntry, &'static str> {
    let Some(email) = email(fields.get("email")) else {
        return Err("Enter an email address, like you@example.com.");
    };
    let role = match fields.get("role") {
        "" => "",
        value => choice(&ROLES, value).ok_or("Choose one of the listed roles.")?,
    };
    Ok(WaitlistEntry {
        email,
        role: role.to_owned(),
        source: source(fields.get("source")),
        created_unix: now,
    })
}

/// A profile from the form. `account_email` is the signed-in account's
/// address, used when the form leaves the email empty.
pub(crate) fn profile(
    fields: &Fields,
    account_id: Option<&str>,
    account_email: Option<&str>,
    now: u64,
) -> Result<Profile, &'static str> {
    let Some(role) = choice(&ROLES, fields.get("role")) else {
        return Err("Choose what describes you best.");
    };
    let typed = fields.get("email");
    let email = match (typed, account_email) {
        ("", Some(account)) => account.to_ascii_lowercase(),
        ("", None) if account_id.is_some() => String::new(),
        ("", None) => return Err("Enter your email so we can match your answers."),
        (typed, _) => email(typed).ok_or("That email address doesn't look right.")?,
    };
    let mut engines = Vec::new();
    for value in fields.all("engines") {
        let engine = choice(&ENGINES, value).ok_or("Choose engines from the list.")?;
        if !engines.iter().any(|e| e == engine) {
            engines.push(engine.to_owned());
        }
    }
    let experience = match fields.get("experience") {
        "" => "",
        value => choice(&EXPERIENCE, value).ok_or("Choose how long you've been making games.")?,
    };
    let make =
        text(fields.get("make")).ok_or("Keep what you want to make under 1,000 characters.")?;
    Ok(Profile {
        account_id: account_id.map(str::to_owned),
        email,
        role: role.to_owned(),
        engines,
        experience: experience.to_owned(),
        make,
        heard: source(fields.get("heard")),
        contact_ok: fields.checked("contact_ok"),
        session: None,
        created_unix: now,
        updated_unix: now,
    })
}

fn number(value: &str, low: u8, high: u8) -> Option<u8> {
    value
        .parse::<u8>()
        .ok()
        .filter(|n| (low..=high).contains(n))
}

/// A survey from the form; the session, device, browser, account, and
/// source come from the request, not the form.
pub(crate) fn survey(fields: &Fields) -> Result<Survey, &'static str> {
    let Some(ease) = number(fields.get("ease"), 1, 5) else {
        return Err("Pick how easy it was, from 1 to 5.");
    };
    let Some(finished) = choice(&FINISHED, fields.get("finished")) else {
        return Err("Tell us whether you finished the tutorial.");
    };
    let Some(again) = number(fields.get("again"), 0, 10) else {
        return Err("Pick how likely you are to use it again, from 0 to 10.");
    };
    let too_long = "Keep each answer under 1,000 characters.";
    let confused = text(fields.get("confused")).ok_or(too_long)?;
    let make = text(fields.get("make")).ok_or(too_long)?;
    let email = match fields.get("email") {
        "" => String::new(),
        typed => email(typed).ok_or("That email address doesn't look right.")?,
    };
    let contact_ok = fields.checked("contact_ok");
    if contact_ok && email.is_empty() {
        return Err("Add your email if you'd like us to follow up.");
    }
    Ok(Survey {
        ease,
        finished: finished.to_owned(),
        confused,
        make,
        again,
        email,
        contact_ok,
        ..Survey::default()
    })
}

/// The device class and browser family a user agent names.
pub(crate) fn device_and_browser(agent: &str) -> (&'static str, &'static str) {
    let device =
        if agent.contains("iPad") || (agent.contains("Android") && !agent.contains("Mobile")) {
            "tablet"
        } else if agent.contains("Mobi") || agent.contains("iPhone") || agent.contains("Android") {
            "mobile"
        } else {
            "desktop"
        };
    let browser = if agent.contains("Edg/") {
        "Edge"
    } else if agent.contains("OPR/") {
        "Opera"
    } else if agent.contains("SamsungBrowser") {
        "Samsung Internet"
    } else if agent.contains("Firefox/") || agent.contains("FxiOS") {
        "Firefox"
    } else if agent.contains("Chrome/") || agent.contains("CriOS") {
        "Chrome"
    } else if agent.contains("Safari/") {
        "Safari"
    } else {
        "Other"
    };
    (device, browser)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> Fields {
        Fields(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    }

    #[test]
    fn emails_are_checked_and_lowercased() {
        assert_eq!(
            email(" Ada@Example.COM ").as_deref(),
            Some("ada@example.com")
        );
        for bad in [
            "",
            "ada",
            "ada@",
            "@x.io",
            "ada@x",
            "a da@x.io",
            "a@.io",
            "a@x.",
            "a@b@c.io",
        ] {
            assert_eq!(email(bad), None, "{bad}");
        }
        assert_eq!(email(&format!("{}@x.io", "a".repeat(200))), None);
    }

    #[test]
    fn the_waitlist_takes_an_email_and_an_optional_known_role() {
        let entry = waitlist(
            &fields(&[("email", "a@b.io"), ("source", "Hacker News!")]),
            7,
        )
        .unwrap();
        assert_eq!(
            (entry.role.as_str(), entry.source.as_str()),
            ("", "hackernews")
        );
        assert!(waitlist(&fields(&[("email", "a@b.io"), ("role", "wizard")]), 7).is_err());
        assert!(waitlist(&fields(&[("email", "nope")]), 7).is_err());
        assert_eq!(
            waitlist(&fields(&[("email", "a@b.io"), ("role", "student")]), 7)
                .unwrap()
                .role,
            "student"
        );
    }

    #[test]
    fn a_profile_needs_a_role_and_a_guest_needs_an_email() {
        let form = fields(&[
            ("role", "indie"),
            ("engines", "godot"),
            ("engines", "unity"),
            ("engines", "godot"),
            ("experience", "some"),
            ("make", "A cozy farming game"),
            ("heard", "twitter"),
            ("contact_ok", "on"),
            ("email", "ada@example.com"),
        ]);
        let guest = profile(&form, None, None, 3).unwrap();
        assert_eq!(guest.engines, ["godot", "unity"]);
        assert!(guest.contact_ok && guest.account_id.is_none());
        let no_email = fields(&[("role", "indie")]);
        assert!(profile(&no_email, None, None, 3).is_err());
        let member = profile(&no_email, Some("acct_1"), Some("Ada@Example.com"), 3).unwrap();
        assert_eq!(member.email, "ada@example.com");
        assert!(profile(&fields(&[("email", "a@b.io")]), None, None, 3).is_err());
        assert!(
            profile(
                &fields(&[("role", "indie"), ("email", "a@b.io"), ("engines", "cobol")]),
                None,
                None,
                3
            )
            .is_err()
        );
        let long = "x".repeat(TEXT_MAX + 1);
        assert!(
            profile(
                &fields(&[("role", "indie"), ("email", "a@b.io"), ("make", &long)]),
                None,
                None,
                3
            )
            .is_err()
        );
    }

    #[test]
    fn a_survey_checks_each_scale_and_the_follow_up_email() {
        let good = [
            ("ease", "4"),
            ("finished", "partial"),
            ("confused", "Where the save button was"),
            ("make", "A haunted maze"),
            ("again", "8"),
        ];
        let survey = super::survey(&fields(&good)).unwrap();
        assert_eq!(
            (survey.ease, survey.again, survey.finished.as_str()),
            (4, 8, "partial")
        );
        for (name, bad) in [
            ("ease", "0"),
            ("ease", "6"),
            ("again", "11"),
            ("again", "x"),
            ("finished", "maybe"),
        ] {
            let mut form: Vec<_> = good.iter().filter(|(k, _)| *k != name).copied().collect();
            form.push((name, bad));
            assert!(super::survey(&fields(&form)).is_err(), "{name}={bad}");
        }
        let mut form = good.to_vec();
        form.push(("contact_ok", "on"));
        assert!(super::survey(&fields(&form)).is_err());
        form.push(("email", "ada@example.com"));
        assert!(super::survey(&fields(&form)).unwrap().contact_ok);
    }

    #[test]
    fn text_is_bounded_and_loses_control_characters() {
        assert_eq!(text(" a\u{0}b\nc ").as_deref(), Some("ab\nc"));
        assert!(text(&"é".repeat(TEXT_MAX)).is_some());
        assert!(text(&"é".repeat(TEXT_MAX + 1)).is_none());
    }

    #[test]
    fn the_user_agent_names_a_device_and_browser() {
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
        assert_eq!(device_and_browser(iphone), ("mobile", "Safari"));
        let chrome = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
        assert_eq!(device_and_browser(chrome), ("desktop", "Chrome"));
        assert_eq!(device_and_browser(&format!("{chrome} Edg/130.0")).1, "Edge");
        assert_eq!(device_and_browser(""), ("desktop", "Other"));
    }
}
