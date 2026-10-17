//! `/contact`: the contact form, sales inquiries first.
//!
//! A topic dropdown (a sales inquiry by default, or `?topic=` from a link),
//! name, work email, company, and a message. Each request is kept in the
//! private chat store (`contact-requests/YYYY-MM/<id>.json`, the site's own
//! database) and mailed to us through Resend, with the sender as the reply
//! address, so a reply goes straight to them. The record says whether the
//! mail went; a request is never lost because mail failed.
//!
//! Spam: a field people never see must stay empty, and one address may send
//! a few requests an hour.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, Render, html};
use openagents_ui::actions::{Alert, Button, ButtonLink, ButtonType, Color};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::{Field, Input, InputType, Select, Textarea};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::App;
use crate::ui_page::UiPage;

pub(crate) const CONTACT: &str = "/contact";

/// Where requests are mailed.
const TO: &str = "chris@openagents.com";
/// The sender, on our verified Resend domain.
const FROM: &str = "OpenAgents <contact@openagents.com>";
/// The Resend API key, from Secret Manager in production.
const KEY_VAR: &str = "OPENAGENTS_RESEND_API_KEY";
const RESEND: &str = "https://api.resend.com/emails";

/// The topics, in the dropdown's order: value and label. The first is the
/// default.
pub(crate) const TOPICS: [(&str, &str); 9] = [
    ("sales", "Sales inquiry"),
    ("investor", "Investor inquiry"),
    ("feedback", "Customer feedback"),
    ("bug", "Bug report"),
    ("support", "Account or billing help"),
    ("partnership", "Partnership"),
    ("press", "Press or media"),
    ("security", "Security report"),
    ("other", "Something else"),
];

/// Requests one address may send an hour.
const PER_HOUR: usize = 5;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(CONTACT, get(page).post(send))
}

#[derive(Deserialize, Default)]
struct TopicQuery {
    #[serde(default)]
    topic: String,
}

fn topic_or_default(topic: &str) -> &'static str {
    // Older links name Enterprise sales.
    let topic = if topic == "enterprise" {
        "sales"
    } else {
        topic
    };
    TOPICS
        .iter()
        .find(|(value, _)| *value == topic)
        .map_or(TOPICS[0].0, |(value, _)| value)
}

fn label_of(topic: &str) -> &'static str {
    TOPICS
        .iter()
        .find(|(value, _)| *value == topic)
        .map_or(TOPICS[0].1, |(_, label)| label)
}

#[derive(Deserialize, Default, Clone)]
struct ContactForm {
    #[serde(default)]
    topic: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    company: String,
    #[serde(default)]
    message: String,
    /// Hidden from people; a bot fills it.
    #[serde(default)]
    website: String,
}

/// One kept request.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct Request {
    pub(crate) id: String,
    pub(crate) topic: String,
    pub(crate) name: String,
    pub(crate) email: String,
    pub(crate) company: String,
    pub(crate) message: String,
    pub(crate) created_unix: u64,
    /// Whether the mail to us went out.
    pub(crate) mailed: bool,
    /// The consented link click this browser held, when it had one.
    /// Absent when there was no cookie. Never the link token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) attribution: Option<crate::attribution::cookie::ContactClick>,
}

async fn page(headers: HeaderMap, Query(query): Query<TopicQuery>) -> Response {
    respond(
        &headers,
        StatusCode::OK,
        &ContactForm {
            topic: topic_or_default(&query.topic).to_owned(),
            ..ContactForm::default()
        },
        None,
    )
}

fn respond(
    headers: &HeaderMap,
    status: StatusCode,
    form: &ContactForm,
    problem: Option<&str>,
) -> Response {
    UiPage::new("Contact us")
        .path(CONTACT)
        .description("Talk to us about OpenAgents for your company, or anything else.")
        .scriptless()
        .status(status)
        .content(PageColumn::new(body(form, problem)).render())
        .respond(headers)
}

fn body(form: &ContactForm, problem: Option<&str>) -> Markup {
    let topic = topic_or_default(&form.topic);
    let mut select = Select::new("topic").id("contact-topic").required(true);
    for (value, label) in TOPICS {
        select = select.option(value, label);
    }
    let select = select.selected(topic);
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Contact us" }
            p { "Tell us what you need. We answer within one business day." }
        }))
        @if let Some(problem) = problem {
            (Alert::new().color(Color::Danger).description(problem))
        }
        form.oa-contact-form method="post" action=(CONTACT) data-slot="contact-form" {
            (Field::new("contact-topic", "What is it about?").control(select))
            (Field::new("contact-name", "Name").required(true).control(
                Input::new("name").id("contact-name").value(&form.name).maxlength(120).required(true).autocomplete("name")
            ))
            (Field::new("contact-email", "Work email").required(true).control(
                Input::new("email").id("contact-email").input_type(InputType::Email).value(&form.email).maxlength(200).required(true).autocomplete("email")
            ))
            (Field::new("contact-company", "Company").optional(true).control(
                Input::new("company").id("contact-company").value(&form.company).maxlength(160).autocomplete("organization")
            ))
            (Field::new("contact-message", "How can we help?").required(true).control(
                Textarea::new("message").id("contact-message").rows(6).value(&form.message).maxlength(5000).required(true)
                    .placeholder("What you need, and for a company: your team, where you'd run it, and any security needs")
            ))
            div.oa-contact-trap aria-hidden="true" {
                label { "Leave this empty" input type="text" name="website" tabindex="-1" autocomplete="off"; }
            }
            (Button::new("Send").kind(ButtonType::Submit))
        }
    }
}

/// A new request id: `ctr_` and 16 random letters and digits.
fn new_id() -> String {
    let random: String = crate::cloud::byo::fresh_request()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .take(16)
        .collect();
    format!("ctr_{random}")
}

fn thanks(headers: &HeaderMap, topic: &str) -> Response {
    UiPage::new("Thanks")
        .path(CONTACT)
        .scriptless()
        .content(
            PageColumn::new(html! {
                (MarkdownRoot::new(html! {
                    h1 { "Thanks, we got it" }
                    p { "Your " (label_of(topic).to_lowercase()) " request is with us. We answer within one business day, by email." }
                }))
                (ButtonLink::new("Back to OpenAgents", "/"))
            })
            .render(),
        )
        .respond(headers)
}

/// The sender's address for the hourly limit: the first forwarded hop.
fn sender(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default()
}

/// Whether `who` may send another request now.
fn allowed(who: &str) -> bool {
    static SEEN: OnceLock<Mutex<HashMap<String, Vec<Instant>>>> = OnceLock::new();
    let Ok(mut seen) = SEEN.get_or_init(Default::default).lock() else {
        return true;
    };
    let now = Instant::now();
    let times = seen.entry(who.to_owned()).or_default();
    times.retain(|at| now.duration_since(*at) < Duration::from_secs(3600));
    if times.len() >= PER_HOUR {
        return false;
    }
    times.push(now);
    true
}

/// The problem with `form`, in plain words, or `None` when it can go.
fn problem(form: &ContactForm) -> Option<&'static str> {
    let email = form.email.trim();
    if form.name.trim().is_empty() || form.message.trim().is_empty() || email.is_empty() {
        return Some("Fill in your name, email and message.");
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Some("That email address doesn't look right.");
    };
    if local.is_empty()
        || !domain.contains('.')
        || email.contains(char::is_whitespace)
        || email.len() > 200
    {
        return Some("That email address doesn't look right.");
    }
    if form.name.len() > 120 || form.company.len() > 160 || form.message.len() > 5000 {
        return Some("That's longer than the form takes.");
    }
    None
}

async fn send(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<ContactForm>,
) -> Response {
    // A bot filled the field people never see: thank it and keep nothing.
    if !form.website.is_empty() {
        return thanks(&headers, topic_or_default(&form.topic));
    }
    if let Some(problem) = problem(&form) {
        return respond(&headers, StatusCode::BAD_REQUEST, &form, Some(problem));
    }
    if !allowed(&sender(&headers)) {
        return respond(
            &headers,
            StatusCode::TOO_MANY_REQUESTS,
            &form,
            Some("You've sent a few already. Try again in an hour, or email chris@openagents.com."),
        );
    }
    let mut request = Request {
        id: new_id(),
        topic: topic_or_default(&form.topic).to_owned(),
        name: form.name.trim().to_owned(),
        email: form.email.trim().to_owned(),
        company: form.company.trim().to_owned(),
        message: form.message.trim().to_owned(),
        created_unix: crate::chat_store::now_unix(),
        mailed: false,
        attribution: crate::attribution::ticket(&app, &headers).map(|ticket| ticket.contact()),
    };
    request.mailed = mail(&request).await;
    if let Err(error) = keep(&app.config.chat_store, &request).await {
        eprintln!(
            "openagents-web: contact request {} not kept: {error}",
            request.id
        );
        if !request.mailed {
            return respond(
                &headers,
                StatusCode::SERVICE_UNAVAILABLE,
                &form,
                Some("We couldn't take that just now. Try again, or email chris@openagents.com."),
            );
        }
    }
    thanks(&headers, &request.topic)
}

fn key_of(request: &Request) -> String {
    let month = month_of(request.created_unix);
    format!("contact-requests/{month}/{}.json", request.id)
}

/// `YYYY-MM` of a Unix time (UTC), from the civil-from-days algorithm.
fn month_of(unix: u64) -> String {
    let days = (unix / 86_400) as i64 + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}")
}

/// Keep `request` in the chat store.
pub(crate) async fn keep(
    store: &crate::chat_store::Store,
    request: &Request,
) -> Result<(), crate::chat_store::Error> {
    let bytes = serde_json::to_vec(request)
        .map_err(|_| crate::chat_store::Error::Invalid("The request could not be saved."))?;
    store.write_key(&key_of(request), bytes, None).await?;
    Ok(())
}

/// The mail to us, as Resend takes it.
fn mail_body(request: &Request) -> serde_json::Value {
    let company = if request.company.is_empty() {
        String::new()
    } else {
        format!(" ({})", request.company)
    };
    let came = request
        .attribution
        .as_ref()
        .map(came_through)
        .unwrap_or_default();
    json!({
        "from": FROM,
        "to": [TO],
        "reply_to": request.email,
        "subject": format!("{}: {}{company}", label_of(&request.topic), request.name),
        "text": format!(
            "{}\n\nFrom: {} <{}>\nCompany: {}\nTopic: {}\nRequest: {}\n{came}",
            request.message,
            request.name,
            request.email,
            if request.company.is_empty() { "-" } else { &request.company },
            label_of(&request.topic),
            request.id,
        ),
    })
}

/// One plain line for the mail, when the sender followed a link.
fn came_through(click: &crate::attribution::cookie::ContactClick) -> String {
    if let Some(code) = &click.code {
        format!("Came through our {code} link.\n")
    } else if let Some(reference) = &click.affiliate {
        format!("Came through an affiliate link.\nReference: {reference}\n")
    } else {
        "Came through a shared link.\n".to_owned()
    }
}

/// Mail `request` to us; false when there is no key or Resend refused.
async fn mail(request: &Request) -> bool {
    let Ok(key) = std::env::var(KEY_VAR) else {
        return false;
    };
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    else {
        return false;
    };
    match client
        .post(RESEND)
        .bearer_auth(key.trim())
        .json(&mail_body(request))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            eprintln!(
                "openagents-web: contact mail for {} refused: {}",
                request.id,
                response.status()
            );
            false
        }
        Err(_) => {
            eprintln!(
                "openagents-web: contact mail for {} didn't send",
                request.id
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> ContactForm {
        ContactForm {
            topic: "sales".into(),
            name: "Ada".into(),
            email: "ada@example.com".into(),
            company: "Analytical".into(),
            message: "We'd like to run it on our own cloud.".into(),
            website: String::new(),
        }
    }

    #[test]
    fn enterprise_sales_is_the_default_and_a_link_picks_another() {
        assert_eq!(topic_or_default(""), "sales");
        assert_eq!(topic_or_default("nonsense"), "sales");
        assert_eq!(topic_or_default("enterprise"), "sales");
        assert_eq!(topic_or_default("press"), "press");
        let page = body(&ContactForm::default(), None).into_string();
        for (value, label) in TOPICS {
            assert!(page.contains(&format!("value=\"{value}\"")), "{value}");
            assert!(page.contains(label), "{label}");
        }
        assert!(page.contains("name=\"website\""));
        crate::copy_guard::assert_plain("/contact", &page);
    }

    #[test]
    fn a_form_is_checked_before_it_goes() {
        assert_eq!(problem(&filled()), None);
        let mut empty = filled();
        empty.message = " ".into();
        assert!(problem(&empty).is_some());
        let mut bad = filled();
        bad.email = "ada at example".into();
        assert!(problem(&bad).is_some());
    }

    #[test]
    fn the_mail_replies_to_the_sender_and_says_what_it_is_about() {
        let request = Request {
            id: "r1".into(),
            topic: "sales".into(),
            name: "Ada".into(),
            email: "ada@example.com".into(),
            company: "Analytical".into(),
            message: "Hello".into(),
            created_unix: 1_792_000_000,
            mailed: false,
            attribution: None,
        };
        let mail = mail_body(&request);
        assert_eq!(mail["to"][0], TO);
        assert_eq!(mail["reply_to"], "ada@example.com");
        assert_eq!(mail["subject"], "Sales inquiry: Ada (Analytical)");
        assert!(mail["text"].as_str().unwrap().contains("Hello"));
        assert!(key_of(&request).starts_with("contact-requests/2026-"));
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_792_000_000), "2026-10");
    }

    #[test]
    fn a_contact_request_keeps_the_consented_click_and_not_the_token() {
        use crate::attribution::cookie::{Basis, ContactClick, Ticket};
        let token = "rfr_00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let ticket = Ticket {
            code: None,
            token: Some(token.into()),
            series: Some("launch".into()),
            click: format!("clk_{}", "ab".repeat(16)),
            at: 1_700_000_000,
            basis: Basis::Consent,
        };
        let click = ticket.contact();
        assert!(click.affiliate.as_ref().unwrap().starts_with("aff-"));
        let mut request = Request {
            id: "ctr_a".into(),
            topic: "sales".into(),
            name: "Ada".into(),
            email: "ada@example.com".into(),
            company: String::new(),
            message: "Hello".into(),
            created_unix: 1_792_000_000,
            mailed: false,
            attribution: Some(click),
        };
        let stored = serde_json::to_string(&request).unwrap();
        assert!(!stored.contains(token));
        assert!(stored.contains("affiliate"));
        let mail = mail_body(&request)["text"].as_str().unwrap().to_owned();
        assert!(mail.contains("Came through an affiliate link."));
        assert!(mail.contains("Reference: aff-"));
        assert!(!mail.contains(token));
        request.attribution = Some(ContactClick {
            link: "go".into(),
            code: Some("yt-cost".into()),
            series: None,
            click: format!("clk_{}", "cd".repeat(16)),
            basis: "notice".into(),
            affiliate: None,
        });
        assert!(
            mail_body(&request)["text"]
                .as_str()
                .unwrap()
                .contains("Came through our yt-cost link.")
        );
        let older = r#"{"id":"ctr_old","topic":"sales","name":"A","email":"a@b.co","company":"","message":"Hi","created_unix":1,"mailed":false}"#;
        let decoded: Request = serde_json::from_str(older).unwrap();
        assert!(decoded.attribution.is_none());
    }

    #[test]
    fn one_address_sends_a_few_an_hour() {
        let who = format!("test-{}", new_id());
        for _ in 0..PER_HOUR {
            assert!(allowed(&who));
        }
        assert!(!allowed(&who));
    }

    #[tokio::test]
    async fn a_request_is_kept_in_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::chat_store::Store::local(dir.path().to_path_buf());
        let request = Request {
            id: new_id(),
            topic: "support".into(),
            name: "Ada".into(),
            email: "ada@example.com".into(),
            company: String::new(),
            message: "Hi".into(),
            created_unix: crate::chat_store::now_unix(),
            mailed: false,
            attribution: None,
        };
        keep(&store, &request).await.unwrap();
        let (bytes, _) = store.read_key(&key_of(&request)).await.unwrap().unwrap();
        let kept: Request = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(kept, request);
    }
}
