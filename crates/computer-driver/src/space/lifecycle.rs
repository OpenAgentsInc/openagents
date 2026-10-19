//! Durable lifecycle request ownership, independent of provider retry retention.
use std::{future::Future, pin::Pin};

pub type JournalFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleClaim {
    Fresh,
    Accepted(String),
    Rejected,
    /// The request was recorded, but its response was never saved.
    Uncertain,
}

/// Save request ownership before sending a mutation, then save the accepted
/// provider operation before waiting. An uncertain request is sent again under the same
/// idempotency key.
pub trait LifecycleJournal: Send + Sync {
    fn claim<'a>(
        &'a self,
        key: &'a str,
        computer: &'a str,
        action: &'a str,
    ) -> JournalFuture<'a, LifecycleClaim>;
    fn accepted<'a>(&'a self, key: &'a str, provider_operation: &'a str) -> JournalFuture<'a, ()>;
    fn rejected<'a>(&'a self, key: &'a str) -> JournalFuture<'a, ()>;
    /// Optional operator authority for a new child of a definitively refused request.
    fn authorized_retry<'a>(
        &'a self,
        _key: &'a str,
        _computer: &'a str,
        _action: &'a str,
    ) -> JournalFuture<'a, Option<String>> {
        Box::pin(async { Ok(None) })
    }
}
