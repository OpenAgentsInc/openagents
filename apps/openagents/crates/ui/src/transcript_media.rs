//! The desktop transcript's media: attachment tiles read from the chat's
//! computer, generated images, inline visuals and saved chats' Lab cards,
//! Mermaid diagrams, tool-call image previews, diff bodies, and the
//! lightbox and image menu that open them.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement, Pixels, Point, SharedString, Styled, StyledImage as _, Task, Window, div, img,
    prelude::*, px, size,
};
use gpui_tokio::Tokio;
use openagents_account::{history::LoadState, reader::Reader};

use crate::attachments::{AttachmentSnapshot, CachedAttachmentImage};
use crate::image_media::MediaImage;
use crate::markdown::mermaid_cache::{self, MermaidCache};
use crate::markdown::render;
use crate::motion;
use crate::theme::Theme;
use crate::transcript::host::TranscriptHost;
use crate::transcript::*;

/// The desktop's transcript.
pub type View = TranscriptView<DesktopMedia>;

pub(crate) struct AccountHistory {
    model: Arc<Reader>,
    epoch: u64,
    images: HashMap<String, Option<MediaImage>>,
    order: VecDeque<String>,
    failed: HashSet<String>,
    /// Lab card rows (#11883), by row.
    labs: HashMap<SharedString, Entity<crate::lab_view::LabView>>,
}
pub struct DesktopMedia {
    /// Transcript attachment being viewed full-size (click a user thumbnail).
    pub(crate) attachment_preview: Option<crate::attachments::PreviewImage>,
    /// Focused while the lightbox is open so Escape reaches it.
    pub(crate) attachment_preview_focus: gpui::FocusHandle,
    pub(crate) attachment_preview_return_focus: Option<gpui::FocusHandle>,
    /// Mermaid fences drawn as diagrams in settled Markdown blocks. Each
    /// row's media handler requests its fences while laying out, so only
    /// painted diagrams are rendered (see [`MermaidCache`]).
    pub(crate) diagrams: Rc<RefCell<MermaidCache>>,
    /// The media handler over `diagrams`, built once and shared by every row.
    pub(crate) diagram_media: Option<render::MediaUi>,
    /// The single serialized diagram render loop, while requests remain.
    pub(crate) diagram_worker: Option<Task<()>>,
    /// Prepared diagram shown in the lightbox: its natural size frames the
    /// enlarged raster, which is released when the lightbox closes.
    pub(crate) diagram_zoom: Option<crate::image_media::MediaImage>,
    /// Expanded tool chips' image previews; freed shortly after collapse.
    pub(crate) tool_images: crate::tool_images::ToolImages,
    /// Right-click menu over any transcript image (Copy path / Copy image).
    pub(crate) image_menu: crate::popover::Popup<ImageMenu>,
    pub(crate) image_menu_focus: gpui::FocusHandle,
    pub(crate) image_copy: Option<Task<()>>,
    /// In-flight ReadAttachmentChunk loads, keyed by device, path and validation
    /// policy; results land in the global attachment cache.
    pub(crate) attachment_loads: HashMap<crate::attachments::AttachmentKey, Task<()>>,
    /// Inline `show_visual` visuals, keyed by row id.
    pub(crate) visuals: HashMap<SharedString, Entity<crate::visuals::VisualFrame>>,
    /// Scheduled retry wake-ups for errored sources (the 2s→15s ladder).
    pub(crate) attachment_retries: HashMap<(String, String), Task<()>>,
    pub(crate) account_history: Option<AccountHistory>,
}

impl TranscriptMedia for DesktopMedia {
    type ToolImages = Vec<(String, crate::tool_images::Thumb)>;

    fn new(cx: &mut Context<View>) -> Self {
        Self {
            attachment_preview: None,
            attachment_preview_focus: cx.focus_handle(),
            attachment_preview_return_focus: None,
            diagrams: Rc::default(),
            diagram_media: None,
            diagram_worker: None,
            diagram_zoom: None,
            tool_images: Default::default(),
            image_menu: Default::default(),
            image_menu_focus: cx.focus_handle(),
            image_copy: None,
            attachment_loads: HashMap::new(),
            visuals: HashMap::new(),
            attachment_retries: HashMap::new(),
            account_history: None,
        }
    }

    fn released(this: &mut View, cx: &mut App) {
        Self::close_diagram_zoom(this, cx);
        crate::image_media::release_media(this.media.diagrams.borrow_mut().drain(), cx);
        this.media.tool_images.clear(cx);
    }

    fn chat_changed(this: &mut View, cx: &mut Context<View>) {
        this.media.tool_images.clear(cx);
    }

    fn rows_built(this: &mut View, new_rows: &[Row]) {
        // Diagram source toggles follow the same stable row identity.
        if this.media.diagrams.borrow().has_source_toggles() {
            let row_ids: HashSet<&str> = new_rows.iter().map(|row| row.id.as_ref()).collect();
            this.media.diagrams.borrow_mut().retain_rows(&row_ids);
        }
    }

    fn rows_changed(this: &mut View, cx: &mut Context<View>) {
        Self::refresh_protected_attachments(this, cx);
        Self::release_departed_tool_images(this, cx);
    }

    fn begin_frame(this: &mut View, column: f32, window: &mut Window, cx: &mut Context<View>) {
        // Release gpui-side decoded copies of any images the attachment LRU
        // evicted since the last frame (no-op when nothing was evicted).
        crate::attachments::flush_evicted(Some(window), cx);
        // Diagram requests are scoped to paint passes; rasters follow the
        // reading column and display density (a no-op while both hold).
        let released_diagrams = {
            let mut diagrams = this.media.diagrams.borrow_mut();
            let mut released = diagrams.begin_frame(crate::theme::style_generation());
            released.extend(diagrams.set_view((column.max(1.0), 480.0), window.scale_factor()));
            released
        };
        if !released_diagrams.is_empty() {
            crate::image_media::release_media(released_diagrams, cx);
        }
    }

    fn after_row(this: &mut View, cx: &mut Context<View>) {
        // Diagram fences this row just requested start rendering after layout.
        Self::ensure_diagram_worker(this, cx);
    }

    fn scrolled_near_top(this: &mut View, cx: &mut Context<View>) {
        Self::load_older_account_messages(this, cx);
    }

    fn user_attachments(
        this: &mut View,
        row_id: &SharedString,
        attachments: &[crate::attachments::UserImageAttachment],
        window: &Window,
        cx: &mut Context<View>,
    ) -> AnyElement {
        Self::render_user_attachments(this, row_id, attachments, window, cx)
    }

    fn sent_chip_overlays(
        this: &mut View,
        row_id: &SharedString,
        mentions: &[crate::composer::SentMentionSpan],
        theme: &Theme,
        cx: &mut Context<View>,
    ) -> Vec<ChipOverlay> {
        Self::chip_overlays(this, row_id, mentions, theme, cx)
    }

    fn open_sent_chip_image(
        this: &mut View,
        path: &SharedString,
        window: &mut Window,
        cx: &mut Context<View>,
    ) {
        Self::open_chip_image(this, path, window, cx)
    }

    fn generated_image(
        this: &mut View,
        row_id: &SharedString,
        owner: &str,
        path: &str,
        name: &str,
        mime_type: &str,
        cx: &mut Context<View>,
    ) -> AnyElement {
        Self::render_generated_image(this, row_id, owner, path, name, mime_type, cx)
    }

    fn visual(
        this: &mut View,
        row_id: &SharedString,
        owner: &str,
        path: &str,
        title: &str,
        cx: &mut Context<View>,
    ) -> AnyElement {
        Self::render_visual(this, row_id, owner, path, title, cx)
    }

    fn markdown_media(this: &mut View, cx: &Context<View>) -> Option<render::MediaUi> {
        Some(Self::diagram_media(this, cx))
    }

    fn release_tool_images(this: &mut View, row_id: &SharedString, cx: &mut Context<View>) {
        this.media
            .tool_images
            .release_row(row_id, Self::tool_images_of, cx);
    }

    fn hold_tool_images(
        this: &mut View,
        row_id: &SharedString,
        tools: &[ToolItem],
        opens: &[bool],
        folds: &[FoldState],
        cx: &mut Context<View>,
    ) -> Vec<Option<Self::ToolImages>> {
        Self::hold_chip_images(this, row_id, tools, opens, folds, cx)
    }

    fn tool_image_strip(
        this: &View,
        chip: &SharedString,
        images: Self::ToolImages,
        collapses: bool,
        theme: &Theme,
        cx: &mut Context<View>,
    ) -> AnyElement {
        Self::render_tool_image_strip(this, chip, images, collapses, theme, cx)
    }

    fn overlays(this: &mut View, window: &mut Window, cx: &mut Context<View>) -> Vec<AnyElement> {
        let mut overlays: Vec<AnyElement> = Self::render_image_menu(this, cx).into_iter().collect();
        // Full-size viewer for a clicked user-bubble thumbnail
        // (AttachmentPreviewDialog: bare lightbox, click closes).
        if let Some(preview) = this.media.attachment_preview.clone() {
            let weak = cx.weak_entity();
            // A diagram's enlarged raster is framed by its natural size.
            let natural_size = this
                .media
                .diagram_zoom
                .as_ref()
                .map(|source| size(px(source.width), px(source.height)));
            overlays.push(crate::attachments::lightbox_with_size(
                window,
                &preview,
                &this.media.attachment_preview_focus,
                natural_size,
                move |window, cx| {
                    if let Ok(focus) = weak.update(cx, |this, cx| {
                        Self::close_diagram_zoom(this, cx);
                        this.media.attachment_preview = None;
                        cx.notify();
                        this.media.attachment_preview_return_focus.take()
                    }) && let Some(focus) = focus
                    {
                        window.focus(&focus, cx);
                    }
                },
                cx,
            ));
        }
        overlays
    }

    fn diff_body(
        file: &crate::changes::FileDiff,
        highlights: Option<Arc<crate::changes::DiffHighlights>>,
        theme: &Theme,
    ) -> AnyElement {
        crate::changes::render_file_body_with_syntax(file, highlights, theme)
    }
}

impl DesktopMedia {
    pub fn account_transcript(
        host: Rc<dyn TranscriptHost>,
        model: Arc<Reader>,
        doc_id: String,
        cx: &mut Context<View>,
    ) -> View {
        let mut view = View::for_doc(host, doc_id, true, cx);
        view.media.account_history = Some(AccountHistory {
            epoch: model.snapshot().epoch,
            model,
            images: HashMap::new(),
            order: VecDeque::new(),
            failed: HashSet::new(),
            labs: HashMap::new(),
        });
        cx.on_release(|view: &mut View, cx| {
            if let Some(source) = &mut view.media.account_history {
                crate::image_media::release_media(
                    source.images.drain().filter_map(|(_, image)| image),
                    cx,
                );
            }
        })
        .detach();
        view
    }

    pub fn reveal_account_message(this: &mut View, id: &str, cx: &mut Context<View>) {
        this.sync(cx);
        if let Some(index) = this
            .rows
            .iter()
            .position(|row| row.entry_id.as_ref().starts_with(id))
        {
            this.pinned = false;
            this.list.scroll_to_reveal_item(index);
            cx.notify();
        }
    }

    pub(crate) fn load_older_account_messages(this: &mut View, cx: &mut Context<View>) {
        let Some(source) = &this.media.account_history else {
            return;
        };
        let snapshot = source.model.snapshot();
        if this.list.logical_scroll_top().item_ix < 4
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

    pub(crate) fn account_attachment(
        this: &mut View,
        path: &str,
        cx: &mut Context<View>,
    ) -> AttachmentSnapshot {
        let source = this.media.account_history.as_mut().unwrap();
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
                let Some(source) = &mut this.media.account_history else {
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
    pub(crate) fn render_lab_row(
        this: &mut View,
        row_id: &SharedString,
        path: &str,
        cx: &mut Context<View>,
    ) -> Option<AnyElement> {
        if !openagents_account::lab_cards::is_card_path(path) {
            return None;
        }
        let Some(source) = this.media.account_history.as_mut() else {
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
                let editor = model.video_editor(path);
                let id = row_id.clone();
                let view = cx.new(|cx| {
                    crate::lab_view::LabView::new(id, saved.card, &space, page, fetch, current, cx)
                        .with_video_editor(editor)
                });
                let id = row_id.clone();
                cx.subscribe(
                    &view,
                    move |this, _, _: &crate::lab_view::LabLayoutChanged, cx| {
                        this.diagram_layout_changed(std::slice::from_ref(&id), cx);
                    },
                )
                .detach();
                Self::follow_lab_row(this, &view, &model, path, cx);
                if let Some(source) = this.media.account_history.as_mut() {
                    source.labs.insert(row_id.clone(), view.clone());
                }
                view
            }
        };
        Some(div().w_full().child(view).into_any_element())
    }

    /// Follows a Lab row's document while the row exists (#11943), redraws
    /// the row when its card changes, and lets it add notes.
    pub(crate) fn follow_lab_row(
        _this: &mut View,
        view: &Entity<crate::lab_view::LabView>,
        model: &Arc<Reader>,
        path: &str,
        cx: &mut Context<View>,
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
        let versions: crate::lab_view::Versions = {
            let (model, path) = (model.clone(), path.to_owned());
            Rc::new(move |cx: &mut gpui::App| {
                let (model, path) = (model.clone(), path.clone());
                let job = Tokio::spawn(cx, async move { model.lab_versions(&path).await });
                cx.spawn(async move |_| job.await.ok().flatten())
            })
        };
        let show: crate::lab_view::ShowVersion = {
            let (model, path) = (model.clone(), path.to_owned());
            Rc::new(move |number, cx: &mut gpui::App| {
                let (model, path) = (model.clone(), path.clone());
                let job = Tokio::spawn(
                    cx,
                    async move { model.lab_show_version(&path, number).await },
                );
                cx.spawn(async move |_| job.await.unwrap_or(false))
            })
        };
        view.update(cx, |view, _| {
            view.set_follower(stop.clone());
            view.set_add_note(add_note);
            view.set_versions(versions, show);
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

    /// Shield the open transcript's attachments from image-cache eviction —
    /// rebuilt on every row sync so a chat switch swaps the set. Without it,
    /// budget pressure evicted thumbnails still on screen (the list caches
    /// rendered rows, so a visible image's LRU tick goes stale).
    pub(crate) fn refresh_protected_attachments(this: &View, cx: &Context<View>) {
        // The protected set is GLOBAL and replaced wholesale — an override
        // instance writing it would clobber the primary transcript's keys.
        if this.doc_override.is_some() {
            return;
        }
        crate::attachments::protect_attachments(Self::protected_attachment_keys(this, cx));
    }

    pub(crate) fn protected_attachment_keys(
        this: &View,
        cx: &Context<View>,
    ) -> HashSet<(String, String)> {
        let devices = Self::attachment_device_ids(this, cx);
        let mut keys = std::collections::HashSet::new();
        for row in &this.rows {
            // Generated images use bounded LRU retention, not history-wide protection.
            if let RowKind::User {
                attachments,
                mentions,
                ..
            } = &row.kind
            {
                let chipped = mentions
                    .iter()
                    .filter(|span| span.kind == crate::composer::ChipKind::Image)
                    .filter_map(|span| span.upload.as_ref().map(|path| path.to_string()));
                for path in attachments
                    .iter()
                    .map(|att| att.path.clone())
                    .chain(chipped)
                {
                    for dev in &devices {
                        keys.insert((dev.clone(), path.clone()));
                    }
                }
            }
        }
        keys
    }

    /// Devices that may own a user message's attachment files: the chat's host
    /// device (uploads targeted it) plus this device (openagents's
    /// `uniqueIds([attachmentDeviceId, m.device_id])`).
    pub(crate) fn attachment_device_ids(this: &View, cx: &Context<View>) -> Vec<String> {
        // `selected_chat_row` belongs to the PRIMARY transcript's chat — an
        // override instance has no chat row, so it claims no devices (its
        // thumbnails degrade to placeholders instead of guessing).
        if this.doc_override.is_some() {
            return Vec::new();
        }
        let mut ids = Vec::new();
        if let Some(place) = this.host.chat_place(None, cx) {
            ids.push(place.device_id);
        }
        if let Some(local) = this.host.local_device_id(cx).map(str::to_owned)
            && !ids.contains(&local)
        {
            ids.push(local);
        }
        ids
    }

    pub(crate) fn generated_attachment_device_ids(
        this: &View,
        owner: &str,
        cx: &Context<View>,
    ) -> Vec<String> {
        let mut fallback = Self::attachment_device_ids(this, cx);
        if let Some(local) = this.host.local_device_id(cx) {
            fallback.push(local.to_owned());
        }
        generated_image_devices(owner, &fallback)
    }

    /// Effective load state for one attachment across its candidate devices:
    /// first Loaded source wins; otherwise loads are (re)claimed and the
    /// snapshot degrades Loading → Error with a scheduled retry wake-up.
    pub(crate) fn attachment_state(
        this: &mut View,
        device_ids: &[String],
        path: &str,
        expected_raster_mime: Option<&str>,
        cx: &mut Context<View>,
    ) -> crate::attachments::AttachmentSnapshot {
        use crate::attachments::{
            AttachmentKey, AttachmentSnapshot, attachment_snapshot_for, begin_load_for,
        };
        if this.media.account_history.is_some() {
            return Self::account_attachment(this, path, cx);
        }
        for dev in device_ids {
            if let AttachmentSnapshot::Loaded(image) =
                attachment_snapshot_for(&AttachmentKey::new(dev, path, expected_raster_mime))
            {
                return AttachmentSnapshot::Loaded(image);
            }
        }
        let mut any_loading = false;
        let mut min_retry: Option<Duration> = None;
        for dev in device_ids {
            if begin_load_for(&AttachmentKey::new(dev, path, expected_raster_mime)) {
                Self::spawn_attachment_load(
                    this,
                    dev.clone(),
                    path.to_string(),
                    expected_raster_mime.map(str::to_owned),
                    cx,
                );
            }
            match attachment_snapshot_for(&AttachmentKey::new(dev, path, expected_raster_mime)) {
                AttachmentSnapshot::Loaded(image) => return AttachmentSnapshot::Loaded(image),
                AttachmentSnapshot::Loading => {
                    // Generated assets try the owner first, falling back only
                    // after failure. Repainting never launches duplicate reads.
                    if expected_raster_mime.is_some() {
                        return AttachmentSnapshot::Loading;
                    }
                    any_loading = true;
                }
                AttachmentSnapshot::Error { retry_in } => {
                    min_retry = Some(min_retry.map_or(retry_in, |m| m.min(retry_in)));
                }
            }
        }
        if any_loading {
            return AttachmentSnapshot::Loading;
        }
        match min_retry {
            Some(retry_in) => {
                if let Some(dev) = device_ids.first() {
                    Self::schedule_attachment_retry(
                        this,
                        (dev.clone(), path.to_string()),
                        retry_in,
                        cx,
                    );
                }
                AttachmentSnapshot::Error { retry_in }
            }
            // No candidate devices at all — the "unavailable" thumb, no retry.
            None => AttachmentSnapshot::Error {
                retry_in: Duration::MAX,
            },
        }
    }

    pub(crate) fn spawn_attachment_load(
        this: &mut View,
        device_id: String,
        path: String,
        expected_raster_mime: Option<String>,
        cx: &mut Context<View>,
    ) {
        use crate::attachments::{
            AttachmentKey, read_attachment_image, store_error_for, store_loaded_for,
        };
        let key = AttachmentKey::new(&device_id, &path, expected_raster_mime.as_deref());
        let Some(engine) = this.host.rpc(cx) else {
            store_error_for(&key);
            return;
        };
        let local = this.host.local_device_id(cx).map(str::to_owned);
        // Relay-forward only for a genuinely remote owner; the local device's
        // files are served directly.
        let target = (local.as_deref() != Some(device_id.as_str())).then(|| device_id.clone());
        let claim = crate::attachments::AttachmentLoadGuard(key.clone());
        let task_key = key.clone();
        let task = cx.spawn(async move |this, cx| {
            let _claim = claim;
            match read_attachment_image(
                &engine,
                cx.background_executor(),
                target.as_deref(),
                &path,
                expected_raster_mime.as_deref(),
            )
            .await
            {
                Some(loaded) => store_loaded_for(&task_key, loaded.name.into(), loaded.image),
                None => store_error_for(&task_key),
            }
            this.update(cx, |transcript, cx| {
                transcript.media.attachment_loads.remove(&task_key);
                cx.notify();
            })
            .ok();
        });
        this.media.attachment_loads.insert(key, task);
    }

    /// One wake-up per errored source: after the backoff elapses, a notify
    /// re-renders the thumb, whose `begin_load` then claims the retry.
    pub(crate) fn schedule_attachment_retry(
        this: &mut View,
        key: (String, String),
        delay: Duration,
        cx: &mut Context<View>,
    ) {
        if delay == Duration::MAX || this.media.attachment_retries.contains_key(&key) {
            return;
        }
        let wake = key.clone();
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(delay + Duration::from_millis(60))
                .await;
            this.update(cx, |transcript, cx| {
                transcript.media.attachment_retries.remove(&wake);
                cx.notify();
            })
            .ok();
        });
        this.media.attachment_retries.insert(key, task);
    }

    /// Open a sent image chip's upload full size, as its thumbnail did. An
    /// image still loading opens on a later click.
    pub(crate) fn open_chip_image(
        this: &mut View,
        path: &SharedString,
        window: &mut Window,
        cx: &mut Context<View>,
    ) {
        let device_ids = Self::attachment_device_ids(this, cx);
        if let crate::attachments::AttachmentSnapshot::Loaded(image) =
            Self::attachment_state(this, &device_ids, path, None, cx)
        {
            let preview = crate::attachments::PreviewImage::new(image.name, image.image.clone());
            this.media.attachment_preview_return_focus = window.focused(cx);
            preview.viewer.reset();
            this.media.attachment_preview = Some(preview);
            window.focus(&this.media.attachment_preview_focus, cx);
            cx.notify();
        }
    }

    /// Whether an attachment ref is still crossing to the chat's host, and
    /// how far along it is. Two ref shapes mean "still crossing": the queued
    /// flow's `pending://` (bytes ship engine-side after the send; the host
    /// rewrites the ref to an absolute path once they land and the run
    /// starts) and the legacy echo's synthetic `pending/`. Percent sources, in
    /// order: this attachment's own relay transfer (`WatchTransfers`, by the
    /// uploadId its ref names — the leg that actually takes time), else the
    /// send-wide staging/legacy upload percent. Neither → no number, so a
    /// ring never shows one that isn't a real transfer position (2026-08-20
    /// report: the staging-only percent blinked out in ~100ms and lied about
    /// the slow part).
    pub(crate) fn upload_status(this: &View, path: &str, cx: &Context<View>) -> (bool, Option<u8>) {
        let sending = path.starts_with("pending://") || path.starts_with("pending/");
        let upload_id = path
            .strip_prefix("pending://")
            .and_then(|rest| rest.split_once('/'))
            .map(|(id, _)| id);
        let percent = upload_id
            .and_then(|id| this.host.transfer_percent(id, cx))
            .or_else(|| {
                sending
                    .then(|| this.host.upload_progress_percent(cx))
                    .flatten()
            });
        (sending, percent)
    }

    /// What a sent bubble's attachment chips show beyond their label: the
    /// progress of an upload still crossing to the host in place of the icon
    /// (the strip's thumbnails used to carry it), a warning for an image that
    /// can't be read back, and a click target opening a loaded image.
    pub(crate) fn chip_overlays(
        this: &mut View,
        row_id: &SharedString,
        mentions: &[crate::composer::SentMentionSpan],
        theme: &Theme,
        cx: &mut Context<View>,
    ) -> Vec<ChipOverlay> {
        use crate::attachments::AttachmentSnapshot;
        if mentions.iter().all(|span| span.upload.is_none()) {
            return Vec::new();
        }
        let device_ids = Self::attachment_device_ids(this, cx);
        let mut overlays = Vec::new();
        for (ix, span) in mentions.iter().enumerate() {
            let Some(upload) = &span.upload else {
                continue;
            };
            let (sending, percent) = Self::upload_status(this, upload, cx);
            let mut status = sending.then(|| match percent {
                Some(pct) => crate::loaders::upload_progress_arc(
                    pct,
                    CHIP_STATUS_SIZE,
                    theme.text_muted.opacity(0.35),
                    theme.text,
                ),
                None => crate::loaders::mini_glyph_spinner(
                    format!("chip-sending-{row_id}-{ix}"),
                    2.0,
                    theme.glyph,
                    cx.entity_id(),
                    cx,
                )
                .into_any_element(),
            });
            let mut open = None;
            if span.kind == crate::composer::ChipKind::Image {
                // Loading here keeps the image ready for a click, as the
                // strip's thumbnail did.
                match Self::attachment_state(this, &device_ids, upload, None, cx) {
                    AttachmentSnapshot::Loaded(_) => open = Some(upload.clone()),
                    AttachmentSnapshot::Error { .. } if !sending => {
                        status = Some(
                            crate::icons::icon(crate::icons::DANGER_TRIANGLE)
                                .size(px(CHIP_STATUS_SIZE - 2.0))
                                .text_color(theme.text_muted)
                                .into_any_element(),
                        );
                    }
                    _ => {}
                }
            }
            if open.is_some() || status.is_some() {
                overlays.push(ChipOverlay {
                    range: span.range.clone(),
                    open,
                    status,
                });
            }
        }
        overlays
    }

    pub(crate) fn tool_images_of(this: &mut View) -> &mut crate::tool_images::ToolImages {
        &mut this.media.tool_images
    }

    /// The chat whose agent produced these rows (a subagent tab borrows the
    /// selected chat's host and cwd).
    pub(crate) fn image_host(this: &View, cx: &gpui::App) -> (Option<String>, Option<String>) {
        let place = this.host.chat_place(this.chat_id.as_deref(), cx);
        (
            place.as_ref().map(|place| place.device_id.clone()),
            place.and_then(|place| place.cwd),
        )
    }

    /// Hold the previews of every chip whose body is mounted; release the
    /// rest. Returns `(absolute path, thumb)` lists per chip index.
    pub(crate) fn hold_chip_images(
        this: &mut View,
        row_id: &SharedString,
        tools: &[ToolItem],
        opens: &[bool],
        folds: &[FoldState],
        cx: &mut Context<View>,
    ) -> Vec<Option<Vec<(String, crate::tool_images::Thumb)>>> {
        if !tools.iter().any(chip_has_images) {
            return vec![None; tools.len()];
        }
        let (device, cwd) = Self::image_host(this, cx);
        let loader = crate::tool_images::Loader {
            engine: this.host.rpc(cx),
            local_device: this.host.local_device_id(cx).map(str::to_owned),
        };
        tools
            .iter()
            .enumerate()
            .map(|(ix, tool)| {
                if !chip_has_images(tool) {
                    return None;
                }
                let chip = SharedString::from(format!("{row_id}#d{ix}"));
                let tweening = folds[ix].epoch > 0
                    && folds[ix]
                        .toggled_at
                        .is_some_and(|at| at.elapsed() < FOLD_TWEEN_WINDOW);
                if !opens[ix] && !tweening {
                    this.media
                        .tool_images
                        .release(&chip, Self::tool_images_of, cx);
                    return None;
                }
                let keys: Vec<_> = tool
                    .images
                    .iter()
                    .map(|path| crate::tool_images::ImageKey {
                        device: device.clone().unwrap_or_default(),
                        path: crate::tool_images::resolve_path(path, cwd.as_deref()),
                    })
                    .collect();
                let paths: Vec<String> = keys.iter().map(|key| key.path.clone()).collect();
                let loader = if device.is_some() {
                    &loader
                } else {
                    &crate::tool_images::Loader {
                        engine: None,
                        local_device: None,
                    }
                };
                let thumbs =
                    this.media
                        .tool_images
                        .hold(&chip, keys, loader, Self::tool_images_of, cx);
                Some(paths.into_iter().zip(thumbs).collect())
            })
            .collect()
    }

    /// Rows left the transcript (compact fold settled, history rebuilt):
    /// their chips' previews start the release grace.
    pub(crate) fn release_departed_tool_images(this: &mut View, cx: &mut Context<View>) {
        if this.media.tool_images.is_idle() {
            return;
        }
        let live: HashSet<&str> = this
            .rows
            .iter()
            .filter(|row| matches!(row.kind, RowKind::ToolGroup { .. }))
            .map(|row| row.id.as_ref())
            .collect();
        this.media
            .tool_images
            .retain_rows(|row| live.contains(row), Self::tool_images_of, cx);
    }

    pub(crate) fn render_tool_image_strip(
        this: &View,
        chip: &SharedString,
        images: Vec<(String, crate::tool_images::Thumb)>,
        collapses: bool,
        theme: &Theme,
        cx: &mut Context<View>,
    ) -> AnyElement {
        use crate::tool_images::Thumb;
        let devices: Rc<[String]> = Self::image_host(this, cx).0.into_iter().collect();
        let frames = images.into_iter().enumerate().map(|(ix, (path, thumb))| {
            let name = SharedString::from(file_badge_name(&path).to_owned());
            let width = match &thumb {
                Thumb::Ready { aspect, .. } => {
                    (TOOL_IMAGE_HEIGHT * aspect).clamp(48.0, TOOL_IMAGE_MAX_WIDTH)
                }
                _ => TOOL_IMAGE_HEIGHT * 4.0 / 3.0,
            };
            let menu_devices = devices.clone();
            let frame = div()
                .id(SharedString::from(format!("{chip}-img{ix}")))
                .w(px(width))
                .h(px(TOOL_IMAGE_HEIGHT))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .overflow_hidden()
                .border_1()
                .border_color(crate::theme::hairline(0.07))
                .bg(crate::theme::ink(0.035))
                .aria_label(name.clone())
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        Self::open_image_menu(
                            this,
                            path.clone(),
                            menu_devices.to_vec(),
                            event.position,
                            window,
                            cx,
                        );
                    }),
                );
            match thumb {
                // Explicit dims: img layout honors intrinsic aspect over a
                // percent height (see the attachment strip).
                Thumb::Ready { image, .. } => frame.child(
                    img(image)
                        .w(px(width - 2.0))
                        .h(px(TOOL_IMAGE_HEIGHT - 2.0))
                        .rounded(px(7.0))
                        .object_fit(ObjectFit::Contain),
                ),
                Thumb::Loading => frame
                    .text_size(px(TOOL_TEXT_SIZE))
                    .text_color(theme.text_faint)
                    .child("Loading image…"),
                Thumb::Failed => frame
                    .flex_col()
                    .gap(px(4.0))
                    .px(px(12.0))
                    .text_size(px(TOOL_TEXT_SIZE))
                    .text_color(theme.text_faint)
                    .child("Image unavailable")
                    .child(div().max_w_full().truncate().child(name)),
            }
        });
        div()
            .h(px(TOOL_IMAGE_STRIP_HEIGHT))
            .flex_none()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(DETAIL_SEPARATOR))
                    .flex_none()
                    .when(!collapses, |line| line.bg(crate::theme::hairline(0.06))),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{chip}-images")))
                    .h(px(TOOL_IMAGE_HEIGHT + 2.0 * TOOL_IMAGE_PAD))
                    .py(px(TOOL_IMAGE_PAD))
                    .when(!collapses, |strip| strip.px(px(TOOL_IMAGE_PAD)))
                    .flex()
                    .flex_row()
                    .gap(px(TOOL_IMAGE_PAD))
                    .overflow_x_scroll()
                    .children(frames),
            )
            .into_any_element()
    }

    /// Open the image menu at the pointer. `devices` are the candidate owners
    /// of `path`, tried in order when copying the image.
    pub(crate) fn open_image_menu(
        this: &mut View,
        path: String,
        devices: Vec<String>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<View>,
    ) {
        this.media.image_menu.open(ImageMenu {
            path,
            devices,
            position,
            active: None,
        });
        window.focus(&this.media.image_menu_focus, cx);
        cx.notify();
    }

    pub(crate) fn close_image_menu(this: &mut View, cx: &mut Context<View>) {
        if this.media.image_menu.begin_close() {
            crate::popover::reap_popup(cx, |this: &mut View| &mut this.media.image_menu);
            cx.notify();
        }
    }

    pub(crate) fn image_menu_key(
        this: &mut View,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<View>,
    ) {
        use crate::popover::MenuKey;
        let Some(menu) = this.media.image_menu.open_mut() else {
            return;
        };
        match crate::popover::classify_key(
            &event.keystroke.key,
            event.keystroke.modifiers.platform,
            event.keystroke.modifiers.control,
        ) {
            MenuKey::Escape => Self::close_image_menu(this, cx),
            MenuKey::Up => menu.active = crate::popover::menu_step(menu.active, 2, -1),
            MenuKey::Down => menu.active = crate::popover::menu_step(menu.active, 2, 1),
            MenuKey::Enter => {
                if let Some(action) = menu.active {
                    Self::dispatch_image_menu(this, action, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn dispatch_image_menu(this: &mut View, action: usize, cx: &mut Context<View>) {
        let Some(menu) = this.media.image_menu.as_open() else {
            return;
        };
        let (path, devices) = (menu.path.clone(), menu.devices.clone());
        Self::close_image_menu(this, cx);
        match action {
            0 => cx.write_to_clipboard(gpui::ClipboardItem::new_string(path)),
            1 => Self::copy_image(this, path, devices, cx),
            _ => {}
        }
    }

    /// Copy the original file (not the preview) from its owning device.
    pub(crate) fn copy_image(
        this: &mut View,
        path: String,
        devices: Vec<String>,
        cx: &mut Context<View>,
    ) {
        let Some(engine) = this.host.rpc(cx) else {
            return;
        };
        let local = this.host.local_device_id(cx).map(str::to_owned);
        this.media.image_copy = Some(cx.spawn(async move |this, cx| {
            let executor = cx.background_executor().clone();
            for device in &devices {
                let target = (local.as_deref() != Some(device.as_str())).then_some(device.as_str());
                if let Some((_, mime, bytes)) = crate::attachments::read_attachment_bytes(
                    &engine, &executor, target, &path, None,
                )
                .await
                {
                    let format =
                        gpui::ImageFormat::from_mime_type(&mime).unwrap_or(gpui::ImageFormat::Png);
                    // SVG is markup an agent may have written: copy the parsed,
                    // re-serialized tree (no scripts or external resources),
                    // as previews render it.
                    let bytes = if format == gpui::ImageFormat::Svg {
                        let svg = executor.spawn(async move {
                            usvg::Tree::from_data(&bytes, &crate::image_media::svg_options())
                                .map(|tree| tree.to_string(&usvg::WriteOptions::default()))
                        });
                        let Ok(svg) = svg.await else {
                            return;
                        };
                        svg.into_bytes()
                    } else {
                        bytes
                    };
                    let image = gpui::Image::from_bytes(format, bytes);
                    this.update(cx, |this, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_image(&image));
                        this.media.image_copy = None;
                    })
                    .ok();
                    return;
                }
            }
        }));
    }

    pub(crate) fn render_image_menu(this: &View, cx: &mut Context<View>) -> Option<AnyElement> {
        let menu = this.media.image_menu.get()?;
        let theme = Theme::of(cx).for_popup();
        let mut card = crate::popover::popover_card(&theme)
            .id("transcript-image-menu-card")
            .track_focus(&this.media.image_menu_focus)
            .role(gpui::Role::Menu)
            .w(px(190.0))
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event, _, cx| Self::image_menu_key(this, event, cx)))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| Self::close_image_menu(this, cx)))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
        for (index, (label, icon)) in [
            ("Copy path", crate::icons::COPY),
            ("Copy image", crate::icons::FILE_IMAGE),
        ]
        .into_iter()
        .enumerate()
        {
            let id = SharedString::from(format!("transcript-image-menu-{index}"));
            card =
                card.child(
                    crate::popover::menu_row(&theme, menu.active == Some(index), id.clone())
                        .id(id)
                        .role(gpui::Role::MenuItem)
                        .aria_label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            Self::dispatch_image_menu(this, index, cx)
                        }))
                        .child(
                            crate::icons::icon(icon)
                                .size(px(16.0))
                                .text_color(theme.text_muted),
                        )
                        .child(label),
                );
        }
        Some(crate::popover::menu_at(
            "transcript-image-menu",
            menu.position,
            card.into_any_element(),
            this.media.image_menu.closing_since(),
        ))
    }

    /// Media wiring for settled Markdown: Mermaid fences become diagrams
    /// once rendered and keep their source until then (or on failure).
    /// Inline images keep their ordinary chat rendering.
    pub(crate) fn diagram_media(this: &mut View, cx: &Context<View>) -> render::MediaUi {
        let cache = this.media.diagrams.clone();
        let owner = cx.weak_entity();
        let media = this
            .media
            .diagram_media
            .get_or_insert_with(|| render::MediaUi {
                image: None,
                diagram: Some(Rc::new(move |code, id, _theme| {
                    let lookup = cache.borrow_mut().request(code, &id);
                    match lookup {
                        mermaid_cache::Lookup::Pending => render::DiagramView::Source,
                        mermaid_cache::Lookup::Failed(reason) => {
                            render::DiagramView::Failed(reason)
                        }
                        mermaid_cache::Lookup::Ready(media) => {
                            let show_source = cache.borrow().source_visible(&id);
                            let body = if show_source {
                                gpui::Empty.into_any_element()
                            } else {
                                let open = owner.clone();
                                let source = media.clone();
                                crate::image_media::preview_element(
                                    &media,
                                    format!("{id}-image").into(),
                                    move |window, cx| {
                                        let _ = open.update(cx, |this, cx| {
                                            Self::open_diagram_preview(
                                                this,
                                                source.clone(),
                                                window,
                                                cx,
                                            )
                                        });
                                    },
                                )
                            };
                            let toggle = owner.clone();
                            render::DiagramView::Diagram(render::DiagramUi {
                                body,
                                show_source,
                                toggle_source: Rc::new(move |_, cx| {
                                    let _ = toggle.update(cx, |this, cx| {
                                        this.media.diagrams.borrow_mut().toggle_source(&id);
                                        let row = mermaid_cache::frame_row(&id).to_owned();
                                        this.diagram_layout_changed(&[row.into()], cx);
                                    });
                                }),
                            })
                        }
                    }
                })),
            });
        media.clone()
    }

    /// Render requested diagrams one at a time off the UI thread. The loop
    /// picks its next source between renders, so rows that scrolled away in
    /// the meantime are skipped rather than queued.
    pub(crate) fn ensure_diagram_worker(this: &mut View, cx: &mut Context<View>) {
        if this.media.diagram_worker.is_some()
            || !this.media.diagrams.borrow_mut().take_new_requests()
        {
            return;
        }
        this.media.diagram_worker = Some(cx.spawn(async move |this, cx| {
            loop {
                let job = this.update(cx, |this, cx| {
                    let job = this.media.diagrams.borrow_mut().next_job();
                    if job.is_none() {
                        this.media.diagram_worker = None;
                    }
                    job.map(|code| {
                        (
                            code,
                            crate::markdown::mermaid::Palette::from_theme(Theme::of(cx)),
                            crate::theme::style_generation(),
                        )
                    })
                });
                let Ok(Some((code, palette, style))) = job else {
                    return;
                };
                let source = code.clone();
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        let svg = crate::markdown::mermaid::render(&source, &palette)?;
                        crate::image_media::decode_image("image/svg+xml", svg.into_bytes())
                    })
                    .await;
                if this
                    .update(cx, |this, cx| {
                        Self::finish_diagram(this, code, style, result, cx)
                    })
                    .is_err()
                {
                    return;
                }
            }
        }));
    }

    pub(crate) fn finish_diagram(
        this: &mut View,
        code: String,
        style: u32,
        result: Result<crate::image_media::MediaImage, String>,
        cx: &mut Context<View>,
    ) {
        let Some((rows, released)) = this.media.diagrams.borrow_mut().finish(code, style, result)
        else {
            return;
        };
        crate::image_media::release_media(released, cx);
        this.diagram_layout_changed(&rows, cx);
    }

    /// Open a diagram in the shared lightbox, rasterized for the window
    /// within the memory the retained diagrams leave available.
    pub(crate) fn open_diagram_preview(
        this: &mut View,
        source: crate::image_media::MediaImage,
        window: &mut Window,
        cx: &mut Context<View>,
    ) {
        Self::close_diagram_zoom(this, cx);
        let viewport = window.viewport_size();
        let available = mermaid_cache::MAX_RETAINED_BYTES
            .saturating_sub(this.media.diagrams.borrow().retained_bytes());
        let enlarged = source.enlarged(
            (
                f32::from(viewport.width) * 0.9,
                f32::from(viewport.height) * 0.85,
            ),
            window.scale_factor(),
            available,
            None,
        );
        this.media.attachment_preview_return_focus = window.focused(cx);
        this.media.attachment_preview = Some(
            crate::attachments::PreviewImage::new("Mermaid diagram", enlarged.image)
                .with_plate(crate::markdown::mermaid::Palette::plate(Theme::of(cx))),
        );
        this.media.diagram_zoom = Some(source);
        window.focus(&this.media.attachment_preview_focus, cx);
        cx.notify();
    }

    /// Release a diagram lightbox's dedicated raster. The row's own preview
    /// stays with the diagram cache.
    pub(crate) fn close_diagram_zoom(this: &mut View, cx: &mut gpui::App) {
        let Some(source) = this.media.diagram_zoom.take() else {
            return;
        };
        if let Some(preview) = this.media.attachment_preview.take()
            && !Arc::ptr_eq(&preview.image, &source.image)
        {
            cx.defer(move |cx| gpui::ImageSource::Image(preview.image).evict(None, cx));
        }
    }

    pub(crate) fn render_generated_image(
        this: &mut View,
        row_id: &SharedString,
        owner: &str,
        path: &str,
        name: &str,
        mime_type: &str,
        cx: &mut Context<View>,
    ) -> AnyElement {
        use crate::attachments::AttachmentSnapshot;
        let devices = Self::generated_attachment_device_ids(this, owner, cx);
        let state = Self::attachment_state(this, &devices, path, Some(mime_type), cx);
        let theme = Theme::of(cx).clone();
        let frame = div()
            .id(SharedString::from(format!("{row_id}-generated")))
            .w(px(512.0))
            .max_w_full()
            .h(px(320.0))
            .max_h(px(420.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(12.0))
            .overflow_hidden()
            .bg(crate::theme::ink(0.045));
        match state {
            AttachmentSnapshot::Loaded(loaded) => {
                let dimensions =
                    crate::appshots::png_dimensions(&loaded.image.bytes).unwrap_or((512, 320));
                let scale = (512.0 / dimensions.0 as f32)
                    .min(420.0 / dimensions.1 as f32)
                    .min(1.0);
                let preview =
                    crate::attachments::PreviewImage::new(name.to_owned(), loaded.image.clone());
                let menu_path = path.to_owned();
                frame
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            Self::open_image_menu(
                                this,
                                menu_path.clone(),
                                devices.clone(),
                                event.position,
                                window,
                                cx,
                            );
                        }),
                    )
                    .w(px(dimensions.0 as f32 * scale))
                    .h(px(dimensions.1 as f32 * scale))
                    .role(gpui::Role::Button)
                    .aria_label("Preview generated image")
                    .tab_index(0)
                    .cursor_pointer()
                    .focus_visible(move |style| style.border_2().border_color(theme.accent))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.media.attachment_preview_return_focus = window.focused(cx);
                        preview.viewer.reset();
                        this.media.attachment_preview = Some(preview.clone());
                        window.focus(&this.media.attachment_preview_focus, cx);
                        cx.notify();
                    }))
                    .child(
                        gpui::img(loaded.image)
                            .size_full()
                            // The frame's overflow clip is rectangular; round the image itself.
                            .rounded(px(12.0))
                            .object_fit(gpui::ObjectFit::Contain),
                    )
                    .into_any_element()
            }
            AttachmentSnapshot::Loading => frame
                .text_color(theme.text_muted)
                .child("Loading generated image…")
                .into_any_element(),
            AttachmentSnapshot::Error { .. } => frame
                .text_color(theme.text_muted)
                .child("Generated image unavailable")
                .into_any_element(),
        }
    }

    pub(crate) fn render_visual(
        this: &mut View,
        row_id: &SharedString,
        owner: &str,
        path: &str,
        title: &str,
        cx: &mut Context<View>,
    ) -> AnyElement {
        if let Some(row) = Self::render_lab_row(this, row_id, path, cx) {
            return row;
        }
        let link_ui = this.link_ui(cx);
        let frame = match this.media.visuals.get(row_id) {
            Some(frame) => frame.clone(),
            None => {
                let engine = this.host.rpc(cx);
                // Relay-forward only for a genuinely remote owner.
                let target =
                    (this.host.local_device_id(cx) != Some(owner)).then(|| owner.to_owned());
                let frame = cx.new(|cx| {
                    crate::visuals::VisualFrame::new(
                        row_id.clone(),
                        title.to_owned(),
                        path.to_owned(),
                        engine,
                        target,
                        cx,
                    )
                });
                // A visual whose tree changes shape (a tab switch, a load)
                // resizes its row, like a diagram finishing.
                let id = row_id.clone();
                cx.subscribe(
                    &frame,
                    move |this, _, _: &crate::visuals::VisualLayoutChanged, cx| {
                        this.diagram_layout_changed(std::slice::from_ref(&id), cx);
                    },
                )
                .detach();
                this.media.visuals.insert(row_id.clone(), frame.clone());
                frame
            }
        };
        frame.update(cx, |frame, cx| frame.set_link_ui(link_ui, cx));
        div().w_full().child(frame).into_any_element()
    }

    /// The right-aligned thumbnail strip above a user bubble.
    pub(crate) fn render_user_attachments(
        this: &mut View,
        row_id: &SharedString,
        atts: &[crate::attachments::UserImageAttachment],
        _window: &Window,
        cx: &mut Context<View>,
    ) -> AnyElement {
        use crate::attachments::AttachmentSnapshot;
        let glyph = Theme::of(cx).glyph;
        let device_ids = Self::attachment_device_ids(this, cx);
        let mut strip = div()
            .w_full()
            .min_w_0()
            .flex_none()
            .flex()
            .flex_row()
            .flex_wrap()
            .justify_end()
            .items_start()
            .gap(px(8.0))
            .px(px(4.0))
            .pt(px(4.0))
            .pb(px(6.0));
        for (aix, att) in atts.iter().enumerate() {
            if !crate::attachments::is_image_path(&att.path) {
                strip = strip.child(user_file_pill(&att.name, Theme::of(cx)));
                continue;
            }
            let state = Self::attachment_state(this, &device_ids, &att.path, None, cx);
            // The in-flight send's progress belongs ON the thumbnail
            // (2026-08-18 user request). No percent → the indeterminate
            // spinner (staged-but-waiting, retry backoff, or
            // committed-awaiting-rewrite); see `upload_status`.
            let (sending, uploading) = Self::upload_status(this, &att.path, cx);
            if let Some(appshot) = &att.appshot {
                let has_image = matches!(&state, AttachmentSnapshot::Loaded(_));
                let theme = Theme::of(cx).clone();
                let accent = theme.accent;
                let width = 240.0;
                let mut card = div()
                    .id(SharedString::from(format!("{row_id}-appshot-{aix}")))
                    .w(px(width))
                    .max_w_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .items_center()
                    .rounded(px(14.0))
                    .p(px(8.0))
                    .gap(px(6.0))
                    .hover(|style| style.bg(crate::theme::ink(0.045)));
                let image_frame = div()
                    .w_full()
                    .h(px(128.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .overflow_hidden();
                card = match state {
                    AttachmentSnapshot::Loaded(image) => {
                        let preview =
                            crate::attachments::PreviewImage::new(image.name, image.image.clone());
                        card.role(gpui::Role::Button)
                            .aria_label(format!(
                                "Preview {} Appshot: {}",
                                appshot.app_name,
                                appshot.title()
                            ))
                            .tab_index(0)
                            .cursor_pointer()
                            .focus_visible(move |style| style.border_2().border_color(accent))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.media.attachment_preview_return_focus = window.focused(cx);
                                preview.viewer.reset();
                                this.media.attachment_preview = Some(preview.clone());
                                window.focus(&this.media.attachment_preview_focus, cx);
                                cx.notify();
                            }))
                            .child(
                                // Inherit the transcript's scroll fade. GPUI replaces
                                // rather than composes nested edge-fade scopes, so a
                                // decorative thumbnail fade would bypass the chrome fade.
                                image_frame.child(
                                    img(image.image)
                                        .w_full()
                                        .h(px(126.0))
                                        .rounded(px(5.0))
                                        .object_fit(ObjectFit::Contain),
                                ),
                            )
                    }
                    AttachmentSnapshot::Loading => card.child(
                        image_frame.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.text_muted)
                                .child(if sending {
                                    "Uploading Appshot…"
                                } else {
                                    "Loading Appshot…"
                                }),
                        ),
                    ),
                    AttachmentSnapshot::Error { .. } => card.child(
                        image_frame.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(theme.text_muted)
                                .child(if sending {
                                    "Uploading Appshot…"
                                } else {
                                    "Appshot unavailable"
                                }),
                        ),
                    ),
                };
                card = card
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .max_w_full()
                            .child(
                                div()
                                    .size(px(24.0))
                                    .flex_none()
                                    .rounded(px(6.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(match crate::appshots::presentation_icon(appshot) {
                                        Some(icon) => img(icon)
                                            .size(px(24.0))
                                            .object_fit(ObjectFit::Contain)
                                            .into_any_element(),
                                        None => crate::icons::icon(crate::icons::MONITOR)
                                            .size(px(15.0))
                                            .text_color(theme.text_muted)
                                            .into_any_element(),
                                    }),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.0))
                                    .text_color(theme.text_muted)
                                    .child(SharedString::from(format!(
                                        "{} · Appshot",
                                        appshot.app_name
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_center()
                            .text_size(px(12.0))
                            .text_color(theme.text)
                            .child(SharedString::from(appshot.title().to_string())),
                    );
                if sending && (has_image || uploading.is_some()) {
                    card = card.child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.text_muted)
                            .child(SharedString::from(
                                uploading
                                    .map(|pct| format!("Uploading {pct}%"))
                                    .unwrap_or_else(|| "Uploading…".into()),
                            )),
                    );
                }
                strip = strip.child(card);
                continue;
            }
            let frame = div()
                .flex_none()
                .w(px(ATT_THUMB_W))
                .h(px(ATT_THUMB_H))
                .rounded(px(8.0))
                .overflow_hidden();
            let thumb: AnyElement = match state {
                AttachmentSnapshot::Loaded(image) => {
                    let preview = crate::attachments::PreviewImage::new(
                        image.name.clone(),
                        image.image.clone(),
                    );
                    let (menu_path, menu_devices) = (att.path.clone(), device_ids.clone());
                    frame
                        .id(SharedString::from(format!("{row_id}#att{aix}")))
                        .on_mouse_down(
                            gpui::MouseButton::Right,
                            cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                                cx.stop_propagation();
                                Self::open_image_menu(
                                    this,
                                    menu_path.clone(),
                                    menu_devices.clone(),
                                    event.position,
                                    window,
                                    cx,
                                );
                            }),
                        )
                        .relative()
                        .border_1()
                        .border_color(crate::theme::hairline(0.11))
                        .bg(crate::theme::ink(0.035))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.media.attachment_preview_return_focus = window.focused(cx);
                            preview.viewer.reset();
                            this.media.attachment_preview = Some(preview.clone());
                            window.focus(&this.media.attachment_preview_focus, cx);
                            cx.notify();
                        }))
                        .child(
                            img(image.image.clone())
                                // EXPLICIT dims, not size_full: img layout
                                // honors the intrinsic aspect ratio over a
                                // percent height (gpui f8d8a90 repoint), so
                                // size_full let a tall photo grow past the
                                // frame and the rectangular overflow clip
                                // squared the bottom corners (2026-08-19).
                                .w(px(ATT_THUMB_W - 2.0))
                                .h(px(ATT_THUMB_H - 2.0))
                                // The IMG needs its own radii: the frame's
                                // rounding only clips rectangularly, so the
                                // sprite must round its own corners (7 = the
                                // frame's 8 minus its 1px border).
                                .rounded(px(7.0))
                                .object_fit(ObjectFit::Cover),
                        )
                        .when(sending, |el| {
                            // The pulse read registers this entity for frames,
                            // so the overlay stays live even once the trailer's
                            // 30s pending-send bridge has lapsed.
                            let pulse = motion::pulse_wave(motion::pulse_delta(
                                &motion::OPENAGENTS_PULSE,
                                cx.entity_id(),
                                cx,
                            ));
                            let indicator: AnyElement = match uploading {
                                Some(pct) => crate::loaders::upload_progress_ring(pct, 34.0),
                                None => crate::loaders::mini_glyph_spinner(
                                    format!("att-sending-{row_id}-{aix}"),
                                    3.0,
                                    glyph,
                                    cx.entity_id(),
                                    cx,
                                )
                                .into_any_element(),
                            };
                            el.child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .rounded(px(7.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(gpui::hsla(0.0, 0.0, 0.0, 0.38 + 0.05 * pulse))
                                    .child(indicator),
                            )
                        })
                        .into_any_element()
                }
                // Errored/unavailable: the dashed "missing" thumb.
                AttachmentSnapshot::Error { .. } => frame
                    .border_1()
                    .border_dashed()
                    .border_color(crate::theme::hairline(0.14))
                    .bg(crate::theme::ink(0.025))
                    .into_any_element(),
                // Loading: the pulsing skeleton (same wash as popover skeletons).
                AttachmentSnapshot::Loading => frame
                    .border_1()
                    .border_color(crate::theme::hairline(0.08))
                    .bg(crate::theme::ink(0.055))
                    .opacity(
                        0.35 + 0.4
                            * motion::pulse_wave(motion::pulse_delta(
                                &motion::OPENAGENTS_PULSE,
                                cx.entity_id(),
                                cx,
                            )),
                    )
                    .into_any_element(),
            };
            strip = strip.child(thumb);
        }
        strip.into_any_element()
    }
}
