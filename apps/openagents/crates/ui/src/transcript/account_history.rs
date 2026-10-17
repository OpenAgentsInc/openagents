//! Account-only loading for the session transcript, without an engine connection.
use super::*;
use crate::attachments::{AttachmentSnapshot, CachedAttachmentImage};
use crate::image_media::MediaImage;
use gpui_tokio::Tokio;
use openagents_account::{history::LoadState, reader::Reader};

pub(super) struct AccountHistory {
    model: Arc<Reader>,
    epoch: u64,
    images: HashMap<String, Option<MediaImage>>,
    order: VecDeque<String>,
    failed: HashSet<String>,
    /// Lab card rows (#11883), by row.
    labs: HashMap<SharedString, Entity<crate::lab_view::LabView>>,
}
impl Transcript {
    pub fn for_account_history(
        state: Entity<AppState>,
        model: Arc<Reader>,
        doc_id: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut view = Self::for_doc(state, doc_id, true, cx);
        view.account_history = Some(AccountHistory {
            epoch: model.snapshot().epoch,
            model,
            images: HashMap::new(),
            order: VecDeque::new(),
            failed: HashSet::new(),
            labs: HashMap::new(),
        });
        cx.on_release(|view: &mut Self, cx| {
            if let Some(source) = &mut view.account_history {
                crate::image_media::release_media(
                    source.images.drain().filter_map(|(_, image)| image),
                    cx,
                );
            }
        })
        .detach();
        view
    }

    pub fn reveal_account_message(&mut self, id: &str, cx: &mut Context<Self>) {
        self.sync(cx);
        if let Some(index) = self
            .rows
            .iter()
            .position(|row| row.entry_id.as_ref().starts_with(id))
        {
            self.pinned = false;
            self.list.scroll_to_reveal_item(index);
            cx.notify();
        }
    }

    pub(super) fn load_older_account_messages(&mut self, cx: &mut Context<Self>) {
        let Some(source) = &self.account_history else {
            return;
        };
        let snapshot = source.model.snapshot();
        if self.list.logical_scroll_top().item_ix < 4
            && snapshot.has_more
            && snapshot.state == LoadState::Ready
        {
            let job = Tokio::spawn(cx, source.model.more());
            cx.spawn(async move |_, _| {
                let _ = job.await;
            })
            .detach();
        }
    }

    pub(super) fn account_attachment(
        &mut self,
        path: &str,
        cx: &mut Context<Self>,
    ) -> AttachmentSnapshot {
        let source = self.account_history.as_mut().unwrap();
        let snapshot = source.model.snapshot();
        if source.epoch != snapshot.epoch || !source.model.current() || snapshot.access_lost {
            crate::image_media::release_media(
                source.images.drain().filter_map(|(_, image)| image),
                cx,
            );
            source.order.clear();
            source.failed.clear();
            source.epoch = snapshot.epoch;
        }
        if !source.model.current() || snapshot.access_lost {
            return AttachmentSnapshot::Error {
                retry_in: Duration::from_secs(60),
            };
        }
        if source.failed.contains(path) {
            return AttachmentSnapshot::Error {
                retry_in: Duration::from_secs(60),
            };
        }
        if let Some(image) = source.images.get(path) {
            return image.as_ref().map_or(AttachmentSnapshot::Loading, |image| {
                AttachmentSnapshot::Loaded(CachedAttachmentImage {
                    name: "Image".into(),
                    image: image.image.clone(),
                })
            });
        }
        while source.images.len() >= 16 {
            let Some(old) = source.order.pop_front() else {
                break;
            };
            if let Some(Some(image)) = source.images.remove(&old) {
                crate::image_media::release_media([image], cx);
            }
        }
        let model = source.model.clone();
        let epoch = snapshot.epoch;
        let path = path.to_owned();
        source.images.insert(path.clone(), None);
        source.order.push_back(path.clone());
        let requested = path.clone();
        let job = Tokio::spawn(cx, async move { model.attachment(&requested).await });
        cx.spawn(async move |this, cx| {
            let bytes = job.await.ok().and_then(Result::ok);
            let image = cx
                .background_executor()
                .spawn(async move {
                    bytes.and_then(|bytes| {
                        crate::image_media::decode_image("application/octet-stream", bytes).ok()
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let Some(source) = &mut this.account_history else {
                    return;
                };
                let snapshot = source.model.snapshot();
                if snapshot.epoch == epoch
                    && source.model.current()
                    && !snapshot.access_lost
                    && source.images.contains_key(&path)
                {
                    if let Some(image) = image {
                        source.images.insert(path, Some(image));
                    } else {
                        source.images.remove(&path);
                        if source.failed.len() >= 16 {
                            source.failed.clear();
                        }
                        source.failed.insert(path);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
        AttachmentSnapshot::Loading
    }

    /// A saved chat's Lab card row (#11883): `lab-ui`'s card, drawn by
    /// [`crate::lab_view::LabView`], for a `lab-card:` visual part. `None`
    /// for any other visual.
    pub(super) fn render_lab_row(
        &mut self,
        row_id: &SharedString,
        path: &str,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !openagents_account::lab_cards::is_card_path(path) {
            return None;
        }
        let Some(source) = self.account_history.as_mut() else {
            return Some(div().into_any_element());
        };
        // A followed chat's card changes as its job runs: draw the new one,
        // keeping its pictures and a playing video when its files didn't change.
        let saved = source.model.lab_card(path);
        let cached = source.labs.get(row_id).cloned();
        if let (Some(view), Some(saved)) = (&cached, &saved)
            && view.read(cx).card() != &saved.card
        {
            let card = saved.card.clone();
            view.update(cx, |view, cx| view.set_card(card, cx));
        }
        let view = match cached {
            Some(view) => view,
            None => {
                let model = source.model.clone();
                let Some(saved) = saved else {
                    return Some(div().into_any_element());
                };
                let page = model.lab_page(path);
                let fetch: crate::lab_view::Fetch = {
                    let model = model.clone();
                    let path = path.to_owned();
                    Rc::new(move |id: String, cx: &mut gpui::App| {
                        let model = model.clone();
                        let path = path.clone();
                        let job =
                            Tokio::spawn(cx, async move { model.lab_media(&path, &id).await });
                        cx.spawn(async move |_| job.await.ok().flatten())
                    })
                };
                let current: crate::lab_view::Current = {
                    let model = model.clone();
                    Rc::new(move || model.current() && !model.snapshot().access_lost)
                };
                let space = saved.document_ref().space().to_owned();
                let id = row_id.clone();
                let view = cx.new(|cx| {
                    crate::lab_view::LabView::new(id, saved.card, &space, page, fetch, current, cx)
                });
                let id = row_id.clone();
                cx.subscribe(
                    &view,
                    move |this, _, _: &crate::lab_view::LabLayoutChanged, cx| {
                        this.diagram_layout_changed(std::slice::from_ref(&id), cx);
                    },
                )
                .detach();
                self.follow_lab_row(&view, &model, path, cx);
                if let Some(source) = self.account_history.as_mut() {
                    source.labs.insert(row_id.clone(), view.clone());
                }
                view
            }
        };
        Some(div().w_full().child(view).into_any_element())
    }

    /// Follows a Lab row's document while the row exists (#11943), redraws
    /// the row when its card changes, and lets it add notes.
    fn follow_lab_row(
        &mut self,
        view: &Entity<crate::lab_view::LabView>,
        model: &Arc<Reader>,
        path: &str,
        cx: &mut Context<Self>,
    ) {
        let stop = tokio_util::sync::CancellationToken::new();
        {
            let (model, path, stop) = (model.clone(), path.to_owned(), stop.clone());
            Tokio::spawn(cx, async move { model.follow_lab(&path, &stop).await }).detach();
        }
        let add_note: crate::lab_view::AddNote = {
            let (model, path) = (model.clone(), path.to_owned());
            Rc::new(move |at, text: String, cx: &mut gpui::App| {
                let (model, path) = (model.clone(), path.clone());
                let job = Tokio::spawn(
                    cx,
                    async move { model.lab_add_note(&path, at, &text).await },
                );
                cx.spawn(async move |_| match job.await {
                    Ok(result) => result.map_err(|e| e.message()),
                    Err(_) => Err(openagents_account::lab_cards::NoteError::Unavailable.message()),
                })
            })
        };
        view.update(cx, |view, _| {
            view.set_follower(stop.clone());
            view.set_add_note(add_note);
        });
        let mut updates = model.watch();
        let (model, path, weak) = (model.clone(), path.to_owned(), view.downgrade());
        cx.spawn(async move |_, cx| {
            while updates.changed().await.is_ok() && !stop.is_cancelled() {
                let Some(saved) = model.lab_card(&path) else {
                    continue;
                };
                if weak
                    .update(cx, |view, cx| view.set_card(saved.card, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
}
