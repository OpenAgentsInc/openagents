//! Durable lifecycle request ownership, independent of provider retry retention.
use std::{future::Future, pin::Pin};

pub type JournalFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

#[derive(Debug, PartialEq, Eq)]
pub enum LifecycleClaim {
    Fresh,
    Accepted(String),
    Rejected,
    /// The request was recorded, but its response was never saved.
    Uncertain,
}

/// Save request ownership before sending a mutation, then save the accepted
/// provider operation before waiting. An uncertain request is never sent again.
pub trait LifecycleJournal: Send + Sync {
    fn claim<'a>(
        &'a self,
        key: &'a str,
        computer: &'a str,
        action: &'a str,
    ) -> JournalFuture<'a, LifecycleClaim>;
    fn accepted<'a>(&'a self, key: &'a str, provider_operation: &'a str) -> JournalFuture<'a, ()>;
    fn rejected<'a>(&'a self, key: &'a str) -> JournalFuture<'a, ()>;
}
