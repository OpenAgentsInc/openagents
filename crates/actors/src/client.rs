//! Typed in-process calls over the same durable store used by HTTP adapters.
use crate::{
    core::{Actor, Handles, Message},
    *,
};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Client {
    store: PgStore,
    caller: Caller,
}
impl Client {
    /// The embedding host authenticates this caller before constructing the client.
    /// Construct a fresh client for each authenticated request. This value does
    /// not refresh session membership, account ownership, or executor grants;
    /// a long-lived background task must reauthenticate before using it.
    pub fn new(store: PgStore, caller: Caller) -> Self {
        Self { store, caller }
    }
    pub fn actor<A: Actor>(&self, key: impl Into<String>) -> Result<ActorRef<A>> {
        let id = ActorId {
            workspace_id: self.caller.workspace_id.clone(),
            actor_type: A::TYPE.into(),
            key: key.into(),
        };
        crate::core::validate_actor_id(&id)?;
        Ok(ActorRef {
            client: self.clone(),
            id,
            input: None,
            marker: PhantomData,
        })
    }
}
pub struct ActorRef<A: Actor> {
    client: Client,
    id: ActorId,
    input: Option<serde_json::Value>,
    marker: PhantomData<fn() -> A>,
}
#[derive(Clone, Default)]
pub struct CallOptions {
    pub idempotency_key: Option<String>,
    pub expected_version: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct TypedReply<T> {
    pub reply: T,
    pub version: u64,
    pub event_seq: u64,
    pub replayed: bool,
}
impl<A: Actor> ActorRef<A> {
    pub fn id(&self) -> &ActorId {
        &self.id
    }
    /// Supply creation input for the first call. Creating and calling commit together.
    pub fn with_input(mut self, input: &A::Input) -> Result<Self>
    where
        A::Input: Serialize,
    {
        self.input = Some(serde_json::to_value(input)?);
        Ok(self)
    }
    pub async fn call<M>(&self, message: &M, options: CallOptions) -> Result<TypedReply<M::Reply>>
    where
        A: Handles<M>,
        M: Message + Serialize,
        M::Reply: DeserializeOwned,
    {
        let reply = self
            .client
            .store
            .call(
                &self.client.caller,
                ActionRequest {
                    id: self.id.clone(),
                    message: Envelope {
                        name: M::NAME.into(),
                        args: serde_json::to_value(message)?,
                        origin: Origin::Action,
                    },
                    input: self.input.clone(),
                    idempotency_key: options.idempotency_key,
                    expected_version: options.expected_version,
                },
            )
            .await?;
        Ok(TypedReply {
            reply: serde_json::from_value(reply.reply)?,
            version: reply.version,
            event_seq: reply.event_seq,
            replayed: reply.replayed,
        })
    }
    pub async fn enqueue<M>(
        &self,
        message: &M,
        idempotency_key: Option<&str>,
    ) -> Result<InboxReceipt>
    where
        A: Handles<M>,
        M: Message + Serialize,
    {
        self.client
            .store
            .enqueue(
                &self.client.caller,
                &self.id,
                Envelope {
                    name: M::NAME.into(),
                    args: serde_json::to_value(message)?,
                    origin: Origin::Inbox,
                },
                idempotency_key,
            )
            .await
    }
    pub async fn view(&self) -> Result<ViewReply> {
        self.client.store.view(&self.client.caller, &self.id).await
    }
}
