//! The hosted Video Lab player uses the same controls as the local player.
use super::*;
use axum::Json;
use serde_json::json;

const PLAYER: &str = include_str!("../../../../scripts/film/lab_player.html");
const TRANSPORT: &str = include_str!("video_transport.js");

pub(super) fn page(card: &Card, space: &str) -> Response {
    let config = json!({"document": card.document, "space": space});
    let config = script_json(&config);
    let bootstrap =
        format!("<script>window.videoLabConfig={config};\n{TRANSPORT}</script>\n<script>");
    crate::chat_html::protect(Html(PLAYER.replacen("<script>", &bootstrap, 1)).into_response())
}

fn script_json(config: &serde_json::Value) -> String {
    config
        .to_string()
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

pub(super) async fn timeline(
    State(app): State<App>,
    headers: HeaderMap,
    Path((document, version)): Path<(String, u64)>,
    Query(page): Query<Page>,
) -> Result<Response, lab_api::ApiError> {
    let space = page.space();
    let authority = crate::labs_api::caller_headers(&app, &headers);
    let version = crate::labs_api::runtime(&app)
        .read_version(&authority, &space, &document, version)
        .await?;
    if version.schema != "cut.v1" {
        return Err(lab_api::ApiError::missing());
    }
    let value = video_lab::timeline::view(&version.content, &json!({})).map_err(|message| {
        lab_api::ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "timeline", &message)
    })?;
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_card_links_keep_the_same_space() {
        let page = Page {
            space: "me".into(),
            v: None,
            project: Some("private".into()),
        };
        assert_eq!(page.space(), "project:private");
        let explicit = Page {
            space: "project:chosen".into(),
            ..page
        };
        assert_eq!(explicit.space(), "project:chosen");
    }

    #[test]
    fn bootstrap_values_cannot_end_a_script_or_add_markup() {
        let value =
            json!({"document":"</script><img src=x onerror=alert(1)>","space":"project:a&b"});
        let escaped = script_json(&value);
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('&'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&escaped).unwrap(),
            value
        );
    }
}
