//! `openagents connections` (#11574, #11578): the services connected to the
//! person's openagents.com account (Google: Drive, Gmail, Calendar), from the command
//! line, through the website's `/v1/connections` routes
//! (`docs/api/connections.md`). Attaching Drive folders and files to a
//! project is `openagents projects sources`.
//!
//! Calls run under the account's own sign-in, as `openagents projects`
//! does: `OPENAGENTS_TOKEN` with `OPENAGENTS_ORIGIN`, else `--account-dir
//! DIR`, else the sign-in `openagents login` or `coder login` saved.
//! Google's consent is a person's click in a browser: `connect google`
//! prints the link, and opens it only on a terminal whose `BROWSER`
//! allows it.

use std::io::IsTerminal;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::keys::Site;
use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents connections [COMMAND] [--account-dir DIR]
  list        The services connected to your openagents.com account: Google
              Drive, whether it's connected and as whom, and the Drive
              sources attached to your projects.
  connect google [--sheets] [--feature FEATURE] [--no-open]
              Print the link that connects Google Drive (read only), and open
              it in your browser on this computer. --sheets also lets chats
              read every tab of your Google Sheets. --feature asks for one
              more thing on its own: gmail (read mail), gmail_compose (write
              drafts), calendar (read events), calendar_events (add events
              you confirm). Open the link logged in to openagents.com as
              you; Google asks you to allow it.
  remove google
              Disconnect Google. Your projects keep their Drive sources for
              when you connect it again.
  drive [QUERY] [--folder FOLDER]
              Browse your Drive (My Drive, or the folder's id or link), or
              search it, to find a folder or file to attach.
  gmail [QUERY] [--limit N]
              Search your Gmail (Gmail's search words: from:, subject:,
              newer_than:7d, is:unread; your inbox when empty): sender,
              subject, date, and link of each message, newest first.
  calendar [RANGE]
              Your Google Calendar's events: RANGE is today (the default),
              tomorrow, or week.
  access list
              Admins: who may use Google Drive connections while Google
              hasn't approved the app. Admins always may.
  access add WHO
              Admins: let someone use them, by email or account id.
  access remove WHO
              Admins: take someone off the list.
Attach a Drive folder or file to a project with openagents projects sources
add PROJECT LINK; chats in that project then answer from it. Log in as
openagents login or coder login saved it, or OPENAGENTS_TOKEN.";

/// What each command does, for the chat router's command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("connect google", Effect::Grants),
    Declared::computer("remove google", Effect::Grants),
    Declared::computer("drive", Effect::ReadOnly),
    Declared::computer("gmail", Effect::ReadOnly),
    Declared::computer("calendar", Effect::ReadOnly),
    Declared::computer("access list", Effect::ReadOnly),
    Declared::computer("access add", Effect::Grants),
    Declared::computer("access remove", Effect::Grants),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &["sheets", "no-open"]) {
        Ok(args) => args,
        Err(message) => return output.usage("connections", &message, USAGE),
    };
    let positional = args.positional();
    let command = positional.first().map_or("list", String::as_str);
    let (command, rest): (String, &[String]) = match command {
        "connect" | "remove" | "access" => match positional.get(1) {
            Some(sub) => (format!("{command} {sub}"), &positional[2..]),
            None if command == "access" => ("access list".to_owned(), &positional[1..]),
            None => {
                return output.usage(
                    "connections",
                    &format!("{command} takes the service: {command} google"),
                    USAGE,
                );
            }
        },
        other => (other.to_owned(), positional.get(1..).unwrap_or_default()),
    };
    let (known, fewest, most): (&[&str], usize, usize) = match command.as_str() {
        "list" => (&["account-dir"], 0, 0),
        "connect google" => (&["account-dir", "sheets", "feature", "no-open"], 0, 0),
        "remove google" => (&["account-dir"], 0, 0),
        "drive" => (&["account-dir", "folder"], 0, 1),
        "gmail" => (&["account-dir", "limit"], 0, 1),
        "calendar" => (&["account-dir"], 0, 1),
        "access list" => (&["account-dir"], 0, 0),
        "access add" | "access remove" => (&["account-dir"], 1, 1),
        "connect" | "remove" => unreachable!("joined above"),
        other if other.starts_with("connect ") || other.starts_with("remove ") => {
            return output.usage(
                "connections",
                "Google is the one service to connect so far: connect google",
                USAGE,
            );
        }
        other => {
            return output.usage("connections", &format!("unknown command `{other}`"), USAGE);
        }
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !known.contains(name))
    {
        return output.usage("connections", &format!("unknown option `--{name}`"), USAGE);
    }
    if rest.len() < fewest || rest.len() > most {
        return output.usage(
            "connections",
            &format!(
                "{command} takes {}",
                if most == 0 {
                    "no arguments"
                } else {
                    "one argument"
                }
            ),
            USAGE,
        );
    }
    let site = match Site::open(args.option("account-dir")) {
        Ok(site) => site,
        Err(message) => return output.fail("connections", &message),
    };
    let api = Api(&site);
    let result = match command.as_str() {
        "list" => api
            .get("/v1/connections")
            .map(|listed| output.emit(&listed, list_text)),
        "connect google" => api
            .send(
                site.http
                    .post(site.url("/v1/connections/google/connect"))
                    .json(&{
                        let mut body = json!({"sheets": args.switch("sheets")});
                        if let Some(feature) = args.option("feature") {
                            body["feature"] = json!(feature);
                        }
                        body
                    }),
            )
            .map(|consent| {
                let url = consent["url"].as_str().unwrap_or_default().to_owned();
                let opened = !output.json() && !args.switch("no-open") && open(&url);
                let mut shown = consent.clone();
                shown["opened"] = json!(opened);
                output.emit(&shown, |value| consent_text(value, opened));
            }),
        "remove google" => api
            .send(site.http.delete(site.url("/v1/connections/google")))
            .map(|removed| output.emit(&removed, removed_text)),
        "drive" => {
            let mut query = Vec::new();
            if let Some(q) = rest.first() {
                query.push(format!("q={}", encode(q)));
            }
            if let Some(folder) = args.option("folder") {
                query.push(format!("folder={}", encode(folder)));
            }
            let path = if query.is_empty() {
                "/v1/connections/google/drive".to_owned()
            } else {
                format!("/v1/connections/google/drive?{}", query.join("&"))
            };
            api.get(&path)
                .map(|listed| output.emit(&listed, drive_text))
        }
        "gmail" => {
            let limit = match args.option("limit").map(str::parse::<u32>) {
                None => 10,
                Some(Ok(n)) if (1..=20).contains(&n) => n,
                Some(_) => {
                    return output.usage("connections", "--limit is 1 to 20", USAGE);
                }
            };
            api.send(
                site.http
                    .post(site.url("/v1/connections/google/tools/gmail_search"))
                    .json(
                        &json!({"query": rest.first().map_or("", String::as_str), "limit": limit}),
                    ),
            )
            .map(|found| output.emit(&found, gmail_text))
        }
        "calendar" => {
            let range = rest.first().map_or("today", String::as_str);
            if !matches!(range, "today" | "tomorrow" | "week") {
                return output.usage(
                    "connections",
                    "calendar takes today, tomorrow, or week",
                    USAGE,
                );
            }
            api.send(
                site.http
                    .post(site.url("/v1/connections/google/tools/calendar_list_events"))
                    .json(&json!({"range": range})),
            )
            .map(|found| output.emit(&found, calendar_text))
        }
        "access list" => api
            .get("/v1/admin/access/drive-connections")
            .map(|listed| output.emit(&listed, access_text)),
        "access add" => {
            let who = rest[0].trim();
            let body = if who.contains('@') {
                json!({"email": who})
            } else {
                json!({"account": who})
            };
            api.send(
                site.http
                    .post(site.url("/v1/admin/access/drive-connections"))
                    .json(&body),
            )
            .map(|listed| {
                output.emit(&listed, |value| {
                    if value["added"] == json!(true) {
                        format!("{who} may use Google Drive connections now. Add them to the test users on Google's Audience page too: https://console.cloud.google.com/auth/audience?project=openagentsgemini")
                    } else {
                        format!("{who} was already on the list.")
                    }
                });
            })
        }
        "access remove" => {
            let who = rest[0].trim();
            api.send(site.http.delete(site.url(&format!(
                "/v1/admin/access/drive-connections/{}",
                encode(who)
            ))))
            .map(|listed| {
                output.emit(&listed, |_| {
                    format!("{who} can't use Google Drive connections now.")
                });
            })
        }
        _ => unreachable!("checked above"),
    };
    match result {
        Ok(()) => 0,
        Err(message) => output.fail("connections", &message),
    }
}

struct Api<'a>(&'a Site);

impl Api<'_> {
    fn get(&self, path: &str) -> Result<Value, String> {
        self.send(self.0.http.get(self.0.url(path)))
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
        Site::answer(request.bearer_auth(self.0.saved.token()).send()).map_err(|message| {
            if message.contains("answered 404") || message == "Not found." {
                "Connections aren't open to your account yet.".to_owned()
            } else {
                message
            }
        })
    }
}

/// `text` for a query value or path segment.
pub(crate) fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Open `url` in a browser: only on a terminal, and only when `BROWSER`
/// and `NO_OPEN_BROWSER` allow it.
fn open(url: &str) -> bool {
    if !std::io::stdout().is_terminal() || std::env::var_os("NO_OPEN_BROWSER").is_some() {
        return false;
    }
    match std::env::var("BROWSER") {
        Ok(browser) => {
            let browser = browser.trim();
            if browser.is_empty()
                || matches!(
                    browser,
                    "true" | "/usr/bin/true" | "/bin/true" | "none" | ":"
                )
            {
                return false;
            }
            std::process::Command::new(browser)
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        }
        Err(_) => openagents_login::open_browser(url),
    }
}

fn status_text(status: &str, email: Option<&str>) -> String {
    match (status, email) {
        ("connected", Some(email)) => format!("connected as {email}"),
        ("connected", None) => "connected".into(),
        ("reconnect", _) => {
            "Google stopped accepting the connection; connect it again with: openagents connections connect google".into()
        }
        ("not_connected", _) => {
            "not connected; connect it with: openagents connections connect google".into()
        }
        _ => "can't be connected on this site yet".into(),
    }
}

fn list_text(listed: &Value) -> String {
    let google = &listed["google"];
    let features = &google["features"];
    let on = |name: &str| features[name] == json!(true);
    let mut lines = vec![format!(
        "Google: {}{}",
        status_text(
            google["status"].as_str().unwrap_or("unavailable"),
            google["email"].as_str()
        ),
        if google["sheets"] == json!(true) {
            ", with Sheets"
        } else {
            ""
        }
    )];
    if google["status"] == "connected" {
        lines.push(format!(
            "Gmail: {}. Calendar: {}.",
            match (on("gmail"), on("gmail_compose")) {
                (true, true) => "reads mail and writes drafts",
                (true, false) => "reads mail",
                _ => "not allowed (openagents connections connect google --feature gmail)",
            },
            match (on("calendar"), on("calendar_events")) {
                (true, true) => "reads events and adds ones you confirm",
                (true, false) => "reads events",
                _ => "not allowed (openagents connections connect google --feature calendar)",
            }
        ));
    }
    let sources = listed["sources"].as_array().cloned().unwrap_or_default();
    if sources.is_empty() {
        lines.push(
            "No Drive sources in your projects. Attach one with: openagents projects sources add PROJECT LINK"
                .into(),
        );
    } else {
        lines.push(format!(
            "{} Drive source{} in your projects:",
            sources.len(),
            if sources.len() == 1 { "" } else { "s" }
        ));
        for source in &sources {
            lines.push(format!(
                "  {}  {}  {}  ({})",
                source["name"].as_str().unwrap_or_default(),
                source["kind"].as_str().unwrap_or_default(),
                source["link"].as_str().unwrap_or_default(),
                source["project"].as_str().unwrap_or_default(),
            ));
        }
    }
    lines.join("\n")
}

fn consent_text(consent: &Value, opened: bool) -> String {
    let url = consent["url"].as_str().unwrap_or_default();
    let mut out = if opened {
        format!("Opened {url} in your browser.")
    } else {
        format!("Open this link in a browser logged in to openagents.com as you:\n{url}")
    };
    out.push_str(
        "\nGoogle asks you to allow read-only access; then check with: openagents connections",
    );
    out
}

fn removed_text(removed: &Value) -> String {
    if removed["deleted"] == json!(true) {
        let kept = removed["sources_kept"].as_u64().unwrap_or(0);
        if kept > 0 {
            format!(
                "Disconnected Google. Your projects keep their {kept} Drive source{} for when you connect it again.",
                if kept == 1 { "" } else { "s" }
            )
        } else {
            "Disconnected Google.".into()
        }
    } else {
        "Google wasn't connected.".into()
    }
}

fn drive_text(listed: &Value) -> String {
    let files = listed["data"].as_array().cloned().unwrap_or_default();
    let mut lines = Vec::new();
    if files.is_empty() {
        lines.push("Nothing here.".to_owned());
    }
    for file in &files {
        lines.push(format!(
            "  {}  {}  {}",
            file["name"].as_str().unwrap_or_default(),
            file["kind"].as_str().unwrap_or_default(),
            file["id"].as_str().unwrap_or_default(),
        ));
    }
    if listed["next"].is_string() {
        lines.push("More files are in this folder than shown; search for one with: openagents connections drive QUERY".into());
    }
    lines.push(
        "Attach one with: openagents projects sources add PROJECT ID; open a folder with --folder ID"
            .into(),
    );
    lines.join("\n")
}

fn gmail_text(found: &Value) -> String {
    let messages = found["result"]["messages"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if messages.is_empty() {
        return "No messages match.".into();
    }
    let mut lines = Vec::new();
    for message in &messages {
        lines.push(format!(
            "{}{}  {}  {}\n  {}",
            if message["unread"] == json!(true) {
                "* "
            } else {
                ""
            },
            message["date"].as_str().unwrap_or_default(),
            message["from"].as_str().unwrap_or_default(),
            message["subject"].as_str().unwrap_or("(no subject)"),
            message["link"].as_str().unwrap_or_default(),
        ));
    }
    lines.join("\n")
}

fn calendar_text(found: &Value) -> String {
    let events = found["result"]["events"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let zone = found["result"]["time_zone"].as_str().unwrap_or("UTC");
    if events.is_empty() {
        return format!("No events ({zone}).");
    }
    let mut lines = vec![format!("Times in {zone}:")];
    for event in &events {
        let when = if event["all_day"] == json!(true) {
            format!("{} (all day)", event["start"].as_str().unwrap_or_default())
        } else {
            let start = event["start"].as_str().unwrap_or_default();
            let end = event["end"].as_str().unwrap_or_default();
            // `2026-10-15T10:00:00-05:00` as `2026-10-15 10:00`.
            let short = |t: &str| t.get(..16).unwrap_or(t).replace('T', " ");
            format!(
                "{} to {}",
                short(start),
                short(end).get(11..).unwrap_or_default()
            )
        };
        lines.push(format!(
            "  {when}  {}\n    {}",
            event["title"].as_str().unwrap_or("(no title)"),
            event["link"].as_str().unwrap_or_default(),
        ));
    }
    lines.join("\n")
}

fn access_text(listed: &Value) -> String {
    let entries = listed["data"].as_array().cloned().unwrap_or_default();
    let mut lines = vec!["Google Drive connections: site admins, and".to_owned()];
    if entries.is_empty() {
        lines.push("  no one else".into());
    }
    for entry in &entries {
        lines.push(format!(
            "  {}",
            entry["email"]
                .as_str()
                .or_else(|| entry["account"].as_str())
                .unwrap_or_default()
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listing_says_what_is_connected_and_how_to_connect() {
        let off = list_text(&json!({"google": {"status": "not_connected"}, "sources": []}));
        assert!(
            off.contains("openagents connections connect google"),
            "{off}"
        );
        assert!(
            off.contains("openagents projects sources add PROJECT LINK"),
            "{off}"
        );
        let on = list_text(&json!({
            "google": {"status": "connected", "email": "chris@openagents.com", "sheets": true},
            "sources": [{"name": "Finances", "kind": "folder", "link": "https://drive.google.com/drive/folders/f1", "project": "prj_a"}],
        }));
        assert!(
            on.starts_with("Google: connected as chris@openagents.com, with Sheets"),
            "{on}"
        );
        assert!(
            on.contains("  Finances  folder  https://drive.google.com/drive/folders/f1  (prj_a)"),
            "{on}"
        );
    }

    #[test]
    fn the_consent_link_is_printed_with_what_to_do() {
        let text = consent_text(
            &json!({"url": "https://openagents.com/auth/google/connect?scope=drive"}),
            false,
        );
        assert!(text.contains("https://openagents.com/auth/google/connect?scope=drive"));
        assert!(text.contains("logged in to openagents.com"));
    }

    #[test]
    fn mail_and_events_are_listed_with_their_links() {
        let mail = gmail_text(&json!({"result": {"messages": [
            {"date": "Tue, 14 Oct 2026", "from": "A <a@b.co>", "subject": "Hi", "link": "https://mail.google.com/mail/#all/t1", "unread": true}
        ]}}));
        assert_eq!(
            mail,
            "* Tue, 14 Oct 2026  A <a@b.co>  Hi\n  https://mail.google.com/mail/#all/t1"
        );
        let events = calendar_text(
            &json!({"result": {"time_zone": "America/Chicago", "events": [
                {"title": "Standup", "start": "2026-10-15T10:00:00-05:00", "end": "2026-10-15T10:15:00-05:00", "link": "https://calendar.google.com/e1"}
            ]}}),
        );
        assert!(
            events.contains("2026-10-15 10:00 to 10:15  Standup"),
            "{events}"
        );
        assert_eq!(
            gmail_text(&json!({"result": {"messages": []}})),
            "No messages match."
        );
    }

    #[test]
    fn a_query_value_is_encoded() {
        assert_eq!(encode("bank statements/2026"), "bank%20statements%2F2026");
    }
}
