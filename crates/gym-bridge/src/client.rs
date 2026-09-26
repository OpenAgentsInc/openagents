//! A client holds only its own key and explicitly granted Gym connection.
use crate::{protocol::*, *};
use secp256k1::SecretKey;

pub struct Client {
    connection: Connection,
    secret: SecretKey,
    policy: RelayPolicy,
    socket: tokio::sync::Mutex<Option<transport::Session>>,
}
impl Client {
    pub fn new(connection: Connection, secret: SecretKey) -> Result<Self> {
        Self::new_with_policy(connection, secret, RelayPolicy::Production)
    }
    pub fn new_with_policy(
        connection: Connection,
        secret: SecretKey,
        policy: RelayPolicy,
    ) -> Result<Self> {
        connection.verify(&secret, unix_time()?, policy)?;
        Ok(Self {
            connection,
            secret,
            policy,
            socket: tokio::sync::Mutex::new(None),
        })
    }
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
    pub async fn snapshot(&self) -> Result<Snapshot> {
        match self.query(Query::Snapshot).await? {
            Response::Snapshot(snapshot) => Ok(*snapshot),
            _ => Err(error(
                ErrorCode::Malformed,
                "Gym response does not contain a snapshot",
            )),
        }
    }
    /// Preserve `request_id` after any error. A new ID authorizes another launch.
    pub async fn launch(
        &self,
        request_id: &str,
        recipe_id: &str,
        revision: &str,
    ) -> Result<LaunchReceipt> {
        match self
            .query(Query::Launch {
                request_id: request_id.into(),
                recipe_id: recipe_id.into(),
                revision: revision.into(),
            })
            .await?
        {
            Response::Launch(receipt) => Ok(receipt),
            _ => Err(error(
                ErrorCode::Malformed,
                "Gym response does not contain a launch receipt",
            )),
        }
    }
    async fn query(&self, query: Query) -> Result<Response> {
        let mut slot = self.socket.lock().await;
        let now = unix_time()?;
        let grant = self.connection.verify(&self.secret, now, self.policy)?;
        query.validate()?;
        if let Query::Launch {
            recipe_id,
            revision,
            ..
        } = &query
            && !grant
                .recipes
                .iter()
                .any(|r| &r.id == recipe_id && &r.revision == revision)
        {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym grant does not authorize this recipe revision",
            ));
        }
        let request = Request {
            v: REQUEST_SCHEMA.into(),
            request: random_id(),
            grant: grant.grant,
            authorization: self.connection.authorization.id.clone(),
            issued_at: now,
            expires_at: self.connection.expires_at.min(now + REQUEST_LIFETIME),
            query,
        };
        request.validate()?;
        let event = seal(
            &request,
            REQUEST_SCHEMA,
            &self.secret,
            &self.connection.host,
            &request.request,
            now,
            request.expires_at,
        )?;
        // Cancellation owns and drops the uncertain socket; only a complete,
        // verified exchange puts it back in the reusable slot.
        let old = slot.take().filter(transport::Session::reusable);
        let (session, response) = tokio::time::timeout(std::time::Duration::from_secs(8), async {
            let mut session = match old {
                Some(s) => s,
                None => {
                    transport::Session::connect(&self.connection.relay, &self.secret, self.policy)
                        .await?
                }
            };
            let reply = session
                .exchange(
                    &event,
                    &request.request,
                    &self.connection.host,
                    &self.connection.client,
                )
                .await?;
            let response = self.verify(&request, &event, &reply, unix_time()?)?;
            Ok::<_, Error>((session, response))
        })
        .await
        .map_err(|_| {
            error(
                ErrorCode::Transport,
                "Gym exchange timed out; launch delivery may be unknown",
            )
        })??;
        *slot = Some(session);
        match response {
            Response::Refused(code) => Err(error(code, "Gym host refused this request")),
            response => Ok(response),
        }
    }
    fn verify(
        &self,
        request: &Request,
        event: &nostr::domain::Event,
        reply: &nostr::domain::Event,
        now: u64,
    ) -> Result<Response> {
        self.connection.verify(&self.secret, now, self.policy)?;
        fresh(request.issued_at, request.expires_at, now)?;
        let body: Reply = open(
            reply,
            &self.secret,
            &self.connection.host,
            &self.connection.client,
            REPLY_SCHEMA,
        )?;
        if body.v != REPLY_SCHEMA
            || body.request != request.request
            || body.request_event != event.id
            || body.grant != request.grant
            || body.issued_at < request.issued_at
            || body.expires_at > request.expires_at
            || reply.tag_values("h").collect::<Vec<_>>() != [request.request.as_str()]
        {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym reply differs from the exact request",
            ));
        }
        window(body.issued_at, body.expires_at, REQUEST_LIFETIME)?;
        fresh(body.issued_at, body.expires_at, now)?;
        match (&request.query, &body.response) {
            (Query::Snapshot, Response::Snapshot(s)) => {
                s.validate(now)?;
                let grant = self.connection.verify(&self.secret, now, self.policy)?;
                if s.recipes.iter().any(|r| !grant.recipes.contains(r)) {
                    return Err(error(
                        ErrorCode::Forbidden,
                        "Gym board advertises an ungranted recipe",
                    ));
                }
            }
            (
                Query::Launch {
                    request_id,
                    recipe_id,
                    revision,
                },
                Response::Launch(r),
            ) => {
                identity(&r.run_id)?;
                if &r.request_id != request_id
                    || &r.recipe_id != recipe_id
                    || &r.revision != revision
                    || r.submitted_at > now
                    || r.finished_at.is_some_and(|t| t < r.submitted_at || t > now)
                    || (matches!(r.status, Status::Running | Status::Queued)
                        && r.finished_at.is_some())
                {
                    return Err(error(
                        ErrorCode::Forbidden,
                        "Gym launch receipt differs from requested work",
                    ));
                }
            }
            (_, Response::Refused(code)) if *code != ErrorCode::Transport => {}
            _ => {
                return Err(error(
                    ErrorCode::Malformed,
                    "Gym reply has an unexpected result type",
                ));
            }
        }
        Ok(body.response)
    }
}
