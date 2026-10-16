//! Site administrators can remove abusive listings or clear a report hold.
use super::*;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Moderation {
    request_id: String,
    expected_revision: u64,
    remove: bool,
}
pub(super) async fn moderate(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<Moderation>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let (service, token, account) = match crate::projects::api::caller(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if !service.app_viewer(&token).await.is_ok_and(|v| v.admin) {
        return refused(
            StatusCode::FORBIDDEN,
            "denied",
            "Only a site administrator can review listings.",
        );
    }
    let Ok(Json(r)) = body else {
        return invalid("Check the review and try again.");
    };
    let owner = crate::chat_store::account_owner(&account);
    let fingerprint =
        crate::chat_store::digest(&serde_json::to_vec(&("moderate", &id, &r)).unwrap_or_default());
    mutate(
        &app.config.chat_store,
        DIRECTORY,
        &owner,
        &r.request_id,
        &fingerprint,
        |b| {
            let versions = b.templates.get_mut(&id).ok_or_else(missing)?;
            let old = versions.last().ok_or_else(missing)?;
            if old.listing.version != r.expected_revision {
                return Err(conflict());
            }
            let mut next = old.clone();
            next.listing.version += 1;
            next.listing.removed = r.remove;
            if !r.remove {
                b.reports.remove(&id);
            }
            let result = json!(next.listing);
            versions.push(next);
            Ok(result)
        },
    )
    .await
}
