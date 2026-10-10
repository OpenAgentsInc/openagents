//! Bounded background dispatch. Executors run after claims commit and outside transactions.
use crate::{postgres::PgStore, *};
use futures_util::{StreamExt, future::BoxFuture, stream::poll_fn};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Notify, watch},
    task::JoinHandle,
};

pub trait EffectExecutor: Send + Sync + 'static {
    /// Use (uid, id) as the receiver's stable operation identity. Respect the
    /// effect's deadline and cancellation; uncertainty must not cause an unsafe retry.
    fn execute<'a>(&'a self, effect: &'a ClaimedEffect)
    -> BoxFuture<'a, Result<serde_json::Value>>;
}
#[derive(Clone, Default)]
pub struct Effects(BTreeMap<String, Arc<dyn EffectExecutor>>);
impl Effects {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(
        &mut self,
        kind: impl Into<String>,
        executor: Arc<dyn EffectExecutor>,
    ) -> Result<()> {
        let kind = kind.into();
        crate::core::validate_name(&kind)?;
        if self.0.contains_key(&kind) {
            return Err(ActorError::new(
                "exists",
                "That operation is already registered.",
            ));
        }
        self.0.insert(kind, executor);
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub poll_interval: Duration,
    pub batch: usize,
    pub effect_concurrency: usize,
    pub listen: bool,
}
impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
            batch: 32,
            effect_concurrency: 4,
            listen: true,
        }
    }
}
#[derive(Default)]
struct Metrics {
    transitions: AtomicU64,
    alarms: AtomicU64,
    expired_work: AtomicU64,
    expired_effects: AtomicU64,
    effects: AtomicU64,
    errors: AtomicU64,
}
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RuntimeStats {
    pub transitions: u64,
    pub alarms: u64,
    pub expired_work: u64,
    pub expired_effects: u64,
    pub effects: u64,
    pub errors: u64,
}
impl Metrics {
    fn read(&self) -> RuntimeStats {
        RuntimeStats {
            transitions: self.transitions.load(Ordering::Relaxed),
            alarms: self.alarms.load(Ordering::Relaxed),
            expired_work: self.expired_work.load(Ordering::Relaxed),
            expired_effects: self.expired_effects.load(Ordering::Relaxed),
            effects: self.effects.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
        }
    }
    fn count(&self, result: Result<usize>, counter: &AtomicU64) {
        match result {
            Ok(n) => {
                counter.fetch_add(n as u64, Ordering::Relaxed);
            }
            Err(_) => {
                self.errors.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
pub struct Runtime {
    store: PgStore,
    effects: Effects,
    config: RuntimeConfig,
}
impl Runtime {
    pub fn new(store: PgStore) -> Self {
        Self {
            store,
            effects: Effects::default(),
            config: RuntimeConfig::default(),
        }
    }
    pub fn effects(mut self, effects: Effects) -> Self {
        self.effects = effects;
        self
    }
    pub fn config(mut self, config: RuntimeConfig) -> Self {
        self.config = config;
        self
    }
    pub fn start(self) -> Result<RuntimeHandle> {
        if self.config.poll_interval < Duration::from_millis(10)
            || self.config.poll_interval > Duration::from_secs(5)
            || !(1..=64).contains(&self.config.batch)
            || !(1..=32).contains(&self.config.effect_concurrency)
        {
            return Err(ActorError::new(
                "bad_args",
                "The worker limits are invalid.",
            ));
        }
        let (stop_tx, stop_rx) = watch::channel(false);
        let metrics = Arc::new(Metrics::default());
        let wake = Arc::new(Notify::new());
        let mut tasks = Vec::new();
        if self.config.listen {
            tasks.push(listener(
                self.store.pool.config(),
                wake.clone(),
                stop_rx.clone(),
                metrics.clone(),
            ));
        }
        let store = self.store.clone();
        let m = metrics.clone();
        let cfg = self.config.clone();
        let mut stop = stop_rx.clone();
        tasks.push(tokio::spawn(async move {
            let mut timer = tokio::time::interval(cfg.poll_interval);
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {_ = stop.changed()=>break,_=timer.tick()=>{},_=wake.notified()=>{}}
                m.count(store.fire_alarms(cfg.batch as u32).await, &m.alarms);
                m.count(store.expire_work(cfg.batch as u32).await, &m.expired_work);
                m.count(
                    store.expire_effects(cfg.batch as u32).await,
                    &m.expired_effects,
                );
                m.count(store.dispatch_once(cfg.batch).await, &m.transitions);
            }
        }));
        let store = self.store;
        let effects = self.effects;
        let m = metrics.clone();
        let cfg = self.config;
        let mut stop = stop_rx;
        tasks.push(tokio::spawn(async move{
            let mut running=tokio::task::JoinSet::new();let mut timer=tokio::time::interval(cfg.poll_interval);timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select!{
                    _=stop.changed()=>break,
                    Some(result)=running.join_next(),if !running.is_empty()=>{if result.is_err(){m.errors.fetch_add(1,Ordering::Relaxed);}},
                    _=timer.tick()=>{
                        let available=cfg.effect_concurrency.saturating_sub(running.len());
                        if available==0{continue;}
                        let kinds:Vec<String>=effects.0.keys().cloned().collect();
                        match store.claim_effects_for_kinds(available as u32,&kinds).await{
                            Ok(claims)=>for effect in claims{
                                let store=store.clone();let executor=effects.0.get(&effect.kind).cloned();let metrics=m.clone();
                                running.spawn(async move{
                                    let result=execute_effect(store,executor,effect).await;
                                    if result.is_err(){metrics.errors.fetch_add(1,Ordering::Relaxed);}else{metrics.effects.fetch_add(1,Ordering::Relaxed);}
                                });
                            },Err(_)=>{m.errors.fetch_add(1,Ordering::Relaxed);}
                        }
                    }
                }
            }
            // Give in-flight calls a short drain. Aborted local futures leave
            // durable claims for the domain-specific expiry/reconciliation path.
            let drained=async{while running.join_next().await.is_some(){}};
            if tokio::time::timeout(Duration::from_secs(10),drained).await.is_err(){running.abort_all();}
        }));
        Ok(RuntimeHandle {
            stop: stop_tx,
            tasks,
            metrics,
        })
    }
}
async fn execute_effect(
    store: PgStore,
    executor: Option<Arc<dyn EffectExecutor>>,
    effect: ClaimedEffect,
) -> Result<()> {
    let Some(executor) = executor else {
        return store
            .fail_effect(
                &effect.uid,
                &effect.id,
                &effect.token,
                "No executor is registered for this operation.",
                false,
            )
            .await;
    };
    let execution = async {
        let future = executor.execute(&effect);
        tokio::pin!(future);
        let mut renew = tokio::time::interval(Duration::from_secs(20));
        renew.tick().await;
        loop {
            tokio::select! {
                result=&mut future=>return Ok::<_,ActorError>(result),
                _=renew.tick()=>{store.renew_effect(&effect.uid,&effect.id,&effect.token).await?;}
            }
        }
    };
    // The outer timeout also bounds time spent renewing a claim. Drop the
    // executor future before recording a timeout; dropping is not remote undo.
    let result = match tokio::time::timeout(
        Duration::from_millis(effect.timeout_ms.clamp(1, 600_000) as u64),
        execution,
    )
    .await
    {
        Ok(result) => result?,
        Err(_) => Err(ActorError::retry(
            "deadline",
            "The operation did not finish before its deadline.",
        )),
    };
    match result {
        Ok(outcome) => {
            store
                .finish_effect(&effect.uid, &effect.id, &effect.token, outcome)
                .await
        }
        Err(error) => {
            store
                .fail_effect(
                    &effect.uid,
                    &effect.id,
                    &effect.token,
                    &error.message,
                    error.retryable,
                )
                .await
        }
    }
}

pub struct RuntimeHandle {
    stop: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
    metrics: Arc<Metrics>,
}
impl RuntimeHandle {
    pub fn stats(&self) -> RuntimeStats {
        self.metrics.read()
    }
    pub async fn shutdown(mut self) {
        let _ = self.stop.send(true);
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
    }
}
impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

/// One dedicated LISTEN connection per runtime, in addition to the bounded pool.
/// Notifications are hints: every restart and missed wake is covered by polling.
fn listener(
    config: tokio_postgres::Config,
    wake: Arc<Notify>,
    mut stop: watch::Receiver<bool>,
    metrics: Arc<Metrics>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if *stop.borrow() {
                break;
            }
            let connected = tokio::select! {_=stop.changed()=>break,result=config.connect(tokio_postgres::NoTls)=>result};
            if let Ok((client, mut connection)) = connected {
                let mut stop_connection = stop.clone();
                let wake_connection = wake.clone();
                let mut task = tokio::spawn(async move {
                    let mut messages = poll_fn(move |cx| connection.poll_message(cx));
                    loop {
                        tokio::select! {
                            _=stop_connection.changed()=>break,
                            message=messages.next()=>match message{
                                Some(Ok(tokio_postgres::AsyncMessage::Notification(_)))=>wake_connection.notify_one(),
                                Some(Ok(_))=>{},_=>break,
                            }
                        }
                    }
                });
                let subscribed = tokio::select! {
                    _=stop.changed()=>false,
                    result=tokio::time::timeout(Duration::from_secs(5),client.batch_execute("LISTEN actors_changed"))=>matches!(result,Ok(Ok(()))),
                };
                if subscribed {
                    wake.notify_one();
                    tokio::select! {_=stop.changed()=>{},_=&mut task=>{}}
                }
                task.abort();
                drop(client);
            }
            if *stop.borrow() {
                break;
            }
            metrics.errors.fetch_add(1, Ordering::Relaxed);
            tokio::select! {_=stop.changed()=>break,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    })
}
