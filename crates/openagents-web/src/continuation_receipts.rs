//! Bounded continuation context, separate from the visible transcript.
use crate::chat_store::{Conversation, Error, Store};
use chat_home::{Continued, Home, PartRef};

fn key(owner: &str, target: &str) -> Result<String, Error> {
    if !chat_home::valid_id(target) {
        return Err(Error::Invalid("Choose a new conversation."));
    }
    Store::owner_key(owner, &format!("continuations/{target}.json"))
}

pub(crate) async fn load(
    store: &Store,
    owner: &str,
    target: &str,
) -> Result<Option<Continued>, Error> {
    store
        .read_key(&key(owner, target)?)
        .await?
        .map(|(bytes, _)| {
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Unavailable("The earlier chat could not be read."))
        })
        .transpose()
}

/// Reserve a destination once. A retry keeps exactly the same context.
pub(crate) async fn reserve(
    store: &Store,
    owner: &str,
    continued: &Continued,
) -> Result<Continued, Error> {
    let bytes = serde_json::to_vec(continued)
        .map_err(|_| Error::Invalid("The earlier chat could not be read."))?;
    match store
        .write_key(&key(owner, &continued.target.id)?, bytes, None)
        .await
    {
        Ok(_) => Ok(continued.clone()),
        Err(Error::Conflict) => {
            let old = load(store, owner, &continued.target.id)
                .await?
                .ok_or(Error::Conflict)?;
            if old.account == continued.account
                && old.source == continued.source
                && old.target == continued.target
            {
                Ok(old)
            } else {
                Err(Error::Invalid(
                    "Choose a new conversation for this continuation.",
                ))
            }
        }
        Err(error) => Err(error),
    }
}

/// Called only after the source points to this destination. Repeating it
/// repairs a lost create response without replacing any messages.
pub(crate) async fn ensure_cloud(
    store: &Store,
    owner: &str,
    continued: &Continued,
    project: Option<String>,
) -> Result<(), Error> {
    if continued.target.home != Home::Cloud {
        return Ok(());
    }
    if let Some(old) = store.load(owner, &continued.target.id).await? {
        return if old.conversation.links.continues.as_ref() == Some(&continued.source)
            && old.conversation.terminal.is_none()
            && !old.conversation.deleted()
        {
            Ok(())
        } else {
            Err(Error::Invalid(
                "Choose a new conversation for this continuation.",
            ))
        };
    }
    let chat = Conversation {
        links: chat_home::Links {
            continues: Some(continued.source.clone()),
            continued_by: None,
        },
        id: continued.target.id.clone(),
        owner: owner.into(),
        revision: 1,
        title: continued.seed.title.chars().take(64).collect(),
        messages: vec![],
        pending: None,
        requests: vec![],
        selection: None,
        updated_unix: crate::chat_store::now_unix(),
        pinned_unix: None,
        archived_unix: None,
        project,
        filed: false,
        terminal: None,
        environment: None,
        tasks: vec![],
        opened_unix: None,
        branch: None,
    };
    match store.create(&chat).await {
        Ok(_) => Ok(()),
        Err(Error::Conflict) => {
            let old = store.load(owner, &chat.id).await?.ok_or(Error::Conflict)?;
            if old.conversation.links.continues.as_ref() == Some(&continued.source) {
                Ok(())
            } else {
                Err(Error::Conflict)
            }
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn source_matches(continued: &Continued, source: &PartRef, target: &PartRef) -> bool {
    continued.source == *source && continued.target == *target && continued.seed.valid()
}
