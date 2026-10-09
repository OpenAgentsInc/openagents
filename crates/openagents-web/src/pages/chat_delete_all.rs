//! Delete all chats (#11038): one confirm step, then every chat of the
//! owner is removed for good. Signed in, that is every chat on the account
//! (opened from Settings); signed out, every chat in this browser (opened
//! from the Archived chats page). A chat still being answered is left, and
//! the page says so.

use super::*;

/// The confirm step and the delete.
pub(crate) const PATH: &str = "/chat/delete-all";

pub(super) fn routes() -> Router<App> {
    Router::new().route(PATH, get(confirm).post(remove))
}

/// How many chats `owner` has saved, or `None` when the store can't say.
pub(crate) async fn saved(app: &App, owner: &str) -> Option<usize> {
    app.config
        .chat_store
        .list(owner)
        .await
        .map(|rows| rows.len())
        .ok()
}

/// Where Cancel and the way back go: Settings when signed in, else the
/// Archived chats page.
fn back(owner: &str) -> &'static str {
    if crate::chat_store::is_account_owner(owner) {
        crate::settings::PAGE
    } else {
        "/chat/archived"
    }
}

/// The sentence on the confirm step.
pub(super) fn question(owner: &str, count: usize) -> String {
    let chats = if count == 1 {
        "your 1 chat".to_owned()
    } else {
        format!("all {count} chats")
    };
    let place = if crate::chat_store::is_account_owner(owner) {
        "on your account"
    } else {
        "in this browser"
    };
    format!("Delete {chats} {place}? This can't be undone.")
}

async fn confirm(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = reader(&app, &headers).await;
    let count = match &owner {
        Some(owner) => match app.config.chat_store.list(owner).await {
            Ok(rows) => rows.len(),
            Err(error) => return unavailable(error),
        },
        None => 0,
    };
    let content = match owner.as_deref().filter(|_| count > 0) {
        None => PageColumn::new(html! {
            (MarkdownRoot::new(html! { p { "You have no saved chats." } }))
            div.oa-page-actions { (crate::ui_page::action_link("Home", "/")) }
        }),
        Some(owner) => PageColumn::new(html! {
            (MarkdownRoot::new(html! { p { (question(owner, count)) } }))
            form method="post" action=(PATH) {
                input type="hidden" name="csrf" value=(csrf(&app, owner));
                div.oa-page-actions {
                    (Button::new("Delete all")
                        .kind(ButtonType::Submit)
                        .color(Color::Danger))
                    (crate::ui_page::action_link("Cancel", back(owner)))
                }
            }
        }),
    };
    let mut page = UiPage::new("Delete all chats")
        .path(PATH)
        .breadcrumb(Breadcrumb::new("Delete all chats"))
        .content(content);
    if let Some(owner) = &owner {
        page = page.sidebar_section(chat_list(&app, owner, None, false, false).await);
    }
    crate::chat_html::protect(page.respond(&headers))
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Form(removal): Form<Removal>,
) -> Response {
    let owner = match validate_form(&app, &headers, &removal.csrf).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match delete_all(&app.config.chat_store, &owner).await {
        Ok(0) => crate::chat_html::protect(Redirect::to("/").into_response()),
        Ok(left) => crate::chat_html::protect(problem(
            StatusCode::CONFLICT,
            "Some chats weren't deleted",
            &if left == 1 {
                "1 chat is still being answered. Delete it when the answer finishes.".to_owned()
            } else {
                format!(
                    "{left} chats are still being answered. Delete them when the answers finish."
                )
            },
            (back(&owner), "Back"),
        )),
        Err(error) => {
            eprintln!("openagents-web: delete all chats: {error}");
            crate::chat_html::protect(problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "Chats not deleted",
                "We couldn't delete your chats right now. Try again.",
                (PATH, "Try again"),
            ))
        }
    }
}

/// Remove every chat of `owner`, each fenced by the generation it was read
/// at (a chat that changes meanwhile is read again). A chat whose answer is
/// still being written stays. Returns how many chats stayed.
pub(super) async fn delete_all(
    store: &crate::chat_store::Store,
    owner: &str,
) -> Result<usize, Error> {
    let rows = store.list(owner).await?;
    let mut left = 0;
    for chat in rows {
        let mut removed = false;
        for _ in 0..3 {
            let Some(loaded) = store.load(owner, &chat.id).await? else {
                removed = true;
                break;
            };
            let stale = match &loaded.conversation.pending {
                Some(pending) if now().saturating_sub(pending.started_unix) <= LEASE_SECONDS => {
                    break;
                }
                Some(pending) => Some(pending.request_id.clone()),
                None => None,
            };
            match store.delete(owner, &chat.id, &loaded.generation).await {
                Ok(_) => {
                    if let Some(request_id) = stale {
                        let _ = store.release(owner, &request_id).await;
                    }
                    removed = true;
                    break;
                }
                Err(Error::Conflict) => continue,
                Err(error) => return Err(error),
            }
        }
        if !removed {
            left += 1;
        }
    }
    Ok(left)
}
