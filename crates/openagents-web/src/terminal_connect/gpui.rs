//! JSON for Settings' terminal forms. The cookie bridge checks every write;
//! approval and sync also check the existing operation-specific ticket.

use super::*;
use axum::Json;
use serde_json::json;

pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/settings/connection-features"
        || path == "/v1/terminal"
        || path.starts_with("/v1/terminal/")
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/settings/connection-features", get(features))
        .route("/v1/terminal", get(read))
        .route("/v1/terminal/approve", post(decide))
        .route("/v1/terminal/sync", post(sync))
}

async fn signed_in<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let service = service(app)?;
    let viewer = service.authenticate(headers).await.map_err(refused)?;
    Ok((service, viewer))
}

async fn features(State(app): State<App>, headers: HeaderMap) -> Response {
    let (_, viewer) = match signed_in(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    crate::coder_sync::answer(
        StatusCode::OK,
        json!({
            "connections": crate::connections::access::permitted(&app, &viewer),
            "vault": true,
            "terminal": true,
        }),
    )
}

fn object(
    headers: &HeaderMap,
    service: &CloudSession,
    viewer: &Viewer,
    step: &Step,
    since: u64,
) -> serde_json::Value {
    let mut value = json!({
        "since": since,
        "install": install_commands(headers)[0].1,
        "login": login_command(&service.pair_code(viewer), local_origin(headers).as_deref()),
        "waiting": [],
        "csrf": choose_csrf(service, headers, viewer, step),
    });
    match step {
        Step::SignIn(waiting) => {
            value["waiting"] = json!(
                waiting
                    .iter()
                    .take(20)
                    .map(|w| json!({"code":w.code,"app":w.app,"computer":w.computer,"csrf":w.csrf}))
                    .collect::<Vec<_>>()
            )
        }
        Step::Choose { computer } => value["computer"] = json!(computer),
        Step::Connected {
            computer,
            choice,
            chats,
        } => {
            value["computer"] = json!(computer);
            value["choice"] = json!(choice.as_str());
            value["chats"] = json!(chats);
        }
    }
    value
}

async fn read(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer) = match signed_in(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let since = query.since.unwrap_or_else(crate::cloud::session::now);
    match step(&app, service, &headers, &viewer, query.computer, since).await {
        Ok(step) => crate::coder_sync::answer(
            StatusCode::OK,
            object(&headers, service, &viewer, &step, since),
        ),
        Err(error) => refused(error),
    }
}

async fn decide(
    State(app): State<App>,
    headers: HeaderMap,
    Json(form): Json<ApproveForm>,
) -> Response {
    let (service, viewer) = match signed_in(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let Some(code) = oa_auth::device::normalize_user_code(&form.code) else {
        return refused(SessionError::InvalidRequest);
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        "terminal-approve",
        &code,
        &form.csrf,
    ) {
        return refused(error);
    }
    let approve = match form.decision.as_str() {
        "approve" => true,
        "deny" => false,
        _ => return refused(SessionError::InvalidRequest),
    };
    match service.device_decide(&headers, &code, approve).await {
        Ok(Ok(request)) => crate::coder_sync::answer(
            StatusCode::OK,
            json!({"approved":approve,"computer":if approve { Some(request.computer) } else { None }}),
        ),
        Ok(Err(_)) => crate::coder_sync::refused(
            StatusCode::BAD_REQUEST,
            "expired",
            "That login expired. Run the command again.",
        ),
        Err(error) => refused(error),
    }
}

async fn sync(
    State(app): State<App>,
    headers: HeaderMap,
    Json(form): Json<ChooseForm>,
) -> Response {
    let (service, viewer) = match signed_in(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let computer = line(&form.computer, 64);
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        "terminal-sync",
        &computer,
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(choice) = SyncChoice::parse(&form.choice) else {
        return refused(SessionError::InvalidRequest);
    };
    match crate::coder_sync::choose(
        &app.config.chat_store,
        &account_owner(&viewer.account_id),
        &computer,
        choice,
    )
    .await
    {
        Ok(Ok(choice)) => {
            crate::coder_sync::answer(StatusCode::OK, json!({"choice":choice.as_str()}))
        }
        Ok(Err(_)) => refused(SessionError::InvalidRequest),
        Err(_) => refused(SessionError::Unavailable),
    }
}

/// Serve the GPUI shell at an existing Settings address when requested.
/// Its start script chooses the page from the address. HTML stays available.
pub(crate) async fn shell(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    path: &str,
) -> Option<Response> {
    let choice = crate::pages::gpui_chat::choice(app.config.features, headers, query);
    if !choice.serves(app, headers) {
        return None;
    }
    let fallback = html_fallback(path, query);
    let mut response = crate::pages::gpui_chat::settings_page(app, headers, &fallback).await;
    choice.remember(app, &mut response);
    Some(response)
}

fn html_fallback(path: &str, query: Option<&str>) -> String {
    let mut fallback = url::form_urlencoded::Serializer::new(String::new());
    if let Some(query) = query {
        fallback.extend_pairs(
            url::form_urlencoded::parse(query.as_bytes())
                .filter(|(key, _)| key != "ui" && key != "page"),
        );
    }
    fallback.append_pair("ui", "html");
    format!("{path}?{}", fallback.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::tests::{Browser, world};
    use axum::body::Body;
    use axum::http::Request;

    #[tokio::test]
    async fn terminal_json_requires_sign_in_same_origin_and_both_tickets() {
        assert!(crate::upstream::owned("/v1/terminal"));
        assert!(crate::upstream::owned("/v1/settings/connection-features"));
        let world = world().await;
        let mut browser = Browser::default();
        assert_ne!(
            browser.get(&world, "/v1/terminal").await.status,
            StatusCode::OK
        );
        browser.sign_in(&world, "octo-local").await;
        let answer = browser
            .send(
                &world,
                Request::get("/v1/terminal").header("sec-fetch-site", "same-origin"),
                Body::empty(),
            )
            .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
        let value: serde_json::Value = serde_json::from_str(&answer.body).unwrap();
        assert!(
            value["login"]
                .as_str()
                .unwrap()
                .contains("coder login --pair")
        );
        assert_eq!(value["waiting"], json!([]));
        let cross = browser
            .send(
                &world,
                Request::post("/v1/terminal/approve")
                    .header("sec-fetch-site", "cross-site")
                    .header(header::CONTENT_TYPE, "application/json"),
                Body::from(r#"{"csrf":"wrong","code":"BCDF-GHJK","decision":"approve"}"#),
            )
            .await;
        assert_eq!(cross.status, StatusCode::FORBIDDEN);
        let missing = browser
            .send(
                &world,
                Request::post("/v1/terminal/sync")
                    .header("sec-fetch-site", "same-origin")
                    .header(header::CONTENT_TYPE, "application/json"),
                Body::from(r#"{"csrf":"wrong","computer":"scratch","choice":"all"}"#),
            )
            .await;
        assert_eq!(missing.status, StatusCode::FORBIDDEN);
    }
    fn page_csrf(html: &str) -> String {
        html.match_indices(r#"name="csrf" value=""#)
            .map(|(at, _)| &html[at + 19..])
            .find_map(|rest| {
                let end = rest.find('"')?;
                rest[end..]
                    .starts_with(r#"" form="chat-form""#)
                    .then(|| rest[..end].to_owned())
            })
            .expect("chat CSRF")
    }

    async fn api(
        browser: &mut Browser,
        world: &crate::projects::tests::World,
        method: &str,
        path: &str,
        csrf: &str,
        value: serde_json::Value,
    ) -> crate::projects::tests::Answer {
        browser
            .send(
                world,
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("sec-fetch-site", "same-origin")
                    .header(header::ORIGIN, world.origin())
                    .header(crate::cookie_api::CSRF_HEADER, csrf)
                    .header(header::CONTENT_TYPE, "application/json"),
                Body::from(value.to_string()),
            )
            .await
    }

    #[tokio::test]
    async fn paired_terminal_can_be_approved_denied_and_choose_sync_in_gpui() {
        let world = world().await;
        let mut browser = Browser::default();
        browser.sign_in(&world, "octo-local").await;
        let page = browser.get(&world, "/chat?ui=html").await;
        let csrf = page_csrf(&page.body);
        let initial = api(&mut browser, &world, "GET", "/v1/terminal", "", json!(null)).await;
        let initial: serde_json::Value = serde_json::from_str(&initial.body).unwrap();
        let pair = initial["login"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap();
        for approve in [false, true] {
            let started = api(
                &mut browser,
                &world,
                "POST",
                "/v1/device/code",
                "",
                json!({"app":"Coder","computer":"Scratch","pair":pair}),
            )
            .await;
            assert_eq!(started.status, StatusCode::OK, "{}", started.body);
            let started: serde_json::Value = serde_json::from_str(&started.body).unwrap();
            let waiting = api(&mut browser, &world, "GET", "/v1/terminal", "", json!(null)).await;
            assert_eq!(waiting.status, StatusCode::OK, "{}", waiting.body);
            let waiting: serde_json::Value = serde_json::from_str(&waiting.body).unwrap();
            let pending = waiting["waiting"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["code"] == started["user_code"])
                .expect("paired request");
            let mut body = json!({"code":pending["code"], "csrf":"wrong", "decision":if approve {"approve"} else {"deny"}});
            let rejected = api(
                &mut browser,
                &world,
                "POST",
                "/v1/terminal/approve",
                &csrf,
                body.clone(),
            )
            .await;
            assert_ne!(rejected.status, StatusCode::OK);
            body["csrf"] = pending["csrf"].clone();
            let decided = api(
                &mut browser,
                &world,
                "POST",
                "/v1/terminal/approve",
                &csrf,
                body,
            )
            .await;
            assert_eq!(decided.status, StatusCode::OK, "{}", decided.body);
            let decided: serde_json::Value = serde_json::from_str(&decided.body).unwrap();
            assert_eq!(decided["approved"], approve);
        }
        let connected = api(
            &mut browser,
            &world,
            "GET",
            "/v1/terminal?computer=Scratch",
            "",
            json!(null),
        )
        .await;
        let connected: serde_json::Value = serde_json::from_str(&connected.body).unwrap();
        let chosen = api(
            &mut browser,
            &world,
            "POST",
            "/v1/terminal/sync",
            &csrf,
            json!({"computer":"Scratch","csrf":connected["csrf"],"choice":"all"}),
        )
        .await;
        assert_eq!(chosen.status, StatusCode::OK, "{}", chosen.body);
        let connected = api(
            &mut browser,
            &world,
            "GET",
            "/v1/terminal?computer=Scratch",
            "",
            json!(null),
        )
        .await;
        let connected: serde_json::Value = serde_json::from_str(&connected.body).unwrap();
        assert_eq!(connected["choice"], "all");
    }
}
