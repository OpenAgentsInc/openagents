//! The homepage: a composer that starts a chat at `/chat/{uuid}`. The
//! header links `/download`; the Grid's screenshot is on `/docs/the-grid`.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::layout::{escape, page};

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/", get(home))
}

/// `2500` as `$25`, `2550` as `$25.50`, nothing for zero or none.
fn credit(cents: Option<u64>) -> Option<String> {
    match cents? {
        0 => None,
        cents if cents % 100 == 0 => Some(format!("${}", cents / 100)),
        cents => Some(format!("${}.{:02}", cents / 100, cents % 100)),
    }
}

/// The credit a new account starts with, when the service gives one.
fn credit_line(credit: Option<&str>) -> String {
    credit
        .map(|credit| {
            format!(
                "<p>Every new account starts with {} of credit.</p>",
                escape(credit)
            )
        })
        .unwrap_or_default()
}

async fn home(State(app): State<App>) -> Response {
    let credit = credit(app.config.backend.new_account_credit_cents());
    let mut response = page(
        "OpenAgents",
        None,
        &format!(
            "{}{}",
            credit_line(credit.as_deref()),
            super::chat::composer("/chat", "Start a chat")
        ),
    );
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(super::chat::COMPOSER_POLICY),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_credit_line_reads_whole_dollars_when_it_can() {
        assert_eq!(credit(Some(2500)).as_deref(), Some("$25"));
        assert_eq!(credit(Some(2550)).as_deref(), Some("$25.50"));
        assert_eq!(credit(Some(5)).as_deref(), Some("$0.05"));
        assert_eq!(credit(Some(0)), None);
        assert_eq!(credit(None), None);
        assert!(credit_line(Some("$25")).contains("Every new account starts with $25 of credit."));
        assert!(credit_line(None).is_empty());
    }
}
