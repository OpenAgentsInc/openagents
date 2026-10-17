//! Move to a linked part without creating a second editable copy of the source.
use crate::{CancellationToken, ChatRef, Client, Error, Session};
pub use chat_home::{
    Claim, Continued, Home, Links, PartRef, REQUEST_OPTION, Role, Seed, Source, TransferClaim,
};
use reqwest::Method;

pub async fn destination(
    client: &Client,
    session: &Session,
    source: &ChatRef,
    cancel: &CancellationToken,
) -> Result<Option<PartRef>, Error> {
    client.check_session(session)?;
    if source.account() != session.account().id {
        return Err(Error::WrongAccount);
    }
    if source.deployment() != client.deployment() {
        return Err(Error::WrongDeployment);
    }
    let request = async {
        client
            .request::<serde_json::Value>(
                Method::GET,
                &["threads", source.id()],
                &[],
                None,
                Some(session.token()),
            )
            .await
    };
    let value = tokio::select! {biased;_=cancel.cancelled()=>return Err(Error::Cancelled),value=request=>value};
    let value = match value {
        Ok(value) => value,
        Err(Error::NotFound | Error::Refused { status: 404, .. }) => {
            return Ok(crate::sessions::get(client, session, source.id(), cancel)
                .await?
                .session
                .continued_by);
        }
        Err(error) => return Err(error),
    };
    if value["thread"]["id"].as_str() != Some(source.id()) {
        return Err(Error::InvalidResponse);
    }
    let part: Option<PartRef> = serde_json::from_value(value["thread"]["continued_by"].clone())
        .map_err(|_| Error::InvalidResponse)?;
    if part.as_ref().is_some_and(|p| !p.valid()) {
        return Err(Error::InvalidResponse);
    }
    Ok(part)
}

pub async fn claim(
    client: &Client,
    session: &Session,
    source: &ChatRef,
    claim: &Claim,
    cancel: &CancellationToken,
) -> Result<Continued, Error> {
    transfer(
        client,
        session,
        source,
        &TransferClaim {
            revision: claim.revision,
            target: claim.target.clone(),
            seed: None,
        },
        cancel,
    )
    .await
}

/// Choose a linked destination. Computer hosts supply a screened seed;
/// saved-chat sources use the history already held by the service.
pub async fn transfer(
    client: &Client,
    session: &Session,
    source: &ChatRef,
    claim: &TransferClaim,
    cancel: &CancellationToken,
) -> Result<Continued, Error> {
    client.check_session(session)?;
    if source.account() != session.account().id {
        return Err(Error::WrongAccount);
    }
    if source.deployment() != client.deployment() {
        return Err(Error::WrongDeployment);
    }
    if !claim.target.valid()
        || claim.target.id == source.id()
        || claim.revision == 0
        || claim.seed.as_ref().is_some_and(|seed| !seed.valid())
    {
        return Err(Error::InvalidReference);
    }
    let request = async {
        client
            .request::<Continued>(
                Method::POST,
                &["threads", source.id(), "continue"],
                &[],
                Some(serde_json::to_value(claim).map_err(|_| Error::InvalidReference)?),
                Some(session.token()),
            )
            .await
    };
    let result = tokio::select! { biased; _ = cancel.cancelled() => return Err(Error::Cancelled), value = request => value? };
    if result.account != session.account().id
        || result.source.id != source.id()
        || result.target != claim.target
        || !result.source.valid()
        || !result.seed.valid()
    {
        return Err(Error::InvalidResponse);
    }
    Ok(result)
}
