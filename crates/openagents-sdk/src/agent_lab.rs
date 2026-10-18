//! Account-owned Agent Lab documents and bounded experiments.
use crate::{CancellationToken, Client, Deployment, Error, Session};
pub use ::agent_lab::{api::*, *};
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
/// An Agent Lab request body tied to one account and deployment. Keep it across retries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request<T> {
    account: String,
    deployment: Deployment,
    pub body: T,
}
/// Calls the Agent Lab routes for one signed-in session.
pub struct AgentLabClient<'a> {
    client: &'a Client,
    session: &'a Session,
}
impl Client {
    /// Returns the Agent Lab client for `session`, or an error if the session belongs to another deployment.
    pub fn agent_lab<'a>(&'a self, session: &'a Session) -> Result<AgentLabClient<'a>, Error> {
        self.check_session(session)?;
        Ok(AgentLabClient {
            client: self,
            session,
        })
    }
}
impl Client {
    /// Fetches a published agent, at `revision` when given or the latest
    /// otherwise. Anyone can read one, signed in or not (#12048).
    pub async fn published_agent(
        &self,
        id: &str,
        revision: Option<u64>,
        cancel: &CancellationToken,
    ) -> Result<PublishedAgent, Error> {
        if !agent_contract::valid_id(id) {
            return Err(Error::InvalidReference);
        }
        let q = revision
            .map(|r| vec![("revision", r.to_string())])
            .unwrap_or_default();
        let path = ["published-agents", id];
        tokio::select! {biased;_=cancel.cancelled()=>Err(Error::Cancelled),r=self.request(Method::GET,&path,&q,None,None)=>r}
    }
}
impl AgentLabClient<'_> {
    /// Ties `body` to this session's account and deployment.
    pub fn bind<T>(&self, body: T) -> Request<T> {
        Request {
            account: self.session.account().id.clone(),
            deployment: self.client.deployment().clone(),
            body,
        }
    }
    fn body<T: Serialize>(&self, request: &Request<T>) -> Result<serde_json::Value, Error> {
        if request.account != self.session.account().id {
            return Err(Error::WrongAccount);
        }
        if request.deployment != *self.client.deployment() {
            return Err(Error::WrongDeployment);
        }
        serde_json::to_value(&request.body).map_err(|_| Error::InvalidReference)
    }
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        self.client.check_session(self.session)?;
        tokio::select! {biased;_=cancel.cancelled()=>Err(Error::Cancelled),r=self.client.request(method,path,query,body,Some(self.session.token()))=>r}
    }
    /// Deletes the Agent Lab document `id`.
    pub async fn delete_document(
        &self,
        id: &str,
        request: &Request<DeleteDocument>,
        cancel: &CancellationToken,
    ) -> Result<Document, Error> {
        self.send(
            Method::DELETE,
            &["labs", "agent", "documents", id],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Lists the computers that can run Agent Lab experiments.
    pub async fn hosts(&self, cancel: &CancellationToken) -> Result<Vec<Host>, Error> {
        self.send(Method::GET, &["labs", "agent", "hosts"], &[], None, cancel)
            .await
    }
    /// Reports that `host` is online and returns the saved host record.
    pub async fn heartbeat(&self, host: &Host, cancel: &CancellationToken) -> Result<Host, Error> {
        self.send(
            Method::POST,
            &["labs", "agent", "hosts"],
            &[],
            Some(serde_json::to_value(host).map_err(|_| Error::InvalidReference)?),
            cancel,
        )
        .await
    }
    /// Lists Agent Lab documents, starting after the cursor `after`.
    pub async fn documents(
        &self,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<Page<Document>, Error> {
        self.send(
            Method::GET,
            &["labs", "agent", "documents"],
            &after.map(|a| vec![("after", a.into())]).unwrap_or_default(),
            None,
            cancel,
        )
        .await
    }
    /// Fetches the Agent Lab document `id`.
    pub async fn document(&self, id: &str, cancel: &CancellationToken) -> Result<Document, Error> {
        self.send(
            Method::GET,
            &["labs", "agent", "documents", id],
            &[],
            None,
            cancel,
        )
        .await
    }
    /// Saves a document: updates `id` when given, or creates a new document otherwise.
    pub async fn save(
        &self,
        id: Option<&str>,
        request: &Request<SaveDocument>,
        cancel: &CancellationToken,
    ) -> Result<Document, Error> {
        let mut path = vec!["labs", "agent", "documents"];
        if let Some(id) = id {
            path.push(id)
        }
        self.send(
            if id.is_some() {
                Method::PUT
            } else {
                Method::POST
            },
            &path,
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Starts a run of a version of the document `id`.
    pub async fn start(
        &self,
        id: &str,
        request: &Request<StartRun>,
        cancel: &CancellationToken,
    ) -> Result<Run, Error> {
        self.send(
            Method::POST,
            &["labs", "agent", "documents", id, "jobs"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Lists runs, optionally for one host, starting after the cursor `after`.
    pub async fn runs(
        &self,
        host: Option<&str>,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<Page<Run>, Error> {
        let mut query = vec![];
        if let Some(h) = host {
            query.push(("host", h.into()))
        }
        if let Some(a) = after {
            query.push(("after", a.into()))
        }
        self.send(
            Method::GET,
            &["labs", "agent", "jobs"],
            &query,
            None,
            cancel,
        )
        .await
    }
    /// Fetches the run `id`.
    pub async fn run(&self, id: &str, cancel: &CancellationToken) -> Result<Run, Error> {
        self.send(
            Method::GET,
            &["labs", "agent", "jobs", id],
            &[],
            None,
            cancel,
        )
        .await
    }
    /// Applies a user action, such as stop, to the run `id`.
    pub async fn change(
        &self,
        id: &str,
        request: &Request<RunChange>,
        cancel: &CancellationToken,
    ) -> Result<Run, Error> {
        self.send(
            Method::POST,
            &["labs", "agent", "jobs", id, "actions"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Reports a host's progress on the run `id`.
    pub async fn host_update(
        &self,
        id: &str,
        request: &Request<HostUpdate>,
        cancel: &CancellationToken,
    ) -> Result<Run, Error> {
        self.send(
            Method::POST,
            &["labs", "agent", "jobs", id, "host"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Saves a version of the document `id` as an agent definition.
    pub async fn promote(
        &self,
        id: &str,
        request: &Request<Promote>,
        cancel: &CancellationToken,
    ) -> Result<agent_contract::AgentDefinition, Error> {
        self.send(
            Method::POST,
            &["labs", "agent", "documents", id, "promote"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Publishes one of your saved agents with the template it was made from,
    /// under the published ID [`published_id`] of the request ID.
    pub async fn publish(
        &self,
        request: &Request<PublishAgent>,
        cancel: &CancellationToken,
    ) -> Result<PublishedAgent, Error> {
        self.send(
            Method::POST,
            &["published-agents"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Adds a revision to the agent you published as `id`.
    pub async fn update_published(
        &self,
        id: &str,
        request: &Request<UpdatePublishedAgent>,
        cancel: &CancellationToken,
    ) -> Result<PublishedAgent, Error> {
        if !agent_contract::valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::PUT,
            &["published-agents", id],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
}
