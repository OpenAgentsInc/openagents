//! Lab rows in live sessions (#11948). A Lab tool that returns a platform
//! card publishes a visual whose script starts with
//! `openagents_account::lab_cards::LIVE_MARKER`; the visual draws the same
//! [`LabView`] a saved chat's card row does, read through the signed-in
//! account by an [`openagents_account::lab_row::LabRow`]. Without a
//! signed-in account the visual runs its script, which shows the title.

use std::rc::Rc;
use std::sync::Arc;

use gpui::{AppContext as _, Context, Entity, SharedString};
use gpui_tokio::Tokio;
use openagents_account::lab_cards::{LiveCard, NoteError};
use openagents_account::lab_row::LabRow;

use crate::lab_view::{AddNote, Current, Fetch, LabLayoutChanged, LabView, ShowVersion, Versions};

/// The live row for `live`, or `None` when no account is signed in.
pub fn row(
    id: SharedString,
    live: LiveCard,
    cx: &mut Context<crate::visuals::VisualFrame>,
) -> Option<Entity<LabView>> {
    let lease = crate::openagents_account::AppAccount::global(cx)?
        .read(cx)
        .controller
        .as_ref()?
        .lease()?;
    let row = LabRow::open(lease, live)?;
    let view = view(id, &row, cx)?;
    cx.subscribe(&view, |_, _, _: &LabLayoutChanged, cx| {
        cx.emit(crate::visuals::VisualLayoutChanged);
    })
    .detach();
    Some(view)
}

/// A [`LabView`] over `row`: its files, notes, versions, and a follower
/// that runs while the view exists.
pub fn view<T: 'static>(
    id: SharedString,
    row: &Arc<LabRow>,
    cx: &mut Context<T>,
) -> Option<Entity<LabView>> {
    let card = row.card()?;
    let fetch: Fetch = {
        let row = row.clone();
        Rc::new(move |artifact: String, cx: &mut gpui::App| {
            let row = row.clone();
            let job = Tokio::spawn(cx, async move { row.media(&artifact).await });
            cx.spawn(async move |_| job.await.ok().flatten())
        })
    };
    let current: Current = {
        let row = row.clone();
        Rc::new(move || row.current())
    };
    let add_note: AddNote = {
        let row = row.clone();
        Rc::new(move |at, text: String, cx: &mut gpui::App| {
            let row = row.clone();
            let job = Tokio::spawn(cx, async move { row.add_note(at, &text).await });
            cx.spawn(async move |_| match job.await {
                Ok(result) => result.map_err(|e| e.message()),
                Err(_) => Err(NoteError::Unavailable.message()),
            })
        })
    };
    let versions: Versions = {
        let row = row.clone();
        Rc::new(move |cx: &mut gpui::App| {
            let row = row.clone();
            let job = Tokio::spawn(cx, async move { row.versions().await });
            cx.spawn(async move |_| job.await.ok().flatten())
        })
    };
    let show: ShowVersion = {
        let row = row.clone();
        Rc::new(move |number, cx: &mut gpui::App| {
            let row = row.clone();
            let job = Tokio::spawn(cx, async move { row.show_version(number).await });
            cx.spawn(async move |_| job.await.unwrap_or(false))
        })
    };
    let page = row.page();
    let space = row.space().to_owned();
    let view = cx.new(|cx| {
        let mut view = LabView::new(id, card, &space, page, fetch, current, cx);
        view.set_add_note(add_note);
        view.set_versions(versions, show);
        view
    });
    let stop = tokio_util::sync::CancellationToken::new();
    {
        let (row, stop) = (row.clone(), stop.clone());
        Tokio::spawn(cx, async move { row.follow(&stop).await }).detach();
    }
    view.update(cx, |view, _| view.set_follower(stop.clone()));
    let mut updates = row.watch();
    let (row, weak) = (row.clone(), view.downgrade());
    cx.spawn(async move |_, cx| {
        while updates.changed().await.is_ok() && !stop.is_cancelled() {
            let Some(card) = row.card() else {
                break;
            };
            if weak.update(cx, |view, cx| view.set_card(card, cx)).is_err() {
                break;
            }
        }
    })
    .detach();
    Some(view)
}
