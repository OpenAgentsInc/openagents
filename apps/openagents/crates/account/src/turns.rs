//! Cloud replies shared by the desktop and phone composers.
use crate::{
    history::ExistingChat,
    openagents::{CancellationToken, ChatRef, ChatSource, Error, chats},
    reader::Reader,
};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Clone, Default)]
pub struct Snapshot {
    pub chat: Option<ChatRef>,
    pub busy: bool,
    pub stopping: bool,
    pub retry: bool,
    pub error: Option<String>,
    pub accepted: u64,
    pub cards: Option<serde_json::Value>,
}
struct State {
    generation: u64,
    paused: bool,
    target: Option<ExistingChat>,
    cancel: CancellationToken,
    message: Option<chats::Send>,
    accepted_request: Option<String>,
    active: Option<chats::Turn>,
    snapshot: Snapshot,
}
pub struct Turns {
    reader: Arc<Reader>,
    state: Mutex<State>,
    updates: watch::Sender<Snapshot>,
}
struct Job {
    generation: u64,
    target: ExistingChat,
    cancel: CancellationToken,
    turn: chats::Turn,
    message: Option<chats::Send>,
}
impl Turns {
    pub fn new(reader: Arc<Reader>) -> Arc<Self> {
        let (updates, _) = watch::channel(Snapshot::default());
        Arc::new(Self {
            reader,
            state: Mutex::new(State {
                generation: 0,
                paused: false,
                target: None,
                cancel: CancellationToken::new(),
                message: None,
                accepted_request: None,
                active: None,
                snapshot: Snapshot::default(),
            }),
            updates,
        })
    }
    pub fn watch(&self) -> watch::Receiver<Snapshot> {
        self.updates.subscribe()
    }
    pub fn snapshot(&self) -> Snapshot {
        let s = self.state.lock().unwrap();
        if s.target.as_ref().is_some_and(|t| {
            !t.is_current() || self.reader.target().is_none_or(|r| r.chat != t.chat)
        }) {
            return Snapshot::default();
        }
        s.snapshot.clone()
    }
    fn publish(&self, s: &State) {
        self.updates.send_replace(s.snapshot.clone());
    }
    pub fn clear(&self) {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.generation += 1;
        s.paused = false;
        s.target = None;
        s.message = None;
        s.accepted_request = None;
        s.active = None;
        s.snapshot = Snapshot::default();
        self.publish(&s);
    }
    /// Pause client observation without stopping the server or discarding retry identity.
    pub fn pause(&self) {
        let mut s = self.state.lock().unwrap();
        s.cancel.cancel();
        s.generation += 1;
        s.paused = true;
        if s.active.is_some() {
            s.snapshot.busy = false;
            s.snapshot.stopping = false;
            s.snapshot.retry = true;
        }
        self.publish(&s);
    }
    pub fn resume(self: &Arc<Self>) -> impl std::future::Future<Output = ()> + Send + 'static {
        {
            let mut s = self.state.lock().unwrap();
            s.paused = false;
        }
        self.retry()
    }
    fn target(&self) -> Option<ExistingChat> {
        let target = self
            .reader
            .target()
            .filter(|t| t.summary.source == ChatSource::Web)?;
        if self
            .reader
            .snapshot()
            .summary
            .as_ref()
            .is_some_and(|s| s.links.continued_by.is_some())
        {
            return None;
        }
        let mut s = self.state.lock().unwrap();
        if s.target
            .as_ref()
            .is_none_or(|t| t.chat != target.chat || !t.is_current())
        {
            s.cancel.cancel();
            s.generation += 1;
            s.cancel = target.lease.cancel.child_token();
            s.target = Some(target.clone());
            s.message = None;
            s.accepted_request = None;
            s.active = None;
            s.snapshot = Snapshot {
                chat: Some(target.chat.clone()),
                ..Snapshot::default()
            };
            self.publish(&s);
        }
        Some(target)
    }
    fn valid(&self, s: &State, job: &Job) -> bool {
        s.generation == job.generation
            && !job.cancel.is_cancelled()
            && job.target.is_current()
            && self
                .reader
                .target()
                .is_some_and(|t| t.chat == job.target.chat)
    }
    fn current(&self, job: &Job) -> bool {
        self.valid(&self.state.lock().unwrap(), job)
    }
    pub fn submit(
        self: &Arc<Self>,
        text: String,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let job = self.target().and_then(|target| {
            let mut s = self.state.lock().unwrap();
            if s.paused || s.snapshot.busy || s.snapshot.retry {
                return None;
            }
            if text.chars().count() > 4000 {
                s.snapshot.error = Some("Write a message of up to 4,000 characters.".into());
                self.publish(&s);
                return None;
            }
            let message = match chats::Send::new(target.chat.clone(), text) {
                Ok(v) => v,
                Err(e) => {
                    s.snapshot.error = Some(e.to_string());
                    self.publish(&s);
                    return None;
                }
            };
            s.generation += 1;
            s.cancel.cancel();
            s.cancel = target.lease.cancel.child_token();
            s.active = Some(message.turn());
            s.message = Some(message.clone());
            s.snapshot.busy = true;
            s.snapshot.error = None;
            self.publish(&s);
            Some(Job {
                generation: s.generation,
                target,
                cancel: s.cancel.clone(),
                turn: message.turn(),
                message: Some(message),
            })
        });
        let this = self.clone();
        async move {
            if let Some(job) = job {
                this.run(job, true).await;
            }
        }
    }
    pub fn retry(self: &Arc<Self>) -> impl std::future::Future<Output = ()> + Send + 'static {
        let job = self.target().and_then(|target| {
            let mut s = self.state.lock().unwrap();
            if s.paused || !s.snapshot.retry {
                return None;
            }
            let turn = s.active.clone()?;
            s.generation += 1;
            s.cancel.cancel();
            s.cancel = target.lease.cancel.child_token();
            s.snapshot.retry = false;
            s.snapshot.busy = true;
            s.snapshot.error = None;
            self.publish(&s);
            Some(Job {
                generation: s.generation,
                target,
                cancel: s.cancel.clone(),
                turn,
                message: s.message.clone(),
            })
        });
        let this = self.clone();
        async move {
            if let Some(job) = job {
                let send = job.message.is_some();
                this.run(job, send).await;
            }
        }
    }
    /// Observe turns started on the website without sending another message.
    pub fn follow(self: &Arc<Self>) -> impl std::future::Future<Output = ()> + Send + 'static {
        let target = self.target().and_then(|target| {
            let s = self.state.lock().unwrap();
            (!s.paused && !s.snapshot.busy && !s.snapshot.retry).then_some((target, s.generation))
        });
        let this = self.clone();
        async move {
            let Some((target, generation)) = target else {
                return;
            };
            let cancel = target.lease.cancel.child_token();
            let Ok(Some(current)) = chats::current(
                target.lease.client(),
                target.lease.session(),
                &target.chat,
                &cancel,
            )
            .await
            else {
                return;
            };
            if current.outcome != "pending"
                || !target.is_current()
                || this.reader.target().is_none_or(|t| t.chat != target.chat)
            {
                return;
            }
            let job = {
                let mut s = this.state.lock().unwrap();
                if s.generation != generation || s.paused || s.snapshot.busy || s.snapshot.retry {
                    return;
                }
                s.generation += 1;
                s.cancel.cancel();
                s.cancel = cancel.clone();
                s.active = Some(current.turn.clone());
                s.message = None;
                s.snapshot.busy = true;
                s.snapshot.error = None;
                this.publish(&s);
                Job {
                    generation: s.generation,
                    target,
                    cancel,
                    turn: current.turn,
                    message: None,
                }
            };
            this.run(job, false).await;
        }
    }
    async fn run(self: Arc<Self>, job: Job, send: bool) {
        let result = async {
            let client = job.target.lease.client();
            let session = job.target.lease.session();
            if send {
                chats::send(
                    client,
                    session,
                    job.message.as_ref().ok_or(Error::InvalidReference)?,
                    &job.cancel,
                )
                .await?;
                if !self.current(&job) {
                    return Err(Error::Cancelled);
                }
                let mut s = self.state.lock().unwrap();
                if !self.valid(&s, &job) {
                    return Err(Error::Cancelled);
                }
                if s.accepted_request.as_deref() != Some(job.turn.request_id()) {
                    s.snapshot.accepted += 1;
                    s.accepted_request = Some(job.turn.request_id().to_owned());
                }
                self.publish(&s);
            }
            self.reader.set_following(true);
            let mut stream = chats::stream(client, session, &job.turn, &job.cancel).await?;
            while let Some(event) = stream.next().await? {
                if !self.current(&job) {
                    return Err(Error::Cancelled);
                }
                match event {
                    chats::Event::Cards { cards, .. } => {
                        let mut s = self.state.lock().unwrap();
                        if !self.valid(&s, &job) {
                            return Err(Error::Cancelled);
                        }
                        s.snapshot.cards = Some(cards);
                        self.publish(&s);
                    }
                    chats::Event::Error { error, .. } => {
                        self.reader.poll().await;
                        let retry = matches!(error.code, chats::RefusalCode::StreamTimeout);
                        let mut s = self.state.lock().unwrap();
                        if !self.valid(&s, &job) {
                            return Err(Error::Cancelled);
                        }
                        s.snapshot.error = Some(error.message);
                        s.snapshot.busy = false;
                        s.snapshot.retry = retry;
                        if !retry {
                            s.active = None;
                            s.message = None;
                        }
                        self.publish(&s);
                        return Ok(());
                    }
                    _ => self.reader.poll().await,
                }
            }
            Ok(())
        }
        .await;
        if !self.current(&job) {
            return;
        }
        let mut s = self.state.lock().unwrap();
        if !self.valid(&s, &job) {
            return;
        }
        s.snapshot.busy = false;
        match result {
            Ok(()) => {
                if !s.snapshot.retry {
                    s.active = None;
                    s.message = None;
                }
            }
            Err(Error::Cancelled) => {}
            Err(error) => {
                s.snapshot.error = Some(error.to_string());
                s.snapshot.retry = matches!(
                    error,
                    Error::Network { .. }
                        | Error::Unavailable { .. }
                        | Error::RateLimited { .. }
                        | Error::InvalidResponse
                );
                if !s.snapshot.retry {
                    s.active = None;
                    s.message = None;
                }
            }
        }
        self.publish(&s);
    }
    pub fn stop(self: &Arc<Self>) -> impl std::future::Future<Output = ()> + Send + 'static {
        let job = self.target().and_then(|target| {
            let mut s = self.state.lock().unwrap();
            if s.snapshot.stopping {
                return None;
            }
            let turn = s.active.clone()?;
            s.snapshot.stopping = true;
            self.publish(&s);
            Some(Job {
                generation: s.generation,
                target,
                cancel: CancellationToken::new(),
                turn,
                message: None,
            })
        });
        let this = self.clone();
        async move {
            let Some(job) = job else {
                return;
            };
            let cancel = job.target.lease.cancel.child_token();
            let result = chats::stop(
                job.target.lease.client(),
                job.target.lease.session(),
                &job.turn,
                &cancel,
            )
            .await;
            if !this.current(&job) {
                return;
            }
            {
                let mut s = this.state.lock().unwrap();
                if !this.valid(&s, &job) {
                    return;
                }
                s.snapshot.stopping = false;
                match result {
                    Ok(_) => {
                        s.cancel.cancel();
                        s.generation += 1;
                        s.snapshot.busy = false;
                        s.snapshot.retry = false;
                        s.snapshot.error = None;
                        s.active = None;
                        s.message = None;
                    }
                    Err(e) => {
                        s.snapshot.error = Some(
                            if matches!(
                                &e,
                                Error::Refused {
                                    code: Some("unsupported_turn"),
                                    ..
                                }
                            ) {
                                "Stop this work on openagents.com.".into()
                            } else {
                                e.to_string()
                            },
                        )
                    }
                }
                this.publish(&s);
            }
            this.reader.poll().await;
        }
    }
}
