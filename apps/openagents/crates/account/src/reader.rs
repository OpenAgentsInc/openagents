//! Read-only saved transcripts and one on-demand attachment, with account-bound cancellation.
use crate::{
    history::{ExistingChat, LoadState},
    openagents::{
        AttachmentRef, CancellationToken, ChatSummary, Error, SavedCapture, SavedMessage,
        TranscriptReader,
    },
};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[derive(Clone, Copy)]
pub struct Limits {
    pub bytes: usize,
    pub messages: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 32 * 1024 * 1024,
            messages: 2000,
        }
    }
}
#[derive(Clone)]
pub struct Download {
    pub reference: AttachmentRef,
    pub name: String,
    pub state: LoadState,
    pub bytes: Option<Arc<Vec<u8>>>,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub epoch: u64,
    pub summary: Option<ChatSummary>,
    pub messages: Arc<Vec<SavedMessage>>,
    pub captures: Vec<SavedCapture>,
    /// Zero-based index of the first message held by this view.
    pub first: usize,
    pub received: usize,
    pub has_more: bool,
    pub state: LoadState,
    pub continue_required: bool,
    pub smaller_page: bool,
    pub single_message: bool,
    pub reload_required: bool,
    pub login_required: bool,
    pub access_lost: bool,
    pub download: Option<Download>,
}
struct State {
    epoch: u64,
    target: Option<ExistingChat>,
    reader: Option<TranscriptReader>,
    cancel: CancellationToken,
    file_cancel: CancellationToken,
    file_epoch: u64,
    bytes: usize,
    replace: bool,
    latest_first: bool,
    page_start: usize,
    following: Option<CancellationToken>,
    polling: bool,
    snapshot: Snapshot,
}
pub struct Reader {
    state: Mutex<State>,
    updates: watch::Sender<Snapshot>,
    limits: Limits,
}
impl Reader {
    pub fn new() -> Arc<Self> {
        Self::with_limits(Limits::default())
    }
    pub fn with_limits(limits: Limits) -> Arc<Self> {
        assert!(limits.bytes > 0 && limits.messages > 0);
        let (updates, _) = watch::channel(Snapshot::default());
        Arc::new(Self {
            state: Mutex::new(State {
                epoch: 0,
                target: None,
                reader: None,
                cancel: CancellationToken::new(),
                file_cancel: CancellationToken::new(),
                file_epoch: 0,
                bytes: 0,
                replace: false,
                latest_first: false,
                page_start: 0,
                following: None,
                polling: false,
                snapshot: Snapshot::default(),
            }),
            updates,
            limits,
        })
    }
    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.updates.subscribe()
    }
    pub fn snapshot(&self) -> Snapshot {
        let s = self.state.lock().unwrap();
        if s.target.as_ref().is_some_and(|t| !t.is_current()) {
            return Snapshot {
                epoch: s.epoch,
                ..Snapshot::default()
            };
        }
        s.snapshot.clone()
    }
    pub fn current(&self) -> bool {
        self.state
            .lock()
            .unwrap()
            .target
            .as_ref()
            .is_some_and(ExistingChat::is_current)
    }
    pub fn target(&self) -> Option<ExistingChat> {
        self.state
            .lock()
            .unwrap()
            .target
            .clone()
            .filter(ExistingChat::is_current)
    }
    fn publish(&self, s: &State) {
        self.updates.send_replace(s.snapshot.clone());
    }
    pub fn clear(&self) {
        self.clear_inner();
    }
    fn clear_inner(&self) -> u64 {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.file_cancel.cancel();
        s.following = None;
        s.polling = false;
        s.epoch += 1;
        s.target = None;
        s.reader = None;
        s.bytes = 0;
        s.replace = false;
        s.latest_first = false;
        s.page_start = 0;
        s.snapshot = Snapshot {
            epoch: s.epoch,
            ..Snapshot::default()
        };
        self.publish(&s);
        s.epoch
    }
    /// Accept an existing navigation target independently of any sidebar index.
    pub fn open(&self, target: ExistingChat) -> Result<(), Error> {
        let epoch = self.clear_inner();
        if !target.is_current() {
            return Err(Error::Cancelled);
        }
        let reader = target
            .lease
            .client()
            .transcript(target.lease.session(), &target.chat)?
            .with_page_size(200.min(self.limits.messages))?;
        let mut s = self.state.lock().unwrap();
        if s.epoch != epoch {
            return Err(Error::Cancelled);
        }
        s.cancel = target.lease.cancel.child_token();
        s.file_cancel = s.cancel.child_token();
        s.reader = Some(reader);
        s.snapshot.summary = Some(target.summary.clone());
        s.snapshot.has_more = true;
        s.target = Some(target);
        self.publish(&s);
        Ok(())
    }
    /// Open the newest page, loading older pages when the person scrolls upward.
    pub fn open_latest(&self, target: ExistingChat) -> Result<(), Error> {
        self.open(target.clone())?;
        let mut s = self.state.lock().unwrap();
        if !s
            .target
            .as_ref()
            .is_some_and(|t| t.chat == target.chat && t.is_current())
        {
            return Err(Error::Cancelled);
        }
        if target.summary.revision == 0 || target.summary.messages == 0 {
            s.latest_first = true;
            s.replace = true;
            self.publish(&s);
            return Ok(());
        }
        let start = target
            .summary
            .messages
            .saturating_sub(200.min(self.limits.messages));
        s.reader = Some(
            s.reader
                .take()
                .ok_or(Error::Cancelled)?
                .starting_at(start, target.summary.revision)?,
        );
        s.latest_first = true;
        s.page_start = start;
        s.snapshot.first = start;
        s.snapshot.received = start;
        self.publish(&s);
        Ok(())
    }

    pub fn refresh(&self) -> Result<(), Error> {
        let mut s = self.state.lock().unwrap();
        let target = s
            .target
            .clone()
            .filter(ExistingChat::is_current)
            .ok_or(Error::Cancelled)?;
        let reader = target
            .lease
            .client()
            .transcript(target.lease.session(), &target.chat)?
            .with_page_size(200.min(self.limits.messages))?;
        s.page_start = 0;
        let following = s.following.is_some();
        s.cancel.cancel();
        s.file_cancel.cancel();
        s.cancel = target.lease.cancel.child_token();
        s.following = following.then(|| s.cancel.child_token());
        s.polling = false;
        s.file_cancel = s.cancel.child_token();
        s.epoch += 1;
        s.reader = Some(reader);
        s.replace = true;
        s.snapshot.epoch = s.epoch;
        s.snapshot.download = None;
        s.snapshot.state = LoadState::Idle;
        s.snapshot.has_more = true;
        s.snapshot.continue_required = false;
        s.snapshot.smaller_page = false;
        s.snapshot.login_required = false;
        s.snapshot.access_lost = false;
        s.snapshot.single_message = false;
        s.snapshot.reload_required = false;
        self.publish(&s);
        Ok(())
    }
    /// Release older messages explicitly before reading the next bounded window.
    pub fn continue_window(&self) {
        let mut s = self.state.lock().unwrap();
        if !s.snapshot.continue_required {
            return;
        }
        if s.latest_first {
            s.snapshot.received = s.snapshot.first;
        } else {
            s.snapshot.first = s.snapshot.received;
        }
        s.snapshot.messages = Arc::new(Vec::new());
        s.bytes = 0;
        s.snapshot.continue_required = false;
        s.snapshot.state = LoadState::Idle;
        self.publish(&s);
    }
    pub fn smaller_page(&self) {
        let mut s = self.state.lock().unwrap();
        if !s.snapshot.smaller_page {
            return;
        }
        if s.latest_first {
            let end = if s.snapshot.messages.is_empty() {
                s.snapshot
                    .summary
                    .as_ref()
                    .map_or(0, |summary| summary.messages)
            } else {
                s.snapshot.first
            };
            s.page_start = end.saturating_sub(1);
            if let (Some(target), Some(summary)) = (&s.target, &s.snapshot.summary) {
                s.reader = target
                    .lease
                    .client()
                    .transcript(target.lease.session(), &target.chat)
                    .and_then(|r| r.with_page_size(1))
                    .and_then(|r| r.starting_at(s.page_start, summary.revision))
                    .ok();
            }
        } else {
            s.reader = s.reader.take().and_then(|r| r.with_page_size(1).ok());
        }
        s.snapshot.smaller_page = false;
        s.snapshot.single_message = true;
        s.snapshot.state = LoadState::Idle;
        self.publish(&s);
    }
    pub fn more(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let request = {
            let mut s = self.state.lock().unwrap();
            if !s.snapshot.has_more
                || s.snapshot.state == LoadState::Loading
                || s.snapshot.continue_required
                || s.snapshot.smaller_page
            {
                None
            } else if !s.replace
                && (s.bytes > self.limits.bytes / 2
                    || s.snapshot.messages.len() >= self.limits.messages)
            {
                s.snapshot.continue_required = true;
                s.snapshot.state = LoadState::Ready;
                self.publish(&s);
                None
            } else if let (Some(target), Some(reader)) = (
                s.target.clone().filter(ExistingChat::is_current),
                s.reader.clone(),
            ) {
                s.snapshot.state = LoadState::Loading;
                self.publish(&s);
                Some((
                    s.epoch,
                    target,
                    reader,
                    s.cancel.clone(),
                    s.latest_first,
                    s.replace,
                    s.page_start,
                    if s.snapshot.single_message {
                        1
                    } else {
                        200.min(this.limits.messages)
                    },
                ))
            } else {
                None
            }
        };
        async move {
            let Some((
                epoch,
                target,
                mut reader,
                cancel,
                latest_first,
                replace,
                mut page_start,
                page_size,
            )) = request
            else {
                return;
            };
            let result = async {
                let mut page = reader
                    .next_page(target.lease.client(), target.lease.session(), &cancel)
                    .await?;
                if latest_first
                    && replace
                    && let Some(probe) = &page
                {
                    page_start = probe.thread.messages.saturating_sub(page_size);
                    if page_start > 0 {
                        reader = target
                            .lease
                            .client()
                            .transcript(target.lease.session(), &target.chat)?
                            .with_page_size(page_size)?
                            .starting_at(page_start, probe.revision)?;
                        page = reader
                            .next_page(target.lease.client(), target.lease.session(), &cancel)
                            .await?;
                    }
                }
                Ok::<_, Error>(page)
            }
            .await;
            let mut s = this.state.lock().unwrap();
            if s.epoch != epoch || !target.is_current() || cancel.is_cancelled() {
                return;
            }
            s.page_start = page_start;
            match result {
                Ok(Some(page)) => {
                    let bytes = page
                        .data
                        .iter()
                        .map(message_bytes)
                        .fold(0usize, usize::saturating_add);
                    if bytes > this.limits.bytes {
                        s.snapshot.smaller_page = !s.snapshot.single_message;
                        if s.snapshot.single_message {
                            s.snapshot.has_more = false;
                        }
                        s.snapshot.state = LoadState::Failed {
                            message: if s.snapshot.single_message {
                                "This message is too large to show in this app."
                            } else {
                                "This page is too large to show. Try one message at a time."
                            }
                            .into(),
                            stale: !s.snapshot.messages.is_empty(),
                        };
                    } else if !s.replace
                        && (s.bytes.saturating_add(bytes) > this.limits.bytes
                            || s.snapshot.messages.len() + page.data.len() > this.limits.messages)
                    {
                        s.snapshot.continue_required = true;
                        s.snapshot.state = LoadState::Ready;
                    } else {
                        if s.replace {
                            s.snapshot.messages = Arc::new(Vec::new());
                            s.snapshot.first = s.page_start;
                            s.snapshot.received = s.page_start;
                            s.bytes = 0;
                            s.replace = false;
                        }
                        s.bytes = s.bytes.saturating_add(bytes);
                        if s.latest_first {
                            let mut messages = page.data;
                            s.snapshot.received =
                                s.snapshot.received.max(s.page_start + messages.len());
                            messages.extend(s.snapshot.messages.iter().cloned());
                            s.snapshot.first = s.page_start;
                            s.snapshot.messages = Arc::new(messages);
                        } else {
                            s.snapshot.received += page.data.len();
                            Arc::make_mut(&mut s.snapshot.messages).extend(page.data);
                        }
                        s.snapshot.summary = Some(page.thread);
                        s.snapshot.captures = page.captures;
                        s.snapshot.has_more = if s.latest_first {
                            s.page_start > 0
                        } else {
                            page.has_more
                        };
                        s.snapshot.state = LoadState::Ready;
                        if s.latest_first && s.page_start > 0 {
                            let end = s.page_start;
                            let start = end.saturating_sub(if s.snapshot.single_message {
                                1
                            } else {
                                200.min(this.limits.messages)
                            });
                            s.page_start = start;
                            s.reader = target
                                .lease
                                .client()
                                .transcript(target.lease.session(), &target.chat)
                                .and_then(|r| r.with_page_size(end - start))
                                .and_then(|r| r.starting_at(start, page.revision))
                                .ok();
                        } else {
                            s.reader = Some(reader);
                        }
                    }
                }
                Ok(None) => {
                    s.snapshot.has_more = false;
                    s.snapshot.state = LoadState::Ready;
                }
                Err(e) => {
                    s.snapshot.login_required = login_required(&e);
                    s.snapshot.access_lost = lost_access(&e);
                    s.snapshot.reload_required = matches!(e, Error::Changed | Error::InvalidCursor);
                    if s.snapshot.access_lost || matches!(e, Error::Changed | Error::InvalidCursor)
                    {
                        s.snapshot.messages = Arc::new(Vec::new());
                        s.snapshot.captures.clear();
                        s.snapshot.download = None;
                        s.file_cancel.cancel();
                        s.snapshot.first = 0;
                        s.snapshot.received = 0;
                        s.bytes = 0;
                        if s.snapshot.access_lost {
                            s.snapshot.summary = None;
                        }
                    }
                    s.snapshot.state = LoadState::Failed {
                        message: e.to_string(),
                        stale: !s.snapshot.messages.is_empty(),
                    };
                }
            }
            this.publish(&s);
        }
    }
    /// Follow only a fully loaded, visible transcript. Pausing cancels an in-flight read.
    pub fn set_following(&self, active: bool) {
        let mut s = self.state.lock().unwrap();
        if !active {
            if let Some(cancel) = s.following.take() {
                cancel.cancel();
            }
        } else if s.following.is_none() && s.target.as_ref().is_some_and(ExistingChat::is_current) {
            s.following = Some(s.cancel.child_token());
        }
    }

    /// Re-read changed content atomically, within the same paging and memory bounds.
    /// Old pages stay on screen during a request; account and selection changes cancel it.
    pub fn poll(self: &Arc<Self>) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let request = {
            let mut s = self.state.lock().unwrap();
            if s.polling
                || (!s.latest_first && (s.snapshot.has_more || s.snapshot.first != 0))
                || s.snapshot.continue_required
                || s.snapshot.smaller_page
                || s.snapshot.login_required
                || s.snapshot.access_lost
                || s.snapshot.reload_required
                || s.snapshot.state == LoadState::Loading
            {
                None
            } else if let (Some(target), Some(cancel)) = (
                s.target.clone().filter(ExistingChat::is_current),
                s.following.clone().filter(|c| !c.is_cancelled()),
            ) {
                let revision = s.snapshot.summary.as_ref().map_or(0, |x| x.revision);
                s.polling = true;
                Some((
                    s.epoch,
                    target,
                    cancel,
                    revision,
                    s.snapshot.messages.clone(),
                    s.latest_first,
                    s.snapshot.first,
                ))
            } else {
                None
            }
        };
        async move {
            let Some((epoch, target, cancel, revision, previous, latest_first, first)) = request
            else {
                return;
            };
            let result = async {
                let mut reader = target
                    .lease
                    .client()
                    .transcript(target.lease.session(), &target.chat)?
                    .with_page_size(200.min(this.limits.messages))?;
                let mut loaded_first = first;
                let mut messages = Vec::new();
                let mut bytes = 0usize;
                let mut summary = None;
                let mut captures = Vec::new();
                let mut has_more = false;
                loop {
                    let mut candidate = reader.clone();
                    let Some(page) = candidate
                        .next_page(target.lease.client(), target.lease.session(), &cancel)
                        .await?
                    else {
                        break;
                    };
                    if summary.is_none() && page.revision == revision {
                        return Ok(None);
                    }
                    if latest_first && summary.is_none() && first > 0 {
                        loaded_first = first.min(
                            page.thread
                                .messages
                                .saturating_sub(200.min(this.limits.messages)),
                        );
                        reader = target
                            .lease
                            .client()
                            .transcript(target.lease.session(), &target.chat)?
                            .with_page_size(200.min(this.limits.messages))?
                            .starting_at(loaded_first, page.revision)?;
                        summary = Some(page.thread);
                        continue;
                    }
                    let page_bytes = page
                        .data
                        .iter()
                        .map(message_bytes)
                        .fold(0usize, usize::saturating_add);
                    if bytes.saturating_add(page_bytes) > this.limits.bytes
                        || messages.len() + page.data.len() > this.limits.messages
                    {
                        if messages.is_empty() {
                            return Err(Error::InvalidResponse);
                        }
                        has_more = true;
                        break;
                    }
                    bytes += page_bytes;
                    messages.extend(page.data);
                    summary = Some(page.thread);
                    captures = page.captures;
                    has_more = page.has_more;
                    reader = candidate;
                    if !has_more
                        || messages.len() >= this.limits.messages
                        || bytes > this.limits.bytes / 2
                    {
                        break;
                    }
                }
                Ok::<_, Error>(Some((
                    reader,
                    messages,
                    bytes,
                    summary,
                    captures,
                    has_more,
                    loaded_first,
                )))
            }
            .await;
            let mut s = this.state.lock().unwrap();
            if s.epoch != epoch {
                return;
            }
            s.polling = false;
            if !target.is_current()
                || cancel.is_cancelled()
                || !Arc::ptr_eq(&previous, &s.snapshot.messages)
                || s.snapshot.state == LoadState::Loading
            {
                return;
            }
            match result {
                Ok(None) => {
                    if s.snapshot.state == LoadState::Ready {
                        return;
                    }
                    s.snapshot.state = LoadState::Ready;
                }
                Ok(Some((reader, messages, bytes, summary, captures, has_more, loaded_first))) => {
                    let first = loaded_first;
                    s.snapshot.first = if latest_first { first } else { 0 };
                    s.file_cancel.cancel();
                    s.file_cancel = s.cancel.child_token();
                    s.snapshot.download = None;
                    s.snapshot.received = if latest_first {
                        first + messages.len()
                    } else {
                        messages.len()
                    };
                    s.snapshot.messages = Arc::new(messages);
                    s.snapshot.summary = summary;
                    s.snapshot.captures = captures;
                    s.snapshot.has_more = if latest_first { first > 0 } else { has_more };
                    s.snapshot.state = LoadState::Ready;
                    s.bytes = bytes;
                    if latest_first && first > 0 {
                        let start = first.saturating_sub(200.min(this.limits.messages));
                        s.page_start = start;
                        s.reader = target
                            .lease
                            .client()
                            .transcript(target.lease.session(), &target.chat)
                            .and_then(|r| r.with_page_size(first - start))
                            .and_then(|r| {
                                r.starting_at(
                                    start,
                                    s.snapshot.summary.as_ref().map_or(0, |x| x.revision),
                                )
                            })
                            .ok();
                    } else {
                        s.reader = Some(reader);
                    }
                }
                Err(Error::Cancelled | Error::Changed | Error::InvalidCursor) => return,
                Err(error) => {
                    s.snapshot.login_required = login_required(&error);
                    s.snapshot.access_lost = lost_access(&error);
                    if s.snapshot.access_lost || s.snapshot.login_required {
                        s.snapshot.messages = Arc::new(Vec::new());
                        s.snapshot.captures.clear();
                        s.snapshot.summary = None;
                        s.snapshot.download = None;
                        s.file_cancel.cancel();
                        s.snapshot.received = 0;
                        s.bytes = 0;
                    }
                    s.snapshot.state = LoadState::Failed {
                        message: "Couldn't update this chat. Check your connection or refresh."
                            .into(),
                        stale: !s.snapshot.messages.is_empty(),
                    };
                }
            }
            this.publish(&s);
        }
    }

    /// Fetch a displayed attachment by its opaque transcript reference.
    pub async fn attachment(&self, path: &str) -> Result<Vec<u8>, Error> {
        let (epoch, target, cancel, reference) = {
            let s = self.state.lock().unwrap();
            let target = s
                .target
                .clone()
                .filter(ExistingChat::is_current)
                .ok_or(Error::Cancelled)?;
            let reference = s
                .snapshot
                .messages
                .iter()
                .find_map(|message| {
                    message.files.iter().find_map(|file| {
                        (crate::chat_entries::file_path(s.epoch, &message.id, file) == path)
                            .then(|| file.download_ref().cloned())
                            .flatten()
                    })
                })
                .ok_or(Error::NotFound)?;
            (s.epoch, target, s.cancel.clone(), reference)
        };
        let result = target
            .lease
            .client()
            .download(target.lease.session(), &reference, &cancel)
            .await;
        let mut s = self.state.lock().unwrap();
        if s.epoch != epoch || !target.is_current() || cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if let Err(error) = &result
            && matches!(
                error,
                Error::Forbidden | Error::Unauthorized | Error::ExpiredSession
            )
        {
            s.snapshot.login_required = login_required(error);
            s.snapshot.access_lost = true;
            s.snapshot.messages = Arc::new(Vec::new());
            s.snapshot.captures.clear();
            s.snapshot.summary = None;
            s.snapshot.download = None;
            s.snapshot.first = 0;
            s.snapshot.received = 0;
            s.snapshot.has_more = false;
            s.snapshot.state = LoadState::Failed {
                message: error.to_string(),
                stale: false,
            };
            s.bytes = 0;
            s.cancel.cancel();
            s.file_cancel.cancel();
            self.publish(&s);
        }
        result
    }

    /// Only one saved attachment is kept in memory. Selecting another cancels and releases it.
    pub fn download(
        self: &Arc<Self>,
        reference: AttachmentRef,
        name: String,
    ) -> impl Future<Output = ()> + Send + 'static {
        let this = self.clone();
        let request = {
            let mut s = self.state.lock().unwrap();
            if let Some(target) = s
                .target
                .clone()
                .filter(ExistingChat::is_current)
                .filter(|t| t.chat == *reference.chat())
            {
                s.file_cancel.cancel();
                s.file_cancel = s.cancel.child_token();
                s.file_epoch += 1;
                s.snapshot.download = Some(Download {
                    reference: reference.clone(),
                    name,
                    state: LoadState::Loading,
                    bytes: None,
                });
                self.publish(&s);
                Some((s.epoch, s.file_epoch, target, s.file_cancel.clone()))
            } else {
                None
            }
        };
        async move {
            let Some((epoch, file_epoch, target, cancel)) = request else {
                return;
            };
            let result = target
                .lease
                .client()
                .download(target.lease.session(), &reference, &cancel)
                .await;
            let mut s = this.state.lock().unwrap();
            if s.epoch != epoch
                || s.file_epoch != file_epoch
                || !target.is_current()
                || cancel.is_cancelled()
            {
                return;
            }
            match result {
                Ok(bytes) => {
                    if let Some(download) = &mut s.snapshot.download {
                        download.state = LoadState::Ready;
                        download.bytes = Some(Arc::new(bytes));
                    }
                }
                Err(e) => {
                    s.snapshot.login_required = login_required(&e);
                    if matches!(
                        e,
                        Error::Forbidden | Error::Unauthorized | Error::ExpiredSession
                    ) {
                        s.snapshot.access_lost = true;
                        s.snapshot.messages = Arc::new(Vec::new());
                        s.snapshot.captures.clear();
                        s.snapshot.summary = None;
                        s.snapshot.download = None;
                        s.snapshot.first = 0;
                        s.snapshot.received = 0;
                        s.snapshot.has_more = false;
                        s.snapshot.state = LoadState::Failed {
                            message: e.to_string(),
                            stale: false,
                        };
                        s.bytes = 0;
                    }
                    if let Some(download) = &mut s.snapshot.download {
                        download.state = LoadState::Failed {
                            message: e.to_string(),
                            stale: false,
                        };
                    }
                }
            }
            this.publish(&s);
        }
    }
    pub fn close_download(&self) {
        let mut s = self.state.lock().unwrap();
        s.file_cancel.cancel();
        s.file_epoch += 1;
        s.snapshot.download = None;
        self.publish(&s);
    }
    pub fn bytes_for_save(&self, epoch: u64, reference: &AttachmentRef) -> Option<Arc<Vec<u8>>> {
        let s = self.state.lock().unwrap();
        if s.epoch != epoch || !s.target.as_ref().is_some_and(ExistingChat::is_current) {
            return None;
        }
        s.snapshot
            .download
            .as_ref()
            .filter(|d| d.reference == *reference)?
            .bytes
            .clone()
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.state.lock().unwrap().cancel.cancel();
    }
}
fn login_required(e: &Error) -> bool {
    matches!(
        e,
        Error::Unauthorized
            | Error::ExpiredSession
            | Error::WrongAccount
            | Error::WrongDeployment
            | Error::InvalidCredential
    )
}
fn lost_access(e: &Error) -> bool {
    login_required(e) || matches!(e, Error::Forbidden | Error::NotFound)
}
fn value_bytes(value: &serde_json::Value) -> usize {
    let own = std::mem::size_of::<serde_json::Value>();
    own.saturating_add(match value {
        serde_json::Value::String(s) => s.len(),
        serde_json::Value::Array(a) => a
            .iter()
            .map(value_bytes)
            .fold(0usize, usize::saturating_add),
        serde_json::Value::Object(o) => o
            .iter()
            .map(|(k, v)| k.len().saturating_add(value_bytes(v)))
            .fold(0usize, usize::saturating_add),
        _ => 0,
    })
}
fn message_bytes(m: &SavedMessage) -> usize {
    let tool = m
        .tool
        .as_ref()
        .map(|t| {
            t.input
                .len()
                .saturating_add(t.output.len())
                .saturating_add(t.summary.len())
                .saturating_add(t.name.len())
        })
        .unwrap_or(0);
    m.text.len().saturating_add(tool).saturating_add(
        m.extra
            .iter()
            .map(|(k, v)| k.len().saturating_add(value_bytes(v)))
            .fold(0usize, usize::saturating_add),
    )
}

/// Read a bounded UTF-8 part without losing characters at a boundary.
pub fn text_part(text: &str, part: usize) -> (String, usize, usize) {
    const SIZE: usize = 64 * 1024;
    let count = text.len().div_ceil(SIZE).max(1);
    let part = part.min(count - 1);
    let mut start = (part * SIZE).min(text.len());
    let mut end = ((part + 1) * SIZE).min(text.len());
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[start..end].to_owned(), count, part)
}
